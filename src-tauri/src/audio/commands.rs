use std::sync::{Arc, Mutex};
use tauri::State;
use crate::models::Sentence;
use super::recorder::Recorder;

/// Starts a standard recording and writes to a WAV file.
#[tauri::command]
pub fn start_recording(
    filename: String,
    state: State<Arc<Mutex<Recorder>>>,
) -> Result<String, String> {
    let recorder_state = Arc::clone(state.inner());
    let mut recorder = recorder_state.lock().unwrap();
    recorder.start_recording(filename)
}

/// Stops the current recording and finalizes the WAV file.
#[tauri::command]
pub fn stop_recording(state: State<Arc<Mutex<Recorder>>>) -> Result<String, String> {
    let recorder_state = Arc::clone(state.inner());
    let mut recorder = recorder_state.lock().unwrap();
    recorder.stop_recording()
}

/// Starts the auto-recording process with sentence detection and silence handling.
#[tauri::command]
pub fn start_auto_record(
    sentences: Vec<Sentence>,
    project_directory: String,
    silence_threshold: f32,
    silence_duration: u64,
    silence_padding: u64,
    window: tauri::Window,
    state: State<Arc<Mutex<Recorder>>>,
) -> Result<(), String> {
    let recorder_state = Arc::clone(state.inner());
    {
        let mut recorder = recorder_state.lock().unwrap();
        recorder.start_auto_record(
            sentences.clone(),
            project_directory.clone(),
            silence_threshold,
            silence_duration,
            silence_padding,
            window.clone(),
        )
        .map_err(|e| e.to_string())?;
    }

    Ok(())
}

/// Stops the auto-recording process.
#[tauri::command]
pub fn stop_auto_record(state: State<Arc<Mutex<Recorder>>>) -> Result<(), String> {
    let recorder_state = Arc::clone(state.inner());
    let mut recorder = recorder_state.lock().unwrap();
    recorder.stop_auto_record()
}

/// Pauses the auto-recording process.
#[tauri::command]
pub fn pause_auto_record(state: State<Arc<Mutex<Recorder>>>) -> Result<(), String> {
    let recorder_state = Arc::clone(state.inner());
    let mut recorder = recorder_state.lock().unwrap();
    recorder.pause_auto_record()
}

/// Resumes the auto-recording process.
#[tauri::command]
pub fn resume_auto_record(state: State<Arc<Mutex<Recorder>>>) -> Result<(), String> {
    let recorder_state = Arc::clone(state.inner());
    let mut recorder = recorder_state.lock().unwrap();
    recorder.resume_auto_record()
}