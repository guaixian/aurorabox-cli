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
        .route("/api/clear-error", post(handlers::clear_error))
        // Config
        .route("/api/config/generate", post(handlers::post_generate_config))
        // Version / info
        .route("/api/version", get(handlers::get_version))
        .route("/api/paths", get(handlers::get_paths))
        // Subscriptions
        .route("/api/subscriptions", get(handlers::list_subscriptions))
        .route("/api/subscriptions", post(handlers::add_subscription))
        .route("/api/subscriptions", delete(handlers::delete_subscription))
        .route("/api/subscriptions/fetch", post(handlers::fetch_subscription))
        // Proxy servers
        .route("/api/proxies", get(handlers::list_proxies))
        .route("/api/proxies", post(handlers::add_proxy))
        .route("/api/proxies", delete(handlers::delete_proxy))
        .route("/api/proxies/test", post(handlers::test_proxies))
        // Proxy port
        .route("/api/proxy/port-check", get(handlers::port_check))
        .route("/api/proxy/kill-orphans", post(handlers::kill_orphans))
        // Proxy groups
        .route("/api/groups", get(handlers::list_groups))
        .route("/api/groups", post(handlers::add_group))
        .route("/api/groups", delete(handlers::delete_group))
        // Logs and traffic
        .route("/api/logs", get(handlers::get_logs))
        .route("/api/traffic", get(handlers::get_traffic))
        // Network
        .route("/api/network/lan-ip", get(handlers::get_lan_ip))
        .route("/api/network/ping", get(handlers::ping_google))
        .route("/api/network/captive", get(handlers::check_captive))
        .route("/api/network/captive-url", get(handlers::captive_url))
        .route("/api/network/open-url", post(handlers::open_url))
        // DNS
        .route("/api/dns/optimal", get(handlers::get_optimal_dns))
        // Chain proxy
        .route("/api/chain/start", post(handlers::start_chain))
        .route("/api/chain/stop", post(handlers::stop_chain))
        // Deep link
        .route("/api/deep-link/pending", get(handlers::pending_deep_link))
        .route("/api/deep-link/verify", post(handlers::verify_deep_link))
        // Engine install/probe
        .route("/api/engine/probe", get(handlers::engine_probe))
        .route("/api/engine/install", post(handlers::engine_install))
        // Theme (no-op)
        .route("/api/theme", post(handlers::set_theme))
        // Database proxy
        .route("/api/db/execute", post(handlers::db_execute))
        .route("/api/db/select", post(handlers::db_select))
        // Health check
        .route("/api/health", get(handlers::health_check));

    // Merge API routes and serve static files as fallback
    Router::new()
        .merge(api)
        .fallback_service(get(super::static_files::serve_static))
}
