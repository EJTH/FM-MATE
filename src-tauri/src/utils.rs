use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Get the current Unix timestamp in nanoseconds (for staging operations)
pub fn unix_timestamp_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

/// Create a temporary staging directory path with the given id and timestamp
pub fn staging_dir_path(base_dir: &std::path::Path, id: &str, timestamp: u128) -> PathBuf {
    base_dir.join(format!(".{}-{}.staging", id, timestamp))
}
