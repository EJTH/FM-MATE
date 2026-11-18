use serde::Serialize;
use std::collections::HashMap;
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};
use tauri::{AppHandle, Manager};
use uuid::Uuid;
use crate::{ensure_safe_name, firmware_root};
use crate::tooldownload::{find_tool_executable, tools_root};

const FLASH_SIZE: u64 = 1024 * 1024;
static SESSIONS: OnceLock<Mutex<HashMap<String, EmulatorSession>>> = OnceLock::new();

struct EmulatorSession {
    temp_file: PathBuf,
    version_root: PathBuf,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct EmulatorSavePrompt {
    session_id: String,
    suggested_name: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SavedEmulatorSession {
    file_name: String,
    warning: Option<String>,
}

fn sessions() -> &'static Mutex<HashMap<String, EmulatorSession>> {
    SESSIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn remove_temp_file(path: &Path) -> Result<(), String> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("Could not remove temporary emulator file: {error}")),
    }
}

fn emulator_executable(app: &AppHandle) -> Result<PathBuf, String> {
    let emulator_root = tools_root(app)?.join("fm1-emu");
    if !emulator_root.is_dir() {
        return Err("Download FM1-Emu from Settings → Extra Downloads first.".into());
    }
    #[cfg(target_os = "windows")]
    let executable_name = "fm1-live.exe";
    #[cfg(not(target_os = "windows"))]
    let executable_name = "fm1-live";
    find_tool_executable(&emulator_root, executable_name)
        .ok_or_else(|| format!("Could not find {executable_name} in the installed FM1-Emu files."))
}

fn run_emulator(app: &AppHandle, id: &str, version: &str, relative_path: Option<&str>) -> Result<EmulatorSavePrompt, String> {
    ensure_safe_name(id)?;
    ensure_safe_name(version)?;
    let version_root = firmware_root(app)?.join(id).join(version);
    let installed_firmware = version_root.join("firmware.fwsc");
    if !installed_firmware.is_file() {
        return Err("That firmware version is not installed.".into());
    }
    let executable = emulator_executable(app)?;
    let mut firmware_file = installed_firmware;
    let mut source_flash = None;
    if let Some(relative_path) = relative_path {
        let relative = Path::new(relative_path);
        if relative.as_os_str().is_empty()
            || relative.is_absolute()
            || relative.components().any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err("Invalid save path.".into());
        }
        let saves_root = version_root.join("saves").canonicalize()
            .map_err(|error| format!("Could not locate firmware saves folder: {error}"))?;
        let selected = saves_root.join(relative).canonicalize()
            .map_err(|error| format!("Could not locate save file: {error}"))?;
        if !selected.starts_with(&saves_root) || !selected.is_file() {
            return Err("The selected file is outside the firmware saves folder.".into());
        }
        let extension = selected.extension().and_then(|value| value.to_str()).unwrap_or("");
        if extension.eq_ignore_ascii_case("fwsc") {
            firmware_file = selected;
        } else if extension.eq_ignore_ascii_case("bin") {
            if selected.metadata().map_err(|error| format!("Could not inspect save file: {error}"))?.len() != FLASH_SIZE {
                return Err("FM1-Emu requires a raw 1 MiB flash image; portable JSON .fm1save files are not supported.".into());
            }
            source_flash = Some(selected);
        } else {
            return Err("FM1-Emu supports .fwsc firmware and raw .bin flash images.".into());
        }
    }

    let session_id = Uuid::new_v4().to_string();
    let temp_dir = app.path().app_cache_dir()
        .map_err(|error| format!("Could not locate emulator temporary folder: {error}"))?
        .join("emulator-sessions");
    std::fs::create_dir_all(&temp_dir)
        .map_err(|error| format!("Could not create emulator temporary folder: {error}"))?;
    let temp_file = temp_dir.join(format!("{session_id}.fm1save"));
    let result = (|| {
        if let Some(source) = source_flash {
            std::fs::copy(source, &temp_file)
                .map_err(|error| format!("Could not copy the save for FM1-Emu: {error}"))?;
        }
        let mut command = Command::new(&executable);
        command.arg("--usb").arg("--fwsc").arg(firmware_file).arg("--flash").arg(&temp_file);
        if let Some(directory) = executable.parent() {
            command.current_dir(directory);
        }
        let output = command.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped()).output()
            .map_err(|error| format!("Could not run FM1-Emu: {error}"))?;
        if !output.status.success() {
            let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(if detail.is_empty() { format!("FM1-Emu exited with {}.", output.status) } else { format!("FM1-Emu failed: {detail}") });
        }
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains(&format!("{}:", temp_file.display())) {
            return Err(format!("FM1-Emu could not write the flash save: {}", stderr.trim()));
        }
        let size = std::fs::metadata(&temp_file)
            .map_err(|error| format!("FM1-Emu did not produce a flash save: {error}"))?.len();
        if size != FLASH_SIZE {
            return Err("FM1-Emu produced an invalid flash save.".into());
        }
        sessions().lock().map_err(|error| format!("Could not store emulator session: {error}"))?
            .insert(session_id.clone(), EmulatorSession { temp_file: temp_file.clone(), version_root });
        Ok(EmulatorSavePrompt {
            suggested_name: format!("{id}_{version}_{}.bin", &session_id[..8]),
            session_id,
        })
    })();
    if let Err(error) = result {
        let mut message = error;
        let mut partial_path = temp_file.as_os_str().to_os_string();
        partial_path.push(".tmp");
        for path in [&temp_file, &PathBuf::from(partial_path)] {
            if let Err(cleanup) = remove_temp_file(path) {
                message.push_str(&format!(" {cleanup}"));
            }
        }
        return Err(message);
    }
    result
}

#[tauri::command]
pub(super) async fn open_firmware_in_emulator(app: AppHandle, id: String, version: String) -> Result<EmulatorSavePrompt, String> {
    tauri::async_runtime::spawn_blocking(move || run_emulator(&app, &id, &version, None))
        .await.map_err(|error| format!("Could not wait for FM1-Emu: {error}"))?
}

#[tauri::command]
pub(super) async fn open_save_in_emulator(app: AppHandle, firmware_id: String, version: String, relative_path: String) -> Result<EmulatorSavePrompt, String> {
    tauri::async_runtime::spawn_blocking(move || run_emulator(&app, &firmware_id, &version, Some(&relative_path)))
        .await.map_err(|error| format!("Could not wait for FM1-Emu: {error}"))?
}

#[tauri::command]
pub(super) fn finish_emulator_session(session_id: String, name: Option<String>) -> Result<Option<SavedEmulatorSession>, String> {
    let mut sessions = sessions().lock().map_err(|error| format!("Could not access emulator session: {error}"))?;
    let session = sessions.get(&session_id).ok_or("Emulator session not found.")?;
    let Some(name) = name else {
        remove_temp_file(&session.temp_file)?;
        sessions.remove(&session_id);
        return Ok(None);
    };
    let name = name.trim();
    if name.is_empty() || name.ends_with('.') || name.chars().any(|c| c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')) {
        return Err("Enter a valid file name without path separators.".into());
    }
    let file_name = if name.to_ascii_lowercase().ends_with(".bin") { name.to_string() } else { format!("{name}.bin") };
    if !session.version_root.join("firmware.fwsc").is_file() {
        return Err("The firmware version is no longer installed; the temporary save has been kept.".into());
    }
    let saves = session.version_root.join("saves");
    std::fs::create_dir_all(&saves).map_err(|error| format!("Could not create saves folder: {error}"))?;
    let target = saves.join(&file_name);
    let mut source = std::fs::File::open(&session.temp_file).map_err(|error| format!("Could not read temporary save: {error}"))?;
    let mut destination = std::fs::OpenOptions::new().write(true).create_new(true).open(&target)
        .map_err(|error| if error.kind() == std::io::ErrorKind::AlreadyExists {
            "A save with that name already exists. Choose another name.".to_string()
        } else { format!("Could not create save file: {error}") })?;
    let copy_result = std::io::copy(&mut source, &mut destination).and_then(|_| destination.flush());
    drop(destination);
    if let Err(error) = copy_result {
        return match std::fs::remove_file(&target) {
            Ok(()) => Err(format!("Could not save emulator session: {error}")),
            Err(cleanup) => Err(format!("Could not save emulator session: {error}. Could not remove incomplete save: {cleanup}")),
        };
    }
    drop(source);
    let warning = remove_temp_file(&session.temp_file).err();
    sessions.remove(&session_id);
    Ok(Some(SavedEmulatorSession { file_name, warning }))
}
