pub mod app_state;
pub mod assets;
pub mod commands;
pub mod device;
pub mod errors;
pub mod flasher;
pub mod protocol;
pub mod screen_status;

use app_state::AppState;
use commands::{copy_log, scan_devices, start_flash};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            scan_devices,
            start_flash,
            copy_log
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
