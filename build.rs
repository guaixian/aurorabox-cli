use std::io::Read;
use std::path::PathBuf;

const SING_BOX_VERSION: &str = "1.13.13";

fn main() {
    let target = std::env::var("TARGET").unwrap_or_default();
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let dest = PathBuf::from(&out_dir).join("sing-box");

    // Only download if the binary doesn't exist yet
    if dest.exists() {
        println!("cargo:warning=sing-box already downloaded for {target}");
        return;
    }

    let (platform, ext) = match_target(&target);
    let url = format!(
        "https://github.com/SagerNet/sing-box/releases/download/v{}/{platform}",
        SING_BOX_VERSION
    );

    println!("cargo:warning=Downloading sing-box v{SING_BOX_VERSION} for {target}...");
    println!("cargo:warning=URL: {url}");

    match download_and_extract(&url, &ext, &dest) {
        Ok(()) => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = std::fs::metadata(&dest).unwrap().permissions();
                perms.set_mode(0o755);
                std::fs::set_permissions(&dest, perms).ok();
            }
            println!("cargo:warning=sing-box embedded successfully ({})", dest.display());
        }
        Err(e) => {
            // Graceful fallback: create an empty file; runtime will use PATH
            println!("cargo:warning=Failed to download sing-box: {e}");
            println!("cargo:warning=sing-box will NOT be embedded — runtime will use PATH");
            std::fs::write(&dest, []).ok();
        }
    }

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=SING_BOX_VERSION");
}

fn match_target(target: &str) -> (&str, &str) {
    match target {
        // Linux
        t if t.contains("x86_64") && t.contains("linux") =>
            ("sing-box-1.13.13-linux-amd64.tar.gz", "tar.gz"),
        t if t.contains("aarch64") && t.contains("linux") =>
            ("sing-box-1.13.13-linux-arm64.tar.gz", "tar.gz"),
        // macOS
        t if t.contains("x86_64") && t.contains("apple") =>
            ("sing-box-1.13.13-darwin-amd64.tar.gz", "tar.gz"),
        t if t.contains("aarch64") && t.contains("apple") =>
            ("sing-box-1.13.13-darwin-arm64.tar.gz", "tar.gz"),
        // Windows
        t if t.contains("windows") =>
            ("sing-box-1.13.13-windows-amd64.zip", "zip"),
        // Fallback
        _ => {
            println!("cargo:warning=Unknown target: {target}, defaulting to linux-amd64");
            ("sing-box-1.13.13-linux-amd64.tar.gz", "tar.gz")
        }
    }
}

fn download_and_extract(url: &str, ext: &str, dest: &PathBuf) -> Result<(), String> {
    let resp = ureq::get(url)
        .call()
        .map_err(|e| format!("HTTP error: {e}"))?;

    let mut data = Vec::new();
    resp.into_reader()
        .read_to_end(&mut data)
        .map_err(|e| format!("Read error: {e}"))?;

    if data.is_empty() {
        return Err("Empty response".to_string());
    }

    match ext {
        "tar.gz" => {
            let gz = flate2::read::GzDecoder::new(&data[..]);
            let mut archive = tar::Archive::new(gz);
            for entry in archive.entries().map_err(|e| format!("Tar error: {e}"))? {
                let mut entry = entry.map_err(|e| format!("Entry error: {e}"))?;
                let path = entry.path().map_err(|e| format!("Path error: {e}"))?;
                if let Some(name) = path.file_name() {
                    let name = name.to_string_lossy();
                    if name == "sing-box" || name == "sing-box.exe" {
                        entry.unpack(dest).map_err(|e| format!("Unpack error: {e}"))?;
                        return Ok(());
                    }
                }
            }
            Err("sing-box binary not found in archive".to_string())
        }
        "zip" => {
            let cursor = std::io::Cursor::new(data);
            let mut archive = zip::ZipArchive::new(cursor)
                .map_err(|e| format!("Zip error: {e}"))?;
            for i in 0..archive.len() {
                let mut entry = archive.by_index(i).map_err(|e| format!("Entry error: {e}"))?;
                let name = entry.name().to_string();
                if name.ends_with("sing-box.exe") || name.ends_with("sing-box") {
                    let mut out = std::fs::File::create(dest)
                        .map_err(|e| format!("Create error: {e}"))?;
                    std::io::copy(&mut entry, &mut out)
                        .map_err(|e| format!("Copy error: {e}"))?;
                    return Ok(());
                }
            }
            Err("sing-box.exe not found in archive".to_string())
        }
        _ => Err(format!("Unknown archive format: {ext}")),
    }
}
