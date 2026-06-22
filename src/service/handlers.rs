use axum::{
    Json,
    extract::Query,
    http::StatusCode,
};
use serde::{Deserialize, Serialize};

// ============================================================
// Request/Response types
// ============================================================

#[derive(Deserialize)]
pub struct StartRequest {
    pub mode: Option<String>,
    pub subscription: Option<String>,
}

#[derive(Deserialize)]
pub struct SubscriptionRequest {
    pub url: String,
    pub name: Option<String>,
}

#[derive(Deserialize)]
pub struct ProxyRequest {
    pub proxy_type: String,
    pub name: String,
    pub server: String,
    pub port: u16,
    pub password: Option<String>,
    pub method: Option<String>,
    pub username: Option<String>,
    pub uuid: Option<String>,
    pub plugin: Option<String>,
    pub plugin_opts: Option<String>,
    pub opts: Option<String>,
}

#[derive(Deserialize)]
pub struct GroupRequest {
    pub name: String,
    pub group_type: Option<String>,
}

#[derive(Deserialize)]
pub struct DeleteById {
    pub id: i64,
}

#[derive(Serialize)]
pub struct StatusResponse {
    pub state: String,
    pub mode: Option<String>,
    pub pid: Option<u32>,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

// ============================================================
// Health check
// ============================================================

pub async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok" }))
}

// ============================================================
// Engine control
// ============================================================

pub async fn get_status() -> Json<StatusResponse> {
    let state = crate::proxy::manager::snapshot();
    let mode = match &state {
        crate::proxy::manager::EngineState::Running { mode, .. } => Some(mode.clone()),
        crate::proxy::manager::EngineState::Starting { mode, .. } => Some(mode.clone()),
        _ => None,
    };
    let pid = crate::proxy::process::get_pid();

    let state_str = match state {
        crate::proxy::manager::EngineState::Idle { .. } => "idle",
        crate::proxy::manager::EngineState::Starting { .. } => "starting",
        crate::proxy::manager::EngineState::Running { .. } => "running",
        crate::proxy::manager::EngineState::Stopping { .. } => "stopping",
        crate::proxy::manager::EngineState::Failed { .. } => "failed",
    };

    Json(StatusResponse {
        state: state_str.to_string(),
        mode,
        pid,
    })
}

pub async fn post_start(
    Json(req): Json<StartRequest>,
) -> Result<Json<StatusResponse>, (StatusCode, Json<ErrorResponse>)> {
    let mode = req.mode.unwrap_or_else(|| "rule".to_string());
    let tun = mode == "tun" || mode == "nic";

    // Generate config
    let config_path = crate::core::generate_and_write_config(&mode, tun, req.subscription.as_deref(), None)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse { error: e.to_string() }),
            )
        })?;

    // Start sing-box
    let proxy_mode = match mode.as_str() {
        "tun" | "nic" => crate::proxy::ProxyMode::Tun,
        _ => crate::proxy::ProxyMode::System,
    };

    crate::proxy::process::start_singbox(&config_path, proxy_mode).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: e.to_string() }),
        )
    })?;

    // Transition state
    crate::proxy::manager::transition(crate::proxy::manager::Intent::Start {
        mode: mode.clone(),
    })
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: e.to_string() }),
        )
    })?;

    // Wait briefly for readiness
    if crate::proxy::readiness::wait_ready(std::time::Duration::from_secs(10)).await {
        let _ = crate::proxy::manager::transition(crate::proxy::manager::Intent::MarkRunning);
    }

    Ok(get_status().await)
}

pub async fn post_stop() -> Result<Json<StatusResponse>, (StatusCode, Json<ErrorResponse>)> {
    crate::proxy::process::stop_singbox().map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: e.to_string() }),
        )
    })?;

    let _ = crate::proxy::manager::transition(crate::proxy::manager::Intent::MarkIdle);

    Ok(get_status().await)
}

pub async fn post_reload() -> Result<Json<StatusResponse>, (StatusCode, Json<ErrorResponse>)> {
    crate::proxy::process::reload_singbox().map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: e.to_string() }),
        )
    })?;
    Ok(get_status().await)
}

pub async fn post_generate_config(
    Json(req): Json<StartRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let mode = req.mode.unwrap_or_else(|| "rule".to_string());
    let tun = mode == "tun" || mode == "nic";

    let config = crate::core::generate_config(&mode, tun, req.subscription.as_deref())
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse { error: e.to_string() }),
            )
        })?;

    Ok(Json(config))
}

// ============================================================
// Subscriptions
// ============================================================

pub async fn list_subscriptions() -> Result<Json<Vec<crate::db::models::Subscription>>, (StatusCode, Json<ErrorResponse>)> {
    let db_path = crate::db::db_path();
    let conn = crate::db::open(&db_path).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: e.to_string() }),
        )
    })?;
    let subs = crate::db::queries::get_all_subscriptions(&conn).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: e.to_string() }),
        )
    })?;
    Ok(Json(subs))
}

pub async fn add_subscription(
    Json(req): Json<SubscriptionRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let db_path = crate::db::db_path();
    let conn = crate::db::open(&db_path).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: e.to_string() }),
        )
    })?;
    let name = req.name.unwrap_or_else(|| "unnamed".to_string());
    let identifier = uuid::Uuid::new_v4().to_string();
    crate::db::queries::insert_subscription(&conn, &identifier, &req.url, &name).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: e.to_string() }),
        )
    })?;
    Ok(Json(serde_json::json!({ "status": "ok", "identifier": identifier })))
}

pub async fn delete_subscription(
    Query(params): Query<DeleteById>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let db_path = crate::db::db_path();
    let conn = crate::db::open(&db_path).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: e.to_string() }),
        )
    })?;
    crate::db::queries::delete_subscription(&conn, params.id).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: e.to_string() }),
        )
    })?;
    Ok(Json(serde_json::json!({ "status": "ok" })))
}

// ============================================================
// Proxy servers
// ============================================================

pub async fn list_proxies() -> Result<Json<Vec<crate::db::models::ProxyServer>>, (StatusCode, Json<ErrorResponse>)> {
    let db_path = crate::db::db_path();
    let conn = crate::db::open(&db_path).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: e.to_string() }),
        )
    })?;
    let servers = crate::db::queries::get_all_proxy_servers(&conn).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: e.to_string() }),
        )
    })?;
    Ok(Json(servers))
}

pub async fn add_proxy(
    Json(req): Json<ProxyRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let db_path = crate::db::db_path();
    let conn = crate::db::open(&db_path).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: e.to_string() }),
        )
    })?;

    let now = chrono::Utc::now().timestamp();
    let identifier = uuid::Uuid::new_v4().to_string();

    let server = crate::db::models::ProxyServer {
        id: 0,
        identifier,
        name: req.name,
        server_address: req.server,
        server_port: req.port as i64,
        password: req.password.unwrap_or_default(),
        encryption_method: req.method.unwrap_or_else(|| "aes-256-gcm".to_string()),
        plugin: req.plugin.unwrap_or_default(),
        plugin_opts: req.plugin_opts.unwrap_or_default(),
        is_active: 0,
        created_at: now,
        updated_at: now,
        proxy_type: req.proxy_type,
        username: req.username.unwrap_or_default(),
        vless_uuid: req.uuid.unwrap_or_default(),
        vless_opts: req.opts.unwrap_or_default(),
    };

    crate::db::queries::insert_proxy_server(&conn, &server).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: e.to_string() }),
        )
    })?;
    Ok(Json(serde_json::json!({ "status": "ok" })))
}

pub async fn delete_proxy(
    Query(params): Query<DeleteById>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let db_path = crate::db::db_path();
    let conn = crate::db::open(&db_path).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: e.to_string() }),
        )
    })?;
    crate::db::queries::delete_proxy_server(&conn, params.id).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: e.to_string() }),
        )
    })?;
    Ok(Json(serde_json::json!({ "status": "ok" })))
}

// ============================================================
// Proxy groups
// ============================================================

pub async fn list_groups() -> Result<Json<Vec<crate::db::models::ProxyGroup>>, (StatusCode, Json<ErrorResponse>)> {
    let db_path = crate::db::db_path();
    let conn = crate::db::open(&db_path).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: e.to_string() }),
        )
    })?;
    let groups = crate::db::queries::get_all_proxy_groups(&conn).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: e.to_string() }),
        )
    })?;
    Ok(Json(groups))
}

pub async fn add_group(
    Json(req): Json<GroupRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let db_path = crate::db::db_path();
    let conn = crate::db::open(&db_path).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: e.to_string() }),
        )
    })?;
    let group_type = req.group_type.unwrap_or_else(|| "fixed".to_string());
    let identifier = crate::db::queries::insert_proxy_group(&conn, &req.name, &group_type)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse { error: e.to_string() }),
            )
        })?;
    Ok(Json(serde_json::json!({ "status": "ok", "identifier": identifier })))
}

pub async fn delete_group(
    Query(params): Query<DeleteById>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let db_path = crate::db::db_path();
    let conn = crate::db::open(&db_path).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: e.to_string() }),
        )
    })?;
    crate::db::queries::delete_proxy_group(&conn, params.id).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: e.to_string() }),
        )
    })?;
    Ok(Json(serde_json::json!({ "status": "ok" })))
}

// ============================================================
// Logs and traffic
// ============================================================

pub async fn get_logs() -> Json<serde_json::Value> {
    // Basic implementation: return engine state as log info
    let state = crate::proxy::manager::snapshot();
    let running = crate::proxy::process::is_running();
    Json(serde_json::json!({
        "engine_state": state,
        "singbox_running": running,
    }))
}

pub async fn get_traffic() -> Json<serde_json::Value> {
    // Basic implementation: return placeholder traffic stats
    // Full implementation would query sing-box clash API at 127.0.0.1:9191
    Json(serde_json::json!({
        "upload_bytes": 0,
        "download_bytes": 0,
        "note": "Traffic stats require querying sing-box clash API"
    }))
}
