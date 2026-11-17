use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use serde::Serialize;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tauri::{AppHandle, Emitter, Manager};
use crate::{ensure_safe_name, firmware_root};
use crate::tooldownload::{find_tool_executable, tools_root};

fn m_flasher_executable(app: &AppHandle) -> Result<PathBuf, String> {
    let root = tools_root(app)?.join("m-flasher");
    #[cfg(target_os = "windows")]
    let names = ["M-Flasher.exe", "m-flasher.exe"];
    #[cfg(target_os = "macos")]
    let names = ["M-Flasher", "m-flasher"];
    #[cfg(target_os = "linux")]
    let names = ["m-flasher", "M-Flasher"];
    for name in names {
        if let Some(path) = find_tool_executable(&root, name) {
            return Ok(path);
        }
    }
    Err("Download M-Flasher from Settings → Extra Downloads first.".into())
}

fn launch_m_flasher_window(app: &AppHandle, executable: &Path) {
    #[cfg(target_os = "windows")]
    {
        let running = Command::new("tasklist").args(["/FI", "IMAGENAME eq M-Flasher.exe", "/NH"])
            .output().ok().map(|result| String::from_utf8_lossy(&result.stdout).to_lowercase().contains("m-flasher.exe")).unwrap_or(false);
        if running {
            let _ = Command::new("powershell").args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", "$p=Get-Process -Name 'M-Flasher' -ErrorAction SilentlyContinue | Where-Object {$_.MainWindowHandle -ne 0} | Select-Object -First 1; if($p){$null=(New-Object -ComObject WScript.Shell).AppActivate($p.Id)}"])
                .stdout(Stdio::null()).stderr(Stdio::null()).status();
            return;
        }
    }
    #[cfg(target_os = "macos")]
    {
        let running = Command::new("pgrep").args(["-f", "M-Flasher.app/Contents/MacOS"]).status().map(|s| s.success()).unwrap_or(false);
        if running {
            let _ = Command::new("osascript").args(["-e", "tell application \"M-Flasher\" to activate"]).stdout(Stdio::null()).stderr(Stdio::null()).status();
            return;
        }
    }
    #[cfg(target_os = "linux")]
    {
        let running = Command::new("pgrep").args(["-f", "m-flasher"]).status().map(|s| s.success()).unwrap_or(false);
        if running {
            let _ = Command::new("wmctrl").args(["-xa", "m-flasher"]).stdout(Stdio::null()).stderr(Stdio::null()).status();
            return;
        }
    }
    let mut command = Command::new(executable);
    if let Some(parent) = executable.parent() { command.current_dir(parent); }
    let _ = command.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn();
    #[cfg(target_os = "windows")]
    {
        let own = app.get_webview_window("main").and_then(|window| {
            let position = window.outer_position().ok()?;
            let size = window.outer_size().ok()?;
            Some((position.x, position.y, size.width, size.height))
        });
        let (x, y) = own.map(|(x, y, w, h)| (x + (w as i32 - 900) / 2, y + (h as i32 - 640) / 2)).unwrap_or((100, 100));
        let script = format!("Start-Sleep -Milliseconds 700; $p=Get-Process -Name 'M-Flasher' -ErrorAction SilentlyContinue | Where-Object {{$_.MainWindowHandle -ne 0}} | Select-Object -First 1; if($p){{Add-Type -Name W -Namespace F -MemberDefinition '[DllImport(\"user32.dll\")] public static extern bool SetWindowPos(IntPtr h, IntPtr a, int x, int y, int cx, int cy, uint f);'; [F.W]::SetWindowPos($p.MainWindowHandle,[IntPtr]::Zero,{x},{y},900,640,0x0040)}}");
        let encoded = BASE64.encode(script.encode_utf16().flat_map(u16::to_le_bytes).collect::<Vec<_>>());
        let _ = Command::new("powershell").args(["-NoProfile", "-WindowStyle", "Hidden", "-EncodedCommand", &encoded]).stdout(Stdio::null()).stderr(Stdio::null()).status();
    }
    #[cfg(target_os = "macos")]
    { let _ = Command::new("osascript").args(["-e", "tell application \"M-Flasher\" to activate", "-e", "tell application \"System Events\" to tell process \"M-Flasher\" to set size of front window to {900, 640}"]).stdout(Stdio::null()).stderr(Stdio::null()).status(); }
    #[cfg(target_os = "linux")]
    { let _ = Command::new("sh").args(["-c", "sleep 0.7; wmctrl -r 'M-Flasher' -b remove,maximized_vert,maximized_horz -e 0,-1,-1,900,640; wmctrl -a 'M-Flasher'"]).stdout(Stdio::null()).stderr(Stdio::null()).status(); }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct MFlasherFindResult { output: String, device_found: bool, download_mode: bool, device_name: Option<String> }

fn output_indicates_download_mode(output: &str) -> bool {
    let output = output.to_ascii_lowercase();
    output.contains("storagedevice") || output.contains("physicaldrive") || output.contains("uboot")
}

fn ensure_device_in_download_mode(executable: &Path) -> Result<String, String> {
    let find_output = Command::new(executable)
        .arg("find")
        .output()
        .map_err(|error| format!("Could not check the connected device: {error}"))?;
    let find_text = format!("{}{}", String::from_utf8_lossy(&find_output.stdout), String::from_utf8_lossy(&find_output.stderr));
    if output_indicates_download_mode(&find_text) {
        return Ok(find_text);
    }
    if !find_text.to_ascii_lowercase().contains("midi") {
        return Err(if find_text.trim().is_empty() { "Connect a device before flashing.".into() } else { find_text });
    }
    let mode_output = Command::new(executable)
        .arg("download-mode")
        .output()
        .map_err(|error| format!("Could not switch the device to download mode: {error}"))?;
    let mode_text = format!("{}{}", String::from_utf8_lossy(&mode_output.stdout), String::from_utf8_lossy(&mode_output.stderr));
    if !mode_output.status.success() || !output_indicates_download_mode(&mode_text) {
        return Err(if mode_text.trim().is_empty() { "M-Flasher could not switch the device to download mode.".into() } else { mode_text });
    }
    Ok(mode_text)
}

#[tauri::command]
pub(super) fn m_flasher_find(app: AppHandle) -> Result<MFlasherFindResult, String> {
    let executable = m_flasher_executable(&app)?;
    let output = Command::new(executable).arg("find").output().map_err(|error| format!("Could not run M-Flasher find: {error}"))?;
    let text = format!("{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr)).trim().to_string();
    let device_name = text.lines().find_map(|line| {
        let line = line.trim();
        if line.is_empty() { return None; }
        let lower = line.to_lowercase();
        if let Some(start) = lower.find("midi") { return Some(line[start + 4..].trim().to_string()); }
        if lower.contains("storagedevice") { return Some("StorageDevice".to_string()); }
        None
    });
    let lower_text = text.to_lowercase();
    let download_mode = output_indicates_download_mode(&text);
    let device_found = device_name.is_some() || lower_text.contains("physicaldrive");
    if !output.status.success() && text.is_empty() { return Err("M-Flasher could not find a connected device.".into()); }
    Ok(MFlasherFindResult { output: text, device_found, download_mode, device_name })
}

#[tauri::command]
pub(super) fn m_flasher_command(app: AppHandle, action: String) -> Result<String, String> {
    if action != "restart" && action != "download-mode" && action != "info" { return Err("Unsupported M-Flasher command.".into()); }
    let executable = m_flasher_executable(&app)?;
    let output = Command::new(executable).arg(&action).output().map_err(|error| format!("Could not run M-Flasher {action}: {error}"))?;
    let text = format!("{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr)).trim().to_string();
    if output.status.success() { Ok(text) } else { Err(if text.is_empty() { format!("M-Flasher {action} failed.") } else { text }) }
}

#[tauri::command]
pub(super) async fn m_flasher_backup(
    app: AppHandle,
    firmware_id: String,
    version: String,
    operation_id: String,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        m_flasher_backup_sync(app, firmware_id, version, operation_id)
    })
    .await
    .map_err(|error| format!("Could not wait for M-Flasher backup task: {error}"))?
}

fn m_flasher_backup_sync(
    app: AppHandle,
    firmware_id: String,
    version: String,
    operation_id: String,
) -> Result<String, String> {
    ensure_safe_name(&firmware_id)?;
    ensure_safe_name(&version)?;
    ensure_safe_name(&operation_id)?;
    let executable = m_flasher_executable(&app)?;
    let version_dir = firmware_root(&app)?.join(&firmware_id).join(&version);
    if !version_dir.join("firmware.fwsc").is_file() { return Err("Select an installed firmware version to store this save.".into()); }
    let saves_dir = version_dir.join("saves");
    std::fs::create_dir_all(&saves_dir).map_err(|error| format!("Could not create saves folder: {error}"))?;
    let mut child = Command::new(executable)
        .arg("backup")
        .arg(&saves_dir)
        .arg("--yes")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("Could not run M-Flasher backup: {error}"))?;
    let stdout = child.stdout.take().ok_or("Could not read M-Flasher output.")?;
    let stderr = child.stderr.take().ok_or("Could not read M-Flasher progress.")?;
    let stdout_app = app.clone();
    let stdout_operation = operation_id.clone();
    let stdout_reader = std::thread::spawn(move || stream_backup_output(stdout, stdout_app, stdout_operation));
    let stderr_reader = std::thread::spawn(move || stream_backup_output(stderr, app, operation_id));
    let status = child.wait().map_err(|error| format!("Could not wait for M-Flasher backup: {error}"))?;
    let stdout_text = stdout_reader.join().map_err(|_| "Could not collect M-Flasher output.")?;
    let stderr_text = stderr_reader.join().map_err(|_| "Could not collect M-Flasher progress.")?;
    let text = format!("{stdout_text}{stderr_text}").trim().to_string();
    if !status.success() { return Err(if text.is_empty() { "M-Flasher backup failed.".into() } else { text }); }
    Ok(if text.is_empty() { format!("Backup saved under {}.", saves_dir.display()) } else { text })
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct BackupProgressEvent {
    operation_id: String,
    output: String,
    progress: Option<u8>,
}

fn stream_backup_output<R: Read>(mut reader: R, app: AppHandle, operation_id: String) -> String {
    // Read small chunks so each carriage-return progress update is forwarded promptly.
    let mut buffer = [0_u8; 32];
    let mut line = Vec::new();
    let mut collected = String::new();
    loop {
        let count = match reader.read(&mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(count) => count,
        };
        for byte in &buffer[..count] {
            if *byte == b'\r' || *byte == b'\n' {
                if line.is_empty() { continue; }
                let output = String::from_utf8_lossy(&line).into_owned();
                line.clear();
                let progress = output.trim().strip_suffix('%')
                    .and_then(|value| value.trim().parse::<u8>().ok());
                collected.push_str(&output);
                collected.push('\n');
                let _ = app.emit("mflasher-backup-progress", BackupProgressEvent {
                    operation_id: operation_id.clone(),
                    output,
                    progress,
                });
            } else {
                line.push(*byte);
            }
        }
    }
    if !line.is_empty() {
        let output = String::from_utf8_lossy(&line).into_owned();
        let progress = output.trim().strip_suffix('%')
            .and_then(|value| value.trim().parse::<u8>().ok());
        collected.push_str(&output);
        let _ = app.emit("mflasher-backup-progress", BackupProgressEvent {
            operation_id,
            output,
            progress,
        });
    }
    collected
}

#[tauri::command]
pub(super) fn m_flasher_flash_save(
    app: AppHandle,
    firmware_id: String,
    version: String,
    relative_path: String,
) -> Result<String, String> {
    ensure_safe_name(&firmware_id)?;
    ensure_safe_name(&version)?;
    let relative = Path::new(&relative_path);
    if relative.as_os_str().is_empty()
        || relative.is_absolute()
        || relative.components().any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err("Invalid backup path.".into());
    }
    let extension = relative.extension().and_then(|extension| extension.to_str()).unwrap_or_default();
    let command_name = if extension.eq_ignore_ascii_case("bin") {
        "restore"
    } else if extension.eq_ignore_ascii_case("fwsc") {
        "firmware"
    } else {
        return Err("Only .bin backups and .fwsc firmware packages can be flashed.".into());
    };
    let version_root = firmware_root(&app)?.join(&firmware_id).join(&version);
    let saves_root = version_root.join("saves").canonicalize()
        .map_err(|error| format!("Could not locate firmware saves folder: {error}"))?;
    let backup_file = saves_root.join(relative).canonicalize()
        .map_err(|error| format!("Could not locate backup file: {error}"))?;
    if !backup_file.starts_with(&saves_root) || !backup_file.is_file() {
        return Err("The selected backup is outside the firmware saves folder.".into());
    }
    let executable = m_flasher_executable(&app)?;
    launch_m_flasher_window(&app, &executable);
    ensure_device_in_download_mode(&executable)?;
    let output = Command::new(executable)
        .arg(command_name)
        .arg(&backup_file)
        .arg("--yes")
        .arg("--restart")
        .output()
        .map_err(|error| format!("Could not run M-Flasher restore: {error}"))?;
    let text = format!("{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr)).trim().to_string();
    if output.status.success() {
        crate::firmwarehub::set_current_firmware_version(app, firmware_id, version)?;
        Ok(text)
    } else {
        Err(if text.is_empty() { "M-Flasher restore failed.".into() } else { text })
    }
}

#[tauri::command]
pub(super) fn m_flasher_flash_firmware(
    app: AppHandle,
    firmware_id: String,
    version: String,
) -> Result<String, String> {
    ensure_safe_name(&firmware_id)?;
    ensure_safe_name(&version)?;
    let firmware_file = firmware_root(&app).map(|root| root.join(&firmware_id).join(&version).join("firmware.fwsc"))?;
    if !firmware_file.is_file() {
        return Err("That firmware version is not installed.".into());
    }
    let executable = m_flasher_executable(&app)?;
    launch_m_flasher_window(&app, &executable);
    ensure_device_in_download_mode(&executable)?;
    let output = Command::new(executable)
        .arg("firmware")
        .arg(&firmware_file)
        .arg("--yes")
        .arg("--restart")
        .output()
        .map_err(|error| format!("Could not run M-Flasher firmware install: {error}"))?;
    let text = format!("{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr)).trim().to_string();
    if output.status.success() {
        crate::firmwarehub::set_current_firmware_version(app, firmware_id, version)?;
        Ok(text)
    } else {
        Err(if text.is_empty() { "M-Flasher firmware install failed.".into() } else { text })
    }
}

