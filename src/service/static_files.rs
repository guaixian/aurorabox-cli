use axum::{
    body::Body,
    http::{Response, StatusCode, header},
    response::IntoResponse,
};

/// Embedded frontend assets (using rust-embed)
/// The static/ directory is populated at build time.
/// If no frontend is available, a simple placeholder UI is served.
#[derive(rust_embed::RustEmbed)]
#[folder = "static/"]
struct FrontendAssets;

/// Serve a static file or fall back to the SPA index page
pub async fn serve_static(
    req: axum::http::Request<axum::body::Body>,
) -> impl IntoResponse {
    let path = req.uri().path();
    let path = path.trim_start_matches('/');

    // Default to index.html
    let path = if path.is_empty() { "index.html" } else { path };

    match FrontendAssets::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            Response::builder()
                .header(header::CONTENT_TYPE, mime.as_ref())
                .status(StatusCode::OK)
                .body(Body::from(content.data.to_vec()))
                .unwrap()
        }
        None => {
            // SPA fallback: serve index.html for any non-file routes
            if let Some(index) = FrontendAssets::get("index.html") {
                Response::builder()
                    .header(header::CONTENT_TYPE, "text/html")
                    .status(StatusCode::OK)
                    .body(Body::from(index.data.to_vec()))
                    .unwrap()
            } else {
                // No frontend at all - serve a simple placeholder
                serve_placeholder()
            }
        }
    }
}

/// Serve a simple placeholder HTML when no frontend is embedded
fn serve_placeholder() -> Response<Body> {
    let html = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>AuroraBox CLI</title>
    <style>
        * { margin: 0; padding: 0; box-sizing: border-box; }
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            background: #0f0f0f; color: #e0e0e0;
            display: flex; justify-content: center; align-items: center;
            min-height: 100vh; padding: 20px;
        }
        .container { max-width: 600px; width: 100%; }
        h1 { font-size: 2rem; margin-bottom: 0.5rem; color: #6c5ce7; }
        .subtitle { color: #888; margin-bottom: 2rem; }
        .card {
            background: #1a1a1a; border: 1px solid #2a2a2a;
            border-radius: 12px; padding: 24px; margin-bottom: 16px;
        }
        .card h2 { font-size: 1.1rem; margin-bottom: 12px; color: #aaa; }
        .status { display: flex; align-items: center; gap: 8px; }
        .dot { width: 10px; height: 10px; border-radius: 50%; background: #555; }
        .dot.running { background: #00d68f; }
        .endpoints { display: grid; gap: 8px; }
        .endpoint {
            display: flex; justify-content: space-between;
            padding: 8px 12px; background: #141414; border-radius: 8px;
            font-family: monospace; font-size: 0.85rem;
        }
        .method { color: #6c5ce7; }
        .path { color: #ccc; }
    </style>
</head>
<body>
    <div class="container">
        <h1>⚡ AuroraBox CLI</h1>
        <p class="subtitle">sing-box proxy manager — API is running</p>

        <div class="card">
            <h2>Engine Status</h2>
            <div class="status">
                <div class="dot" id="status-dot"></div>
                <span id="status-text">Loading...</span>
            </div>
        </div>

        <div class="card">
            <h2>REST API Endpoints</h2>
            <div class="endpoints">
                <div class="endpoint"><span class="method">GET</span><span class="path">/api/status</span></div>
                <div class="endpoint"><span class="method">POST</span><span class="path">/api/start</span></div>
                <div class="endpoint"><span class="method">POST</span><span class="path">/api/stop</span></div>
                <div class="endpoint"><span class="method">POST</span><span class="path">/api/reload</span></div>
                <div class="endpoint"><span class="method">GET</span><span class="path">/api/subscriptions</span></div>
                <div class="endpoint"><span class="method">POST</span><span class="path">/api/subscriptions</span></div>
                <div class="endpoint"><span class="method">GET</span><span class="path">/api/proxies</span></div>
                <div class="endpoint"><span class="method">POST</span><span class="path">/api/proxies</span></div>
                <div class="endpoint"><span class="method">GET</span><span class="path">/api/groups</span></div>
                <div class="endpoint"><span class="method">POST</span><span class="path">/api/groups</span></div>
            </div>
        </div>
    </div>
    <script>
        async function checkStatus() {
            try {
                const r = await fetch("/api/status");
                const data = await r.json();
                document.getElementById("status-text").textContent =
                    "State: " + data.state + (data.mode ? " (" + data.mode + ")" : "");
                const dot = document.getElementById("status-dot");
                if (data.state === "running") dot.classList.add("running");
                else dot.classList.remove("running");
            } catch(e) {
                document.getElementById("status-text").textContent = "API error";
            }
        }
        checkStatus();
        setInterval(checkStatus, 5000);
    </script>
</body>
</html>"#;

    Response::builder()
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .status(StatusCode::OK)
        .body(Body::from(html))
        .unwrap()
}
