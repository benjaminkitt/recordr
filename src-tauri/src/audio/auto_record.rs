use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use crate::models::Sentence;
use super::config::{AudioConfig, RecordingState};

// Main AutoRecordState struct
#[derive(Debug)]
pub struct AutoRecordState {
  pub sentences: Vec<Sentence>,
  pub project_directory: String,
  pub silence_threshold: f32,
  pub silence_duration: Duration,
  pub silence_padding: Duration,
  pub current_sentence_index: usize,
  pub audio_config: AudioConfig,
  pub state: RecordingState,
  pub is_speaking: Arc<Mutex<bool>>,
  pub last_active_time: Arc<Mutex<Instant>>,
}

impl AutoRecordState {
    // State transition methods
    pub fn start_recording(&mut self) -> Result<(), &'static str> {
        match self.state {
            RecordingState::Idle => {
                self.state = RecordingState::Recording;
                Ok(())
            },
            _ => Err("Can only start recording from Idle state"),
        }
    }

    pub fn pause_recording(&mut self) -> Result<(), &'static str> {
        match self.state {
            RecordingState::Recording => {
                self.state = RecordingState::Paused;
                Ok(())
            },
            _ => Err("Can only pause from Recording state"),
        }
    }

    pub fn resume_recording(&mut self) -> Result<(), &'static str> {
        match self.state {
            RecordingState::Paused => {
                self.state = RecordingState::Recording;
                Ok(())
            },
            _ => Err("Can only resume from Paused state"),
        }
    }

    pub fn stop_recording(&mut self) -> Result<(), &'static str> {
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
pub struct AutoRecordStateBuilder {
    sentences: Option<Vec<Sentence>>,
    project_directory: Option<String>,
    silence_threshold: Option<f32>,
    silence_duration: Option<Duration>,
    silence_padding: Option<Duration>,
    audio_config: Option<AudioConfig>,
}

impl AutoRecordStateBuilder {
    pub fn new() -> Self {
        Self {
            sentences: None,
            project_directory: None,
            silence_threshold: None,
            silence_duration: None,
            silence_padding: None,
            audio_config: None,
        }
    }

    pub fn sentences(mut self, sentences: Vec<Sentence>) -> Self {
        self.sentences = Some(sentences);
        self
    }

    pub fn project_directory(mut self, project_directory: String) -> Self {
        self.project_directory = Some(project_directory);
        self
    }

    pub fn silence_threshold(mut self, silence_threshold: f32) -> Self {
        self.silence_threshold = Some(silence_threshold);
        self
    }

    pub fn silence_duration(mut self, silence_duration_ms: u64) -> Self {
        self.silence_duration = Some(Duration::from_millis(silence_duration_ms));
        self
    }

    pub fn silence_padding(mut self, silence_padding_ms: u64) -> Self {
        self.silence_padding = Some(Duration::from_millis(silence_padding_ms));
        self
    }

    pub fn audio_config(mut self, audio_config: AudioConfig) -> Self {
        self.audio_config = Some(audio_config);
        self
    }

    pub fn build(self) -> Result<AutoRecordState, String> {
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