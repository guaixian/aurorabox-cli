use serde_json::{json, Value};

use crate::db::models::ProxyServer;

/// Build a sing-box outbound JSON object from a ProxyServer DB record.
/// This is a direct port of `buildOutboundJSON` from the TypeScript frontend.
pub fn build_outbound(server: &ProxyServer) -> anyhow::Result<Value> {
    let ptype = server.proxy_type.as_str();
    let tag = format!("{}-{}", ptype, &server.identifier[..server.identifier.len().min(8)]);

    let mut base = json!({
        "tag": tag,
        "server": server.server_address,
        "server_port": server.server_port,
        "domain_resolver": "system",
    });

    match ptype {
        "hysteria2" => {
            base["type"] = json!("hysteria2");
            base["password"] = json!(server.password);

            let opts = parse_opts(&server.vless_opts);
            let sni = opts.get("sni").cloned().unwrap_or_else(|| server.server_address.clone());

            base["tls"] = json!({
                "enabled": true,
                "server_name": sni,
                "insecure": opts.get("insecure").map(|v| v == "1")
                    .or_else(|| opts.get("allowInsecure").map(|v| v == "1"))
                    .unwrap_or(false)
            });

            if let Some(obfs_pw) = opts.get("obfs") {
                base["obfs"] = json!({
                    "type": "salamander",
                    "password": obfs_pw
                });
            }

            // up_mbps / down_mbps
            if let Some(up) = opts.get("upmbps").and_then(|v| v.parse::<u32>().ok()) {
                base["up_mbps"] = json!(up);
            }
            if let Some(down) = opts.get("downmbps").and_then(|v| v.parse::<u32>().ok()) {
                base["down_mbps"] = json!(down);
            }
        }

        "vless" => {
            base["type"] = json!("vless");
            base["uuid"] = json!(server.vless_uuid);

            let opts = parse_opts(&server.vless_opts);
            let sec = opts.get("security").map(|s| s.as_str()).unwrap_or("none");

            if sec != "none" {
                let sni = opts.get("sni").cloned().unwrap_or_default();
                base["tls"] = json!({
                    "enabled": true,
                    "server_name": sni
                });

                if sec == "reality" {
                    base["tls"]["reality"] = json!({
                        "enabled": true,
                        "public_key": opts.get("publicKey").cloned().unwrap_or_default(),
                        "short_id": opts.get("shortId").cloned().unwrap_or_default()
                    });
                }
            }

            if let Some(flow) = opts.get("flow") {
                base["flow"] = json!(flow);
            }

            // Transport
            if let Some(transport_type) = opts.get("type") {
                if transport_type != "tcp" {
                    let mut transport = json!({ "type": transport_type });
                    if let Some(path) = opts.get("path") {
                        transport["path"] = json!(path);
                    }
                    if let Some(host) = opts.get("host") {
                        transport["headers"] = json!({ "Host": host });
                    }
                    if let Some(service_name) = opts.get("serviceName") {
                        transport["service_name"] = json!(service_name);
                    }
                    base["transport"] = transport;
                }
            }
        }

        "trojan" => {
            base["type"] = json!("trojan");
            base["password"] = json!(server.password);

            let opts = parse_opts(&server.vless_opts);
            if let Some(sec) = opts.get("security") {
                if sec != "none" {
                    let sni = opts.get("sni").cloned().unwrap_or_else(|| server.server_address.clone());
                    base["tls"] = json!({
                        "enabled": true,
                        "server_name": sni
                    });
                }
            }
        }

        "socks5" => {
            base["type"] = json!("socks");
            base["version"] = json!("5");
            if !server.username.is_empty() {
                base["username"] = json!(server.username);
            }
            if !server.password.is_empty() {
                base["password"] = json!(server.password);
            }
        }

        "http" => {
            base["type"] = json!("http");
            if !server.username.is_empty() {
                base["username"] = json!(server.username);
            }
            if !server.password.is_empty() {
                base["password"] = json!(server.password);
            }
            base["tcp_fast_open"] = json!(true);
        }

        // Default: shadowsocks
        _ => {
            base["type"] = json!("shadowsocks");
            base["method"] = json!(server.encryption_method);
            base["password"] = json!(server.password);

            // Plugin support
            if !server.plugin.is_empty() {
                base["plugin"] = json!(server.plugin);
                if !server.plugin_opts.is_empty() {
                    base["plugin_opts"] = json!(server.plugin_opts);
                }
            }
        }
    }

    Ok(base)
}

/// Parse vless_opts JSON string into a key-value map
fn parse_opts(raw: &str) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    if raw.is_empty() {
        return map;
    }
    if let Ok(obj) = serde_json::from_str::<Value>(raw) {
        if let Some(obj_map) = obj.as_object() {
            for (k, v) in obj_map {
                map.insert(k.clone(), match v {
                    Value::String(s) => s.clone(),
                    Value::Number(n) => n.to_string(),
                    Value::Bool(b) => b.to_string(),
                    _ => v.to_string(),
                });
            }
        }
    }
    map
}
