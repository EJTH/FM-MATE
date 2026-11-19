use std::path::PathBuf;
use tauri::{AppHandle, Manager};

const DEFAULT_SDK_URL: &str =
    "https://github.com/m-vave/fm1_open/raw/refs/heads/main/sdk/firmware/FM-1_015.fwsc";

mod firmwarehub;
mod app_release;
mod mflasher;
mod tooldownload;
mod editor_helper;
mod emulator_helper;
mod utils;

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

pub(crate) fn default_sdk_firmware_path(app: &AppHandle) -> Result<PathBuf, String> {
    let app_data = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("Could not locate app data folder: {error}"))?;
    Ok(app_data.join("sdk").join("FM-1_015.fwsc"))
}

async fn ensure_default_sdk_downloaded(app: AppHandle) -> Result<(), String> {
    let sdk_file = default_sdk_firmware_path(&app)?;
    if sdk_file.is_file() {
        return Ok(());
    }

    std::fs::create_dir_all(sdk_file.parent().ok_or("Invalid SDK firmware path.")?)
        .map_err(|error| format!("Could not create SDK folder: {error}"))?;

    let response = firmwarehub::make_http_client()?
        .get(DEFAULT_SDK_URL)
        .send()
        .await
        .map_err(|error| format!("Could not download default SDK firmware: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Default SDK firmware download failed: {error}"))?;
    let bytes = response
        .bytes()
        .await
        .map_err(|error| format!("Could not read default SDK firmware download: {error}"))?;
    if bytes.is_empty() {
        return Err("Default SDK firmware download was empty.".into());
    }
    std::fs::write(&sdk_file, &bytes)
        .map_err(|error| format!("Could not save default SDK firmware: {error}"))?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .setup(|app| {
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(error) = ensure_default_sdk_downloaded(app_handle).await {
                    eprintln!("{error}");
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            firmwarehub::get_firmware_thumbnail,
            firmwarehub::clear_thumbnail_cache,
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
            firmwarehub::scan_installed_saves,
            firmwarehub::delete_installed_save,
            tooldownload::get_tools_status,
            tooldownload::check_or_update_tool,
            app_release::check_app_update,
            emulator_helper::open_firmware_in_emulator,
            emulator_helper::open_save_in_emulator,
            emulator_helper::finish_emulator_session,
            mflasher::m_flasher_find,
            mflasher::m_flasher_command,
            mflasher::m_flasher_backup,
            mflasher::m_flasher_flash_save,
            mflasher::m_flasher_flash_firmware,
            mflasher::m_flasher_flash_bootloader
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
