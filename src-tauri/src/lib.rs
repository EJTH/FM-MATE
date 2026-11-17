use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};
use std::io::{Cursor, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tar::Archive as TarArchive;
use tauri::{AppHandle, Manager};
use zip::ZipArchive;

static EDITOR_SERVER: OnceLock<Mutex<Option<(String, Arc<RwLock<PathBuf>>)>>> = OnceLock::new();

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

fn ensure_safe_name(name: &str) -> Result<(), String> {
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

fn firmware_root(app: &AppHandle) -> Result<PathBuf, String> {
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

#[tauri::command]
fn get_firmware_folder_path(app: AppHandle) -> Result<String, String> {
    let folder = firmware_root(&app)?;
    std::fs::create_dir_all(&folder)
        .map_err(|error| format!("Could not create firmware folder: {error}"))?;
    let folder = folder.canonicalize().unwrap_or(folder);
    Ok(folder.to_string_lossy().into_owned())
}

#[tauri::command]
fn set_firmware_folder_path(app: AppHandle, path: String) -> Result<String, String> {
    let path = if path.trim().is_empty() {
        app.path()
            .app_data_dir()
            .map_err(|error| format!("Could not locate app data folder: {error}"))?
            .join("firmwares")
    } else {
        let path = PathBuf::from(path.trim());
        if !path.is_absolute() {
            return Err("Enter an absolute folder path.".into());
        }
        path
    };
    std::fs::create_dir_all(&path)
        .map_err(|error| format!("Could not create firmware folder: {error}"))?;
    let path = path
        .canonicalize()
        .map_err(|error| format!("Could not resolve firmware folder: {error}"))?;
    let app_data = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("Could not locate app data folder: {error}"))?;
    std::fs::write(
        app_data.join("firmware-folder.json"),
        serde_json::to_vec_pretty(&serde_json::json!({ "path": path }))
            .map_err(|error| error.to_string())?,
    )
    .map_err(|error| format!("Could not save firmware folder: {error}"))?;
    Ok(path.to_string_lossy().into_owned())
}

fn decode_url_path_segment(segment: &str) -> Option<String> {
    let bytes = segment.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len() {
                return None;
            }
            let hex = std::str::from_utf8(&bytes[index + 1..index + 3]).ok()?;
            decoded.push(u8::from_str_radix(hex, 16).ok()?);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).ok()
}

fn content_type_for(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "html" | "htm" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" | "map" => "application/json; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "ico" => "image/x-icon",
        "wasm" => "application/wasm",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "txt" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

fn write_editor_response(
    stream: &mut TcpStream,
    status: &str,
    mime: &str,
    body: &[u8],
    head_only: bool,
) {
    let header = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(header.as_bytes());
    if !head_only {
        let _ = stream.write_all(body);
    }
}

fn handle_editor_request(mut stream: TcpStream, firmware_root: &Path) {
    let mut request = [0_u8; 8192];
    let Ok(size) = stream.read(&mut request) else {
        return;
    };
    let Ok(request) = std::str::from_utf8(&request[..size]) else {
        return;
    };
    let Some(line) = request.lines().next() else {
        return;
    };
    let mut parts = line.split_whitespace();
    let Some(method) = parts.next() else { return };
    let Some(target) = parts.next() else { return };
    let head_only = method == "HEAD";
    if method != "GET" && !head_only {
        write_editor_response(
            &mut stream,
            "405 Method Not Allowed",
            "text/plain",
            b"Method not allowed",
            false,
        );
        return;
    }
    let path = target.split('?').next().unwrap_or("");
    let segments: Vec<_> = path.split('/').filter(|part| !part.is_empty()).collect();
    if segments.len() < 4 || segments[0] != "edit" || segments[3] != "editor" {
        write_editor_response(
            &mut stream,
            "404 Not Found",
            "text/plain",
            b"Not found",
            head_only,
        );
        return;
    }
    let Some(id) = decode_url_path_segment(segments[1]) else {
        return;
    };
    let Some(version) = decode_url_path_segment(segments[2]) else {
        return;
    };
    if ensure_safe_name(&id).is_err() || ensure_safe_name(&version).is_err() {
        write_editor_response(
            &mut stream,
            "400 Bad Request",
            "text/plain",
            b"Bad path",
            head_only,
        );
        return;
    }

    let editor_root = firmware_root.join(&id).join(&version).join("editor");
    let mut file_path = editor_root.join("editor.html");
    if segments.len() > 4 {
        let mut relative = PathBuf::new();
        for segment in &segments[4..] {
            let Some(decoded) = decode_url_path_segment(segment) else {
                return;
            };
            if decoded.is_empty()
                || decoded == "."
                || decoded == ".."
                || decoded.contains('/')
                || decoded.contains('\\')
            {
                write_editor_response(
                    &mut stream,
                    "400 Bad Request",
                    "text/plain",
                    b"Bad path",
                    head_only,
                );
                return;
            }
            relative.push(decoded);
        }
        file_path = editor_root.join(relative);
    }

    let Ok(canonical_root) = editor_root.canonicalize() else {
        write_editor_response(
            &mut stream,
            "404 Not Found",
            "text/plain",
            b"Editor not found",
            head_only,
        );
        return;
    };
    let Ok(canonical_file) = file_path.canonicalize() else {
        write_editor_response(
            &mut stream,
            "404 Not Found",
            "text/plain",
            b"File not found",
            head_only,
        );
        return;
    };
    if !canonical_file.starts_with(&canonical_root) || !canonical_file.is_file() {
        write_editor_response(
            &mut stream,
            "404 Not Found",
            "text/plain",
            b"File not found",
            head_only,
        );
        return;
    }
    match std::fs::read(&canonical_file) {
        Ok(body) => write_editor_response(
            &mut stream,
            "200 OK",
            content_type_for(&canonical_file),
            &body,
            head_only,
        ),
        Err(_) => write_editor_response(
            &mut stream,
            "500 Internal Server Error",
            "text/plain",
            b"Could not read editor file",
            head_only,
        ),
    }
}

fn ensure_editor_server(firmware_root: &Path) -> Result<String, String> {
    let server = EDITOR_SERVER.get_or_init(|| Mutex::new(None));
    let mut server = server
        .lock()
        .map_err(|_| "Editor server lock failed.".to_string())?;
    if let Some((url, root)) = server.as_ref() {
        *root
            .write()
            .map_err(|_| "Editor server path lock failed.".to_string())? =
            firmware_root.to_path_buf();
        return Ok(url.clone());
    }
    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|error| format!("Could not start local editor server: {error}"))?;
    let address = listener.local_addr().map_err(|error| error.to_string())?;
    let root = Arc::new(RwLock::new(firmware_root.to_path_buf()));
    let request_root = Arc::clone(&root);
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let root = Arc::clone(&request_root);
            std::thread::spawn(move || {
                let Ok(firmware_root) = root.read().map(|path| path.clone()) else {
                    return;
                };
                handle_editor_request(stream, &firmware_root);
            });
        }
    });
    let url = format!("http://{address}");
    *server = Some((url.clone(), root));
    Ok(url)
}

fn make_http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent("FM-MATE")
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|error| format!("Could not create HTTP client: {error}"))
}

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
    assets: Vec<GithubReleaseAsset>,
}

#[derive(Deserialize)]
struct GithubReleaseAsset {
    name: String,
    browser_download_url: String,
    size: u64,
    #[serde(default)]
    content_type: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolStatus {
    id: String,
    installed: bool,
    version: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolUpdateResult {
    status: String,
    version: String,
}

#[derive(Serialize, Deserialize)]
struct InstalledToolMarker {
    version: String,
    platform: String,
}

#[derive(Clone, Copy)]
enum ToolArchiveKind {
    Zip,
    TarGz,
    Dmg,
}

fn current_tool_platform() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}

fn asset_archive_kind(name: &str, content_type: &str) -> Option<ToolArchiveKind> {
    let name = name.to_ascii_lowercase();
    let content_type = content_type.to_ascii_lowercase();
    if content_type == "application/x-apple-diskimage" {
        Some(ToolArchiveKind::Dmg)
    } else if name.ends_with(".tar.gz")
        || name.ends_with(".tgz")
        || content_type == "application/gzip"
        || content_type == "application/x-gzip"
    {
        Some(ToolArchiveKind::TarGz)
    } else if name.ends_with(".zip")
        || content_type == "application/zip"
        || content_type == "application/x-zip-compressed"
    {
        Some(ToolArchiveKind::Zip)
    } else if name.ends_with(".dmg") {
        Some(ToolArchiveKind::Dmg)
    } else {
        None
    }
}

fn release_asset_matches_platform(name: &str, content_type: &str, id: &str) -> bool {
    let name = name.to_ascii_lowercase();
    let content_type = content_type.to_ascii_lowercase();
    let tool_matches = match id {
        "m-flasher" => {
            name.contains("m-flasher")
                || name.ends_with(".dmg")
                || (cfg!(target_os = "macos") && content_type == "application/x-apple-diskimage")
        }
        "fm1-emu" => name.contains("fm1-emu") && !name.contains("fm1-plugin"),
        _ => false,
    };
    if !tool_matches {
        return false;
    }

    let (os_matches, arch_matches) = if cfg!(target_os = "windows") {
        let is_windows =
            name.contains("windows") || name.contains("win32") || name.contains("win64");
        let is_arm = name.contains("arm64") || name.contains("aarch64");
        let is_x64 = name.contains("x86_64")
            || name.contains("x86-64")
            || name.contains("amd64")
            || name.contains("x64");
        let arch_matches = if is_arm {
            std::env::consts::ARCH == "aarch64"
        } else if is_x64 {
            std::env::consts::ARCH == "x86_64"
        } else {
            std::env::consts::ARCH == "x86_64"
        };
        (is_windows, arch_matches)
    } else if cfg!(target_os = "macos") {
        let is_macos = name.contains("macos")
            || name.contains("macosx")
            || name.contains("darwin")
            || name.contains("osx")
            || name.ends_with(".dmg")
            || (id == "m-flasher" && content_type == "application/x-apple-diskimage");
        let is_arm = name.contains("arm64") || name.contains("aarch64");
        let is_x64 = name.contains("x86_64")
            || name.contains("x86-64")
            || name.contains("amd64")
            || name.contains("x64");
        let arch_matches = if is_arm {
            std::env::consts::ARCH == "aarch64"
        } else if is_x64 {
            std::env::consts::ARCH == "x86_64"
        } else {
            true
        };
        (is_macos, arch_matches)
    } else if cfg!(target_os = "linux") {
        let is_linux = name.contains("linux");
        let is_arm = name.contains("arm64") || name.contains("aarch64");
        let is_x64 = name.contains("x86_64")
            || name.contains("x86-64")
            || name.contains("amd64")
            || name.contains("x64");
        let arch_matches = if is_arm {
            std::env::consts::ARCH == "aarch64"
        } else if is_x64 {
            std::env::consts::ARCH == "x86_64"
        } else {
            true
        };
        (is_linux, arch_matches)
    } else {
        (false, false)
    };
    os_matches && arch_matches
}

fn tool_repository(id: &str) -> Option<&'static str> {
    match id {
        "m-flasher" => Some("jvitkauskas/m-flasher"),
        "fm1-emu" => Some("joakimeriksson/fm1-emu"),
        _ => None,
    }
}

fn tools_root(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|error| format!("Could not locate app data folder: {error}"))?
        .join("tools"))
}

fn find_tool_executable(directory: &Path, expected_name: &str) -> Option<PathBuf> {
    let entries = std::fs::read_dir(directory).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(kind) = entry.file_type() else { continue };
        if kind.is_dir() {
            if let Some(found) = find_tool_executable(&path, expected_name) {
                return Some(found);
            }
        } else if kind.is_file()
            && path.file_name().and_then(|name| name.to_str()) == Some(expected_name)
        {
            return Some(path);
        }
    }
    None
}

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
struct MFlasherFindResult { output: String, device_found: bool, download_mode: bool, device_name: Option<String> }

#[tauri::command]
fn m_flasher_find(app: AppHandle) -> Result<MFlasherFindResult, String> {
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
    let download_mode = text.to_lowercase().contains("storagedevice");
    let device_found = device_name.is_some() || text.to_lowercase().contains("physicaldrive");
    if !output.status.success() && text.is_empty() { return Err("M-Flasher could not find a connected device.".into()); }
    Ok(MFlasherFindResult { output: text, device_found, download_mode, device_name })
}

#[tauri::command]
fn m_flasher_command(app: AppHandle, action: String) -> Result<String, String> {
    if action != "restart" && action != "info" { return Err("Unsupported M-Flasher command.".into()); }
    let executable = m_flasher_executable(&app)?;
    let output = Command::new(executable).arg(&action).output().map_err(|error| format!("Could not run M-Flasher {action}: {error}"))?;
    let text = format!("{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr)).trim().to_string();
    if output.status.success() { Ok(text) } else { Err(if text.is_empty() { format!("M-Flasher {action} failed.") } else { text }) }
}

#[tauri::command]
fn m_flasher_backup(app: AppHandle, firmware_id: String, version: String) -> Result<String, String> {
    ensure_safe_name(&firmware_id)?;
    ensure_safe_name(&version)?;
    let executable = m_flasher_executable(&app)?;
    launch_m_flasher_window(&app, &executable);
    let version_dir = firmware_root(&app)?.join(&firmware_id).join(&version);
    if !version_dir.join("firmware.fwsc").is_file() { return Err("Select an installed firmware version to store this save.".into()); }
    let saves_dir = version_dir.join("saves");
    std::fs::create_dir_all(&saves_dir).map_err(|error| format!("Could not create saves folder: {error}"))?;
    let output = Command::new(executable).arg("backup").arg(&saves_dir).arg("--yes").output().map_err(|error| format!("Could not run M-Flasher backup: {error}"))?;
    let text = format!("{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr)).trim().to_string();
    if !output.status.success() { return Err(if text.is_empty() { "M-Flasher backup failed.".into() } else { text }); }
    Ok(if text.is_empty() { format!("Backup saved under {}.", saves_dir.display()) } else { text })
}

#[tauri::command]
fn open_firmware_in_emulator(app: AppHandle, id: String, version: String) -> Result<(), String> {
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
    let emulator_flash_file = saves_dir.join("emulator.fm1save");
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
        .arg("--flash")
        .arg(&emulator_flash_file)
        .arg("--fwsc")
        .arg(&firmware_file);
    if let Some(directory) = executable.parent() {
        command.current_dir(directory);
    }
    command.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null())
        .spawn().map(|_| ())
        .map_err(|error| format!("Could not open firmware in FM1-Emu: {error}"))
}

#[tauri::command]
fn get_tools_status(app: AppHandle) -> Result<Vec<ToolStatus>, String> {
    let root = tools_root(&app)?;
    std::fs::create_dir_all(&root)
        .map_err(|error| format!("Could not create tools folder: {error}"))?;
    Ok(["m-flasher", "fm1-emu"]
        .into_iter()
        .map(|id| {
            let tool_dir = root.join(id);
            let marker = std::fs::read(tool_dir.join(".fm-mate-tool.json"))
                .ok()
                .and_then(|bytes| serde_json::from_slice::<InstalledToolMarker>(&bytes).ok());
            ToolStatus {
                id: id.to_string(),
                installed: tool_dir.is_dir(),
                version: marker
                    .filter(|value| value.platform == current_tool_platform())
                    .map(|value| value.version),
            }
        })
        .collect())
}

fn extract_tool_zip(bytes: &[u8], destination: &Path) -> Result<(), String> {
    let mut archive = ZipArchive::new(Cursor::new(bytes))
        .map_err(|error| format!("Could not open tool ZIP: {error}"))?;
    let mut unpacked_size = 0_u64;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| format!("Could not read ZIP entry: {error}"))?;
        unpacked_size = unpacked_size.saturating_add(entry.size());
        if unpacked_size > 1024 * 1024 * 1024 {
            return Err("The extracted tool is larger than 1 GiB.".into());
        }
        let relative = entry
            .enclosed_name()
            .ok_or_else(|| "The tool ZIP contains an unsafe file path.".to_string())?
            .to_path_buf();
        if relative.as_os_str().is_empty() {
            continue;
        }
        let output = destination.join(relative);
        if entry.is_dir() {
            std::fs::create_dir_all(&output)
                .map_err(|error| format!("Could not create tool folder: {error}"))?;
            continue;
        }
        if let Some(parent) = output.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("Could not create tool folder: {error}"))?;
        }
        let mut file = std::fs::File::create(&output)
            .map_err(|error| format!("Could not write tool file: {error}"))?;
        std::io::copy(&mut entry, &mut file)
            .map_err(|error| format!("Could not extract tool file: {error}"))?;
        #[cfg(unix)]
        if let Some(mode) = entry.unix_mode() {
            use std::os::unix::fs::PermissionsExt;
            let _ =
                std::fs::set_permissions(&output, std::fs::Permissions::from_mode(mode & 0o777));
        }
    }
    Ok(())
}

fn extract_tool_tar_gz(bytes: &[u8], destination: &Path) -> Result<(), String> {
    let decoder = GzDecoder::new(Cursor::new(bytes));
    let mut archive = TarArchive::new(decoder);
    let mut unpacked_size = 0_u64;
    let entries = archive
        .entries()
        .map_err(|error| format!("Could not read tool tarball: {error}"))?;
    for entry in entries {
        let mut entry =
            entry.map_err(|error| format!("Could not read tool tarball entry: {error}"))?;
        unpacked_size = unpacked_size.saturating_add(
            entry
                .header()
                .size()
                .map_err(|error| format!("Could not read tool file size: {error}"))?,
        );
        if unpacked_size > 1024 * 1024 * 1024 {
            return Err("The extracted tool is larger than 1 GiB.".into());
        }
        let archive_path = entry
            .path()
            .map_err(|error| format!("Could not read tool path: {error}"))?;
        let mut relative = PathBuf::new();
        for component in archive_path.components() {
            match component {
                std::path::Component::Normal(part) => relative.push(part),
                std::path::Component::CurDir => {}
                _ => return Err("The tool tarball contains an unsafe file path.".into()),
            }
        }
        if relative.as_os_str().is_empty() {
            continue;
        }
        let output = destination.join(relative);
        let entry_type = entry.header().entry_type();
        if entry_type.is_dir() {
            std::fs::create_dir_all(&output)
                .map_err(|error| format!("Could not create tool folder: {error}"))?;
        } else if entry_type.is_file() {
            if let Some(parent) = output.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|error| format!("Could not create tool folder: {error}"))?;
            }
            let mut file = std::fs::File::create(&output)
                .map_err(|error| format!("Could not write tool file: {error}"))?;
            std::io::copy(&mut entry, &mut file)
                .map_err(|error| format!("Could not extract tool file: {error}"))?;
            #[cfg(unix)]
            if let Ok(Some(mode)) = entry.header().mode() {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(
                    &output,
                    std::fs::Permissions::from_mode(mode & 0o777),
                );
            }
        } else {
            return Err("The tool tarball contains an unsupported link or special file.".into());
        }
    }
    Ok(())
}

fn extract_tool_dmg(bytes: &[u8], destination: &Path) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let image_path = destination.join("tool-release.dmg");
        std::fs::write(&image_path, bytes)
            .map_err(|error| format!("Could not stage macOS disk image: {error}"))?;
        let mount_path = destination.join(".mounted-image");
        std::fs::create_dir_all(&mount_path)
            .map_err(|error| format!("Could not prepare disk image mount: {error}"))?;
        let mounted = Command::new("hdiutil")
            .args(["attach", "-nobrowse", "-readonly", "-mountpoint"])
            .arg(&mount_path)
            .arg(&image_path)
            .output()
            .map_err(|error| format!("Could not mount macOS tool image: {error}"))?;
        if !mounted.status.success() {
            let message = String::from_utf8_lossy(&mounted.stderr).trim().to_string();
            return Err(if message.is_empty() {
                "Could not mount macOS tool image.".into()
            } else {
                message
            });
        }
        let result = (|| {
            let app_bundle = std::fs::read_dir(&mount_path)
                .map_err(|error| format!("Could not read mounted tool image: {error}"))?
                .flatten()
                .map(|entry| entry.path())
                .find(|path| path.extension().and_then(|ext| ext.to_str()) == Some("app"))
                .ok_or_else(|| {
                    "The macOS disk image does not contain an application bundle.".to_string()
                })?;
            let target = destination.join(app_bundle.file_name().unwrap_or_default());
            let copied = Command::new("ditto")
                .arg(&app_bundle)
                .arg(&target)
                .output()
                .map_err(|error| format!("Could not copy macOS application: {error}"))?;
            if copied.status.success() {
                Ok(())
            } else {
                Err(format!(
                    "Could not copy macOS application: {}",
                    String::from_utf8_lossy(&copied.stderr).trim()
                ))
            }
        })();
        let _ = Command::new("hdiutil")
            .args(["detach", "-quiet"])
            .arg(&mount_path)
            .output();
        let _ = std::fs::remove_dir_all(&mount_path);
        let _ = std::fs::remove_file(&image_path);
        result
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (bytes, destination);
        Err("macOS disk images can only be installed on macOS.".into())
    }
}

#[tauri::command]
async fn check_or_update_tool(app: AppHandle, id: String) -> Result<ToolUpdateResult, String> {
    ensure_safe_name(&id)?;
    let repository = tool_repository(&id).ok_or_else(|| "Unknown tool.".to_string())?;
    let root = tools_root(&app)?;
    std::fs::create_dir_all(&root)
        .map_err(|error| format!("Could not create tools folder: {error}"))?;
    let tool_dir = root.join(&id);
    let installed = std::fs::read(tool_dir.join(".fm-mate-tool.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<InstalledToolMarker>(&bytes).ok());

    let client = make_http_client()?;
    let api_url = format!("https://api.github.com/repos/{repository}/releases/latest");
    let release = client
        .get(api_url)
        .header(reqwest::header::ACCEPT, "application/vnd.github+json")
        .send()
        .await
        .map_err(|error| format!("Could not check GitHub releases: {error}"))?
        .error_for_status()
        .map_err(|error| format!("GitHub release request failed: {error}"))?
        .json::<GithubRelease>()
        .await
        .map_err(|error| format!("Could not read GitHub release: {error}"))?;

    if installed.as_ref().is_some_and(|marker| {
        marker.version == release.tag_name && marker.platform == current_tool_platform()
    }) {
        return Ok(ToolUpdateResult {
            status: "up-to-date".into(),
            version: release.tag_name,
        });
    }
    let (asset, archive_kind) = release
        .assets
        .iter()
        .filter_map(|asset| asset_archive_kind(&asset.name, &asset.content_type).map(|kind| (asset, kind)))
        .filter(|(asset, _)| release_asset_matches_platform(&asset.name, &asset.content_type, &id))
        .max_by_key(|(asset, kind)| {
            let preferred = match (std::env::consts::OS, id.as_str(), kind) {
                ("windows", _, ToolArchiveKind::Zip) => 2,
                ("macos", "m-flasher", ToolArchiveKind::Dmg) => 2,
                ("macos", "fm1-emu", ToolArchiveKind::TarGz) => 2,
                ("linux", _, ToolArchiveKind::TarGz) => 2,
                _ => 1,
            };
            (preferred, asset.size)
        })
        .ok_or_else(|| {
            format!(
                "The latest release has no compatible asset for {}-{}.",
                std::env::consts::OS,
                std::env::consts::ARCH
            )
        })?;
    if asset.size > 512 * 1024 * 1024 {
        return Err("The tool download is larger than 512 MiB.".into());
    }
    let asset_url = reqwest::Url::parse(&asset.browser_download_url)
        .map_err(|_| "GitHub returned an invalid release asset URL.".to_string())?;
    if asset_url.scheme() != "https" || asset_url.host_str() != Some("github.com") {
        return Err("GitHub returned an unexpected release asset URL.".into());
    }
    let response = client
        .get(asset_url)
        .send()
        .await
        .map_err(|error| format!("Could not download tool: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Tool download failed: {error}"))?;
    if response
        .content_length()
        .is_some_and(|size| size > 512 * 1024 * 1024)
    {
        return Err("The tool download is larger than 512 MiB.".into());
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|error| format!("Could not read tool download: {error}"))?;
    if bytes.is_empty() || bytes.len() > 512 * 1024 * 1024 {
        return Err("The tool download is empty or larger than 512 MiB.".into());
    }

    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let staging = root.join(format!(".{id}-{stamp}.staging"));
    std::fs::create_dir_all(&staging)
        .map_err(|error| format!("Could not prepare tool update: {error}"))?;
    let extraction = match archive_kind {
        ToolArchiveKind::Zip => extract_tool_zip(&bytes, &staging),
        ToolArchiveKind::TarGz => extract_tool_tar_gz(&bytes, &staging),
        ToolArchiveKind::Dmg => extract_tool_dmg(&bytes, &staging),
    };
    if let Err(error) = extraction {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(error);
    }
    let entries = std::fs::read_dir(&staging)
        .map_err(|error| format!("Could not inspect extracted tool: {error}"))?
        .flatten()
        .collect::<Vec<_>>();
    let payload = if entries.len() == 1
        && entries[0]
            .file_type()
            .map(|kind| kind.is_dir())
            .unwrap_or(false)
    {
        entries[0].path()
    } else {
        staging.clone()
    };
    let marker_path = payload.join(".fm-mate-tool.json");
    let marker = serde_json::to_vec_pretty(&InstalledToolMarker {
        version: release.tag_name.clone(),
        platform: current_tool_platform(),
    })
    .map_err(|error| error.to_string())?;
    std::fs::write(marker_path, marker)
        .map_err(|error| format!("Could not save tool version: {error}"))?;

    let backup = root.join(format!(".{id}-{stamp}.backup"));
    let had_existing = tool_dir.exists();
    if had_existing {
        std::fs::rename(&tool_dir, &backup)
            .map_err(|error| format!("Could not prepare to replace the existing tool: {error}"))?;
    }
    if let Err(error) = std::fs::rename(&payload, &tool_dir) {
        if had_existing {
            let _ = std::fs::rename(&backup, &tool_dir);
        }
        let _ = std::fs::remove_dir_all(&staging);
        return Err(format!("Could not install tool files: {error}"));
    }
    if had_existing {
        let _ = std::fs::remove_dir_all(&backup);
    }
    let _ = std::fs::remove_dir_all(&staging);
    Ok(ToolUpdateResult {
        status: if had_existing {
            "updated"
        } else {
            "downloaded"
        }
        .into(),
        version: release.tag_name,
    })
}

#[tauri::command]
async fn get_firmware_thumbnail(
    app: AppHandle,
    id: String,
    image_url: String,
    is_card: bool,
) -> Result<String, String> {
    ensure_safe_name(&id)?;
    let url = reqwest::Url::parse(&image_url).map_err(|_| "Invalid thumbnail URL.".to_string())?;
    if url.scheme() != "https"
        || !matches!(
            url.host_str(),
            Some("thefirmwarehub.com" | "www.thefirmwarehub.com")
        )
    {
        return Err("Thumbnails must be hosted by the Firmware Hub over HTTPS.".into());
    }
    let extension = url
        .path_segments()
        .and_then(|mut segments| segments.next_back())
        .and_then(|file| {
            file.rsplit_once('.')
                .map(|(_, ext)| ext.to_ascii_lowercase())
        })
        .unwrap_or_else(|| "webp".into());
    let content_type = match extension.as_str() {
        "webp" => "image/webp",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        _ => return Err("Unsupported thumbnail image type.".into()),
    };
    let cache_dir = app
        .path()
        .app_cache_dir()
        .map_err(|error| format!("Could not locate thumbnail cache: {error}"))?
        .join("thumbnails");
    std::fs::create_dir_all(&cache_dir)
        .map_err(|error| format!("Could not create thumbnail cache: {error}"))?;
    let image_path = cache_dir.join(format!(
        "{id}{}{extension}",
        if is_card { "-card." } else { "." }
    ));

    let image = if image_path.is_file() {
        std::fs::read(&image_path)
            .map_err(|error| format!("Could not read cached thumbnail: {error}"))?
    } else {
        let response = make_http_client()?
            .get(url)
            .send()
            .await
            .map_err(|error| format!("Could not download thumbnail: {error}"))?;
        if !response.status().is_success() {
            return Err(format!("Thumbnail request failed ({}).", response.status()));
        }
        let bytes = response
            .bytes()
            .await
            .map_err(|error| format!("Could not read thumbnail: {error}"))?;
        let valid_image = match content_type {
            "image/webp" => {
                bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP"
            }
            "image/png" => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
            "image/jpeg" => bytes.starts_with(&[0xff, 0xd8]),
            _ => false,
        };
        if bytes.len() > 4 * 1024 * 1024 || !valid_image {
            return Err("The thumbnail response is not a valid image.".into());
        }
        std::fs::write(&image_path, &bytes)
            .map_err(|error| format!("Could not cache thumbnail: {error}"))?;
        bytes.to_vec()
    };

    Ok(format!("{content_type};base64,{}", BASE64.encode(image)).replace("image/", "data:image/"))
}

#[tauri::command]
async fn download_firmware(
    app: AppHandle,
    url: String,
    id: String,
    version: String,
    thumbnail_url: String,
    firmware_info: serde_json::Value,
) -> Result<String, String> {
    ensure_safe_name(&id)?;
    ensure_safe_name(&version)?;
    let parsed_url = reqwest::Url::parse(&url).map_err(|_| "Invalid download URL.".to_string())?;
    if !matches!(parsed_url.scheme(), "https" | "http") {
        return Err("Firmware downloads must use HTTP or HTTPS.".into());
    }

    let firmware_dir = firmware_root(&app)?.join(&id);
    let version_dir: PathBuf = firmware_dir.join(&version);
    let saves_dir = version_dir.join("saves");
    std::fs::create_dir_all(&saves_dir)
        .map_err(|error| format!("Could not create firmware folders: {error}"))?;
    let info_path = firmware_dir.join("info.json");
    let version_info_path = version_dir.join("info.json");
    let file_path = version_dir.join("firmware.fwsc");
    let previous_current_version = std::fs::read(&info_path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .and_then(|info| {
            info.get("currentVersion")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        });
    if !file_path.is_file() {
        let response = make_http_client()?
            .get(parsed_url)
            .send()
            .await
            .map_err(|error| format!("Firmware download failed: {error}"))?;
        if !response.status().is_success() {
            return Err(format!("Firmware download failed ({}).", response.status()));
        }
        if response
            .content_length()
            .is_some_and(|size| size > 128 * 1024 * 1024)
        {
            return Err("Firmware file is larger than 128 MiB.".into());
        }
        let bytes = response
            .bytes()
            .await
            .map_err(|error| format!("Could not read firmware file: {error}"))?;
        if bytes.is_empty() || bytes.len() > 128 * 1024 * 1024 {
            return Err("Firmware file is empty or larger than 128 MiB.".into());
        }
        std::fs::write(&file_path, bytes)
            .map_err(|error| format!("Could not save firmware file: {error}"))?;
    }

    let mut root_info = firmware_info.clone();
    if let Some(object) = root_info.as_object_mut() {
        object.insert(
            "currentVersion".into(),
            previous_current_version
                .map(serde_json::Value::String)
                .unwrap_or(serde_json::Value::Null),
        );
    }
    let version_info = serde_json::to_vec_pretty(&firmware_info)
        .map_err(|error| format!("Could not encode firmware info: {error}"))?;
    std::fs::write(&version_info_path, version_info)
        .map_err(|error| format!("Could not save version info: {error}"))?;
    let info = serde_json::to_vec_pretty(&root_info)
        .map_err(|error| format!("Could not encode firmware info: {error}"))?;
    std::fs::write(&info_path, info)
        .map_err(|error| format!("Could not save firmware info: {error}"))?;

    if let (Ok(client), Ok(url)) = (make_http_client(), reqwest::Url::parse(&thumbnail_url)) {
        if url.scheme() == "https" && url.host_str() == Some("thefirmwarehub.com") {
            if let Ok(response) = client.get(url).send().await {
                if response.status().is_success() {
                    if let Ok(bytes) = response.bytes().await {
                        let extension = if bytes.len() >= 12
                            && &bytes[0..4] == b"RIFF"
                            && &bytes[8..12] == b"WEBP"
                        {
                            Some("webp")
                        } else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
                            Some("png")
                        } else if bytes.starts_with(&[0xff, 0xd8]) {
                            Some("jpg")
                        } else {
                            None
                        };
                        if bytes.len() <= 4 * 1024 * 1024 {
                            if let Some(extension) = extension {
                                let thumbnail_path =
                                    firmware_dir.join(format!("thumbnail.{extension}"));
                                let _ = std::fs::write(thumbnail_path, bytes);
                            }
                        }
                    }
                }
            }
        }
    }

    if let Some(editor_url) = firmware_info
        .get("editor")
        .and_then(serde_json::Value::as_str)
    {
        if let Ok(editor_url) = reqwest::Url::parse(editor_url) {
            if matches!(editor_url.scheme(), "https" | "http") {
                if let Ok(client) = make_http_client() {
                    if let Ok(response) = client.get(editor_url).send().await {
                        if response.status().is_success()
                            && !response
                                .content_length()
                                .is_some_and(|size| size > 16 * 1024 * 1024)
                        {
                            if let Ok(bytes) = response.bytes().await {
                                if !bytes.is_empty() && bytes.len() <= 16 * 1024 * 1024 {
                                    let editor_dir = version_dir.join("editor");
                                    if std::fs::create_dir_all(&editor_dir).is_ok() {
                                        let _ =
                                            std::fs::write(editor_dir.join("editor.html"), bytes);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(file_path.to_string_lossy().into_owned())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InstalledFirmware {
    id: String,
    name: String,
    version: String,
    kind: String,
    description: String,
    author: String,
    file_name: String,
    path: String,
    downloaded_at: String,
    current_version: Option<String>,
    saves_path: String,
    saves_count: usize,
    saves: Vec<String>,
    thumbnail_data: Option<String>,
    info: serde_json::Value,
    editor_path: Option<String>,
}

fn json_string(value: &serde_json::Value, key: &str, fallback: &str) -> String {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
        .unwrap_or(fallback)
        .to_string()
}

fn count_files(path: &Path) -> usize {
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    entries
        .flatten()
        .map(|entry| {
            let Ok(kind) = entry.file_type() else {
                return 0;
            };
            if kind.is_file() {
                1
            } else if kind.is_dir() {
                count_files(&entry.path())
            } else {
                0
            }
        })
        .sum()
}

fn list_save_files(root: &Path) -> Vec<String> {
    fn visit(root: &Path, current: &Path, files: &mut Vec<String>) {
        let Ok(entries) = std::fs::read_dir(current) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                visit(root, &path, files);
            } else if kind.is_file() {
                if let Ok(relative) = path.strip_prefix(root) {
                    files.push(relative.to_string_lossy().replace('\\', "/"));
                }
            }
        }
    }
    let mut files = Vec::new();
    visit(root, root, &mut files);
    files.sort();
    files
}

fn read_firmware_thumbnail(firmware_dir: &Path) -> Option<String> {
    for (extension, mime) in [
        ("webp", "image/webp"),
        ("png", "image/png"),
        ("jpg", "image/jpeg"),
    ] {
        if let Ok(bytes) = std::fs::read(firmware_dir.join(format!("thumbnail.{extension}"))) {
            return Some(format!("data:{mime};base64,{}", BASE64.encode(bytes)));
        }
    }
    None
}

#[tauri::command]
fn scan_installed_firmwares(app: AppHandle) -> Result<Vec<InstalledFirmware>, String> {
    let root = firmware_root(&app)?;
    if !root.exists() {
        return Ok(Vec::new());
    }

    let mut installed = Vec::new();
    let firmware_dirs = std::fs::read_dir(&root)
        .map_err(|error| format!("Could not read firmware folder: {error}"))?;
    for firmware_entry in firmware_dirs.flatten() {
        if !firmware_entry
            .file_type()
            .map(|kind| kind.is_dir())
            .unwrap_or(false)
        {
            continue;
        }
        let firmware_dir = firmware_entry.path();
        let folder_id = firmware_entry.file_name().to_string_lossy().into_owned();
        let root_info = std::fs::read(firmware_dir.join("info.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
            .unwrap_or_else(|| serde_json::json!({}));
        let mut thumbnail_data = read_firmware_thumbnail(&firmware_dir);

        let version_dirs = match std::fs::read_dir(&firmware_dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for version_entry in version_dirs.flatten() {
            if !version_entry
                .file_type()
                .map(|kind| kind.is_dir())
                .unwrap_or(false)
            {
                continue;
            }
            let version_dir = version_entry.path();
            let firmware_file = version_dir.join("firmware.fwsc");
            if !firmware_file.is_file() {
                continue;
            }
            let version = version_entry.file_name().to_string_lossy().into_owned();
            let version_info = std::fs::read(version_dir.join("info.json"))
                .ok()
                .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
                .unwrap_or_else(|| root_info.clone());
            let saves_path = version_dir.join("saves");
            let saves = list_save_files(&saves_path);
            installed.push(InstalledFirmware {
                id: json_string(&root_info, "id", &folder_id),
                name: json_string(&root_info, "name", &folder_id),
                version: json_string(&version_info, "version", &version),
                kind: json_string(&root_info, "kind", "Other"),
                description: json_string(&root_info, "what", ""),
                author: json_string(&root_info, "author", ""),
                file_name: "firmware.fwsc".into(),
                path: firmware_file.to_string_lossy().into_owned(),
                downloaded_at: json_string(&version_info, "downloadedAt", ""),
                current_version: root_info
                    .get("currentVersion")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned),
                saves_path: saves_path.to_string_lossy().into_owned(),
                saves_count: saves.len(),
                saves,
                thumbnail_data: thumbnail_data.take(),
                info: root_info.clone(),
                editor_path: {
                    let path = version_dir.join("editor").join("editor.html");
                    path.is_file().then(|| path.to_string_lossy().into_owned())
                },
            });
        }
    }
    installed.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(installed)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InstalledSave {
    firmware_id: String,
    firmware_name: String,
    version: String,
    file_name: String,
    relative_path: String,
    modified_at: u64,
    size: u64,
}

#[tauri::command]
fn scan_installed_saves(app: AppHandle) -> Result<Vec<InstalledSave>, String> {
    fn collect_saves(
        firmware_id: &str,
        firmware_name: &str,
        version: &str,
        root: &Path,
        current: &Path,
        output: &mut Vec<InstalledSave>,
    ) {
        let Ok(entries) = std::fs::read_dir(current) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_dir() {
                collect_saves(firmware_id, firmware_name, version, root, &path, output);
            } else if file_type.is_file() {
                let Ok(metadata) = entry.metadata() else {
                    continue;
                };
                let relative_path = match path.strip_prefix(root) {
                    Ok(value) => value.to_string_lossy().replace('\\', "/"),
                    Err(_) => continue,
                };
                let modified_at = metadata
                    .modified()
                    .ok()
                    .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|duration| duration.as_millis().min(u64::MAX as u128) as u64)
                    .unwrap_or(0);
                output.push(InstalledSave {
                    firmware_id: firmware_id.to_string(),
                    firmware_name: firmware_name.to_string(),
                    version: version.to_string(),
                    file_name: entry.file_name().to_string_lossy().into_owned(),
                    relative_path,
                    modified_at,
                    size: metadata.len(),
                });
            }
        }
    }

    let root = firmware_root(&app)?;
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut saves = Vec::new();
    let firmware_dirs = std::fs::read_dir(&root)
        .map_err(|error| format!("Could not read firmware folder: {error}"))?;
    for firmware_entry in firmware_dirs.flatten() {
        if !firmware_entry
            .file_type()
            .map(|kind| kind.is_dir())
            .unwrap_or(false)
        {
            continue;
        }
        let firmware_dir = firmware_entry.path();
        let folder_id = firmware_entry.file_name().to_string_lossy().into_owned();
        let info = std::fs::read(firmware_dir.join("info.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
            .unwrap_or_default();
        let firmware_id = json_string(&info, "id", &folder_id);
        let firmware_name = json_string(&info, "name", &folder_id);
        let Ok(versions) = std::fs::read_dir(&firmware_dir) else {
            continue;
        };
        for version_entry in versions.flatten() {
            if !version_entry
                .file_type()
                .map(|kind| kind.is_dir())
                .unwrap_or(false)
            {
                continue;
            }
            let version_dir = version_entry.path();
            if !version_dir.join("firmware.fwsc").is_file() {
                continue;
            }
            let version = version_entry.file_name().to_string_lossy().into_owned();
            let saves_dir = version_dir.join("saves");
            collect_saves(
                &firmware_id,
                &firmware_name,
                &version,
                &saves_dir,
                &saves_dir,
                &mut saves,
            );
        }
    }
    saves.sort_by(|a, b| {
        b.modified_at
            .cmp(&a.modified_at)
            .then_with(|| a.file_name.cmp(&b.file_name))
    });
    Ok(saves)
}

#[tauri::command]
fn delete_installed_save(
    app: AppHandle,
    firmware_id: String,
    version: String,
    relative_path: String,
) -> Result<(), String> {
    ensure_safe_name(&firmware_id)?;
    ensure_safe_name(&version)?;
    let mut relative = PathBuf::new();
    for segment in relative_path.split('/') {
        if segment.is_empty()
            || segment == "."
            || segment == ".."
            || segment.contains('\\')
            || segment.contains(':')
        {
            return Err("Invalid save file path.".into());
        }
        relative.push(segment);
    }
    let version_root = firmware_root(&app)?.join(&firmware_id).join(&version);
    if !version_root.join("firmware.fwsc").is_file() {
        return Err("That firmware version is not installed.".into());
    }
    let saves_root = version_root
        .join("saves")
        .canonicalize()
        .map_err(|error| format!("Could not locate saves folder: {error}"))?;
    let save_file = saves_root
        .join(relative)
        .canonicalize()
        .map_err(|error| format!("Could not locate save file: {error}"))?;
    if !save_file.starts_with(&saves_root) || !save_file.is_file() {
        return Err("Invalid save file path.".into());
    }
    std::fs::remove_file(save_file).map_err(|error| format!("Could not delete save file: {error}"))
}

#[tauri::command]
fn get_firmware_folder(app: AppHandle) -> Result<String, String> {
    let folder = firmware_root(&app)?;
    std::fs::create_dir_all(&folder)
        .map_err(|error| format!("Could not create firmware folder: {error}"))?;
    Ok(folder.to_string_lossy().into_owned())
}

#[tauri::command]
fn open_firmware_folder(app: AppHandle) -> Result<(), String> {
    let folder = firmware_root(&app)?;
    std::fs::create_dir_all(&folder)
        .map_err(|error| format!("Could not create firmware folder: {error}"))?;
    #[cfg(target_os = "windows")]
    let mut command = Command::new("explorer.exe");
    #[cfg(target_os = "macos")]
    let mut command = Command::new("open");
    #[cfg(target_os = "linux")]
    let mut command = Command::new("xdg-open");
    command
        .arg(folder)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("Could not open the firmware folder: {error}"))
}

#[tauri::command]
fn get_firmware_saves_folder(
    app: AppHandle,
    id: String,
    version: String,
) -> Result<String, String> {
    ensure_safe_name(&id)?;
    ensure_safe_name(&version)?;
    let folder = firmware_root(&app)?.join(id).join(version).join("saves");
    std::fs::create_dir_all(&folder)
        .map_err(|error| format!("Could not create saves folder: {error}"))?;
    Ok(folder.to_string_lossy().into_owned())
}

#[tauri::command]
fn delete_firmware_version(app: AppHandle, id: String, version: String) -> Result<(), String> {
    ensure_safe_name(&id)?;
    ensure_safe_name(&version)?;
    let firmware_dir = firmware_root(&app)?.join(&id);
    let version_dir = firmware_dir.join(&version);
    if !version_dir.join("firmware.fwsc").is_file() {
        return Err("That firmware version is not installed.".into());
    }
    let canonical_firmware_dir = firmware_dir
        .canonicalize()
        .map_err(|error| format!("Could not locate firmware folder: {error}"))?;
    let canonical_version_dir = version_dir
        .canonicalize()
        .map_err(|error| format!("Could not locate firmware version: {error}"))?;
    if canonical_version_dir.parent() != Some(canonical_firmware_dir.as_path()) {
        return Err("Invalid firmware version folder.".into());
    }
    std::fs::remove_dir_all(&canonical_version_dir)
        .map_err(|error| format!("Could not delete firmware version: {error}"))?;

    let info_path = firmware_dir.join("info.json");
    if let Ok(bytes) = std::fs::read(&info_path) {
        if let Ok(mut info) = serde_json::from_slice::<serde_json::Value>(&bytes) {
            if info
                .get("currentVersion")
                .and_then(serde_json::Value::as_str)
                == Some(version.as_str())
            {
                if let Some(object) = info.as_object_mut() {
                    object.insert("currentVersion".into(), serde_json::Value::Null);
                    if let Ok(updated) = serde_json::to_vec_pretty(&info) {
                        let _ = std::fs::write(info_path, updated);
                    }
                }
            }
        }
    }
    Ok(())
}

#[tauri::command]
fn serve_firmware_editor(app: AppHandle, id: String, version: String) -> Result<String, String> {
    ensure_safe_name(&id)?;
    ensure_safe_name(&version)?;
    let firmware_root = firmware_root(&app)?;
    let editor_file = firmware_root
        .join(&id)
        .join(&version)
        .join("editor")
        .join("editor.html");
    if !editor_file.is_file() {
        return Err("This firmware version does not have a downloaded editor.".into());
    }
    let base_url = ensure_editor_server(&firmware_root)?;
    Ok(format!("{base_url}/edit/{id}/{version}/editor/editor.html"))
}

#[tauri::command]
fn launch_editor_in_chrome(url: String) -> bool {
    let Ok(parsed_url) = reqwest::Url::parse(&url) else {
        return false;
    };
    if parsed_url.scheme() != "http" || parsed_url.host_str() != Some("127.0.0.1") {
        return false;
    }

    let mut candidates = vec![PathBuf::from("chrome"), PathBuf::from("chrome.exe")];
    #[cfg(target_os = "windows")]
    {
        for variable in ["LOCALAPPDATA", "PROGRAMFILES", "PROGRAMFILES(X86)"] {
            if let Some(directory) = std::env::var_os(variable) {
                candidates
                    .push(PathBuf::from(directory).join("Google/Chrome/Application/chrome.exe"));
            }
        }
    }
    #[cfg(target_os = "macos")]
    candidates.push(PathBuf::from(
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    ));
    #[cfg(target_os = "linux")]
    candidates.extend([
        PathBuf::from("google-chrome"),
        PathBuf::from("google-chrome-stable"),
    ]);

    candidates.into_iter().any(|candidate| {
        Command::new(candidate)
            .arg(format!("--app={url}"))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .is_ok()
    })
}

#[tauri::command]
fn set_current_firmware_version(app: AppHandle, id: String, version: String) -> Result<(), String> {
    ensure_safe_name(&id)?;
    ensure_safe_name(&version)?;
    let firmware_dir = firmware_root(&app)?.join(id);
    if !firmware_dir.join(&version).join("firmware.fwsc").is_file() {
        return Err("That firmware version is not installed.".into());
    }
    let info_path = firmware_dir.join("info.json");
    let mut info = std::fs::read(&info_path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    let object = info
        .as_object_mut()
        .ok_or("Firmware info must be a JSON object.")?;
    object.insert("currentVersion".into(), serde_json::Value::String(version));
    let bytes = serde_json::to_vec_pretty(&info).map_err(|error| error.to_string())?;
    std::fs::write(info_path, bytes)
        .map_err(|error| format!("Could not update firmware info: {error}"))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .invoke_handler(tauri::generate_handler![
            greet,
            get_firmware_thumbnail,
            download_firmware,
            get_firmware_folder_path,
            set_firmware_folder_path,
            get_firmware_folder,
            open_firmware_folder,
            get_firmware_saves_folder,
            delete_firmware_version,
            serve_firmware_editor,
            launch_editor_in_chrome,
            scan_installed_firmwares,
            set_current_firmware_version,
            scan_installed_saves,
            delete_installed_save,
            get_tools_status,
            check_or_update_tool,
            open_firmware_in_emulator,
            m_flasher_find,
            m_flasher_command,
            m_flasher_backup
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
