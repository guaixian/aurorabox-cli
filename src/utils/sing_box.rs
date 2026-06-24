use std::path::PathBuf;

/// Embedded sing-box binary (populated by build.rs).
/// Empty slice means sing-box was not downloaded during build.
static EMBEDDED_SING_BOX: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/sing-box"));

/// Resolve the sing-box binary path.
///
/// 1. If embedded binary exists, extract to cache dir and use that
/// 2. Check SING_BOX_PATH environment variable
/// 3. Check "sing-box" in PATH
pub fn sing_box_path() -> String {
    // Use embedded binary if available
    if EMBEDDED_SING_BOX.len() > 1024 {
        let cached = cached_singbox_path();
        if !cached.exists() {
            if let Err(e) = extract_embedded(&cached) {
                log::warn!("Failed to extract embedded sing-box: {}", e);
            }
        }
        if cached.exists() {
            log::debug!("Using embedded sing-box: {}", cached.display());
            return cached.to_string_lossy().to_string();
        }
    }

    // Check environment variable
    if let Ok(path) = std::env::var("SING_BOX_PATH") {
        if std::path::Path::new(&path).exists() {
            log::debug!("Using sing-box from SING_BOX_PATH: {}", path);
            return path;
        }
    }

    // Check PATH
    if which_exists("sing-box") {
        log::debug!("Using sing-box from PATH");
        return "sing-box".to_string();
    }

    // Last resort
    log::error!("sing-box not found. The embedded binary may have failed to extract.");
    "sing-box".to_string()
}

/// Path where the embedded sing-box binary is cached
fn cached_singbox_path() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());

    let mut path = PathBuf::from(home);
    path.push(".local");
    path.push("share");
    path.push("aurorabox");
    path.push("bin");
    path.push("sing-box");

    #[cfg(target_os = "windows")]
    path.set_extension("exe");

    path
}

/// Extract the embedded sing-box binary to the cache directory
fn extract_embedded(dest: &std::path::Path) -> anyhow::Result<()> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }

    log::info!(
        "Extracting embedded sing-box ({:.1} MB)...",
        EMBEDDED_SING_BOX.len() as f64 / 1_048_576.0
    );

    std::fs::write(dest, EMBEDDED_SING_BOX)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(dest)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(dest, perms)?;
    }

    log::info!("sing-box extracted to: {}", dest.display());
    Ok(())
}

/// Check if a command exists in PATH
fn which_exists(cmd: &str) -> bool {
    std::env::var("PATH")
        .map(|path| {
            path.split(':')
                .any(|dir| std::path::Path::new(dir).join(cmd).exists())
        })
        .unwrap_or(false)
}

/// Get the sing-box version string
pub fn get_singbox_version() -> anyhow::Result<String> {
    let path = sing_box_path();
    let output = std::process::Command::new(&path)
        .arg("version")
        .output()
        .map_err(|e| anyhow::anyhow!("Failed to run sing-box version: {}", e))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Ok("sing-box (version unknown)".to_string())
    }
}

/// No-op: sing-box is now bundled. Kept for backward compat.
pub fn download_singbox(_version: &str, _target_dir: &str) -> anyhow::Result<()> {
    log::info!("sing-box is bundled into the binary — no download needed");
    let cached = cached_singbox_path();
    if !cached.exists() && EMBEDDED_SING_BOX.len() > 1024 {
        extract_embedded(&cached)?;
    }
    Ok(())
}
