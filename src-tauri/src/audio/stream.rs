
use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::{SampleFormat, Stream};
use hound::{WavSpec, WavWriter, SampleFormat as HoundSampleFormat};
use log::{debug, trace, error};
use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use webrtc_vad::{SampleRate, Vad, VadMode};
use crossbeam_channel::{bounded, Receiver, Sender};
use crate::models::Sentence;
use super::auto_record::AutoRecordState;
use super::config::{AudioEvent, RecordingState};
use super::errors::RecorderError;
use super::recording_session::RecordingSession;

type AudioStream = Result<Stream, RecorderError>;


pub fn record_sentence(state_arc: &Arc<Mutex<AutoRecordState>>) -> Result<(), RecorderError> {
  debug!("record_sentence: Starting to record sentence");
  let (sentence, writer, path) = prepare_recording(state_arc)?;
  let (active_buffer, voice_tx, voice_rx) = initialize_recording_buffers();

  debug!("record_sentence: Recording sentence: {} ({})", sentence.id, sentence.text);

  let stream = build_audio_stream(state_arc, writer.clone(), active_buffer.clone(), voice_tx)?;
  
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

fn prepare_recording(state_arc: &Arc<Mutex<AutoRecordState>>) -> Result<(Sentence, Arc<Mutex<WavWriter<BufWriter<File>>>>, PathBuf), RecorderError> {
  let state = state_arc.lock().unwrap();
  let sentence = state.sentences[state.current_sentence_index].clone();
  let project_dir = get_or_create_project_directory(&state.project_directory)?;
  
  debug!("Initializing writer for sentence: {}", sentence.id);

  // Create WAV file path
  let path = project_dir.join(format!("{}.wav", sentence.text.trim().replace(" ", "_")));
  
  // Create WAV writer
  let spec = WavSpec {
      channels: state.audio_config.config.channels() as u16,
      sample_rate: state.audio_config.sample_rate as u32,
      bits_per_sample: 16,
      sample_format: HoundSampleFormat::Int,
  };

  // Add trace logs for audio configuration
  trace!("Audio configuration:");
  trace!("  Channels: {}", spec.channels);
  trace!("  Sample rate: {} Hz", spec.sample_rate);
  trace!("  Bits per sample: {}", spec.bits_per_sample);
  trace!("  Sample format: {:?}", spec.sample_format);
  trace!("  Device: {:?}", state.audio_config.device.0.name());

  let writer = Arc::new(Mutex::new(WavWriter::create(&path, spec)?));
  
  Ok((sentence, writer, path))
}

fn initialize_recording_buffers() -> (Arc<Mutex<Vec<i16>>>, Sender<()>, Receiver<()>) {
  let active_buffer = Arc::new(Mutex::new(Vec::new()));
  let (voice_tx, voice_rx) = bounded(1);
  (active_buffer, voice_tx, voice_rx)
}

fn wait_for_audio_event(state_arc: &Arc<Mutex<AutoRecordState>>, event: AudioEvent, voice_rx: &Receiver<()>) -> Result<(), RecorderError> {
  debug!("Waiting for audio event: {:?}", event);
  loop {
      check_recording_state(state_arc)?;
      
      match event {
          AudioEvent::Voice => {
              if voice_rx.try_recv().is_ok() {
                  trace!("Voice detected");
                  break;
              }
          },
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

/// Builds the audio input stream with VAD (Voice Activity Detection).
fn build_audio_stream(
  state_arc: &Arc<Mutex<AutoRecordState>>,
  writer: Arc<Mutex<WavWriter<BufWriter<File>>>>,
  active_buffer: Arc<Mutex<Vec<i16>>>,
  voice_tx: Sender<()>,
) -> AudioStream {
  debug!("Building audio stream");
  let sample_format = {
      let state = state_arc.lock().unwrap();
      state.audio_config.config.sample_format()
  };

  trace!("Audio stream sample format: {:?}", sample_format);

  let err_fn = |err| eprintln!("Stream error: {}", err);

  match sample_format {
      SampleFormat::I16 => {
          let input_data_fn = {
              let state_arc = Arc::clone(state_arc);
              let writer = Arc::clone(&writer);
              let active_buffer = Arc::clone(&active_buffer);
              let voice_tx = voice_tx.clone();

              move |data: &[i16], _: &cpal::InputCallbackInfo| {
                  trace!("Input callback data length: {}", data.len());
                  let mut vad = Vad::new_with_rate(SampleRate::Rate16kHz);
                  vad.set_mode(VadMode::VeryAggressive);

                  process_audio_chunk(
                      data,
                      &mut vad,
                      &state_arc,
                      &active_buffer,
                      &writer,
                      &voice_tx,
                  );
              }
          };

          let state = state_arc.lock().unwrap();
          trace!("Building input stream with config: {:?}", state.audio_config.config.config());
          state.audio_config.device.0.build_input_stream(
              &state.audio_config.config.config(),
              input_data_fn,
              err_fn,
          )
          .map_err(RecorderError::CpalBuildStreamError)
      }
      SampleFormat::F32 => {
          let input_data_fn = {
              let state_arc = Arc::clone(state_arc);
              let writer = Arc::clone(&writer);
              let active_buffer = Arc::clone(&active_buffer);
              let voice_tx = voice_tx.clone();

              move |data: &[f32], _: &cpal::InputCallbackInfo| {
                trace!("Input callback data length: {}", data.len());
                  let data_i16: Vec<i16> = data
                      .iter()
                      .map(|&sample| (sample * i16::MAX as f32) as i16)
                      .collect();

                  let mut vad = Vad::new_with_rate(SampleRate::Rate16kHz);
                  vad.set_mode(VadMode::VeryAggressive);

                  process_audio_chunk(
                      &data_i16,
                      &mut vad,
                      &state_arc,
                      &active_buffer,
                      &writer,
                      &voice_tx,
                  );
              }
          };

          let state = state_arc.lock().unwrap();
          trace!("Building input stream with config: {:?}", state.audio_config.config.config());
          state.audio_config.device.0.build_input_stream(
              &state.audio_config.config.config(),
              input_data_fn,
              err_fn,
          )
          .map_err(RecorderError::CpalBuildStreamError)
      }
      _ => Err(RecorderError::Other("Unsupported sample format".into())),
  }
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
  let frame_length = 160;

  for chunk in data.chunks(frame_length) {
      trace!("Chunk length: {}, Frame length: {}", chunk.len(), frame_length);
      if chunk.len() < frame_length {
          trace!("Chunk too short, skipping");
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

      trace!("Processing audio chunk: is_voice: {}, speaking: {}, elapsed: {}", is_voice, speaking, elapsed.as_millis());

      if is_voice {
          handle_voice_detected(state_arc, active_buffer, chunk, elapsed, voice_tx);
      } else if speaking {
          handle_silence_detected(state_arc, active_buffer, writer, chunk, elapsed);
      }
  }
}

fn handle_voice_detected(
  state_arc: &Arc<Mutex<AutoRecordState>>,
  active_buffer: &Arc<Mutex<Vec<i16>>>,
  chunk: &[i16],
  elapsed: Duration,
  voice_tx: &Sender<()>,
) {
  // Voice detected, reset last active time
  {
      let state = state_arc.lock().unwrap();
      *state.last_active_time.lock().unwrap() = Instant::now();
  }

  if elapsed >= Duration::from_millis(200) {
      trace!("Voice detected, notifying voice_tx");
      // Set speaking to true and send voice signal
      {
          let state = state_arc.lock().unwrap();
          *state.is_speaking.lock().unwrap() = true;
      }
      let _ = voice_tx.try_send(());
  }

  trace!("Extending active buffer with chunk");
  let mut buffer = active_buffer.lock().unwrap();
  buffer.extend_from_slice(chunk);
}

fn handle_silence_detected(
  state_arc: &Arc<Mutex<AutoRecordState>>,
  active_buffer: &Arc<Mutex<Vec<i16>>>,
  writer: &Arc<Mutex<WavWriter<BufWriter<File>>>>,
  chunk: &[i16],
  elapsed: Duration,
) {
  let silence_duration = {
      let state = state_arc.lock().unwrap();
      state.silence_duration
  };

  trace!("Silence detected, elapsed: {}, silence_duration: {}", elapsed.as_millis(), silence_duration.as_millis());
  if elapsed >= silence_duration {
      // Silence duration reached, stop speaking and write trimmed audio
      trace!("Silence duration reached, stopping speaking and writing trimmed audio");
      {
          let state = state_arc.lock().unwrap();
          *state.is_speaking.lock().unwrap() = false;
      }

      write_trimmed_audio(state_arc, active_buffer, writer);
  } else {
      trace!("Extending active buffer with chunk");
      let mut buffer = active_buffer.lock().unwrap();
      buffer.extend_from_slice(chunk);
  }
}

fn write_trimmed_audio(
  state_arc: &Arc<Mutex<AutoRecordState>>,
  active_buffer: &Arc<Mutex<Vec<i16>>>,
  writer: &Arc<Mutex<WavWriter<BufWriter<File>>>>,
) {
  let (silence_padding, sample_rate) = {
      let state = state_arc.lock().unwrap();
      (state.silence_padding, state.audio_config.sample_rate)
  };

  let padding_samples = (silence_padding.as_secs_f32() * sample_rate as f32) as usize;
  let mut buffer = active_buffer.lock().unwrap();
  let end_index = buffer.len().saturating_sub(padding_samples);
  let trimmed_audio = &buffer[..end_index];

  debug!("Writing trimmed audio to file");
  let mut writer = writer.lock().unwrap();
  for &sample in trimmed_audio {
      writer.write_sample(sample).unwrap_or_else(|e| {
          eprintln!("Failed to write sample: {}", e);
      });
  }

  buffer.clear();
}

/// Creates or gets the project directory based on the provided path.
fn get_or_create_project_directory(
  project_directory: &str,
) -> Result<std::path::PathBuf, RecorderError> {
  debug!("Getting or creating project directory: {}", project_directory);
  let project_dir = tauri::api::path::home_dir()
      .map(|home| home.join(project_directory))
      .unwrap_or_else(|| std::path::PathBuf::from(project_directory));

  std::fs::create_dir_all(&project_dir)?;

  Ok(project_dir)
}