use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream, SupportedStreamConfig};
use std::sync::{Arc, Mutex};
use tauri::State;
use tauri::api::path::home_dir;
use webrtc_vad::{Vad, SampleRate, VadMode};
use hound::{WavWriter, WavSpec, SampleFormat as HoundSampleFormat};
use std::time::{Duration, Instant};
use std::io::BufWriter;
use std::fs::File;

const NOISE_GRACE_PERIOD_MS: u64 = 200;
const CLAMP_MIN: i16 = -20000;
const CLAMP_MAX: i16 = 20000;
const CLIPPING_THRESHOLD: i16 = 32767;

// Thread-local storage for recording state
thread_local! {
    static RECORDING: std::cell::RefCell<Option<(Stream, Arc<Mutex<WavWriter<std::io::BufWriter<std::fs::File>>>>)>> = std::cell::RefCell::new(None);
}

#[tauri::command]
pub fn start_recording(filename: String) -> Result<String, String> {
    RECORDING.with(|recording| {
        let mut recording = recording.borrow_mut();
        if recording.is_some() {
            return Err("Recording is already in progress".into());
        }

        // Validate the filename
        if filename.contains("..") {
            return Err("Invalid filename".into());
        }

        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or("No input device available")?;
        let config = device.default_input_config().map_err(|e| e.to_string())?;

        let channels = config.channels();
        let sample_rate = config.sample_rate().0;

        let spec = WavSpec {
            channels: channels as u16,
            sample_rate: sample_rate as u32,
            bits_per_sample: 16,
            sample_format: HoundSampleFormat::Int, // Use Int for i16 samples
        };

        let writer = hound::WavWriter::create(&filename, spec).map_err(|e| e.to_string())?;
        let writer = Arc::new(Mutex::new(writer));

        let writer_clone = Arc::clone(&writer);

        let err_fn = move |err| {
            eprintln!("An error occurred on stream: {}", err);
        };

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
            _ => return Err("Unsupported sample format".into()),
        }
        .map_err(|e| e.to_string())?;

        stream.play().map_err(|e| e.to_string())?;

        *recording = Some((stream, writer));

        Ok("Recording started".into())
    })
}

#[tauri::command]
pub fn stop_recording() -> Result<String, String> {
    RECORDING.with(|recording| {
        let mut recording = recording.borrow_mut();
        if let Some((stream, _writer)) = recording.take() {
            drop(stream); // Stops the stream
            // _writer is dropped here, finalizing the WAV file
            Ok("Recording stopped".into())
        } else {
            Err("No recording in progress".into())
        }
    })
}

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

#[tauri::command]
pub fn start_auto_record(
    sentences: Vec<String>,
    project_directory: String, // Change &str to String
    silence_threshold: f32,
    silence_duration: u64,
    silence_padding: u64,
    window: tauri::Window,
    state: State<Arc<Mutex<RecordingState>>>,
) -> Result<(), String> {
    println!("Starting auto-recording");

    // Set is_auto_recording to true before starting the thread
    {
        let mut recording_state = state.lock().unwrap();
        recording_state.is_auto_recording = true;
    }

    // Clone the inner Arc to move into the thread
    let recording_state_arc = state.inner().clone();

    // Clone project_directory so it can be moved into the thread
    let project_directory_clone = project_directory.clone();

    // Spawn a new thread for the auto-recording process
    std::thread::spawn(move || {
        println!(
            "Starting auto-recording with {} sentences",
            sentences.len()
        );
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .expect("Failed to get default input device");

        // Find a supported configuration
        let supported_configs = device.supported_input_configs().expect("Error querying configs");
        let config = supported_configs
            .filter_map(|config_range| {
                let min_rate = config_range.min_sample_rate().0;
                let max_rate = config_range.max_sample_rate().0;

                [8000, 16000, 32000, 48000].iter().find_map(|&rate| {
                    if rate >= min_rate && rate <= max_rate {
                        Some(config_range.clone().with_sample_rate(cpal::SampleRate(rate)))
                    } else {
                        None
                    }
                })
            })
            .next()
            .expect("No supported sample rate found");

        // Iterate over each sentence
        for (index, sentence) in sentences.iter().enumerate() {
            println!(
                "Starting auto-recording for sentence {}: {}",
                index + 1,
                sentence,
            );

            // Update recording state
            {
                let mut recording_state = recording_state_arc.lock().unwrap();
                if !recording_state.is_auto_recording {
                    break;
                }
                recording_state.current_sentence_index = index;
            }

            // Emit event to frontend
            window
                .emit("auto-record-start-sentence", index)
                .unwrap();

            // Implement recording logic for each sentence
            if let Err(e) = record_sentence(
                &device,
                &config,
                sentence,
                &project_directory_clone, // Pass the cloned project directory here
                silence_threshold,
                silence_duration,
                silence_padding,
            ) {
                eprintln!("Error recording sentence {}: {}", index + 1, e);
                break;
            }

            // Emit event to frontend
            window
                .emit("auto-record-finish-sentence", index)
                .unwrap();
        }

        // Emit completion event
        window.emit("auto-record-complete", true).unwrap();

        // Update recording state
        let mut recording_state = recording_state_arc.lock().unwrap();
        recording_state.is_auto_recording = false;
    });

    Ok(())
}

fn record_sentence(
    device: &cpal::Device,
    config: &SupportedStreamConfig,
    sentence: &str,
    project_directory: &str, // Pass project directory from the frontend
    _silence_threshold: f32,
    silence_duration_ms: u64,
    silence_padding_ms: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    // Configure the audio input stream
    let stream_config = config.config();
    let sample_rate = stream_config.sample_rate.0 as usize;
    let channels = stream_config.channels as usize;
    let silence_duration = Duration::from_millis(silence_duration_ms);
    let silence_padding = Duration::from_millis(silence_padding_ms);

    // Get the home directory (or use a provided project directory)
    let project_dir = match home_dir() {
        Some(home) => home.join(project_directory),
        None => std::path::PathBuf::from(project_directory),
    };

    // Ensure the directory exists
    std::fs::create_dir_all(&project_dir)?;

    // Save the file to the project directory
    let path = project_dir.join(format!("{}.wav", sentence.trim().replace(" ", "_")));
    let spec = WavSpec {
        channels: channels as u16,
        sample_rate: sample_rate as u32,
        bits_per_sample: 16,
        sample_format: HoundSampleFormat::Int,
    };
    let writer = WavWriter::create(path, spec)?;
    let writer = Arc::new(Mutex::new(writer));

    // State variables
    let active_buffer = Arc::new(Mutex::new(Vec::new()));
    let is_speaking = Arc::new(Mutex::new(false));
    let last_active_time = Arc::new(Mutex::new(Instant::now()));

    let writer_clone = Arc::clone(&writer);
    let active_buffer_clone = Arc::clone(&active_buffer);
    let is_speaking_clone = Arc::clone(&is_speaking);
    let last_active_time_clone = Arc::clone(&last_active_time);

    let stream = match config.sample_format() {
        SampleFormat::I16 => {
            let silence_duration = silence_duration.clone();
            let silence_padding = silence_padding.clone();
            let sample_rate = sample_rate; // Capture sample_rate into the closure
            device.build_input_stream(
                &stream_config,
                move |data: &[i16], _: &cpal::InputCallbackInfo| {
                    let vad_sample_rate = match sample_rate {
                        8000 => SampleRate::Rate8kHz,
                        16000 => SampleRate::Rate16kHz,
                        32000 => SampleRate::Rate32kHz,
                        48000 => SampleRate::Rate48kHz,
                        _ => {
                            eprintln!("Unsupported sample rate: {}", sample_rate);
                            return;
                        }
                    };

                    let frame_length = match vad_sample_rate {
                        SampleRate::Rate8kHz => 160,
                        SampleRate::Rate16kHz => 320,
                        SampleRate::Rate32kHz => 640,
                        SampleRate::Rate48kHz => 960,
                    };

                    let mut vad = Vad::new_with_rate(vad_sample_rate);
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
            )?
        }
        SampleFormat::F32 => {
            let silence_duration = silence_duration.clone();
            let silence_padding = silence_padding.clone();
            let sample_rate = sample_rate; // Capture sample_rate into the closure
            device.build_input_stream(
                &config.config(),
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    let vad_sample_rate = match sample_rate {
                        8000 => SampleRate::Rate8kHz,
                        16000 => SampleRate::Rate16kHz,
                        32000 => SampleRate::Rate32kHz,
                        48000 => SampleRate::Rate48kHz,
                        _ => {
                            eprintln!("Unsupported sample rate: {}", sample_rate);
                            return;
                        }
                    };

                    let frame_length = match vad_sample_rate {
                        SampleRate::Rate8kHz => 80,
                        SampleRate::Rate16kHz => 160,
                        SampleRate::Rate32kHz => 320,
                        SampleRate::Rate48kHz => 480,
                    };

                    let mut vad = Vad::new_with_rate(vad_sample_rate);
                    vad.set_mode(VadMode::VeryAggressive);

                    // Convert f32 samples to i16
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
            )?
        }
        _ => return Err("Unsupported sample format".into()),
    };

    stream.play()?;

    // Wait until silence is detected
    loop {
        let last_active = {
            let last_active_time = last_active_time.lock().unwrap();
            *last_active_time
        };
        println!("last active: {:?}, silence duration: {:#?}", last_active.elapsed(), silence_duration);
        if last_active.elapsed() >= silence_duration {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    // Stop the stream and finalize the WAV file
    drop(stream);
    {
        let mut writer = writer.lock().unwrap();
        writer.flush()?;
    }

    Ok(())
}

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
            continue;
        }

        let clamped_chunk: Vec<i16> = chunk.iter().map(|&sample| sample.clamp(CLAMP_MIN, CLAMP_MAX)).collect();
        let is_voice = vad.is_voice_segment(&clamped_chunk).unwrap_or(false);
        let max_amplitude = chunk.iter().max().unwrap_or(&0);

        let mut last_active = last_active_time.lock().unwrap();
        let mut speaking = is_speaking.lock().unwrap();
        let mut buffer = active_buffer.lock().unwrap();

        if is_voice {
            // Speech detected
            if last_active.elapsed() >= Duration::from_millis(NOISE_GRACE_PERIOD_MS) {
                *speaking = true;
                *last_active = Instant::now();
            }
            buffer.extend_from_slice(chunk);
        } else if *speaking && last_active.elapsed() >= silence_duration {
            // Silence detected beyond threshold
            *speaking = false;
            let padding_samples = (silence_padding.as_secs_f32() * sample_rate as f32) as usize;
            let end_index = buffer.len().saturating_sub(padding_samples);
            let trimmed_audio = &buffer[..end_index];

            let mut writer = writer.lock().unwrap();
            for &sample in trimmed_audio {
                writer.write_sample(sample).unwrap();
            }
            buffer.clear();
            return; // End this sentence
        } else {
            // Buffer silence if still within speaking duration
            buffer.extend_from_slice(chunk);
        }
    }
}

#[tauri::command]
pub fn stop_auto_record(state: State<'_, Arc<Mutex<RecordingState>>>) {
    let mut recording_state = state.lock().unwrap();
    recording_state.is_auto_recording = false;
}

