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

use chrono::{DateTime, Utc};

#[derive(Serialize, Deserialize)]
pub struct ProjectMetadata {
    pub name: String,
    pub created_version: String,
    pub last_updated_version: String,
    pub created_at: DateTime<Utc>,
    pub last_modified: DateTime<Utc>,
    pub directory: String,
}

#[derive(Serialize, Deserialize)]
pub struct Project {
    pub metadata: ProjectMetadata,
    pub sentences: Vec<Sentence>,
}
