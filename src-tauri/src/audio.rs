use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream, SupportedStreamConfig};
use std::sync::{Arc, Mutex};
use tauri::State;
use webrtc_vad::{Vad, SampleRate, VadMode};
use hound::{WavWriter, WavSpec, SampleFormat as HoundSampleFormat};
use std::time::{Duration, Instant};
use std::io::BufWriter;
use std::fs::File;

// Thread-local storage for recording state. It stores the current recording stream and writer.
thread_local! {
    static RECORDING: std::cell::RefCell<Option<(Stream, Arc<Mutex<WavWriter<std::io::BufWriter<std::fs::File>>>>)>> = std::cell::RefCell::new(None);
}

// Starts a standard recording and writes to a WAV file.
#[tauri::command]
pub fn start_recording(filename: String) -> Result<String, String> {
    RECORDING.with(|recording| {
        let mut recording = recording.borrow_mut();

        // Prevent starting a new recording if one is already in progress.
        if recording.is_some() {
            return Err("Recording is already in progress".into());
        }

        // Validate the filename to prevent directory traversal attacks.
        if filename.contains("..") {
            return Err("Invalid filename".into());
        }

        // Get default audio input device and configuration.
        let host = cpal::default_host();
        let device = host.default_input_device().ok_or("No input device available")?;
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

        let writer = hound::WavWriter::create(&filename, spec).map_err(|e| e.to_string())?;
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
                    write_input_data::<f32>(data, &mut *writer);
                },
                err_fn,
            ),
            SampleFormat::I16 => device.build_input_stream(
                &config.config(),
                move |data: &[i16], _| {
                    let mut writer = writer_clone.lock().unwrap();
                    write_input_data::<i16>(data, &mut *writer);
                },
                err_fn,
            ),
            SampleFormat::U16 => device.build_input_stream(
                &config.config(),
                move |data: &[u16], _| {
                    let mut writer = writer_clone.lock().unwrap();
                    write_input_data::<u16>(data, &mut *writer);
                },
                err_fn,
            ),
        }
        .map_err(|e| e.to_string())?;

        stream.play().map_err(|e| e.to_string())?;

        // Save the stream and writer in the thread-local storage.
        *recording = Some((stream, writer));

        Ok("Recording started".into())
    })
}

// Stops the current recording and finalizes the WAV file.
#[tauri::command]
pub fn stop_recording() -> Result<String, String> {
    RECORDING.with(|recording| {
        let mut recording = recording.borrow_mut();
        if let Some((stream, _writer)) = recording.take() {
            drop(stream); // Stops the audio stream.
            // Writer is dropped here, finalizing the WAV file.
            Ok("Recording stopped".into())
        } else {
            Err("No recording in progress".into())
        }
    })
}

// Writes the input audio data to the WAV file, converting it to i16 format.
fn write_input_data<T>(
    input: &[T],
    writer: &mut hound::WavWriter<std::io::BufWriter<std::fs::File>>,
) where
    T: cpal::Sample,
{
    for &sample in input.iter() {
        let sample_i16 = sample.to_i16();
        writer.write_sample(sample_i16).unwrap();
    }
}

// Struct to track the state of the auto-recording process.
pub struct RecordingState {
    pub is_auto_recording: bool,
    pub current_sentence_index: usize,
}

impl RecordingState {
    pub fn new() -> Self {
        Self {
            is_auto_recording: false,
            current_sentence_index: 0,
        }
    }
}

// Starts the auto-recording process with sentence detection and silence handling.
#[tauri::command]
pub fn start_auto_record(
    sentences: Vec<String>,
    project_directory: String, // Directory to save the recordings.
    silence_threshold: f32,
    silence_duration: u64,
    silence_padding: u64,
    window: tauri::Window,
    state: State<Arc<Mutex<RecordingState>>>,
) -> Result<(), String> {
    println!("Starting auto-recording");

    {
        // Set is_auto_recording to true before starting the recording thread.
        let mut recording_state = state.lock().unwrap();
        recording_state.is_auto_recording = true;
    }

    // Clone the necessary data to move into the thread.
    let recording_state_arc = state.inner().clone();
    let project_directory_clone = project_directory.clone();

    // Spawn a new thread for the auto-recording process.
    std::thread::spawn(move || {
        println!(
            "Starting auto-recording with {} sentences",
            sentences.len()
        );

        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .expect("Failed to get default input device");

        // Find a supported configuration with specific sample rates.
        let config = find_supported_config(&device).expect("No supported sample rate found");

        // Iterate over each sentence and record.
        for (index, sentence) in sentences.iter().enumerate() {
            println!(
                "Starting auto-recording for sentence {}: {}",
                index + 1,
                sentence,
            );

            // Update the recording state.
            {
                let mut recording_state = recording_state_arc.lock().unwrap();
                if !recording_state.is_auto_recording {
                    break;
                }
                recording_state.current_sentence_index = index;
            }

            // Notify the frontend about the current sentence.
            window.emit("auto-record-start-sentence", index).unwrap();

            // Record the sentence with silence detection.
            if let Err(e) = record_sentence(
                &device,
                &config,
                sentence,
                &project_directory_clone,
                silence_threshold,
                silence_duration,
                silence_padding,
            ) {
                eprintln!("Error recording sentence {}: {}", index + 1, e);
                break;
            }

            // Notify the frontend after finishing the sentence.
            window.emit("auto-record-finish-sentence", index).unwrap();
        }

        // Notify the frontend that auto-recording is complete.
        window.emit("auto-record-complete", true).unwrap();

        // Set the is_auto_recording flag to false after completion.
        let mut recording_state = recording_state_arc.lock().unwrap();
        recording_state.is_auto_recording = false;
    });

    Ok(())
}

// Helper function to find a supported audio configuration.
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

// Handles the recording of a single sentence, saving it to a WAV file.
fn record_sentence(
    device: &cpal::Device,
    config: &SupportedStreamConfig,
    sentence: &str,
    project_directory: &str,
    _silence_threshold: f32,
    silence_duration_ms: u64,
    silence_padding_ms: u64,
) -> Result<(), Box<dyn std::error::Error>> {
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

    // Clone variables to be used inside the stream callback.
    let writer_clone = Arc::clone(&writer);
    let active_buffer_clone = Arc::clone(&active_buffer);
    let is_speaking_clone = Arc::clone(&is_speaking);
    let last_active_time_clone = Arc::clone(&last_active_time);

    // Build the input stream based on the sample format.
    let stream = build_audio_stream(
        device,
        config,
        silence_duration,
        silence_padding,
        sample_rate,
        writer_clone,
        active_buffer_clone,
        is_speaking_clone,
        last_active_time_clone,
    )?;

    stream.play()?; // Start the audio stream.

    // Wait until silence is detected for the configured duration.
    wait_for_silence(silence_duration, &last_active_time)?;

    // Stop the stream and finalize the WAV file.
    drop(stream);
    {
        let mut writer = writer.lock().unwrap();
        writer.flush()?; // Finalize the WAV file.
    }

    Ok(())
}

// Builds the audio input stream with VAD (Voice Activity Detection).
fn build_audio_stream(
    device: &cpal::Device,
    config: &SupportedStreamConfig,
    silence_duration: Duration,
    silence_padding: Duration,
    sample_rate: usize,
    writer_clone: Arc<Mutex<WavWriter<BufWriter<File>>>>,
    active_buffer_clone: Arc<Mutex<Vec<i16>>>,
    is_speaking_clone: Arc<Mutex<bool>>,
    last_active_time_clone: Arc<Mutex<Instant>>,
) -> Result<Stream, Box<dyn std::error::Error>> {
    match config.sample_format() {
        SampleFormat::I16 => {
            let frame_length = get_frame_length(sample_rate)?;
            Ok(device.build_input_stream(
                &config.config(),
                move |data: &[i16], _: &cpal::InputCallbackInfo| {
                    let mut vad = Vad::new_with_rate(SampleRate::Rate16kHz); // Set VAD mode to very aggressive
                    vad.set_mode(VadMode::VeryAggressive);
                    process_audio_chunk(
                        data,
                        &mut vad,
                        &active_buffer_clone,
                        &is_speaking_clone,
                        &last_active_time_clone,
                        silence_duration,
                        silence_padding,
                        &writer_clone,
                        sample_rate,
                        frame_length,
                    );
                },
                |err| eprintln!("Stream error: {}", err),
            )?)
        }
        SampleFormat::F32 => {
            let frame_length = get_frame_length(sample_rate)?;
            Ok(device.build_input_stream(
                &config.config(),
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    let mut vad = Vad::new_with_rate(SampleRate::Rate16kHz); // Set VAD mode to very aggressive
                    vad.set_mode(VadMode::VeryAggressive);

                    let data_i16: Vec<i16> = data
                        .iter()
                        .map(|&sample| (sample * i16::MAX as f32) as i16)
                        .collect();

                    process_audio_chunk(
                        &data_i16,
                        &mut vad,
                        &active_buffer_clone,
                        &is_speaking_clone,
                        &last_active_time_clone,
                        silence_duration,
                        silence_padding,
                        &writer_clone,
                        sample_rate,
                        frame_length,
                    );
                },
                |err| eprintln!("Stream error: {}", err),
            )?)
        }
        _ => Err("Unsupported sample format".into()),
    }
}

// Waits for silence to be detected for the given duration.
fn wait_for_silence(
    silence_duration: Duration,
    last_active_time: &Arc<Mutex<Instant>>,
) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        let last_active = {
            let last_active_time = last_active_time.lock().unwrap();
            *last_active_time
        };

        if last_active.elapsed() >= silence_duration {
            break;
        }

        std::thread::sleep(Duration::from_millis(100));
    }

    Ok(())
}

// Returns the frame length for the given sample rate, used in VAD.
fn get_frame_length(sample_rate: usize) -> Result<usize, Box<dyn std::error::Error>> {
    Ok(match sample_rate {
        8000 => 160,
        16000 => 320,
        32000 => 640,
        48000 => 960,
        _ => return Err(format!("Unsupported sample rate: {}", sample_rate).into()),
    })
}

// Creates or gets the project directory based on the provided path.
fn get_or_create_project_directory(
    project_directory: &str,
) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let project_dir = match tauri::api::path::home_dir() {
        Some(home) => home.join(project_directory),
        _none => std::path::PathBuf::from(project_directory),
    };

    // Ensure the project directory exists.
    std::fs::create_dir_all(&project_dir)?;

    Ok(project_dir)
}

// Processes an audio chunk using VAD, updating state and writing data when speech is detected.
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
) {
    for chunk in data.chunks(frame_length) {
        if chunk.len() < frame_length {
            continue; // Skip if the chunk is incomplete.
        }

        let clamped_chunk: Vec<i16> = chunk.iter().map(|&sample| sample.clamp(-20000, 20000)).collect();
        let is_voice = vad.is_voice_segment(&clamped_chunk).unwrap_or(false);
        let max_amplitude = chunk.iter().max().unwrap_or(&0);

        let elapsed = {
            let last_active = last_active_time.lock().unwrap();
            last_active.elapsed()
        };

        let speaking = {
            let speaking = is_speaking.lock().unwrap();
            *speaking
        };

        if is_voice {
            // If speech is detected, reset silence detection and update state.
            println!("Speech detected after {}s, max amplitude: {}", elapsed.as_secs_f32(), max_amplitude);
            if elapsed >= Duration::from_millis(200) {
                *is_speaking.lock().unwrap() = true;
                *last_active_time.lock().unwrap() = Instant::now();
            }

            let mut buffer = active_buffer.lock().unwrap();
            buffer.extend_from_slice(chunk);
        } else {
            if speaking {
                if elapsed >= silence_duration {
                    // If silence is detected beyond the threshold, finalize the sentence.
                    println!("Silence detected after {}s, finalizing sentence", elapsed.as_secs_f32());
                    *is_speaking.lock().unwrap() = false;

                    let padding_samples = (silence_padding.as_secs_f32() * sample_rate as f32) as usize;
                    let mut buffer = active_buffer.lock().unwrap();
                    let end_index = buffer.len().saturating_sub(padding_samples);
                    let trimmed_audio = &buffer[..end_index];

                    let mut writer = writer.lock().unwrap();
                    for &sample in trimmed_audio {
                        writer.write_sample(sample).unwrap();
                    }

                    buffer.clear();
                    return; // Move to the next sentence.
                } else {
                    println!("Buffering silence, elapsed time: {}s", elapsed.as_secs_f32());
                    let mut buffer = active_buffer.lock().unwrap();
                    buffer.extend_from_slice(chunk);
                }
            }
        }
    }
}

#[tauri::command]
pub fn stop_auto_record(state: State<'_, Arc<Mutex<RecordingState>>>) {
    let mut recording_state = state.lock().unwrap();
    recording_state.is_auto_recording = false;
}