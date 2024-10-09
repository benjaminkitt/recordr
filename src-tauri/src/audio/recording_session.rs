use super::auto_record::AutoRecordState;
use super::config::RecordingState;
use cpal::Stream;
use hound::WavWriter;
use log::{debug, error};
use std::fs::File;
use std::io::BufWriter;
use std::ops::Drop;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub struct RecordingSession {
    pub stream: Option<Stream>,
    pub writer: Arc<Mutex<WavWriter<BufWriter<File>>>>,
    pub path: PathBuf,
    pub state_arc: Arc<Mutex<AutoRecordState>>,
}

impl Drop for RecordingSession {
    fn drop(&mut self) {
        if let Some(stream) = self.stream.take() {
            drop(stream);
        }

        let mut writer = self.writer.lock().unwrap();
        if let Err(e) = writer.flush() {
            error!("Failed to flush writer: {}", e);
        }
        drop(writer);

        let is_stopped = {
            let state = self.state_arc.lock().unwrap();
            state.state == RecordingState::Idle
        };

        if is_stopped {
            debug!("Auto-record stopped. Cleaning up WAV file.");
            if let Err(e) = std::fs::remove_file(&self.path) {
                error!("Failed to remove WAV file: {}", e);
            }
        }
    }
}
