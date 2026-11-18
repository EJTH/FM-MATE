use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tauri::AppHandle;
use serde::{Serialize};
use crate::{ensure_safe_name, firmware_root};
use crate::tooldownload::tools_root;
use crate::tooldownload::find_tool_executable;
use crate::utils::{unix_timestamp_secs, temp_file_path};


#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmulatorSavePrompt {
    temp_file: PathBuf,
    suggested_name: String,
}

enum EmulatorMode {
    WaitForClose,
    Fire,
}

fn invoke_emulator(
    app: &AppHandle,
    id: &str,
    version: &str,
    firmware_file: &Path,
    flash_file: Option<&Path>,
    mode: EmulatorMode,
) -> Result<EmulatorSavePrompt, String> {
    let emulator_root = tools_root(app)?.join("fm1-emu");
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
        .arg("--fwsc")
        .arg(firmware_file);
    
    if let Some(flash_file) = flash_file {
        command.arg("--flash").arg(flash_file);
    }
    
    if let Some(directory) = executable.parent() {
        command.current_dir(directory);
    }
    
    command.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    
    match mode {
        EmulatorMode::WaitForClose => {
            let mut child = command.spawn()
                .map_err(|error| format!("Could not open firmware in FM1-Emu: {error}"))?;
            child.wait()
                .map_err(|error| format!("Could not wait for FM1-Emu to close: {error}"))?;
        }
        EmulatorMode::Fire => {
            command.status()
                .map_err(|error| format!("Could not start FM1-Emu: {error}"))?;
        }
    }

    let version_root = firmware_root(&app)?.join(&id).join(&version);
    let saves_dir = version_root.join("saves");
    std::fs::create_dir_all(&saves_dir)
        .map_err(|error| format!("Could not create firmware saves folder: {error}"))?;
    
    let timestamp = unix_timestamp_secs();
    let temp_file = temp_file_path(&saves_dir, "emulator_temp", timestamp);
    let suggested_name = format!("{}_{}.fm1save", id, version);
    let final_path = saves_dir.join(&suggested_name);
    
    std::fs::rename(&temp_file, &final_path)
        .map_err(|error| format!("Could not rename save file: {error}"))?;


    Ok(EmulatorSavePrompt {
        temp_file: final_path,
        suggested_name,
    })
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
        
        invoke_emulator(&app, &id, &version, &firmware_file, None, EmulatorMode::WaitForClose)
    })
    .await
    .map_err(|error| format!("Could not wait for FM1-Emu: {error}"))?
}

#[tauri::command]
pub(super) async fn try_firmware_in_emulator(
    app: AppHandle,
    id: String,
    version: String,
) -> Result<EmulatorSavePrompt, String>{
    tauri::async_runtime::spawn_blocking(move || {
        ensure_safe_name(&id)?;
        ensure_safe_name(&version)?;
        let version_root = firmware_root(&app)?.join(&id).join(&version);
        let firmware_file = version_root.join("firmware.fwsc");
        if !firmware_file.is_file() {
            return Err("That firmware version is not installed.".into());
        }
        Ok(invoke_emulator(&app, &id, &version, &firmware_file, None, EmulatorMode::Fire)?)
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
        
        invoke_emulator(&app, &firmware_id, &version, &firmware_file, Some(&save_file), EmulatorMode::WaitForClose)
    })
    .await
    .map_err(|error| format!("Could not wait for FM1-Emu: {error}"))?
}