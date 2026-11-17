use std::path::PathBuf;
use tauri::{AppHandle, Manager};

mod firmwarehub;
mod mflasher;
mod tooldownload;
mod editor_helper;

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

pub(crate) fn ensure_safe_name(name: &str) -> Result<(), String> {
    if name.is_empty()
        || name == "."
        || name == ".."
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err("Invalid file name.".into());
    }
    Ok(())
}

pub(crate) fn firmware_root(app: &AppHandle) -> Result<PathBuf, String> {
    let app_data = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("Could not locate app data folder: {error}"))?;
    let config_path = app_data.join("firmware-folder.json");
    if let Ok(bytes) = std::fs::read(&config_path) {
        if let Ok(config) = serde_json::from_slice::<serde_json::Value>(&bytes) {
            if let Some(path) = config
                .get("path")
                .and_then(serde_json::Value::as_str)
                .filter(|path| !path.trim().is_empty())
            {
                let path = PathBuf::from(path);
                if !path.is_absolute() {
                    return Err("Firmware folder path must be absolute.".into());
                }
                return Ok(path);
            }
        }
    }
    Ok(app_data.join("firmwares"))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .invoke_handler(tauri::generate_handler![
            greet,
            firmwarehub::get_firmware_thumbnail,
            firmwarehub::download_firmware,
            firmwarehub::get_firmware_folder_path,
            firmwarehub::set_firmware_folder_path,
            firmwarehub::get_firmware_folder,
            firmwarehub::open_firmware_folder,
            firmwarehub::open_installed_save_folder,
            firmwarehub::get_firmware_saves_folder,
            firmwarehub::delete_firmware_version,
            firmwarehub::serve_firmware_editor,
            firmwarehub::launch_editor_in_chrome,
            firmwarehub::scan_installed_firmwares,
            firmwarehub::set_current_firmware_version,
            firmwarehub::scan_installed_saves,
            firmwarehub::delete_installed_save,
            tooldownload::get_tools_status,
            tooldownload::check_or_update_tool,
            tooldownload::check_app_update,
            tooldownload::open_firmware_in_emulator,
            tooldownload::try_firmware_in_emulator,
            tooldownload::open_save_in_emulator,
            mflasher::m_flasher_find,
            mflasher::m_flasher_command,
            mflasher::m_flasher_backup,
            mflasher::m_flasher_flash_save,
            mflasher::m_flasher_flash_firmware
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
