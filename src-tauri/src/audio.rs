use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream, SupportedStreamConfig};
use hound::{WavSpec, WavWriter, SampleFormat as HoundSampleFormat};
use std::cell::RefCell;
use std::fs::File;
use std::io::BufWriter;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::State;
use webrtc_vad::{SampleRate, Vad, VadMode};
use crate::models::Sentence;

// Shared state for the recorder.
pub struct Recorder {
    writer: Option<Arc<Mutex<WavWriter<BufWriter<File>>>>>,
    is_auto_recording: bool,
    current_sentence_index: usize,
    is_paused: bool,
}

impl Recorder {
    pub fn new() -> Self {
        Self {
            writer: None,
            is_auto_recording: false,
            current_sentence_index: 0,
            is_paused: false,
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
        recorder_state: Arc<Mutex<Recorder>>, // Pass the shared recorder state
    ) -> Result<(), String> {
        println!("Starting auto-recording with {} sentences", sentences.len());

        self.is_auto_recording = true;

        // Clone variables to move into the closure
        let recorder_state_clone = Arc::clone(&recorder_state);
        let sentences_clone = sentences.clone();
        let project_directory_clone = project_directory.clone();
        let window_clone = window.clone();

        // Spawn a new thread for the auto-recording process.
        std::thread::spawn(move || {
            let host = cpal::default_host();
            let device = match host.default_input_device() {
                Some(device) => device,
                None => {
                    eprintln!("Failed to get default input device");
                    return;
                }
            };

            // Find a supported configuration with specific sample rates.
            let config = match find_supported_config(&device) {
                Some(config) => config,
                None => {
                    eprintln!("No supported sample rate found");
                    return;
                }
            };

            // Iterate over each sentence and record.
            for (index, sentence) in sentences.iter().enumerate() {
                println!("Processing sentence {}/{}: {}", index + 1, sentences.len(), sentence.text);

                // Update the recording state.
                {
                    let mut recorder = recorder_state.lock().unwrap();
                    if !recorder.is_auto_recording {
                        break;
                    }
                    recorder.current_sentence_index = index;
                }

                // Notify the frontend about the current sentence.
                window
                    .emit("auto-record-start-sentence", sentence.id)
                    .unwrap_or_else(|e| eprintln!("Failed to emit event: {}", e));

                // Record the sentence with silence detection.
                if let Err(e) = record_sentence(
                    &device,
                    &config,
                    &sentence.text,
                    &project_directory_clone,
                    silence_threshold,
                    silence_duration_ms,
                    silence_padding_ms,
                ) {
                    eprintln!("Error recording sentence {}: {}", index + 1, e);
                    break;
                }

                println!("Finished processing sentence {}/{}", index + 1, sentences.len());

                // Notify the frontend after finishing the sentence.
                window
                    .emit("auto-record-finish-sentence", sentence.id)
                    .unwrap_or_else(|e| eprintln!("Failed to emit event: {}", e));
            }

            // Notify the frontend that auto-recording is complete.
            window
                .emit("auto-record-complete", true)
                .unwrap_or_else(|e| eprintln!("Failed to emit event: {}", e));

            // Set the is_auto_recording flag to false after completion.
            let mut recorder = recorder_state_clone.lock().unwrap();
            recorder.is_auto_recording = false;
        });

        Ok(())
    }

    /// Stops the auto-recording process.
    pub fn stop_auto_record(&mut self) {
        self.is_auto_recording = false;
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
    let supported_configs = device.supported_input_configs().ok()?;
    supported_configs
        .filter_map(|config_range| {
            let min_rate = config_range.min_sample_rate().0;
            let max_rate = config_range.max_sample_rate().0;

            [8000, 16000, 32000, 48000]
                .iter()
                .find_map(|&rate| {
                    if rate >= min_rate && rate <= max_rate {
                        Some(config_range.clone().with_sample_rate(cpal::SampleRate(rate)))
                    } else {
                        None
                    }
                })
        })
        .next()
}

/// Handles the recording of a single sentence, saving it to a WAV file.
use std::sync::atomic::{AtomicBool, Ordering};
use crossbeam_channel::{bounded, Sender};

fn record_sentence(
    device: &cpal::Device,
    config: &SupportedStreamConfig,
    sentence: &str,
    project_directory: &str,
    _silence_threshold: f32,
    silence_duration_ms: u64,
    silence_padding_ms: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting to record sentence: {}", sentence);

    let stream_config = config.config();
    let sample_rate = stream_config.sample_rate.0 as usize;
    let channels = stream_config.channels as usize;
    let silence_duration = Duration::from_millis(silence_duration_ms);
    let silence_padding = Duration::from_millis(silence_padding_ms);

    // Get or create the project directory.
    let project_dir = get_or_create_project_directory(project_directory)?;

    // Create the WAV file for the sentence.
    let path = project_dir.join(format!("{}.wav", sentence.trim().replace(" ", "_")));

    let spec = WavSpec {
        channels: channels as u16,
        sample_rate: sample_rate as u32,
        bits_per_sample: 16,
        sample_format: HoundSampleFormat::Int,
    };
    let writer = WavWriter::create(path, spec)?;
    let writer = Arc::new(Mutex::new(writer));

    // Initialize buffers and state variables for the recording.
    let active_buffer = Arc::new(Mutex::new(Vec::new()));
    let is_speaking = Arc::new(Mutex::new(false));
    let last_active_time = Arc::new(Mutex::new(Instant::now()));
    let (voice_tx, voice_rx) = bounded(1);

    // Build the input stream based on the sample format.
    let stream = build_audio_stream(
        device,
        config,
        silence_duration,
        silence_padding,
        sample_rate,
        writer.clone(),
        active_buffer.clone(),
        is_speaking.clone(),
        last_active_time.clone(),
        voice_tx,
    )?;

    stream.play()?;
    println!("Audio stream started for sentence: {}", sentence);

    println!("Waiting for voice to be detected...");
    voice_rx.recv()?;
    println!("Voice detected, now waiting for silence...");
    wait_for_silence(silence_duration, &last_active_time)?;
    println!("Silence detected, finishing recording for sentence: {}", sentence);

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
    device: &cpal::Device,
    config: &SupportedStreamConfig,
    silence_duration: Duration,
    silence_padding: Duration,
    sample_rate: usize,
    writer: Arc<Mutex<WavWriter<BufWriter<File>>>>,
    active_buffer: Arc<Mutex<Vec<i16>>>,
    is_speaking: Arc<Mutex<bool>>,
    last_active_time: Arc<Mutex<Instant>>,
    voice_tx: Sender<()>,
) -> Result<Stream, Box<dyn std::error::Error>> {
    let frame_length = get_frame_length(sample_rate)?;
    let err_fn = |err| eprintln!("Stream error: {}", err);

    match config.sample_format() {
        SampleFormat::I16 => {
            let input_data_fn = {
                let active_buffer = Arc::clone(&active_buffer);
                let is_speaking = Arc::clone(&is_speaking);
                let last_active_time = Arc::clone(&last_active_time);
                let writer = Arc::clone(&writer);
                let voice_tx = voice_tx.clone();

                move |data: &[i16], _: &cpal::InputCallbackInfo| {
                    let mut vad = Vad::new_with_rate(SampleRate::Rate16kHz);
                    vad.set_mode(VadMode::VeryAggressive);

                    process_audio_chunk(
                        data,
                        &mut vad,
                        &active_buffer,
                        &is_speaking,
                        &last_active_time,
                        silence_duration,
                        silence_padding,
                        &writer,
                        sample_rate,
                        frame_length,
                        &voice_tx,
                    );
                }
            };

            Ok(device.build_input_stream(&config.config(), input_data_fn, err_fn)?)
        }
        SampleFormat::F32 => {
            let input_data_fn = {
                let active_buffer = Arc::clone(&active_buffer);
                let is_speaking = Arc::clone(&is_speaking);
                let last_active_time = Arc::clone(&last_active_time);
                let writer = Arc::clone(&writer);
                let voice_tx = voice_tx.clone();

                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    let data_i16: Vec<i16> = data
                        .iter()
                        .map(|&sample| (sample * i16::MAX as f32) as i16)
                        .collect();

                    let mut vad = Vad::new_with_rate(SampleRate::Rate16kHz);
                    vad.set_mode(VadMode::VeryAggressive);

                    process_audio_chunk(
                        &data_i16,
                        &mut vad,
                        &active_buffer,
                        &is_speaking,
                        &last_active_time,
                        silence_duration,
                        silence_padding,
                        &writer,
                        sample_rate,
                        frame_length,
                        &voice_tx,
                    );
                }
            };

            Ok(device.build_input_stream(&config.config(), input_data_fn, err_fn)?)
        }
        _ => Err("Unsupported sample format".into()),
    }
}


/// Waits for silence to be detected for the given duration.
fn wait_for_silence(
    silence_duration: Duration,
    last_active_time: &Arc<Mutex<Instant>>,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Entering wait_for_silence");
    loop {
        let last_active = {
            let last_active_time = last_active_time.lock().unwrap();
            *last_active_time
        };

        let elapsed = last_active.elapsed();
        println!("Current silence duration: {:?}", elapsed);

        if elapsed >= silence_duration {
            println!("Silence duration reached: {:?}", elapsed);
            break;
        }

        std::thread::sleep(Duration::from_millis(100));
    }

    println!("Exiting wait_for_silence");
    Ok(())
}

/// Returns the frame length for the given sample rate, used in VAD.
fn get_frame_length(sample_rate: usize) -> Result<usize, Box<dyn std::error::Error>> {
    Ok(match sample_rate {
        8000 => 160,
        16000 => 320,
        32000 => 640,
        48000 => 960,
        _ => return Err(format!("Unsupported sample rate: {}", sample_rate).into()),
    })
}

/// Creates or gets the project directory based on the provided path.
fn get_or_create_project_directory(
    project_directory: &str,
) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let project_dir = match tauri::api::path::home_dir() {
        Some(home) => home.join(project_directory),
        None => std::path::PathBuf::from(project_directory),
    };

    // Ensure the project directory exists.
    std::fs::create_dir_all(&project_dir)?;

    Ok(project_dir)
}

/// Processes an audio chunk using VAD, updating state and writing data when speech is detected.
fn process_audio_chunk(
    data: &[i16],
    vad: &mut Vad,
    active_buffer: &Arc<Mutex<Vec<i16>>>,
    is_speaking: &Arc<Mutex<bool>>,
    last_active_time: &Arc<Mutex<Instant>>,
    silence_duration: Duration,
    silence_padding: Duration,
    writer: &Arc<Mutex<WavWriter<BufWriter<File>>>>,
    sample_rate: usize,
    frame_length: usize,
    voice_tx: &Sender<()>,
) {
    for chunk in data.chunks(frame_length) {
        if chunk.len() < frame_length {
            continue;
        }

        let is_voice = vad.is_voice_segment(chunk).unwrap_or(false);

        let elapsed = {
            let last_active = last_active_time.lock().unwrap();
            last_active.elapsed()
        };

        println!("Current elapsed time since last activity: {:?}", elapsed);

        let speaking = {
            let speaking = is_speaking.lock().unwrap();
            *speaking
        };

        if is_voice {
            println!("Voice detected, resetting last_active_time");
            *last_active_time.lock().unwrap() = Instant::now();
            if elapsed >= Duration::from_millis(200) {
                *is_speaking.lock().unwrap() = true;
                let _ = voice_tx.try_send(());
            }

            let mut buffer = active_buffer.lock().unwrap();
            buffer.extend_from_slice(chunk);
        } else if speaking {
            if elapsed >= silence_duration {
                println!("Silence duration reached after {:?}", elapsed);
                *is_speaking.lock().unwrap() = false;

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
            Arc::clone(&recorder_state),
        )?;
    }

    Ok(())
}

/// Stops the auto-recording process.
#[tauri::command]
pub fn stop_auto_record(state: State<Arc<Mutex<Recorder>>>) {
    let recorder_state = Arc::clone(state.inner());
    let mut recorder = recorder_state.lock().unwrap();
    recorder.stop_auto_record();
}