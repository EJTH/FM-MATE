use serde::{Deserialize, Serialize};

use crate::firmwarehub::make_http_client;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct AppUpdateInfo {
    is_update_available: bool,
    current_version: String,
    latest_version: String,
    name: Option<String>,
    body: Option<String>,
    html_url: Option<String>,
}

#[derive(Deserialize)]
struct AppRelease {
    tag_name: String,
    name: Option<String>,
    body: Option<String>,
    html_url: Option<String>,
}

#[tauri::command]
pub(super) async fn check_app_update() -> Result<AppUpdateInfo, String> {
    let current_version = env!("FM_MATE_VERSION").to_string();
    let client = make_http_client()?;
    let release = client
        .get("https://api.github.com/repos/EJTH/FM-MATE/releases/latest")
        .header(reqwest::header::ACCEPT, "application/vnd.github+json")
        .send()
        .await
        .map_err(|error| format!("Could not check GitHub releases: {error}"))?
        .error_for_status()
        .map_err(|error| format!("GitHub release request failed: {error}"))?
        .json::<AppRelease>()
        .await
        .map_err(|error| format!("Could not read GitHub release: {error}"))?;

    let latest_version = release.tag_name;
    let is_update_available = compare_versions(&current_version, &latest_version);

    Ok(AppUpdateInfo {
        is_update_available,
        current_version,
        latest_version,
        name: release.name,
        body: release.body,
        html_url: release.html_url,
    })
}

fn compare_versions(current: &str, latest: &str) -> bool {
    let current_parts: Vec<u32> = current
        .trim_start_matches('v')
        .split('.')
        .filter_map(|part| part.parse::<u32>().ok())
        .collect();
    let latest_parts: Vec<u32> = latest
        .trim_start_matches('v')
        .split('.')
        .filter_map(|part| part.parse::<u32>().ok())
        .collect();

    for (curr, next) in current_parts.iter().zip(latest_parts.iter()) {
        if next > curr {
            return true;
        } else if next < curr {
            return false;
        }
    }

    latest_parts.len() > current_parts.len()
}
