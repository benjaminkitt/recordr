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