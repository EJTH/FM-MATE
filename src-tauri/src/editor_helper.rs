use reqwest::Client;
use std::path::{Path, PathBuf};
use std::collections::HashSet;
use regex::Regex;
use once_cell::sync::Lazy;

use crate::firmwarehub::make_http_client;

const MAX_EDITOR_SIZE: u64 = 16 * 1024 * 1024; // 16 MiB

static SCRIPT_SRC_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"<script\s+[^>]*src\s*=\s*['"](.*?)['"][^>]*>"#).unwrap()
});

static LINK_HREF_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"<link\s+[^>]*href\s*=\s*['"](.*?)['"][^>]*>"#).unwrap()
});

static IMG_SRC_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"<img\s+[^>]*src\s*=\s*['"](.*?)['"][^>]*>"#).unwrap()
});

/// Downloads an editor from a URL and saves it to the target directory.
/// Handles both relative URLs and full HTTP/HTTPS URLs.
/// Also downloads and rewrites all asset references (scripts, stylesheets, etc.)
pub async fn download_editor(editor_url: &str, target_dir: &Path) -> Result<(), String> {
    let url = reqwest::Url::parse(editor_url)
        .map_err(|e| format!("Invalid editor URL: {}", e))?;

    if !matches!(url.scheme(), "https" | "http") {
        return Err("Editor URL must use HTTP or HTTPS".to_string());
    }

    let client = make_http_client()?;

    // Create the editor directory if it doesn't exist
    std::fs::create_dir_all(target_dir)
        .map_err(|e| format!("Failed to create editor directory: {}", e))?;

    // Download the main HTML file
    let html_content = download_file(&client, &url).await?;

    // Parse HTML and extract asset URLs
    let (rewritten_html, assets) = parse_and_rewrite_html(&html_content, &url)?;

    // Save the main HTML file
    std::fs::write(target_dir.join("editor.html"), &rewritten_html)
        .map_err(|e| format!("Failed to save editor.html: {}", e))?;

    // Create assets directory
    let assets_dir = target_dir.join("assets");
    std::fs::create_dir_all(&assets_dir)
        .map_err(|e| format!("Failed to create assets directory: {}", e))?;

    // Download all assets
    for (asset_url, asset_path) in assets {
        if let Err(e) = download_asset(&client, &asset_url, &assets_dir, &asset_path).await {
            eprintln!("Warning: Failed to download asset {}: {}", asset_url, e);
            // Continue with other assets even if one fails
        }
    }

    Ok(())
}

/// Downloads a single file and returns its content as bytes
async fn download_file(client: &Client, url: &reqwest::Url) -> Result<Vec<u8>, String> {
    let response = client
        .get(url.clone())
        .send()
        .await
        .map_err(|e| format!("Failed to download file: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("HTTP error: {}", response.status()));
    }

    if let Some(content_length) = response.content_length() {
        if content_length > MAX_EDITOR_SIZE {
            return Err("File is larger than 16 MiB".to_string());
        }
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("Failed to read file: {}", e))?;

    if bytes.is_empty() || bytes.len() > MAX_EDITOR_SIZE as usize {
        return Err("File is empty or larger than 16 MiB".to_string());
    }

    Ok(bytes.to_vec())
}

/// Parses HTML, extracts asset URLs, rewrites them to point to local assets,
/// and returns the modified HTML along with a list of assets to download
fn parse_and_rewrite_html(
    html: &[u8],
    base_url: &reqwest::Url,
) -> Result<(Vec<u8>, Vec<(String, PathBuf)>), String> {
    let html_str = String::from_utf8(html.to_vec())
        .map_err(|e| format!("Invalid UTF-8 in HTML: {}", e))?;

    let mut rewritten = html_str.clone();
    let mut assets = Vec::new();
    let mut processed_urls = HashSet::new();

    // Find and extract script src attributes
    for cap in SCRIPT_SRC_REGEX.captures_iter(&html_str) {
        if let Some(src) = cap.get(1) {
            let src_url = src.as_str();
            if let Some((asset_path, full_url)) = resolve_asset_url(src_url, base_url) {
                if !processed_urls.contains(&full_url) {
                    processed_urls.insert(full_url.clone());
                    assets.push((full_url.clone(), asset_path.clone()));
                    rewritten = rewritten.replace(src_url, &format!("assets/{}", asset_path.display()));
                }
            }
        }
    }

    // Find and extract link href attributes (stylesheets)
    for cap in LINK_HREF_REGEX.captures_iter(&html_str) {
        if let Some(href) = cap.get(1) {
            let href_url = href.as_str();
            if let Some((asset_path, full_url)) = resolve_asset_url(href_url, base_url) {
                if !processed_urls.contains(&full_url) {
                    processed_urls.insert(full_url.clone());
                    assets.push((full_url.clone(), asset_path.clone()));
                    rewritten = rewritten.replace(href_url, &format!("assets/{}", asset_path.display()));
                }
            }
        }
    }

    // Find and extract img src attributes
    for cap in IMG_SRC_REGEX.captures_iter(&html_str) {
        if let Some(src) = cap.get(1) {
            let src_url = src.as_str();
            if let Some((asset_path, full_url)) = resolve_asset_url(src_url, base_url) {
                if !processed_urls.contains(&full_url) {
                    processed_urls.insert(full_url.clone());
                    assets.push((full_url.clone(), asset_path.clone()));
                    rewritten = rewritten.replace(src_url, &format!("assets/{}", asset_path.display()));
                }
            }
        }
    }

    Ok((rewritten.into_bytes(), assets))
}

/// Resolves a potentially relative URL to an absolute URL and determines the local asset path
fn resolve_asset_url(url: &str, base_url: &reqwest::Url) -> Option<(PathBuf, String)> {
    // Skip data URIs and other non-http(s) protocols
    if url.starts_with("data:") || url.starts_with("javascript:") || url.is_empty() {
        return None;
    }

    // Try to parse as absolute URL
    if let Ok(absolute_url) = reqwest::Url::parse(url) {
        if matches!(absolute_url.scheme(), "https" | "http") {
            let path = extract_filename(&absolute_url);
            return Some((PathBuf::from(&path), absolute_url.to_string()));
        }
    }

    // Try to resolve as relative URL
    if let Ok(absolute_url) = base_url.join(url) {
        if matches!(absolute_url.scheme(), "https" | "http") {
            let path = extract_filename(&absolute_url);
            return Some((PathBuf::from(&path), absolute_url.to_string()));
        }
    }

    None
}

/// Extracts a reasonable filename from a URL
fn extract_filename(url: &reqwest::Url) -> String {
    let path = url.path();
    
    // Get the last segment of the path
    if let Some(filename) = path.rsplit('/').next() {
        if !filename.is_empty() {
            return filename.to_string();
        }
    }

    // Fallback: use a generic name based on the host
    if let Some(host) = url.host_str() {
        return format!("{}_{}", host.replace('.', "_"), 
                      uuid::Uuid::new_v4().to_string());
    }

    format!("asset_{}", uuid::Uuid::new_v4().to_string())
}

/// Downloads a single asset file
async fn download_asset(
    client: &Client,
    asset_url: &str,
    assets_dir: &Path,
    asset_path: &Path,
) -> Result<(), String> {
    let url = reqwest::Url::parse(asset_url)
        .map_err(|e| format!("Invalid asset URL: {}", e))?;

    let bytes = download_file(client, &url).await?;

    let full_path = assets_dir.join(asset_path);

    // Create parent directories if needed
    if let Some(parent) = full_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directory: {}", e))?;
    }

    std::fs::write(&full_path, bytes)
        .map_err(|e| format!("Failed to write asset: {}", e))?;

    Ok(())
}

