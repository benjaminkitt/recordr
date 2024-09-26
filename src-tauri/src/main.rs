use tauri::{generate_context, generate_handler};
use std::sync::{Arc, Mutex};

mod audio;
mod file_utils;
mod models;

use audio::{
    start_recording,
    stop_recording,
    start_auto_record,
    stop_auto_record,
    Recorder, // Import the Recorder struct
};

use file_utils::{
    import_sentences,
    create_new_project,
    open_project,
    save_project,
};

fn main() {
    // Initialize the Recorder instance inside an Arc and Mutex for shared state management
    let recorder = Arc::new(Mutex::new(Recorder::new()));

    tauri::Builder::default()
        .manage(recorder) // Manage the Recorder instance
        .invoke_handler(generate_handler![
            start_recording,
            stop_recording,
            start_auto_record,
            stop_auto_record,
            import_sentences,
            create_new_project,
            open_project,
            save_project,
        ])
        .run(generate_context!())
        .expect("error while running tauri application");
}
