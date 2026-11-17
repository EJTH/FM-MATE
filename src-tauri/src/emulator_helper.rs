use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::AppHandle;
use serde::{Serialize};
use serde_json;
use base64::Engine;
use crate::{ensure_safe_name, firmware_root};
use crate::tooldownload::tools_root;
use crate::tooldownload::find_tool_executable;


#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmulatorSavePrompt {
    temp_file: PathBuf,
    suggested_name: String,
}

#[tauri::command]
pub(super) async fn open_firmware_in_emulator(
    app: AppHandle,
    id: String,
    version: String,
) -> Result<EmulatorSavePrompt, String> {
    tauri::async_runtime::spawn_blocking(move || {
        ensure_safe_name(&id)?;
        ensure_safe_name(&version)?;
        let version_root = firmware_root(&app)?.join(&id).join(&version);
        let firmware_file = version_root.join("firmware.fwsc");
        if !firmware_file.is_file() {
            return Err("That firmware version is not installed.".into());
        }
        let saves_dir = version_root.join("saves");
        std::fs::create_dir_all(&saves_dir)
            .map_err(|error| format!("Could not create firmware saves folder: {error}"))?;
        
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let temp_file = saves_dir.join(format!(".emulator_temp_{}.mem", timestamp));
        
        let firmware_name = firmware_file.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("firmware");
        let suggested_name = format!("{}_{}.bin", firmware_name, timestamp);
        
        let emulator_root = tools_root(&app)?.join("fm1-emu");
        if !emulator_root.is_dir() {
            return Err("Download FM1-Emu from Settings → Extra Downloads first.".into());
        }
        #[cfg(target_os = "windows")]
        let executable_name = "fm1-live.exe";
        #[cfg(not(target_os = "windows"))]
        let executable_name = "fm1-live";
        let executable = find_tool_executable(&emulator_root, executable_name)
            .ok_or_else(|| format!("Could not find {executable_name} in the installed FM1-Emu files."))?;
        
        let mut command = Command::new(&executable);
        command
            .arg("--usb")
            .arg("--flash")
            .arg(&temp_file)
            .arg("--fwsc")
            .arg(&firmware_file);
        if let Some(directory) = executable.parent() {
            command.current_dir(directory);
        }
        command.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        
        let mut child = command.spawn()
            .map_err(|error| format!("Could not open firmware in FM1-Emu: {error}"))?;
        
        child.wait()
            .map_err(|error| format!("Could not wait for FM1-Emu to close: {error}"))?;
        
        Ok(EmulatorSavePrompt {
            temp_file,
            suggested_name,
        })
    })
    .await
    .map_err(|error| format!("Could not wait for FM1-Emu: {error}"))?
}

#[tauri::command]
pub(super) async fn try_firmware_in_emulator(
    app: AppHandle,
    id: String,
    version: String,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        ensure_safe_name(&id)?;
        ensure_safe_name(&version)?;
        let version_root = firmware_root(&app)?.join(&id).join(&version);
        let firmware_file = version_root.join("firmware.fwsc");
        if !firmware_file.is_file() {
            return Err("That firmware version is not installed.".into());
        }
        let saves_dir = version_root.join("saves");
        std::fs::create_dir_all(&saves_dir)
            .map_err(|error| format!("Could not create firmware saves folder: {error}"))?;
        let emulator_flash_file = saves_dir.join("emulator_autosave.mem");
        let emulator_root = tools_root(&app)?.join("fm1-emu");
        if !emulator_root.is_dir() {
            return Err("Download FM1-Emu from Settings → Extra Downloads first.".into());
        }
        #[cfg(target_os = "windows")]
        let executable_name = "fm1-live.exe";
        #[cfg(not(target_os = "windows"))]
        let executable_name = "fm1-live";
        let executable = find_tool_executable(&emulator_root, executable_name)
            .ok_or_else(|| format!("Could not find {executable_name} in the installed FM1-Emu files."))?;
        let mut command = Command::new(&executable);
        command.arg("--usb").arg("--flash").arg(&emulator_flash_file).arg("--fwsc").arg(&firmware_file);
        if let Some(directory) = executable.parent() {
            command.current_dir(directory);
        }
        command.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null())
            .status().map_err(|error| format!("Could not start FM1-Emu: {error}"))?;
        Ok(())
    })
    .await
    .map_err(|error| format!("Could not wait for FM1-Emu: {error}"))?
}

#[tauri::command]
pub(super) async fn open_save_in_emulator(
    app: AppHandle,
    firmware_id: String,
    version: String,
    relative_path: String,
) -> Result<EmulatorSavePrompt, String> {
    tauri::async_runtime::spawn_blocking(move || {
        ensure_safe_name(&firmware_id)?;
        ensure_safe_name(&version)?;
        let relative = Path::new(&relative_path);
        if relative.as_os_str().is_empty()
            || relative.is_absolute()
            || relative.components().any(|component| !matches!(component, std::path::Component::Normal(_)))
        {
            return Err("Invalid save path.".into());
        }
        let version_root = firmware_root(&app)?.join(&firmware_id).join(&version);
        let bin_file = version_root.join("firmware.bin");
        let fwsc_file = version_root.join("firmware.fwsc");
        let firmware_file = if bin_file.is_file() {
            bin_file
        } else if fwsc_file.is_file() {
            fwsc_file
        } else {
            return Err("That firmware version is not installed.".into());
        };
        let saves_root = version_root.join("saves").canonicalize()
            .map_err(|error| format!("Could not locate firmware saves folder: {error}"))?;
        let save_file = saves_root.join(relative).canonicalize()
            .map_err(|error| format!("Could not locate save file: {error}"))?;
        if !save_file.starts_with(&saves_root) || !save_file.is_file() {
            return Err("The selected file is outside the firmware saves folder.".into());
        }
        
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        
        let _flash_file = if save_file.extension().and_then(|s| s.to_str()) == Some("mem") {
            save_file.clone()
        } else {
            let filename = save_file.file_name().and_then(|n| n.to_str()).ok_or("Could not get save filename.")?;
            let temp_file = saves_root.join(format!("{}.mem", filename));
            temp_file
        };
        
        let save_stem = save_file.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("save");
        let suggested_name = format!("{}_{}.bin", save_stem, timestamp);
        let temp_file = saves_root.join(format!(".emulator_temp_{}.mem", timestamp));
        
        let emulator_root = tools_root(&app)?.join("fm1-emu");
        if !emulator_root.is_dir() {
            return Err("Download FM1-Emu from Settings → Extra Downloads first.".into());
        }
        #[cfg(target_os = "windows")]
        let executable_name = "fm1-live.exe";
        #[cfg(not(target_os = "windows"))]
        let executable_name = "fm1-live";
        let executable = find_tool_executable(&emulator_root, executable_name)
            .ok_or_else(|| format!("Could not find {executable_name} in the installed FM1-Emu files."))?;
        
        let mut command = Command::new(&executable);
        command.arg("--usb").arg("--flash").arg(&temp_file).arg("--fwsc").arg(&firmware_file);
        if let Some(directory) = executable.parent() {
            command.current_dir(directory);
        }
        command.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        
        let mut child = command.spawn()
            .map_err(|error| format!("Could not open save in FM1-Emu: {error}"))?;
        
        child.wait()
            .map_err(|error| format!("Could not wait for FM1-Emu to close: {error}"))?;
        
        Ok(EmulatorSavePrompt {
            temp_file,
            suggested_name,
        })
    })
    .await
    .map_err(|error| format!("Could not wait for FM1-Emu: {error}"))?
}

fn firmware_root_internal() -> Result<PathBuf, String> {
    std::env::var("APPDATA")
        .or_else(|_| std::env::var("HOME"))
        .map(PathBuf::from)
        .map_err(|_| "Could not determine app data directory".to_string())
        .map(|p| p.join("FM-MATE").join("firmwares"))
}
