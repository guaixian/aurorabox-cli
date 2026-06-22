use axum::{
    body::Body,
    http::{Response, StatusCode, header},
    response::IntoResponse,
};

/// Embedded frontend assets (using rust-embed)
/// Contains the AuroraBox React frontend dist files plus tauri-shim.js
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
            let body = if path == "index.html" || path.ends_with("index.html") {
                // Inject tauri-shim.js before the first <script> tag
                inject_shim_script(&content.data)
            } else {
                content.data.to_vec()
            };

            Response::builder()
                .header(header::CONTENT_TYPE, mime.as_ref())
                .status(StatusCode::OK)
                .body(Body::from(body))
                .unwrap()
        }
        None => {
            // SPA fallback: serve index.html for any non-file routes
            if let Some(index) = FrontendAssets::get("index.html") {
                let body = inject_shim_script(&index.data);
                Response::builder()
                    .header(header::CONTENT_TYPE, "text/html")
                    .status(StatusCode::OK)
                    .body(Body::from(body))
                    .unwrap()
            } else {
                serve_placeholder()
            }
        }
    }
}

/// Inject the tauri-shim.js script tag into the HTML before any other scripts.
/// This ensures the Tauri compatibility layer loads before the React app.
fn inject_shim_script(html_bytes: &[u8]) -> Vec<u8> {
    let html = String::from_utf8_lossy(html_bytes);

    // We need to inject: <script src="/tauri-shim.js"></script>
    // The shim must load BEFORE any module scripts (like the Vite bundle).
    // We inject it right after <head> or before the first <script> tag.

    let shim_tag = r#"<script src="/tauri-shim.js"></script>"#;

    // Strategy: inject right before the first <script> tag in <head>
    if let Some(script_pos) = html.find("<script") {
        let mut result = String::with_capacity(html.len() + shim_tag.len() + 20);
        result.push_str(&html[..script_pos]);
        // Add a blocking script tag (no defer/async) so it loads first
        result.push_str(&format!("\n    {}\n    ", shim_tag));
        result.push_str(&html[script_pos..]);
        return result.into_bytes();
    }

    // Fallback: inject after <head>
    if let Some(head_pos) = html.find("<head>") {
        let insert_pos = head_pos + 6;
        let mut result = String::with_capacity(html.len() + shim_tag.len() + 20);
        result.push_str(&html[..insert_pos]);
        result.push_str(&format!("\n    {}\n", shim_tag));
        result.push_str(&html[insert_pos..]);
        return result.into_bytes();
    }

    // Last resort: prepend to body
    html_bytes.to_vec()
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
        .container { max-width: 600px; width: 100%; text-align: center; }
        h1 { font-size: 2rem; margin-bottom: 0.5rem; color: #6c5ce7; }
        p { color: #888; margin-bottom: 1rem; }
        .warning {
            background: #2a1a00; border: 1px solid #554400; color: #ffa500;
            padding: 12px 20px; border-radius: 8px; margin-top: 2rem;
        }
        code { background: #1a1a1a; padding: 2px 8px; border-radius: 4px; color: #6c5ce7; }
    </style>
</head>
<body>
    <div class="container">
        <h1>⚡ AuroraBox CLI</h1>
        <p>API server is running. To enable the full web UI:</p>
        <div class="warning">
            <strong>Frontend not built.</strong><br>
            Build the AuroraBox frontend and copy <code>dist/</code> to <code>static/</code>,<br>
            or run <code>cargo build</code> with the frontend available.
        </div>
        <p style="margin-top: 2rem;">
            <a href="/api/health" style="color: #6c5ce7;">API Health</a> ·
            <a href="/api/status" style="color: #6c5ce7;">Engine Status</a>
        </p>
    </div>
</body>
</html>"#;

    Response::builder()
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .status(StatusCode::OK)
        .body(Body::from(html))
        .unwrap()
}
