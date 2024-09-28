use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, SampleFormat, Stream, SupportedStreamConfig};
use hound::{WavSpec, WavWriter, SampleFormat as HoundSampleFormat};
use std::cell::RefCell;
use std::fs::File;
use std::fmt;
use std::io::BufWriter;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::State;
use webrtc_vad::{SampleRate, Vad, VadMode};
use crossbeam_channel::{bounded, Sender};
use crate::models::{Sentence, RecorderError};

struct DeviceWrapper(Device);

impl fmt::Debug for DeviceWrapper {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Device({})", self.0.name().unwrap_or_default())
    }
}

#[derive(Debug)]
struct AudioConfig {
    device: DeviceWrapper,
    config: SupportedStreamConfig,
    sample_rate: usize,
}

// Enum for recording state
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum RecordingState {
    Idle,
    Recording,
    Paused,
}

// Main AutoRecordState struct
#[derive(Debug)]
struct AutoRecordState {
    sentences: Vec<Sentence>,
    project_directory: String,
    silence_threshold: f32,
    silence_duration: Duration,
    silence_padding: Duration,
    current_sentence_index: usize,
    audio_config: AudioConfig,
    state: RecordingState,
    is_speaking: Arc<Mutex<bool>>,
    last_active_time: Arc<Mutex<Instant>>,
}

impl AutoRecordState {
    // State transition methods
    fn start_recording(&mut self) -> Result<(), &'static str> {
        match self.state {
            RecordingState::Idle => {
                self.state = RecordingState::Recording;
                Ok(())
            },
            _ => Err("Can only start recording from Idle state"),
        }
    }

    fn pause_recording(&mut self) -> Result<(), &'static str> {
        match self.state {
            RecordingState::Recording => {
                self.state = RecordingState::Paused;
                Ok(())
            },
            _ => Err("Can only pause from Recording state"),
        }
    }

    fn resume_recording(&mut self) -> Result<(), &'static str> {
        match self.state {
            RecordingState::Paused => {
                self.state = RecordingState::Recording;
                Ok(())
            },
            _ => Err("Can only resume from Paused state"),
        }
    }

    fn stop_recording(&mut self) -> Result<(), &'static str> {
        match self.state {
            RecordingState::Recording | RecordingState::Paused => {
                self.state = RecordingState::Idle;
                Ok(())
            },
            _ => Err("Can only stop from Recording or Paused state"),
        }
    }
}

// Builder for AutoRecordState
struct AutoRecordStateBuilder {
    sentences: Option<Vec<Sentence>>,
    project_directory: Option<String>,
    silence_threshold: Option<f32>,
    silence_duration: Option<Duration>,
    silence_padding: Option<Duration>,
    audio_config: Option<AudioConfig>,
}

impl AutoRecordStateBuilder {
    fn new() -> Self {
        Self {
            sentences: None,
            project_directory: None,
            silence_threshold: None,
            silence_duration: None,
            silence_padding: None,
            audio_config: None,
        }
    }

    fn sentences(mut self, sentences: Vec<Sentence>) -> Self {
        self.sentences = Some(sentences);
        self
    }

    fn project_directory(mut self, project_directory: String) -> Self {
        self.project_directory = Some(project_directory);
        self
    }

    fn silence_threshold(mut self, silence_threshold: f32) -> Self {
        self.silence_threshold = Some(silence_threshold);
        self
    }

    fn silence_duration(mut self, silence_duration_ms: u64) -> Self {
        self.silence_duration = Some(Duration::from_millis(silence_duration_ms));
        self
    }

    fn silence_padding(mut self, silence_padding_ms: u64) -> Self {
        self.silence_padding = Some(Duration::from_millis(silence_padding_ms));
        self
    }

    fn audio_config(mut self, audio_config: AudioConfig) -> Self {
        self.audio_config = Some(audio_config);
        self
    }

    fn build(self) -> Result<AutoRecordState, String> {
        Ok(AutoRecordState {
            sentences: self.sentences.ok_or("Sentences not set")?,
            project_directory: self.project_directory.ok_or("Project directory not set")?,
            silence_threshold: self.silence_threshold.ok_or("Silence threshold not set")?,
            silence_duration: self.silence_duration.ok_or("Silence duration not set")?,
            silence_padding: self.silence_padding.ok_or("Silence padding not set")?,
            current_sentence_index: 0,
            audio_config: self.audio_config.ok_or("Audio config not set")?,
            state: RecordingState::Idle,
            is_speaking: Arc::new(Mutex::new(false)),
            last_active_time: Arc::new(Mutex::new(Instant::now())),
        })
    }
}

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
    pub fn start_recording(&mut self, filename: String) -> Result<String, String> {
        // Prevent starting a new recording if one is already in progress.
        if self.writer.is_some() {
            return Err("Recording is already in progress".into());
        }

        // Validate the filename to prevent directory traversal attacks.
        if filename.contains("..") {
            return Err("Invalid filename".into());
        }

        // Get default audio input device and configuration.
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or("No input device available")?;
        let config = device
            .default_input_config()
            .map_err(|e| e.to_string())?;

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
            eprintln!("An error occurred on stream: {}", err);
        };

        // Build the input stream based on the sample format.
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
        if let Some(state_arc) = self.auto_record_state.take() {
            let mut state = state_arc.lock().unwrap();
            state.stop_recording().map_err(|e| e.to_string())?;
            Ok(())
        } else {
            Err("No auto-recording in progress".into())
        }
    }

    pub fn pause_auto_record(&mut self) -> Result<(), String> {
        if let Some(state_arc) = &self.auto_record_state {
            let mut state = state_arc.lock().unwrap();
            state.pause_recording().map_err(|e| e.to_string())
        } else {
            Err("No auto-recording in progress".into())
        }
    }

    pub fn resume_auto_record(&mut self) -> Result<(), String> {
        if let Some(state_arc) = &self.auto_record_state {
            let mut state = state_arc.lock().unwrap();
            state.resume_recording().map_err(|e| e.to_string())
        } else {
            Err("No auto-recording in progress".into())
        }
    }

    fn create_audio_config(&self) -> Result<AudioConfig, String> {
        let host = cpal::default_host();
        let device = host.default_input_device()
            .ok_or("No input device available")?;

        let config = find_supported_config(&device)
            .ok_or("No supported audio configuration found")?;

        Ok(AudioConfig {
            device: DeviceWrapper(device),
            config: config.clone(),
            sample_rate: config.sample_rate().0 as usize,
        })
    }

    fn run_auto_record(&mut self, state_arc: Arc<Mutex<AutoRecordState>>, window: tauri::Window) -> Result<(), String> {
        let thread_state_arc = Arc::clone(&state_arc);

        std::thread::spawn(move || {
            loop {
                let should_continue = {
                    let state = thread_state_arc.lock().unwrap();
                    state.state != RecordingState::Idle
                };
    
                if !should_continue {
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
                    window.emit("auto-record-start-sentence", sentence.id)
                        .unwrap_or_else(|e| eprintln!("Failed to emit event: {}", e));

                    match record_sentence(&thread_state_arc) {
                        Ok(()) => handle_successful_recording(&thread_state_arc, &window),
                        Err(RecorderError::RecordingPaused) => {
                            if !handle_paused_recording(&thread_state_arc) {
                                break;
                            }
                        },
                        Err(e) => {
                            eprintln!("Error recording sentence: {}", e);
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

/// Writes the input audio data to the WAV file, converting it to i16 format.
fn write_input_data<T>(
    input: &[T],
    writer: &mut WavWriter<BufWriter<File>>,
) where
    T: cpal::Sample,
{
    for &sample in input.iter() {
        let sample_i16 = sample.to_i16();
        writer.write_sample(sample_i16).unwrap_or_else(|e| {
            eprintln!("Failed to write sample: {}", e);
        });
    }
}

/// Helper function to find a supported audio configuration.
fn find_supported_config(device: &cpal::Device) -> Option<SupportedStreamConfig> {
    device.supported_input_configs().ok()?.find_map(|config_range| {
        let min_rate = config_range.min_sample_rate().0;
        let max_rate = config_range.max_sample_rate().0;

        [8000, 16000, 32000, 48000]
            .iter()
            .find(|&&rate| rate >= min_rate && rate <= max_rate)
            .map(|&rate| config_range.with_sample_rate(cpal::SampleRate(rate)))
    })
}

fn record_sentence(state_arc: &Arc<Mutex<AutoRecordState>>) -> Result<(), RecorderError> {
    let sentence = {
        let state = state_arc.lock().unwrap();
        state.sentences[state.current_sentence_index].clone()
    };

    // Get or create the project directory.
    let project_dir = {
        let state = state_arc.lock().unwrap();
        get_or_create_project_directory(&state.project_directory)?
    };

    // Create the WAV file for the sentence.
    let path = project_dir.join(format!("{}.wav", sentence.text.trim().replace(" ", "_")));

    let spec = {
        let state = state_arc.lock().unwrap();
        WavSpec {
            channels: state.audio_config.config.channels() as u16,
            sample_rate: state.audio_config.sample_rate as u32,
            bits_per_sample: 16,
            sample_format: HoundSampleFormat::Int,
        }
    };
    let writer = WavWriter::create(&path, spec)?;
    let writer = Arc::new(Mutex::new(writer));

    // Initialize buffers and state variables for the recording.
    let active_buffer = Arc::new(Mutex::new(Vec::new()));
    let (voice_tx, voice_rx) = bounded(1);

    // Build the input stream based on the sample format.
    let stream = build_audio_stream(state_arc, writer.clone(), active_buffer.clone(), voice_tx)?;

    stream.play()?;
    println!("Audio stream started for sentence: {}", sentence.text);

    println!("Waiting for voice to be detected...");
    loop {
        {
            let state = state_arc.lock().unwrap();
            if state.state == RecordingState::Paused {
                println!("Recording paused, aborting");
                // Clean up
                drop(stream);
                {
                    let mut writer = writer.lock().unwrap();
                    writer.flush()?; // Finalize the WAV file.
                }
                std::fs::remove_file(&path)?;
                return Err(RecorderError::RecordingPaused);
            }
            if state.state == RecordingState::Idle {
                println!("Recording stopped, aborting");
                // Clean up
                drop(stream);
                {
                    let mut writer = writer.lock().unwrap();
                    writer.flush()?; // Finalize the WAV file.
                }
                std::fs::remove_file(&path)?;
                return Err(RecorderError::RecordingStopped);
            }
        }

        match voice_rx.try_recv() {
            Ok(_) => {
                println!("Voice detected, now waiting for silence...");
                break;
            }
            Err(crossbeam_channel::TryRecvError::Empty) => {
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(e) => {
                return Err(RecorderError::Other(e.to_string()));
            }
        }
    }

    // Wait for silence
    let wait_result = wait_for_silence(state_arc);
    match wait_result {
        Ok(()) => {
            println!("Silence detected, finishing recording for sentence: {}", sentence.text);
            // Continue normal processing
        }
        Err(RecorderError::RecordingPaused) => {
            println!("Wait for silence aborted due to pause");
            // Clean up resources, delete the partially recorded file, etc.
            drop(stream);
            {
                let mut writer = writer.lock().unwrap();
                writer.flush()?; // Finalize the WAV file.
            }
            std::fs::remove_file(&path)?;
            return Err(RecorderError::RecordingPaused);
        }
        Err(e) => {
            return Err(e); // Propagate other errors
        }
    }

    // Stop the stream and finalize the WAV file.
    drop(stream);
    {
        let mut writer = writer.lock().unwrap();
        writer.flush()?; // Finalize the WAV file.
    }

    Ok(())
}

/// Builds the audio input stream with VAD (Voice Activity Detection).
fn build_audio_stream(
    state_arc: &Arc<Mutex<AutoRecordState>>,
    writer: Arc<Mutex<WavWriter<BufWriter<File>>>>,
    active_buffer: Arc<Mutex<Vec<i16>>>,
    voice_tx: Sender<()>,
) -> Result<Stream, RecorderError> {
    let sample_format = {
        let state = state_arc.lock().unwrap();
        state.audio_config.config.sample_format()
    };

    let err_fn = |err| eprintln!("Stream error: {}", err);

    let input_data_fn = create_input_data_fn(
        Arc::clone(state_arc),
        Arc::clone(&writer),
        Arc::clone(&active_buffer),
        voice_tx.clone(),
        sample_format,
    );

    let state = state_arc.lock().unwrap();
    state.audio_config.device.0.build_input_stream(
        &state.audio_config.config.config(),
        input_data_fn,
        err_fn,
    )
    .map_err(RecorderError::CpalBuildStreamError)
}

fn create_input_data_fn(
    state_arc: Arc<Mutex<AutoRecordState>>,
    writer: Arc<Mutex<WavWriter<BufWriter<File>>>>,
    active_buffer: Arc<Mutex<Vec<i16>>>,
    voice_tx: Sender<()>,
    sample_format: SampleFormat,
) -> Box<dyn FnMut(&[f32], &cpal::InputCallbackInfo) + Send + 'static> {
    Box::new(move |input_data: &[f32], _: &cpal::InputCallbackInfo| {
        let processed_data: Vec<i16> = match sample_format {
            SampleFormat::I16 => input_data.iter().map(|&x| x as i16).collect(),
            SampleFormat::F32 => input_data.iter().map(|&x| (x * i16::MAX as f32) as i16).collect(),
            _ => return, // Early return for unsupported formats
        };

        let mut vad = Vad::new_with_rate(SampleRate::Rate16kHz);
        vad.set_mode(VadMode::VeryAggressive);

        process_audio_chunk(
            &processed_data,
            &mut vad,
            &state_arc,
            &active_buffer,
            &writer,
            &voice_tx,
        );
    })
}

/// Processes an audio chunk using VAD, updating state and writing data when speech is detected.
fn process_audio_chunk(
    data: &[i16],
    vad: &mut Vad,
    state_arc: &Arc<Mutex<AutoRecordState>>,
    active_buffer: &Arc<Mutex<Vec<i16>>>,
    writer: &Arc<Mutex<WavWriter<BufWriter<File>>>>,
    voice_tx: &Sender<()>,
) {
    let frame_length = {
        let state = state_arc.lock().unwrap();
        get_frame_length(state.audio_config.sample_rate).unwrap_or(160)
    };

    for chunk in data.chunks(frame_length) {
        if chunk.len() < frame_length {
            continue;
        }

        let is_voice = vad.is_voice_segment(chunk).unwrap_or(false);

        let elapsed = {
            let state = state_arc.lock().unwrap();
            let last_active = state.last_active_time.lock().unwrap();
            last_active.elapsed()
        };

        let speaking = {
            let state = state_arc.lock().unwrap();
            let speaking = state.is_speaking.lock().unwrap();
            *speaking
        };

        if is_voice {
            println!("Voice detected, resetting last_active_time");
            {
                let state = state_arc.lock().unwrap();
                *state.last_active_time.lock().unwrap() = Instant::now();
            }
            if elapsed >= Duration::from_millis(200) {
                {
                    let state = state_arc.lock().unwrap();
                    *state.is_speaking.lock().unwrap() = true;
                }
                let _ = voice_tx.try_send(());
            }

            let mut buffer = active_buffer.lock().unwrap();
            buffer.extend_from_slice(chunk);
        } else if speaking {
            let silence_duration = {
                let state = state_arc.lock().unwrap();
                state.silence_duration
            };

            if elapsed >= silence_duration {
                println!("Silence duration reached after {:?}", elapsed);
                {
                    let state = state_arc.lock().unwrap();
                    *state.is_speaking.lock().unwrap() = false;
                }

                let silence_padding = {
                    let state = state_arc.lock().unwrap();
                    state.silence_padding
                };
                let sample_rate = {
                    let state = state_arc.lock().unwrap();
                    state.audio_config.sample_rate
                };

                let padding_samples = (silence_padding.as_secs_f32() * sample_rate as f32) as usize;
                let mut buffer = active_buffer.lock().unwrap();
                let end_index = buffer.len().saturating_sub(padding_samples);
                let trimmed_audio = &buffer[..end_index];

                let mut writer = writer.lock().unwrap();
                for &sample in trimmed_audio {
                    writer.write_sample(sample).unwrap_or_else(|e| {
                        eprintln!("Failed to write sample: {}", e);
                    });
                }

                buffer.clear();
                return;
            } else {
                println!("In silence, but duration not yet reached. Elapsed: {:?}", elapsed);
                let mut buffer = active_buffer.lock().unwrap();
                buffer.extend_from_slice(chunk);
            }
        }
    }
}

/// Waits for silence to be detected for the given duration.
fn wait_for_silence(state_arc: &Arc<Mutex<AutoRecordState>>) -> Result<(), RecorderError> {
    println!("Entering wait_for_silence");

    while {
        let state = state_arc.lock().unwrap();
        let last_active = *state.last_active_time.lock().unwrap();
        let elapsed = last_active.elapsed();
        
        println!("Current silence duration: {:?}", elapsed);
        
        state.state == RecordingState::Recording && elapsed < state.silence_duration
    } {
        std::thread::sleep(Duration::from_millis(100));
        
        let state = state_arc.lock().unwrap();
        if state.state == RecordingState::Paused {
            println!("Recording paused, aborting wait_for_silence");
            return Err(RecorderError::RecordingPaused);
        }
        if state.state == RecordingState::Idle {
            println!("Recording stopped, aborting wait_for_silence");
            return Err(RecorderError::RecordingStopped);
        }
    }

    println!("Exiting wait_for_silence");
    Ok(())
}

/// Returns the frame length for the given sample rate, used in VAD.
fn get_frame_length(sample_rate: usize) -> Result<usize, RecorderError> {
    match sample_rate {
        8000 => Ok(160),
        16000 => Ok(320),
        32000 => Ok(640),
        48000 => Ok(960),
        _ => Err(RecorderError::Other(format!("Unsupported sample rate: {}", sample_rate))),
    }
}

/// Creates or gets the project directory based on the provided path.
fn get_or_create_project_directory(
    project_directory: &str,
) -> Result<std::path::PathBuf, RecorderError> {
    let project_dir = tauri::api::path::home_dir()
        .map(|home| home.join(project_directory))
        .unwrap_or_else(|| std::path::PathBuf::from(project_directory));

    std::fs::create_dir_all(&project_dir)?;

    Ok(project_dir)
}

fn handle_successful_recording(state_arc: &Arc<Mutex<AutoRecordState>>, window: &tauri::Window) {
    let mut state = state_arc.lock().unwrap();
    let current_index = state.current_sentence_index;
    let total_sentences = state.sentences.len();
    let sentence_id = state.sentences[current_index].id;

    println!("Finished processing sentence {}/{}", current_index + 1, total_sentences);

    window
        .emit("auto-record-finish-sentence", sentence_id)
        .unwrap_or_else(|e| eprintln!("Failed to emit event: {}", e));

    state.current_sentence_index += 1;
}

fn handle_paused_recording(state_arc: &Arc<Mutex<AutoRecordState>>) -> bool {
    println!("Recording paused during sentence {}. Waiting to resume...", {
        let state = state_arc.lock().unwrap();
        state.current_sentence_index + 1
    });

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
    window
        .emit("auto-record-complete", true)
        .unwrap_or_else(|e| eprintln!("Failed to emit event: {}", e));

    let mut state = state_arc.lock().unwrap();
    state.state = RecordingState::Idle;
}

thread_local! {
    static RECORDING_STREAM: RefCell<Option<Stream>> = RefCell::new(None);
}

/// Starts a standard recording and writes to a WAV file.
#[tauri::command]
pub fn start_recording(
    filename: String,
    state: State<Arc<Mutex<Recorder>>>,
) -> Result<String, String> {
    let recorder_state = Arc::clone(state.inner());
    let mut recorder = recorder_state.lock().unwrap();
    recorder.start_recording(filename)
}

/// Stops the current recording and finalizes the WAV file.
#[tauri::command]
pub fn stop_recording(state: State<Arc<Mutex<Recorder>>>) -> Result<String, String> {
    let recorder_state = Arc::clone(state.inner());
    let mut recorder = recorder_state.lock().unwrap();
    recorder.stop_recording()
}

/// Starts the auto-recording process with sentence detection and silence handling.
#[tauri::command]
pub fn start_auto_record(
    sentences: Vec<Sentence>,
    project_directory: String,
    silence_threshold: f32,
    silence_duration: u64,
    silence_padding: u64,
    window: tauri::Window,
    state: State<Arc<Mutex<Recorder>>>,
) -> Result<(), String> {
    let recorder_state = Arc::clone(state.inner());
    {
        let mut recorder = recorder_state.lock().unwrap();
        recorder.start_auto_record(
            sentences.clone(),
            project_directory.clone(),
            silence_threshold,
            silence_duration,
            silence_padding,
            window.clone(),
        )
        .map_err(|e| e.to_string())?;
    }

    Ok(())
}

/// Stops the auto-recording process.
#[tauri::command]
pub fn stop_auto_record(state: State<Arc<Mutex<Recorder>>>) -> Result<(), String> {
    let recorder_state = Arc::clone(state.inner());
    let mut recorder = recorder_state.lock().unwrap();
    recorder.stop_auto_record()
}

/// Pauses the auto-recording process.
#[tauri::command]
pub fn pause_auto_record(state: State<Arc<Mutex<Recorder>>>) -> Result<(), String> {
    let recorder_state = Arc::clone(state.inner());
    let mut recorder = recorder_state.lock().unwrap();
    recorder.pause_auto_record()
}

/// Resumes the auto-recording process.
#[tauri::command]
pub fn resume_auto_record(state: State<Arc<Mutex<Recorder>>>) -> Result<(), String> {
    let recorder_state = Arc::clone(state.inner());
    let mut recorder = recorder_state.lock().unwrap();
    recorder.resume_auto_record()
}
