use super::auto_record::{AutoRecordState, AutoRecordStateBuilder};
use super::config::{AudioConfig, DeviceWrapper, RecordingState};
use super::errors::RecorderError;
use super::stream::record_sentence;
use super::utils::{find_supported_config, write_input_data};
use crate::models::Sentence;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream};
use hound::{SampleFormat as HoundSampleFormat, WavSpec, WavWriter};
use log::{debug, error};
use serde_json::json;
use std::cell::RefCell;
use std::fs::File;
use std::io::BufWriter;
use std::sync::{Arc, Mutex};
use std::time::Duration;

// Shared state for the recorder.
pub struct Recorder {
    auto_record_state: Option<Arc<Mutex<AutoRecordState>>>,
    writer: Option<Arc<Mutex<WavWriter<BufWriter<File>>>>>,
}

impl Recorder {
    pub fn new() -> Self {
        Self {
            auto_record_state: None,
            writer: None,
        }
    }

    /// Starts a standard recording and writes to a WAV file.
    ///
    /// This function sets up an audio input stream, configures a WAV file writer with the
    /// appropriate sample rate and channels, and starts recording audio data to the WAV file.
    /// If a recording is already in progress, it returns an error. The filename is also
    /// validated to prevent directory traversal attacks.
    ///
    /// # Arguments
    /// * `filename` - The name of the WAV file to create.
    ///
    /// # Returns
    /// * `Ok(String)` - A success message indicating the recording has started.
    /// * `Err(String)` - An error message if the recording could not be started.
    pub fn start_recording(&mut self, filename: String) -> Result<String, String> {
        // Prevent starting a new recording if one is already in progress.
        if self.writer.is_some() {
            return Err("Recording is already in progress".into());
        }

        // Validate the filename to prevent directory traversal attacks.
        if filename.contains("..") {
            return Err("Invalid filename".into());
        }

        debug!("Setting up audio inputs and writer...");
        // Get default audio input device and configuration.
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or("No input device available")?;
        let config = device.default_input_config().map_err(|e| e.to_string())?;

        // Configure WAV file writer with the sample rate and channels from the audio device.
        let channels = config.channels();
        let sample_rate = config.sample_rate().0;
        let spec = WavSpec {
            channels: channels as u16,
            sample_rate: sample_rate as u32,
            bits_per_sample: 16,
            sample_format: HoundSampleFormat::Int,
        };

        let writer = WavWriter::create(&filename, spec).map_err(|e| e.to_string())?;
        let writer = Arc::new(Mutex::new(writer));

        // Clone the writer to use within the audio stream callback.
        let writer_clone = Arc::clone(&writer);

        // Error handling for the audio stream.
        let err_fn = move |err| {
            error!("An error occurred on stream: {}", err);
        };

        debug!("Building audio stream...");
        let stream = match config.sample_format() {
            SampleFormat::F32 => device.build_input_stream(
                &config.config(),
                move |data: &[f32], _| {
                    let mut writer = writer_clone.lock().unwrap();
                    write_input_data(data, &mut *writer);
                },
                err_fn,
            ),
            SampleFormat::I16 => device.build_input_stream(
                &config.config(),
                move |data: &[i16], _| {
                    let mut writer = writer_clone.lock().unwrap();
                    write_input_data(data, &mut *writer);
                },
                err_fn,
            ),
            SampleFormat::U16 => device.build_input_stream(
                &config.config(),
                move |data: &[u16], _| {
                    let mut writer = writer_clone.lock().unwrap();
                    write_input_data(data, &mut *writer);
                },
                err_fn,
            ),
            _ => return Err("Unsupported sample format".into()),
        }
        .map_err(|e| e.to_string())?;

        // Play the stream
        stream.play().map_err(|e| e.to_string())?;

        // Save the writer in the recorder state
        self.writer = Some(writer);

        // Save the stream in thread-local storage
        RECORDING_STREAM.with(|s| {
            *s.borrow_mut() = Some(stream);
        });

        Ok("Recording started".into())
    }

    /// Stops the current recording and finalizes the WAV file.
    pub fn stop_recording(&mut self) -> Result<String, String> {
        if self.writer.is_some() {
            self.writer = None; // Dropping the writer finalizes the WAV file.

            // Stop the stream
            RECORDING_STREAM.with(|s| {
                if let Some(stream) = s.borrow_mut().take() {
                    drop(stream); // Stops the audio stream
                }
            });

            Ok("Recording stopped".into())
        } else {
            Err("No recording in progress".into())
        }
    }

    /// Starts the auto-recording process with sentence detection and silence handling.
    pub fn start_auto_record(
        &mut self,
        sentences: Vec<Sentence>,
        project_directory: String,
        silence_threshold: f32,
        silence_duration_ms: u64,
        silence_padding_ms: u64,
        window: tauri::Window,
    ) -> Result<(), String> {
        debug!("Starting auto-recording...");
        let audio_config = self.create_audio_config()?;

        let auto_record_state = AutoRecordStateBuilder::new()
            .sentences(sentences)
            .project_directory(project_directory)
            .silence_threshold(silence_threshold)
            .silence_duration(silence_duration_ms)
            .silence_padding(silence_padding_ms)
            .audio_config(audio_config)
            .build()?;

        let state_arc = Arc::new(Mutex::new(auto_record_state));

        self.auto_record_state = Some(Arc::clone(&state_arc));

        {
            let mut state = state_arc.lock().unwrap();
            state.start_recording().map_err(|e| e.to_string())?;
        }

        self.run_auto_record(state_arc, window)
    }

    pub fn stop_auto_record(&mut self) -> Result<(), String> {
        debug!("Stopping auto-recording...");
        if let Some(state_arc) = self.auto_record_state.take() {
            let mut state = state_arc.lock().unwrap();
            state.stop_recording().map_err(|e| e.to_string())?;
            Ok(())
        } else {
            Err("No auto-recording in progress".into())
        }
    }

    pub fn pause_auto_record(&mut self) -> Result<(), String> {
        debug!("Pausing auto-recording...");
        if let Some(state_arc) = &self.auto_record_state {
            let mut state = state_arc.lock().unwrap();
            state.pause_recording().map_err(|e| e.to_string())
        } else {
            Err("No auto-recording in progress".into())
        }
    }

    pub fn resume_auto_record(&mut self) -> Result<(), String> {
        debug!("Resuming auto-recording...");
        if let Some(state_arc) = &self.auto_record_state {
            let mut state = state_arc.lock().unwrap();
            state.resume_recording().map_err(|e| e.to_string())
        } else {
            Err("No auto-recording in progress".into())
        }
    }

    fn create_audio_config(&self) -> Result<AudioConfig, String> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or("No input device available")?;

        let config =
            find_supported_config(&device).ok_or("No supported audio configuration found")?;

        Ok(AudioConfig {
            device: DeviceWrapper(device),
            config: config.clone(),
            sample_rate: config.sample_rate().0 as usize,
        })
    }

    fn run_auto_record(
        &mut self,
        state_arc: Arc<Mutex<AutoRecordState>>,
        window: tauri::Window,
    ) -> Result<(), String> {
        debug!("Moving auto-record to thread");
        let thread_state_arc = Arc::clone(&state_arc);

        std::thread::spawn(move || {
            loop {
                let should_continue = {
                    let state = thread_state_arc.lock().unwrap();
                    state.state != RecordingState::Idle
                };

                if !should_continue {
                    debug!("Auto-record thread exiting as RecordingState is Idle");
                    break;
                }

                let sentence_option = {
                    let state = thread_state_arc.lock().unwrap();
                    if state.current_sentence_index >= state.sentences.len() {
                        None
                    } else {
                        Some(state.sentences[state.current_sentence_index].clone())
                    }
                };

                if let Some(sentence) = sentence_option {
                    // Let the UI know that we're starting a new sentence
                    window
                        .emit("auto-record-start-sentence", sentence.id)
                        .unwrap_or_else(|e| eprintln!("Failed to emit event: {}", e));

                    match record_sentence(&thread_state_arc) {
                        Ok(()) => handle_successful_recording(&thread_state_arc, &window),
                        Err(RecorderError::RecordingPaused) => {
                            if !handle_paused_recording(&thread_state_arc) {
                                break;
                            }
                        }
                        Err(e) => {
                            error!("Error recording sentence: {}", e);
                            break;
                        }
                    }
                } else {
                    break;
                }
            }

            finalize_recording(&thread_state_arc, &window);
        });

        Ok(())
    }
}

fn handle_successful_recording(state_arc: &Arc<Mutex<AutoRecordState>>, window: &tauri::Window) {
    let mut state = state_arc.lock().unwrap();
    let current_index = state.current_sentence_index;
    let total_sentences = state.sentences.len();
    let sentence = &state.sentences[current_index];
    let sentence_id = sentence.id;
    let audio_file_path = sentence.audio_file_path.clone().unwrap_or_default();

    debug!(
        "Finished processing sentence {}/{}",
        current_index + 1,
        total_sentences
    );

    // Let the UI know that we've finished processing the sentence
    window
        .emit(
            "auto-record-finish-sentence",
            json!({
                "id": sentence_id,
                "audioFilePath": audio_file_path
            }),
        )
        .unwrap_or_else(|e| eprintln!("Failed to emit event: {}", e));

    state.current_sentence_index += 1;
}

fn handle_paused_recording(state_arc: &Arc<Mutex<AutoRecordState>>) -> bool {
    println!(
        "Recording paused during sentence {}. Waiting to resume...",
        {
            let state = state_arc.lock().unwrap();
            state.current_sentence_index + 1
        }
    );

    while {
        let state = state_arc.lock().unwrap();
        match state.state {
            RecordingState::Idle => return false,
            RecordingState::Paused => true,
            RecordingState::Recording => return true,
        }
    } {
        std::thread::sleep(Duration::from_millis(100));
    }

    false // This line is unreachable, but Rust requires it for completeness
}

fn finalize_recording(state_arc: &Arc<Mutex<AutoRecordState>>, window: &tauri::Window) {
    // Let the UI know that we've finished the auto-recording process
    window
        .emit("auto-record-complete", true)
        .unwrap_or_else(|e| eprintln!("Failed to emit event: {}", e));

    let mut state = state_arc.lock().unwrap();
    state.state = RecordingState::Idle;
}

thread_local! {
  static RECORDING_STREAM: RefCell<Option<Stream>> = RefCell::new(None);
}
