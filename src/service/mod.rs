pub mod handlers;
pub mod router;
pub mod static_files;

/// Start the web server on the given host and port
pub async fn start_web_server(host: &str, port: u16) -> anyhow::Result<()> {
    let app = router::create_router();
    let addr = format!("{}:{}", host, port);

    log::info!("Starting web server on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
