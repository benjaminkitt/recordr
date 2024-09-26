use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sentence {
    pub id: usize,
    pub text: String,
    pub recorded: bool,
    pub audio_file_path: Option<String>,
}
