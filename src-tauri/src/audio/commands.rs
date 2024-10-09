use super::recorder::Recorder;
use crate::models::Sentence;
use std::fs::File;
use std::io::Read;
use std::sync::{Arc, Mutex};
use tauri::State;

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

/// Starts the auto-recording process with sentence detection and silence
/// handling.
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
        recorder
            .start_auto_record(
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

#[tauri::command]
pub fn load_audio_file(file_path: String) -> Result<Vec<u8>, String> {
    let mut file = File::open(&file_path).map_err(|e| e.to_string())?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer).map_err(|e| e.to_string())?;
    Ok(buffer)
}
