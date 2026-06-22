use axum::{
    Router,
    routing::{delete, get, post},
};

use super::handlers;

/// Create the full Axum router with all API routes and static file serving
pub fn create_router() -> Router {
    let api = Router::new()
        // Engine control
        .route("/api/status", get(handlers::get_status))
        .route("/api/start", post(handlers::post_start))
        .route("/api/stop", post(handlers::post_stop))
        .route("/api/reload", post(handlers::post_reload))
        // Config
        .route("/api/config/generate", post(handlers::post_generate_config))
        // Subscriptions
        .route("/api/subscriptions", get(handlers::list_subscriptions))
        .route("/api/subscriptions", post(handlers::add_subscription))
        .route("/api/subscriptions", delete(handlers::delete_subscription))
        // Proxy servers
        .route("/api/proxies", get(handlers::list_proxies))
        .route("/api/proxies", post(handlers::add_proxy))
        .route("/api/proxies", delete(handlers::delete_proxy))
        // Proxy groups
        .route("/api/groups", get(handlers::list_groups))
        .route("/api/groups", post(handlers::add_group))
        .route("/api/groups", delete(handlers::delete_group))
        // Logs and traffic
        .route("/api/logs", get(handlers::get_logs))
        .route("/api/traffic", get(handlers::get_traffic))
        // Health check
        .route("/api/health", get(handlers::health_check));

    // Mount API routes and serve static files as fallback
    Router::new()
        .nest("/", api)
        .fallback_service(get(super::static_files::serve_static))
}
