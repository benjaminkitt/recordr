use log::info;
use std::sync::{Arc, Mutex};
use tauri::{generate_context, generate_handler};

mod audio;
mod file_utils;
mod models;

use audio::{
    load_audio_file,
    pause_auto_record,
    resume_auto_record,
    start_auto_record,
    start_recording,
    stop_auto_record,
    stop_recording,
    Recorder, // Import the Recorder struct
};

use file_utils::{
    add_recent_project, create_new_project, get_recent_projects, import_sentences, open_project,
    save_project,
};

fn main() {
    // Initialize the logger
    env_logger::init();

    info!("Starting the application");

    // Initialize the Recorder instance inside an Arc and Mutex for shared state
    // management
    let recorder = Arc::new(Mutex::new(Recorder::new()));

    tauri::Builder::default()
        .manage(recorder) // Manage the Recorder instance
        .invoke_handler(generate_handler![
            start_recording,
            stop_recording,
            start_auto_record,
            stop_auto_record,
            pause_auto_record,
            resume_auto_record,
            import_sentences,
            get_recent_projects,
            add_recent_project,
            create_new_project,
            open_project,
            save_project,
            load_audio_file,
        ])
        .run(generate_context!())
        .expect("error while running tauri application");
}
