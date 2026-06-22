use std::net::TcpStream;
use std::time::{Duration, Instant};

/// Wait for the sing-box clash API to become ready by TCP probing port 9191.
/// Returns true if ready within timeout, false otherwise.
pub async fn wait_ready(timeout: Duration) -> bool {
    let start = Instant::now();
    let addr = "127.0.0.1:9191";

    loop {
        if start.elapsed() >= timeout {
            log::warn!("Readiness probe timed out after {:?}", timeout);
            return false;
        }

        match TcpStream::connect_timeout(&addr.parse().unwrap(), Duration::from_millis(200)) {
            Ok(_) => {
                log::info!("sing-box is ready (clash API reachable on {})", addr);
                return true;
            }
            Err(_) => {
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        }
    }
}
