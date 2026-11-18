use reqwest::Client;
use std::path::{Path, PathBuf};
use std::collections::HashSet;
use scraper::{Html, Selector};
use regex::Regex;
use once_cell::sync::Lazy;

use crate::firmwarehub::make_http_client;

const MAX_EDITOR_SIZE: u64 = 16 * 1024 * 1024; // 16 MiB

static CSS_URL_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"url\(\s*(?:['"]([^'"]*)['"]|([^\s)]+))\s*\)"#).unwrap()
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

    // Download all assets and collect CSS files for further processing
    let mut css_files = Vec::new();
    for (asset_url, asset_path) in assets {
        if let Err(e) = download_asset(&client, &asset_url, &assets_dir, &asset_path).await {
            eprintln!("Warning: Failed to download asset {}: {}", asset_url, e);
            continue;
        }
        
        // Track CSS files for processing
        if asset_path.extension().and_then(|s| s.to_str()) == Some("css") {
            css_files.push((asset_url, asset_path));
        }
    }

    // Process CSS files for embedded URLs
    for (_css_url, css_path) in css_files {
        let full_css_path = assets_dir.join(&css_path);
        if let Ok(css_content) = std::fs::read_to_string(&full_css_path) {
            if let Ok((rewritten_css, css_assets)) = parse_and_rewrite_css(&css_content, &url) {
                // Download CSS assets
                for (asset_url, asset_path) in css_assets {
                    if let Err(e) = download_asset(&client, &asset_url, &assets_dir, &asset_path).await {
                        eprintln!("Warning: Failed to download CSS asset {}: {}", asset_url, e);
                    }
                }
                // Write rewritten CSS back
                std::fs::write(&full_css_path, rewritten_css)
                    .map_err(|e| format!("Failed to write CSS file: {}", e))?;
            }
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

/// Parses HTML, extracts asset URLs, rewrites them to point to original URLs,
/// and returns the modified HTML along with a list of assets to download
fn parse_and_rewrite_html(
    html: &[u8],
    base_url: &reqwest::Url,
) -> Result<(Vec<u8>, Vec<(String, PathBuf)>), String> {
    let html_str = String::from_utf8(html.to_vec())
        .map_err(|e| format!("Invalid UTF-8 in HTML: {}", e))?;

    let document = Html::parse_document(&html_str);
    let mut assets = Vec::new();
    let mut processed_urls = HashSet::new();
    let mut html_output = html_str.clone();

    // Process script src attributes
    let script_selector = Selector::parse("script[src]").unwrap();
    for element in document.select(&script_selector) {
        if let Some(src) = element.value().attr("src") {
            if let Some((asset_path, full_url)) = resolve_asset_url(src, base_url) {
                if !processed_urls.contains(&full_url) {
                    processed_urls.insert(full_url.clone());
                    assets.push((full_url.clone(), asset_path.clone()));
                    // Rewrite to point to assets folder
                    html_output = html_output.replace(src, &format!("assets/{}", asset_path.display()));
                }
            }
        }
    }

    // Process link href attributes (stylesheets, etc.)
    let link_selector = Selector::parse("link[href]").unwrap();
    for element in document.select(&link_selector) {
        if let Some(href) = element.value().attr("href") {
            if let Some((asset_path, full_url)) = resolve_asset_url(href, base_url) {
                if !processed_urls.contains(&full_url) {
                    processed_urls.insert(full_url.clone());
                    assets.push((full_url.clone(), asset_path.clone()));
                    // Rewrite to point to assets folder
                    html_output = html_output.replace(href, &format!("assets/{}", asset_path.display()));
                }
            }
        }
    }

    // Process img src attributes
    let img_selector = Selector::parse("img[src]").unwrap();
    for element in document.select(&img_selector) {
        if let Some(src) = element.value().attr("src") {
            if let Some((asset_path, full_url)) = resolve_asset_url(src, base_url) {
                if !processed_urls.contains(&full_url) {
                    processed_urls.insert(full_url.clone());
                    assets.push((full_url.clone(), asset_path.clone()));
                    // Rewrite to point to assets folder
                    html_output = html_output.replace(src, &format!("assets/{}", asset_path.display()));
                }
            }
        }
    }

    // Process embedded style tags
    let style_selector = Selector::parse("style").unwrap();
    for element in document.select(&style_selector) {
        let text_content: String = element.text().collect();
        if !text_content.is_empty() {
            let (rewritten_css, css_assets) = parse_and_rewrite_css(&text_content, base_url)?;
            let rewritten_css_str = String::from_utf8(rewritten_css)
                .map_err(|e| format!("Invalid UTF-8 in CSS: {}", e))?;
            
            for (asset_url, asset_path) in css_assets {
                if !processed_urls.contains(&asset_url) {
                    processed_urls.insert(asset_url.clone());
                    assets.push((asset_url, asset_path));
                }
            }
            
            html_output = html_output.replace(&text_content, &rewritten_css_str);
        }
    }

    Ok((html_output.into_bytes(), assets))
}

/// Parses CSS content, extracts url() references, and rewrites them to point to asset folder
fn parse_and_rewrite_css(
    css: &str,
    base_url: &reqwest::Url,
) -> Result<(Vec<u8>, Vec<(String, PathBuf)>), String> {
    let mut rewritten_css = css.to_string();
    let mut assets = Vec::new();
    let mut processed_urls = HashSet::new();

    for cap in CSS_URL_REGEX.captures_iter(css) {
        let url_str = cap.get(1).or_else(|| cap.get(2)).map(|m| m.as_str());
        if let Some(url_str) = url_str {
            if let Some((asset_path, full_url)) = resolve_asset_url(url_str, base_url) {
                if !processed_urls.contains(&full_url) {
                    processed_urls.insert(full_url.clone());
                    assets.push((full_url.clone(), asset_path.clone()));
                    let old_url = cap.get(0).unwrap().as_str();
                    let new_url = format!("url(assets/{})", asset_path.display());
                    rewritten_css = rewritten_css.replacen(old_url, &new_url, 1);
                }
            }
        }
    }

    Ok((rewritten_css.into_bytes(), assets))
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

