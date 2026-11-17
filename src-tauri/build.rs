fn main() {
    tauri_build::build();

    println!("cargo:rerun-if-env-changed=FM_MATE_VERSION");
    let version = get_version();
    println!("cargo:rustc-env=FM_MATE_VERSION={}", version);
}

fn get_version() -> String {
    if std::env::var("PROFILE").as_deref() == Ok("debug") {
        return "0.0.0".to_string();
    }

    // Release builds pass the requested version explicitly. Local builds use
    // the package version, which is also updated by the release workflow.
    if let Ok(version) = std::env::var("FM_MATE_VERSION") {
        let version = version.trim();
        if !version.is_empty() {
            return version.to_string();
        }
    }

    env!("CARGO_PKG_VERSION").to_string()
}
