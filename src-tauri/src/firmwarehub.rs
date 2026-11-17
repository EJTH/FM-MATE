use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::time::Duration;
use tauri::{AppHandle, Manager};
use crate::{ensure_safe_name, firmware_root};

static EDITOR_SERVER: OnceLock<Mutex<Option<(String, Arc<RwLock<PathBuf>>)>>> = OnceLock::new();

pub(crate) fn make_http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent("FM-MATE")
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|error| format!("Could not create HTTP client: {error}"))
}

#[tauri::command]
pub(super) async fn get_firmware_thumbnail(
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
pub(super) async fn download_firmware(
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
        let editor_dir = version_dir.join("editor");
        if let Err(e) = crate::editor_helper::download_editor(editor_url, &editor_dir).await {
            eprintln!("Warning: Failed to download editor: {}", e);
        }
    }

    Ok(file_path.to_string_lossy().into_owned())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct InstalledFirmware {
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

pub(crate) fn json_string(value: &serde_json::Value, key: &str, fallback: &str) -> String {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
        .unwrap_or(fallback)
        .to_string()
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
pub(super) fn scan_installed_firmwares(app: AppHandle) -> Result<Vec<InstalledFirmware>, String> {
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

#[tauri::command]
pub(super) fn get_firmware_folder_path(app: AppHandle) -> Result<String, String> {
    let folder = firmware_root(&app)?;
    std::fs::create_dir_all(&folder)
        .map_err(|error| format!("Could not create firmware folder: {error}"))?;
    let folder = folder.canonicalize().unwrap_or(folder);
    Ok(folder.to_string_lossy().into_owned())
}

#[tauri::command]
pub(super) fn set_firmware_folder_path(app: AppHandle, path: String) -> Result<String, String> {
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct InstalledSave {
    firmware_id: String,
    firmware_name: String,
    version: String,
    file_name: String,
    relative_path: String,
    modified_at: u64,
    size: u64,
}

#[tauri::command]
pub(super) fn scan_installed_saves(app: AppHandle) -> Result<Vec<InstalledSave>, String> {
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
        .map_err(|error| format!("Could not scan saves in {}: {error}", root.display()))?;
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
pub(super) fn delete_installed_save(
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
pub(super) fn get_firmware_folder(app: AppHandle) -> Result<String, String> {
    let folder = firmware_root(&app)?;
    std::fs::create_dir_all(&folder)
        .map_err(|error| format!("Could not create firmware folder: {error}"))?;
    Ok(folder.to_string_lossy().into_owned())
}

#[tauri::command]
pub(super) fn open_firmware_folder(app: AppHandle) -> Result<(), String> {
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
pub(super) fn open_installed_save_folder(
    app: AppHandle,
    firmware_id: String,
    version: String,
    relative_path: String,
) -> Result<(), String> {
    ensure_safe_name(&firmware_id)?;
    ensure_safe_name(&version)?;
    let relative = Path::new(&relative_path);
    if relative.as_os_str().is_empty()
        || relative.is_absolute()
        || relative.components().any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err("Invalid save path.".into());
    }
    let saves_root = firmware_root(&app)?.join(firmware_id).join(version).join("saves").canonicalize()
        .map_err(|error| format!("Could not locate firmware saves folder: {error}"))?;
    let save_file = saves_root.join(relative).canonicalize()
        .map_err(|error| format!("Could not locate save file: {error}"))?;
    if !save_file.starts_with(&saves_root) || !save_file.is_file() {
        return Err("The selected save is outside the firmware saves folder.".into());
    }
    let folder = save_file.parent().ok_or("Could not locate the save folder.")?;
    #[cfg(target_os = "windows")]
    let mut command = Command::new("explorer.exe");
    #[cfg(target_os = "macos")]
    let mut command = Command::new("open");
    #[cfg(target_os = "linux")]
    let mut command = Command::new("xdg-open");
    command.arg(folder).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null())
        .spawn().map(|_| ())
        .map_err(|error| format!("Could not open the save folder: {error}"))
}

#[tauri::command]
pub(super) fn get_firmware_saves_folder(
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
pub(super) fn delete_firmware_version(app: AppHandle, id: String, version: String) -> Result<(), String> {
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
pub(super) fn serve_firmware_editor(app: AppHandle, id: String, version: String) -> Result<String, String> {
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
pub(super) fn launch_editor_in_chrome(url: String) -> bool {
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
pub(super) fn set_current_firmware_version(app: AppHandle, id: String, version: String) -> Result<(), String> {
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
