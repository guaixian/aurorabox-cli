use std::io::{BufRead, BufReader};
use std::process::{ChildStdout, ChildStderr};
use std::sync::atomic::{AtomicBool, Ordering};

static EXIT_FLAG: AtomicBool = AtomicBool::new(false);

/// Start monitoring sing-box stdout/stderr in background tasks
pub fn start_monitor(pid: u32, stdout: Option<ChildStdout>, stderr: Option<ChildStderr>) {
    // Monitor stdout
    if let Some(stdout) = stdout {
        std::thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                match line {
                    Ok(line) if !line.is_empty() => {
                        log::info!("[sing-box:{}] {}", pid, line);
                    }
                    Err(e) => {
                        log::debug!("sing-box stdout closed: {}", e);
                        break;
                    }
                    _ => {}
                }
            }
        });
    }

    // Monitor stderr
    if let Some(stderr) = stderr {
        std::thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines() {
                match line {
                    Ok(line) if !line.is_empty() => {
                        log::warn!("[sing-box:{}:err] {}", pid, line);
                        scan_for_bind_error(&line);
                    }
                    Err(e) => {
                        log::debug!("sing-box stderr closed: {}", e);
                        break;
                    }
                    _ => {}
                }
            }
            // stderr closed means process likely exited
            EXIT_FLAG.store(true, Ordering::SeqCst);
            log::info!("sing-box (PID: {}) monitor ended", pid);
        });
    }
}

fn scan_for_bind_error(line: &str) {
    let lower = line.to_lowercase();
    if lower.contains("address already in use") || lower.contains("eaddrinuse") {
        log::error!(
            "[sing-box] PORT ALREADY IN USE — another process may be using the mixed port"
        );
    }
    if lower.contains("bind: permission denied") {
        log::error!(
            "[sing-box] PERMISSION DENIED — port <1024 requires root or TUN mode may need admin"
        );
    }
}

/// Wait for the sing-box process to exit (blocks current thread/task)
pub async fn wait_for_exit() {
    // Poll the exit flag periodically
    loop {
        if EXIT_FLAG.load(Ordering::SeqCst) {
            log::info!("sing-box process has exited");
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
}

/// Check if the process has exited
pub fn has_exited() -> bool {
    EXIT_FLAG.load(Ordering::SeqCst)
}
