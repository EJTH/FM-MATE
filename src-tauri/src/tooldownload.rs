use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};
use std::io::Cursor;
use std::path::{Path, PathBuf};
use tar::Archive as TarArchive;
use tauri::{AppHandle, Manager};
use zip::ZipArchive;
use crate::ensure_safe_name;
use crate::firmwarehub::make_http_client;

#[cfg(target_os = "macos")]
use std::process::Command;

use crate::utils::{staging_dir_path, unix_timestamp_nanos};

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
pub(super) struct ToolStatus {
    id: String,
    installed: bool,
    version: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ToolUpdateResult {
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

pub(crate) fn tools_root(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|error| format!("Could not locate app data folder: {error}"))?
        .join("tools"))
}

pub(crate) fn find_tool_executable(directory: &Path, expected_name: &str) -> Option<PathBuf> {
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

#[tauri::command]
pub(super) fn get_tools_status(app: AppHandle) -> Result<Vec<ToolStatus>, String> {
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
            if let Ok(mode) = entry.header().mode() {
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
pub(super) async fn check_or_update_tool(app: AppHandle, id: String) -> Result<ToolUpdateResult, String> {
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

    let stamp = unix_timestamp_nanos();
    let staging = staging_dir_path(&root, &id, stamp);
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

