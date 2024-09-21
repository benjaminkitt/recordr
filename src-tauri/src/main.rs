use tauri::{generate_context, generate_handler};
use std::sync::{Arc, Mutex};

mod audio;
mod file_utils;

use audio::{
    start_recording,
    stop_recording,
    start_auto_record,
    stop_auto_record,
    RecordingState,
};

use file_utils::{
    import_sentences,
    create_new_project,
    open_project,
    save_project,
};

fn main() {
    let recording_state = Arc::new(Mutex::new(RecordingState::new()));

    tauri::Builder::default()
        .manage(recording_state)
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
