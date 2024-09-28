use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sentence {
    pub id: usize,
    pub text: String,
    pub recorded: bool,
    pub audio_file_path: Option<String>,
}

impl fmt::Display for Sentence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Sentence {{ id: {}, text: {} }}", self.id, self.text)
    }
}

#[derive(Debug)]
pub enum RecorderError {
    RecordingPaused,
    RecordingStopped,
    IoError(std::io::Error),
    CpalStreamError(cpal::StreamError),
    CpalBuildStreamError(cpal::BuildStreamError),
    CpalPlayStreamError(cpal::PlayStreamError),
    CpalDefaultStreamConfigError(cpal::DefaultStreamConfigError),
    HoundError(hound::Error),
    Other(String),
}

// Implement the Display trait for user-friendly error messages
impl fmt::Display for RecorderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RecorderError::RecordingPaused => write!(f, "Recording paused"),
            RecorderError::RecordingStopped => write!(f, "Recording stopped"),
            RecorderError::IoError(e) => write!(f, "I/O error: {}", e),
            RecorderError::CpalStreamError(e) => write!(f, "Audio stream error: {}", e),
            RecorderError::CpalBuildStreamError(e) => write!(f, "Failed to build audio stream: {}", e),
            RecorderError::CpalPlayStreamError(e) => write!(f, "Failed to play audio stream: {}", e),
            RecorderError::CpalDefaultStreamConfigError(e) => write!(f, "Failed to get default stream config: {}", e),
            RecorderError::HoundError(e) => write!(f, "Audio processing error: {}", e),
            RecorderError::Other(msg) => write!(f, "{}", msg),
        }
    }
}

// Implement the Error trait
impl std::error::Error for RecorderError {}

// Implement From traits for easy conversion from other error types
impl From<std::io::Error> for RecorderError {
    fn from(error: std::io::Error) -> Self {
        RecorderError::IoError(error)
    }
}

impl From<hound::Error> for RecorderError {
    fn from(error: hound::Error) -> Self {
        RecorderError::HoundError(error)
    }
}

impl From<cpal::StreamError> for RecorderError {
    fn from(error: cpal::StreamError) -> Self {
        RecorderError::CpalStreamError(error)
    }
}

impl From<cpal::BuildStreamError> for RecorderError {
    fn from(error: cpal::BuildStreamError) -> Self {
        RecorderError::CpalBuildStreamError(error)
    }
}

impl From<cpal::PlayStreamError> for RecorderError {
    fn from(error: cpal::PlayStreamError) -> Self {
        RecorderError::CpalPlayStreamError(error)
    }
}

impl From<cpal::DefaultStreamConfigError> for RecorderError {
    fn from(error: cpal::DefaultStreamConfigError) -> Self {
        RecorderError::CpalDefaultStreamConfigError(error)
    }
}

impl From<String> for RecorderError {
    fn from(error: String) -> Self {
        RecorderError::Other(error)
    }
}
