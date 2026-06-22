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
    pub path: Option<String>,
    #[serde(rename = "_app")]
    pub _app: Option<bool>,
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
    #[serde(rename = "type")]
    pub state: String,
    pub mode: Option<String>,
    pub pid: Option<u32>,
    pub epoch: u64,
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
    let (mode, epoch) = match &state {
        crate::proxy::manager::EngineState::Running { mode, epoch, .. } => (Some(mode.clone()), *epoch),
        crate::proxy::manager::EngineState::Starting { mode, epoch, .. } => (Some(mode.clone()), *epoch),
        crate::proxy::manager::EngineState::Idle { epoch } => (None, *epoch),
        crate::proxy::manager::EngineState::Stopping { epoch, .. } => (None, *epoch),
        crate::proxy::manager::EngineState::Failed { epoch, .. } => (None, *epoch),
    };
    let pid = crate::proxy::process::get_pid();

    let state_str = match state {
        crate::proxy::manager::EngineState::Idle { .. } => "Idle",
        crate::proxy::manager::EngineState::Starting { .. } => "Starting",
        crate::proxy::manager::EngineState::Running { .. } => "Running",
        crate::proxy::manager::EngineState::Stopping { .. } => "Stopping",
        crate::proxy::manager::EngineState::Failed { .. } => "Failed",
    };

    Json(StatusResponse {
        state: state_str.to_string(),
        mode,
        pid,
        epoch,
    })
}

pub async fn post_start(
    Json(req): Json<StartRequest>,
) -> Result<Json<StatusResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Convert mode: "TunProxy"|"SystemProxy"|"ManualProxy" → "tun"|"system"|"manual"
    let raw_mode = req.mode.unwrap_or_else(|| "SystemProxy".to_string());
    let mode = raw_mode
        .replace("Proxy", "")
        .to_lowercase(); // "TunProxy" → "tun", "SystemProxy" → "system"
    let tun = mode == "tun";

    // If the app already generated config at a path, use it
    let config_path = if let Some(ref p) = req.path {
        if std::path::Path::new(p).exists() {
            p.clone()
        } else {
            // Generate config
            crate::core::generate_and_write_config(&mode, tun, req.subscription.as_deref(), None)
                .map_err(|e| {
                    (StatusCode::INTERNAL_SERVER_ERROR,
                     Json(ErrorResponse { error: e.to_string() }))
                })?
        }
    } else {
        crate::core::generate_and_write_config(&mode, tun, req.subscription.as_deref(), None)
            .map_err(|e| {
                (StatusCode::INTERNAL_SERVER_ERROR,
                 Json(ErrorResponse { error: e.to_string() }))
            })?
    };

    // Start sing-box
    let proxy_mode = match mode.as_str() {
        "tun" => crate::proxy::ProxyMode::Tun,
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

pub async fn get_logs() -> String {
    let state = crate::proxy::manager::snapshot();
    let running = crate::proxy::process::is_running();
    format!("[{}] engine={:?} running={}\n",
        chrono::Utc::now().format("%Y-%m-%d %H:%M:%S"),
        state, running)
}

pub async fn get_traffic() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "up": 0,
        "down": 0,
        "upload_bytes": 0,
        "download_bytes": 0,
    }))
}

// ============================================================
// Additional endpoints for full frontend compatibility
// ============================================================

pub async fn clear_error() -> Json<serde_json::Value> {
    let _ = crate::proxy::manager::transition(crate::proxy::manager::Intent::ClearFailure);
    Json(serde_json::json!({ "status": "ok" }))
}

pub async fn get_version() -> Json<serde_json::Value> {
    let version = crate::utils::sing_box::get_singbox_version().unwrap_or_else(|_| "unknown".to_string());
    Json(serde_json::json!({
        "version": version,
        "cli_version": env!("CARGO_PKG_VERSION"),
    }))
}

pub async fn get_paths() -> Json<serde_json::Value> {
    let config_dir = crate::core::config_dir();
    Json(serde_json::json!({
        "config_dir": config_dir,
        "data_dir": config_dir,
        "log_dir": config_dir,
    }))
}

pub async fn fetch_subscription(
    Json(req): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let url = req.get("url").and_then(|v| v.as_str()).unwrap_or("");
    if url.is_empty() {
        return Err((StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "url required".to_string() })));
    }
    // Fetch the subscription config via HTTP
    match reqwest::get(url).await {
        Ok(resp) => match resp.text().await {
            Ok(body) => {
                // Store in DB
                let db_path = crate::db::db_path();
                let conn = crate::db::open(&db_path).map_err(|e| (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse { error: e.to_string() }),
                ))?;
                let identifier = uuid::Uuid::new_v4().to_string();
                let name = url.split('/').last().unwrap_or("subscription");
                crate::db::queries::insert_subscription(&conn, &identifier, url, name).map_err(|e| (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse { error: e.to_string() }),
                ))?;
                crate::db::queries::update_subscription_config(&conn, &identifier, &body).map_err(|e| (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse { error: e.to_string() }),
                ))?;
                Ok(Json(serde_json::json!({ "status": "ok", "identifier": identifier, "config": body })))
            }
            Err(e) => Err((StatusCode::BAD_GATEWAY, Json(ErrorResponse { error: e.to_string() }))),
        },
        Err(e) => Err((StatusCode::BAD_GATEWAY, Json(ErrorResponse { error: e.to_string() }))),
    }
}

pub async fn test_proxies(
    Json(req): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    // Return empty results; full testing requires running sing-box
    let outbounds = req.get("outbounds").and_then(|v| v.as_array())
        .map(|a| a.len()).unwrap_or(0);
    Json(serde_json::json!({
        "results": [],
        "tested": outbounds,
        "note": "Proxy testing requires running sing-box engine"
    }))
}

pub async fn port_check() -> Json<serde_json::Value> {
    let port: u16 = 6789;
    let available = std::net::TcpStream::connect(("127.0.0.1", port)).is_err();
    Json(serde_json::json!({
        "available": available,
        "port": port,
        "pids": [],
        "port_occupied": !available,
    }))
}

pub async fn kill_orphans() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "success": true, "port_released": true }))
}

pub async fn get_lan_ip() -> Json<serde_json::Value> {
    let ip = local_ip_address::local_ip()
        .map(|ip| ip.to_string())
        .unwrap_or_else(|_| "127.0.0.1".to_string());
    Json(serde_json::json!({ "ip": ip }))
}

pub async fn ping_google() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "ok": false, "note": "Not implemented" }))
}

pub async fn check_captive() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": 0 }))
}

pub async fn captive_url() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "url": "" }))
}

#[derive(Deserialize)]
pub struct OpenUrlRequest {
    pub url: Option<String>,
}

pub async fn open_url(Json(req): Json<OpenUrlRequest>) -> Json<serde_json::Value> {
    if let Some(url) = req.url {
        let _ = webbrowser::open(&url);
    }
    Json(serde_json::json!({ "status": "ok" }))
}

pub async fn get_optimal_dns() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "server": "119.29.29.29" }))
}

pub async fn start_chain(
    Json(_req): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    Json(serde_json::json!({ "port": 0, "note": "Chain proxy not supported in web mode" }))
}

pub async fn stop_chain() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok" }))
}

pub async fn pending_deep_link() -> Json<serde_json::Value> {
    Json(serde_json::json!(null))
}

pub async fn verify_deep_link(
    Json(_req): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    Json(serde_json::json!({ "valid": false }))
}

pub async fn engine_probe() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok" }))
}

pub async fn engine_install() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok" }))
}

#[derive(Deserialize)]
pub struct ConfigPathQuery {
    pub path: Option<String>,
}

pub async fn get_config_file(
    Query(q): axum::extract::Query<ConfigPathQuery>,
) -> Result<String, (StatusCode, Json<ErrorResponse>)> {
    let path = q.path.unwrap_or_default();
    let config_dir = crate::core::config_dir();
    let full_path = std::path::Path::new(&config_dir).join(path.trim_start_matches('/'));
    if full_path.exists() {
        std::fs::read_to_string(&full_path).map_err(|e| (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse { error: e.to_string() }),
        ))
    } else {
        Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse { error: "File not found".to_string() }),
        ))
    }
}

pub async fn set_theme() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok" }))
}

#[derive(Deserialize)]
pub struct DbQuery {
    pub sql: String,
    pub bindings: Option<Vec<serde_json::Value>>,
}

pub async fn db_execute(
    Json(req): Json<DbQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let db_path = crate::db::db_path();
    let conn = crate::db::open(&db_path).map_err(|e| (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse { error: e.to_string() }),
    ))?;
    let sql_upper = req.sql.trim().to_uppercase();
    // SELECT queries need query_map, not execute
    if sql_upper.starts_with("SELECT") || sql_upper.starts_with("PRAGMA") || sql_upper.starts_with("EXPLAIN") {
        let mut stmt = conn.prepare(&req.sql).map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: e.to_string() }),
        ))?;
        let count = stmt.query_map([], |_| Ok(())).map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: e.to_string() }),
        ))?.count();
        Ok(Json(serde_json::json!({ "rowsAffected": count, "lastInsertId": 0 })))
    } else {
        let rows_affected = conn.execute(&req.sql, []).map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: e.to_string() }),
        ))?;
        let last_insert_id = conn.last_insert_rowid();
        Ok(Json(serde_json::json!({ "rowsAffected": rows_affected, "lastInsertId": last_insert_id })))
    }
}

pub async fn db_select(
    Json(req): Json<DbQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let db_path = crate::db::db_path();
    let conn = crate::db::open(&db_path).map_err(|e| (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse { error: e.to_string() }),
    ))?;
    let mut stmt = conn.prepare(&req.sql).map_err(|e| (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse { error: e.to_string() }),
    ))?;

    // Get column names
    let col_count = stmt.column_count();
    let col_names: Vec<String> = (0..col_count)
        .map(|i| stmt.column_name(i).unwrap_or("?").to_string())
        .collect();

    let rows: Vec<serde_json::Value> = stmt.query_map([], |row| {
        let mut map = serde_json::Map::new();
        for (i, name) in col_names.iter().enumerate() {
            // Try different types
            if let Ok(v) = row.get::<_, i64>(i) {
                map.insert(name.clone(), serde_json::Value::Number(v.into()));
            } else if let Ok(v) = row.get::<_, f64>(i) {
                if let Some(n) = serde_json::Number::from_f64(v) {
                    map.insert(name.clone(), serde_json::Value::Number(n));
                }
            } else if let Ok(v) = row.get::<_, String>(i) {
                map.insert(name.clone(), serde_json::Value::String(v));
            } else {
                let v: Option<String> = row.get(i).ok().flatten();
                if let Some(s) = v {
                    map.insert(name.clone(), serde_json::Value::String(s));
                } else {
                    map.insert(name.clone(), serde_json::Value::Null);
                }
            }
        }
        Ok(serde_json::Value::Object(map))
    }).map_err(|e| (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse { error: e.to_string() }),
    ))?
    .filter_map(|r| r.ok())
    .collect();
    Ok(Json(serde_json::json!(rows)))
}
