use super::auto_record::AutoRecordState;
use super::config::{AudioChunkWithVAD, AudioEvent, RecordingState};
use super::errors::RecorderError;
use super::recording_session::RecordingSession;
use crate::models::Sentence;
use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::{SampleFormat, Stream};
use crossbeam_channel::{bounded, Receiver, Sender};
use hound::{SampleFormat as HoundSampleFormat, WavSpec, WavWriter};
use log::{debug, error, trace};
use samplerate::{ConverterType, Samplerate};
use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use voice_activity_detector::VoiceActivityDetector;

type AudioStream = Result<Stream, RecorderError>;

/**
 * Record a sentence. This function initializes the recording buffers,
 * builds the audio stream, then waits for two audio events; detection of
 * voice, to signify that the recording has begun, and detection of silence,
 * to determine when to end the sentence recording.
 */
pub fn record_sentence(state_arc: &Arc<Mutex<AutoRecordState>>) -> Result<(), RecorderError> {
    debug!("record_sentence: Starting to record sentence");
    let (sentence, writer, path) = prepare_recording(state_arc)?;
    let (audio_chunks, voice_tx, voice_rx) = initialize_recording_buffers();

    debug!(
        "record_sentence: Recording sentence: {} ({})",
        sentence.id, sentence.text
    );

    let stream = build_audio_stream(state_arc, writer.clone(), audio_chunks.clone(), voice_tx)?;

    let session = RecordingSession {
        stream: Some(stream),
        writer: writer.clone(),
        path: path.clone(),
        state_arc: state_arc.clone(),
    };

    trace!("record_sentence: Recording session initialized, starting stream");
    if let Err(e) = session.stream.as_ref().unwrap().play() {
        error!("record_sentence: Failed to start stream. Error: {}", e);
        return Err(RecorderError::StreamPlayError(e.to_string()));
    }

    let result = (|| {
        wait_for_audio_event(state_arc, AudioEvent::Voice, &voice_rx)?;
        wait_for_audio_event(state_arc, AudioEvent::Silence, &voice_rx)?;
        Ok(())
    })();

    if let Err(e) = &result {
        error!("record_sentence: Error during recording: {:?}", e);
    } else {
        debug!("record_sentence: Successfully recorded sentence");
    }

    result
}

fn prepare_recording(
    state_arc: &Arc<Mutex<AutoRecordState>>,
) -> Result<(Sentence, Arc<Mutex<WavWriter<BufWriter<File>>>>, PathBuf), RecorderError> {
    let state = state_arc.lock().unwrap();
    let sentence = state.sentences[state.current_sentence_index].clone();
    let project_dir = get_or_create_project_directory(&state.project_directory)?;

    debug!("Initializing writer for sentence: {}", sentence.id);

    // Create WAV file path
    let path = project_dir.join(format!("{}.wav", sentence.text.trim().replace(" ", "_")));

    // Create WAV writer
    let spec = WavSpec {
        channels: state.audio_config.supported_config.channels() as u16,
        sample_rate: state.audio_config.sample_rate as u32,
        bits_per_sample: 16,
        sample_format: HoundSampleFormat::Int,
    };

    // Output a debug log of the audio configuration
    debug!("Audio configuration:");
    debug!("  Channels: {}", spec.channels);
    debug!("  Sample rate: {} Hz", spec.sample_rate);
    debug!("  Bits per sample: {}", spec.bits_per_sample);
    debug!("  Sample format: {:?}", spec.sample_format);
    debug!("  Device: {:?}", state.audio_config.device.0.name());

    let writer = Arc::new(Mutex::new(WavWriter::create(&path, spec)?));

    Ok((sentence, writer, path))
}

fn initialize_recording_buffers() -> (Arc<Mutex<Vec<AudioChunkWithVAD>>>, Sender<()>, Receiver<()>)
{
    let audio_chunks: Arc<Mutex<Vec<AudioChunkWithVAD>>> = Arc::new(Mutex::new(Vec::new()));
    let (voice_tx, voice_rx) = bounded(1);
    (audio_chunks, voice_tx, voice_rx)
}

/**
 * This is the main loop that waits for audio events. When an event is
 * received, a break allows the record_sentence function to continue.
 */
fn wait_for_audio_event(
    state_arc: &Arc<Mutex<AutoRecordState>>,
    event: AudioEvent,
    voice_rx: &Receiver<()>,
) -> Result<(), RecorderError> {
    debug!("Waiting for audio event: {:?}", event);
    loop {
        check_recording_state(state_arc)?;

        match event {
            AudioEvent::Voice => {
                if voice_rx.try_recv().is_ok() {
                    trace!("Voice detected");
                    break;
                }
            }
            AudioEvent::Silence => {
                let state = state_arc.lock().unwrap();
                let last_active = *state.last_active_time.lock().unwrap();
                let elapsed = last_active.elapsed();

                if elapsed >= state.silence_duration {
                    trace!("Silence detected");
                    break;
                }
            }
        }

        std::thread::sleep(Duration::from_millis(100));
    }
    debug!("Finished waiting for audio event: {:?}", event);
    Ok(())
}

fn check_recording_state(state_arc: &Arc<Mutex<AutoRecordState>>) -> Result<(), RecorderError> {
    let state = state_arc.lock().unwrap();
    match state.state {
        RecordingState::Paused => Err(RecorderError::RecordingPaused),
        RecordingState::Idle => Err(RecorderError::RecordingStopped),
        RecordingState::Recording => Ok(()),
    }
}

/**
 * Builds an audio input stream and configured the VAD that is used to
 * detect speech.
 */
fn build_audio_stream(
    state_arc: &Arc<Mutex<AutoRecordState>>,
    writer: Arc<Mutex<WavWriter<BufWriter<File>>>>,
    audio_chunks: Arc<Mutex<Vec<AudioChunkWithVAD>>>,
    voice_tx: Sender<()>,
) -> AudioStream {
    debug!("Building audio stream");
    let (sample_format, original_sample_rate) = {
        let state = state_arc.lock().unwrap();
        (
            state.audio_config.supported_config.sample_format(),
            state.audio_config.sample_rate,
        )
    };

    trace!("Audio stream sample format: {:?}", sample_format);

    let err_fn = |err| eprintln!("Stream error: {}", err);

    let chunk_size = get_chunk_size(original_sample_rate)?;
    let downsampled_chunk_size = get_chunk_size(16000)?;
    trace!("Using chunk size of {} for original audio and chunk size of {} for downsampled audio (VAD)", chunk_size, downsampled_chunk_size);
    let mut vad = VoiceActivityDetector::builder()
        .sample_rate(16000)
        .chunk_size(downsampled_chunk_size)
        .build()
        .expect("Failed to build VAD");

    // Create a shared buffer for accumulating data
    let data_buffer = Arc::new(Mutex::new(Vec::new()));

    match sample_format {
        SampleFormat::I16 => {
            let input_data_fn = {
                let state_arc = Arc::clone(state_arc);
                let writer = Arc::clone(&writer);
                let audio_chunks = Arc::clone(&audio_chunks);
                let voice_tx = voice_tx.clone();
                let data_buffer = Arc::clone(&data_buffer);

                move |data: &[i16], _: &cpal::InputCallbackInfo| {
                    trace!("Input callback data length: {}", data.len());
                    let mut converter = Samplerate::new(
                        ConverterType::SincBestQuality,
                        original_sample_rate as u32,
                        16000,
                        1,
                    )
                    .expect("Failed to create Samplerate converter");

                    let chunk_size = get_chunk_size(original_sample_rate).unwrap();
                    let mut buffer = data_buffer.lock().unwrap();

                    // Accumulate the incoming data
                    buffer.extend_from_slice(data);

                    while buffer.len() >= chunk_size {
                        // Split off a chunk of the required size
                        let chunk: Vec<i16> = buffer.drain(..chunk_size).collect();

                        process_audio_chunk(
                            &chunk,
                            &mut vad,
                            &mut converter,
                            &state_arc,
                            &audio_chunks,
                            &writer,
                            &voice_tx,
                            chunk_size,
                        );
                    }
                }
            };

            let state = state_arc.lock().unwrap();
            trace!(
                "Building input stream with config: {:?}",
                state.audio_config.config
            );
            state
                .audio_config
                .device
                .0
                .build_input_stream(&state.audio_config.config, input_data_fn, err_fn)
                .map_err(RecorderError::CpalBuildStreamError)
        }
        SampleFormat::F32 => {
            let input_data_fn = {
                let state_arc = Arc::clone(state_arc);
                let writer = Arc::clone(&writer);
                let audio_chunks = Arc::clone(&audio_chunks);
                let voice_tx = voice_tx.clone();
                let data_buffer = Arc::clone(&data_buffer);

                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    trace!("Input callback data length: {}", data.len());

                    let mut converter = Samplerate::new(
                        ConverterType::SincBestQuality,
                        original_sample_rate as u32,
                        16000,
                        1,
                    )
                    .expect("Failed to create Samplerate converter");

                    // Convert f32 samples to i16
                    let data_i16: Vec<i16> = data
                        .iter()
                        .map(|&sample| (sample * i16::MAX as f32) as i16)
                        .collect();

                    let chunk_size = get_chunk_size(original_sample_rate).unwrap();
                    let mut buffer = data_buffer.lock().unwrap();

                    // Accumulate the incoming data
                    buffer.extend_from_slice(&data_i16);

                    // Process chunks while we have enough data
                    while buffer.len() >= chunk_size {
                        // Split off a chunk of the required size
                        let chunk: Vec<i16> = buffer.drain(..chunk_size).collect();

                        process_audio_chunk(
                            &chunk,
                            &mut vad,
                            &mut converter,
                            &state_arc,
                            &audio_chunks,
                            &writer,
                            &voice_tx,
                            chunk_size,
                        );
                    }
                }
            };

            let state = state_arc.lock().unwrap();
            trace!(
                "Building input stream with config: {:?}",
                state.audio_config.config
            );
            state
                .audio_config
                .device
                .0
                .build_input_stream(&state.audio_config.config, input_data_fn, err_fn)
                .map_err(RecorderError::CpalBuildStreamError)
        }
        _ => Err(RecorderError::Other("Unsupported sample format".into())),
    }
}

fn get_chunk_size(sample_rate: usize) -> Result<usize, RecorderError> {
    let chunk_size = (sample_rate as f32 / 31.25).round() as usize;
    // Ensure chunk_size is a multiple of 256 for compatibility
    let chunk_size = ((chunk_size + 255) / 256) * 256;
    Ok(chunk_size)
}

/**
 * Processes an audio chunk using VAD, calculating the probability of
 * speech as well as keeping track of the elapsed time since silence was
 * detected.
 */
fn process_audio_chunk(
    data: &[i16],
    vad: &mut VoiceActivityDetector,
    converter: &mut Samplerate,
    state_arc: &Arc<Mutex<AutoRecordState>>,
    audio_chunks: &Arc<Mutex<Vec<AudioChunkWithVAD>>>,
    writer: &Arc<Mutex<WavWriter<BufWriter<File>>>>,
    voice_tx: &Sender<()>,
    chunk_size: usize,
) {
    let ratio = converter.ratio();
    let adjusted_chunk_size = (chunk_size as f64 / ratio).ceil() as usize;

    trace!(
        "Adjusted chunk size: {} based on ratio: {}",
        adjusted_chunk_size,
        ratio
    );

    let mut remaining_data = data;

    while !remaining_data.is_empty() {
        let (chunk, rest) = if remaining_data.len() >= adjusted_chunk_size {
            remaining_data.split_at(adjusted_chunk_size)
        } else {
            (remaining_data, &[][..])
        };

        let chunk_f32: Vec<f32> = chunk.iter().map(|&s| s as f32 / 32768.0).collect();
        let downsampled_chunk = { converter.process(&chunk_f32).expect("Failed to downsample") };
        trace!(
            "Original chunk length: {}, Downsampled chunk length: {}",
            chunk_f32.len(),
            downsampled_chunk.len()
        );

        let probability = vad.predict(downsampled_chunk.clone());
        let is_voice = probability >= 0.5;

        {
            let mut chunks = audio_chunks.lock().unwrap();
            chunks.push(AudioChunkWithVAD {
                chunk: chunk.to_vec(),
                is_voice,
            });
        }

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

        trace!("Processing audio chunk: voice_probability: {}, is_voice: {}, speaking: {}, elapsed: {}", probability, is_voice, speaking, elapsed.as_millis());

        if is_voice {
            handle_voice_detected(state_arc, elapsed, voice_tx);
        } else {
            handle_silence_detected(state_arc, audio_chunks, writer, elapsed);
        }

        remaining_data = rest;
    }
}

fn handle_voice_detected(
    state_arc: &Arc<Mutex<AutoRecordState>>,
    elapsed: Duration,
    voice_tx: &Sender<()>,
) {
    {
        let state = state_arc.lock().unwrap();
        *state.last_active_time.lock().unwrap() = Instant::now();
    }

    if elapsed >= Duration::from_millis(200) {
        trace!("Voice detected, notifying voice_tx");
        {
            let state = state_arc.lock().unwrap();
            *state.is_speaking.lock().unwrap() = true;
        }
        let _ = voice_tx.try_send(());
    }
}

fn handle_silence_detected(
    state_arc: &Arc<Mutex<AutoRecordState>>,
    audio_chunks: &Arc<Mutex<Vec<AudioChunkWithVAD>>>,
    writer: &Arc<Mutex<WavWriter<BufWriter<File>>>>,
    elapsed: Duration,
) {
    let silence_duration = {
        let state = state_arc.lock().unwrap();
        state.silence_duration
    };

    trace!(
        "Silence detected, elapsed: {}, silence_duration: {}",
        elapsed.as_millis(),
        silence_duration.as_millis()
    );

    if elapsed >= silence_duration {
        let state = state_arc.lock().unwrap();
        if *state.is_speaking.lock().unwrap() {
            debug!("Silence duration reached, stopping speaking and writing trimmed audio");
            *state.is_speaking.lock().unwrap() = false;
            drop(state);
            write_trimmed_audio(state_arc, audio_chunks, writer);
            debug!("Finished writing trimmed audio");
        }
    }
}

fn write_trimmed_audio(
    state_arc: &Arc<Mutex<AutoRecordState>>,
    audio_chunks: &Arc<Mutex<Vec<AudioChunkWithVAD>>>,
    writer: &Arc<Mutex<WavWriter<BufWriter<File>>>>,
) {
    let (silence_padding, sample_rate) = {
        let state = state_arc.lock().unwrap();
        (state.silence_padding, state.audio_config.sample_rate)
    };

    let padding_samples = (silence_padding.as_secs_f32() * sample_rate as f32) as usize;
    let chunk_size = get_chunk_size(sample_rate).unwrap();
    let chunks = audio_chunks.lock().unwrap();

    /*
     * Finds the start and end indices of the speech portion within the
     * audio chunks.
     *
     * The start index is the index of the first chunk that contains speech,
     * minus one. The end index is the index of the last chunk that
     * contains speech, plus one. This ensures that silence before and
     * after speech is trimmed and replaced with the silence padding.
     */
    let start_index = chunks
        .iter()
        .position(|chunk| chunk.is_voice)
        .unwrap_or(0)
        .saturating_sub(1);

    let end_index = chunks
        .iter()
        .rposition(|chunk| chunk.is_voice)
        .map(|i| (i + 1).min(chunks.len() - 1))
        .unwrap_or(chunks.len().saturating_sub(1));

    if start_index >= chunks.len() || end_index >= chunks.len() || start_index > end_index {
        debug!(
            "Invalid chunk indices: start={}, end={}, total chunks={}",
            start_index,
            end_index,
            chunks.len()
        );
        return; // or handle this error case as appropriate
    }

    let mut writer = writer.lock().unwrap();

    let mut write_samples = |chunks: &[AudioChunkWithVAD]| {
        for chunk in chunks {
            for &sample in &chunk.chunk {
                if let Err(e) = writer.write_sample(sample) {
                    error!("Failed to write sample: {}", e);
                    return Err(e);
                }
            }
        }
        Ok(())
    };

    // Write padding before speech
    if let Err(e) = write_samples(
        &chunks[start_index.saturating_sub(padding_samples / chunk_size)..start_index],
    ) {
        error!("Error writing padding before speech: {}", e);
        return;
    }

    // Write speech
    if let Err(e) = write_samples(&chunks[start_index..=end_index]) {
        error!("Error writing speech: {}", e);
        return;
    }

    // Write padding after speech
    let padding_end = (end_index + 1 + padding_samples / chunk_size).min(chunks.len());
    if let Err(e) = write_samples(&chunks[end_index + 1..padding_end]) {
        error!("Error writing padding after speech: {}", e);
        return;
    }
}

/**
 * Creates or gets the project directory based on the provided path.
 */
fn get_or_create_project_directory(
    project_directory: &str,
) -> Result<std::path::PathBuf, RecorderError> {
    debug!(
        "Getting or creating project directory: {}",
        project_directory
    );
    let project_dir = tauri::api::path::home_dir()
        .map(|home| home.join(project_directory))
        .unwrap_or_else(|| std::path::PathBuf::from(project_directory));

    std::fs::create_dir_all(&project_dir)?;

    Ok(project_dir)
}
