use std::path::PathBuf;

/// Resolve the sing-box binary path.
/// Checks in order:
/// 1. SING_BOX_PATH environment variable
/// 2. "sing-box" in PATH
/// 3. ~/.local/share/aurorabox/bin/sing-box
/// 4. Relative to executable directory
pub fn sing_box_path() -> String {
    // Check environment variable
    if let Ok(path) = std::env::var("SING_BOX_PATH") {
        if std::path::Path::new(&path).exists() {
            log::debug!("Using sing-box from SING_BOX_PATH: {}", path);
            return path;
        }
    }

    // Check if "sing-box" is in PATH
    if which_exists("sing-box") {
        log::debug!("Using sing-box from PATH");
        return "sing-box".to_string();
    }

    // Check managed directory
    let managed = managed_singbox_path();
    if managed.exists() {
        log::debug!("Using sing-box from managed path: {:?}", managed);
        return managed.to_string_lossy().to_string();
    }

    // Fall back to "sing-box" (let the system figure it out or fail with a clear error)
    log::warn!("sing-box not found in known locations, using 'sing-box' as fallback");
    "sing-box".to_string()
}

/// Get the managed sing-box binary path
fn managed_singbox_path() -> PathBuf {
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

/// Check if a command exists in PATH
fn which_exists(cmd: &str) -> bool {
    std::env::var("PATH")
        .map(|path| {
            path.split(':')
                .any(|dir| std::path::Path::new(dir).join(cmd).exists())
        })
        .unwrap_or(false)
}

/// Download sing-box binary from GitHub releases
pub fn download_singbox(version: &str, target_dir: &str) -> anyhow::Result<()> {
    std::fs::create_dir_all(target_dir)?;

    let platform = detect_platform();
    let arch = detect_arch();

    let filename = format!("sing-box-{}-{}-{}.tar.gz", version, platform, arch);
    let url = format!(
        "https://github.com/SagerNet/sing-box/releases/download/v{}/{}",
        version, filename
    );

    log::info!("Downloading sing-box from: {}", url);

    let rt = tokio::runtime::Runtime::new()?;
    let response = rt.block_on(async {
        reqwest::get(&url).await?.bytes().await
    })?;

    // Extract tar.gz
    log::info!("Extracting sing-box...");
    let tar_gz = flate2::read::GzDecoder::new(&response[..]);
    let mut archive = tar::Archive::new(tar_gz);

    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?;
        if let Some(name) = path.file_name() {
            if name == "sing-box" || name == "sing-box.exe" {
                let dest = std::path::Path::new(target_dir).join(name);
                entry.unpack(&dest)?;
                set_executable(&dest)?;
                log::info!("sing-box installed to: {:?}", dest);
                return Ok(());
            }
        }
    }

    Err(anyhow::anyhow!(
        "Could not find sing-box binary in downloaded archive"
    ))
}

#[cfg(unix)]
fn set_executable(path: &std::path::Path) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(path)?.permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(path, perms)?;
    Ok(())
}

#[cfg(not(unix))]
fn set_executable(_path: &std::path::Path) -> anyhow::Result<()> {
    Ok(())
}

fn detect_platform() -> &'static str {
    if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "macos") {
        "darwin"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "linux"
    }
}

fn detect_arch() -> &'static str {
    if cfg!(target_arch = "x86_64") {
        "amd64"
    } else if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        "amd64"
    }
}
