use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Get the current Unix timestamp in seconds
pub fn unix_timestamp_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Get the current Unix timestamp in nanoseconds (for staging operations)
pub fn unix_timestamp_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

/// Create a temporary file path with the given prefix and directory
pub fn temp_file_path(dir: &std::path::Path, prefix: &str, timestamp: u64) -> PathBuf {
    dir.join(format!(".{}_{}.fm1save", prefix, timestamp))
}

/// Create a temporary staging directory path with the given id and timestamp
pub fn staging_dir_path(base_dir: &std::path::Path, id: &str, timestamp: u128) -> PathBuf {
    base_dir.join(format!(".{}-{}.staging", id, timestamp))
}
