use serde::{Deserialize, Serialize};

/// Corresponds to the `subscriptions` table
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subscription {
    pub id: i64,
    pub identifier: String,
    pub name: Option<String>,
    pub used_traffic: i64,
    pub total_traffic: i64,
    pub subscription_url: Option<String>,
    pub official_website: Option<String>,
    pub expire_time: i64,
    pub last_update_time: i64,
}

/// Corresponds to the `subscription_configs` table
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionConfig {
    pub id: i64,
    pub identifier: String,
    pub config_content: Option<String>,
}

/// Corresponds to the `proxy_servers` table
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyServer {
    pub id: i64,
    pub identifier: String,
    pub name: String,
    pub server_address: String,
    pub server_port: i64,
    pub password: String,
    pub encryption_method: String,
    pub plugin: String,
    pub plugin_opts: String,
    pub is_active: i64,
    pub created_at: i64,
    pub updated_at: i64,
    pub proxy_type: String,
    pub username: String,
    pub vless_uuid: String,
    pub vless_opts: String,
}

/// Corresponds to the `proxy_groups` table
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyGroup {
    pub id: i64,
    pub identifier: String,
    pub name: String,
    pub group_type: String,
    pub is_active: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Corresponds to the `proxy_group_members` table
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyGroupMember {
    pub id: i64,
    pub group_identifier: String,
    pub server_identifier: String,
    pub sort_order: i64,
}

impl ProxyServer {
    /// Create a new ProxyServer from a CLI protocol argument
    pub fn from_protocol(proto: crate::cli::ProxyProtocol) -> anyhow::Result<Self> {
        let now = chrono::Utc::now().timestamp();
        let identifier = uuid::Uuid::new_v4().to_string();

        match proto {
            crate::cli::ProxyProtocol::Ss {
                name,
                server,
                port,
                password,
                method,
                plugin,
                plugin_opts,
            } => Ok(ProxyServer {
                id: 0,
                identifier,
                name,
                server_address: server,
                server_port: port as i64,
                password,
                encryption_method: method,
                plugin: plugin.unwrap_or_default(),
                plugin_opts: plugin_opts.unwrap_or_default(),
                is_active: 0,
                created_at: now,
                updated_at: now,
                proxy_type: "ss".to_string(),
                username: String::new(),
                vless_uuid: String::new(),
                vless_opts: String::new(),
            }),

            crate::cli::ProxyProtocol::Socks5 {
                name,
                server,
                port,
                username,
                password,
            } => Ok(ProxyServer {
                id: 0,
                identifier,
                name,
                server_address: server,
                server_port: port as i64,
                password: password.unwrap_or_default(),
                encryption_method: String::new(),
                plugin: String::new(),
                plugin_opts: String::new(),
                is_active: 0,
                created_at: now,
                updated_at: now,
                proxy_type: "socks5".to_string(),
                username: username.unwrap_or_default(),
                vless_uuid: String::new(),
                vless_opts: String::new(),
            }),

            crate::cli::ProxyProtocol::Http {
                name,
                server,
                port,
                username,
                password,
            } => Ok(ProxyServer {
                id: 0,
                identifier,
                name,
                server_address: server,
                server_port: port as i64,
                password: password.unwrap_or_default(),
                encryption_method: String::new(),
                plugin: String::new(),
                plugin_opts: String::new(),
                is_active: 0,
                created_at: now,
                updated_at: now,
                proxy_type: "http".to_string(),
                username: username.unwrap_or_default(),
                vless_uuid: String::new(),
                vless_opts: String::new(),
            }),

            crate::cli::ProxyProtocol::Trojan {
                name,
                server,
                port,
                password,
                opts,
            } => Ok(ProxyServer {
                id: 0,
                identifier,
                name,
                server_address: server,
                server_port: port as i64,
                password,
                encryption_method: String::new(),
                plugin: String::new(),
                plugin_opts: String::new(),
                is_active: 0,
                created_at: now,
                updated_at: now,
                proxy_type: "trojan".to_string(),
                username: String::new(),
                vless_uuid: String::new(),
                vless_opts: opts.unwrap_or_default(),
            }),

            crate::cli::ProxyProtocol::Vless {
                name,
                server,
                port,
                uuid: vless_uuid,
                opts,
            } => Ok(ProxyServer {
                id: 0,
                identifier,
                name,
                server_address: server,
                server_port: port as i64,
                password: String::new(),
                encryption_method: String::new(),
                plugin: String::new(),
                plugin_opts: String::new(),
                is_active: 0,
                created_at: now,
                updated_at: now,
                proxy_type: "vless".to_string(),
                username: String::new(),
                vless_uuid,
                vless_opts: opts.unwrap_or_default(),
            }),

            crate::cli::ProxyProtocol::Hysteria2 {
                name,
                server,
                port,
                password,
                opts,
            } => Ok(ProxyServer {
                id: 0,
                identifier,
                name,
                server_address: server,
                server_port: port as i64,
                password,
                encryption_method: String::new(),
                plugin: String::new(),
                plugin_opts: String::new(),
                is_active: 0,
                created_at: now,
                updated_at: now,
                proxy_type: "hysteria2".to_string(),
                username: String::new(),
                vless_uuid: String::new(),
                vless_opts: opts.unwrap_or_default(),
            }),
        }
    }
}
