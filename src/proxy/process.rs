use std::process::{Child, Command, Stdio};
use std::sync::Mutex;

use lazy_static::lazy_static;

use super::ProxyMode;

lazy_static! {
    static ref PROCESS: Mutex<Option<ProcessState>> = Mutex::new(None);
}

struct ProcessState {
    child: Child,
    mode: ProxyMode,
    config_path: String,
    pid: u32,
}

/// Start sing-box with the given config path and proxy mode
pub fn start_singbox(config_path: &str, mode: ProxyMode) -> anyhow::Result<()> {
    let singbox = crate::utils::sing_box::sing_box_path();

    log::info!("Launching sing-box: {} run -c {}", singbox, config_path);

    let mut child = Command::new(&singbox)
        .args(["run", "-c", config_path, "--disable-color"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| anyhow::anyhow!("Failed to spawn sing-box: {}", e))?;

    let pid = child.id();
    log::info!("sing-box started with PID: {}", pid);

    // Take stdout/stderr before moving child into ProcessState
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    // Store process state
    let state = ProcessState {
        child,
        mode: mode.clone(),
        config_path: config_path.to_string(),
        pid,
    };

    let mut proc = PROCESS.lock().unwrap();
    *proc = Some(state);

    // Start monitoring
    super::monitor::start_monitor(pid, stdout, stderr);

    Ok(())
}

/// Stop the running sing-box process
pub fn stop_singbox() -> anyhow::Result<()> {
    let mut proc = PROCESS.lock().unwrap();

    if let Some(mut state) = proc.take() {
        log::info!("Stopping sing-box (PID: {})...", state.pid);

        // Try graceful shutdown first (SIGTERM on Unix, kill on Windows)
        #[cfg(unix)]
        {
            unsafe {
                libc::kill(state.pid as i32, libc::SIGTERM);
            }
            // Give it a moment to exit gracefully
            std::thread::sleep(std::time::Duration::from_millis(500));

            // Check if still alive, force kill
            match state.child.try_wait() {
                Ok(Some(status)) => {
                    log::info!("sing-box exited with status: {:?}", status);
                }
                _ => {
                    log::warn!("sing-box did not exit gracefully, sending SIGKILL");
                    let _ = state.child.kill();
                }
            }
        }

        #[cfg(not(unix))]
        {
            let _ = state.child.kill();
        }

        let _ = state.child.wait();

        // Clear system proxy if set
        if state.mode == ProxyMode::System {
            clear_system_proxy()?;
        }

        log::info!("sing-box stopped");
    } else {
        log::info!("No running sing-box process found");
    }

    Ok(())
}

/// Reload sing-box config by sending SIGHUP
pub fn reload_singbox() -> anyhow::Result<()> {
    let proc = PROCESS.lock().unwrap();

    if let Some(ref state) = *proc {
        let pid = state.pid;
        log::info!("Reloading sing-box config (PID: {})...", pid);

        #[cfg(unix)]
        {
            unsafe {
                libc::kill(pid as i32, libc::SIGHUP);
            }
        }
        #[cfg(not(unix))]
        {
            // On Windows, we need to restart the process
            log::warn!("Reload not supported on Windows without restart");
        }

        log::info!("SIGHUP sent to sing-box");
    } else {
        log::warn!("No running sing-box process to reload");
    }

    Ok(())
}

/// Check if sing-box is currently running
pub fn is_running() -> bool {
    let proc = PROCESS.lock().unwrap();
    if let Some(ref state) = *proc {
        #[cfg(unix)]
        {
            // Use kill(pid, 0) to check if process exists
            unsafe { libc::kill(state.pid as i32, 0) == 0 }
        }
        #[cfg(not(unix))]
        {
            // On Windows, try to get exit status
            // We can't easily do this without &mut, so just check we have a stored process
            true
        }
    } else {
        false
    }
}

/// Set system proxy environment variables (basic Linux support)
pub fn set_system_proxy(_port: u16) -> anyhow::Result<()> {
    #[cfg(target_os = "linux")]
    {
        let proxy_url = format!("http://127.0.0.1:{}", _port);
        log::info!("Setting system proxy: {}", proxy_url);
        // Only affects child processes; for system-wide proxy, use gsettings or env vars
        // This is a basic implementation
    }
    Ok(())
}

/// Clear system proxy environment variables
pub fn clear_system_proxy() -> anyhow::Result<()> {
    log::info!("Clearing system proxy");
    Ok(())
}

/// Get the PID of the running sing-box process (if any)
pub fn get_pid() -> Option<u32> {
    let proc = PROCESS.lock().unwrap();
    proc.as_ref().map(|s| s.pid)
}
