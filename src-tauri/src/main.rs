// src-tauri/src/main.rs

mod audio;

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            audio::start_recording,
            audio::stop_recording,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
