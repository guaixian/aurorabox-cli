use serde_json::Value;

/// Port of the desktop version's patchRuleSetCDN.
///
/// Strategy:
/// 1. Empty the route.rule_set array (sing-box won't download anything)
/// 2. Remove all route rules that reference remote rule_sets
/// 3. Add inline fallback rules for private IPs and DNS
///
/// This matches exactly what the AuroraBox desktop does.
pub fn patch_rule_set_cdn(config: &mut Value) {
    let route = match config.get_mut("route") {
        Some(r) => r,
        None => return,
    };

    // 1. Empty rule_sets — no downloads, no errors
    if let Some(obj) = route.as_object_mut() {
        obj.insert("rule_set".to_string(), Value::Array(vec![]));
    }

    // 2. Remove rules that reference remote rule_sets
    if let Some(rules) = route.get_mut("rules") {
        if let Some(arr) = rules.as_array_mut() {
            arr.retain(|rule| {
                // Keep rules that don't reference rule_sets
                !rule.as_object().map_or(false, |o| o.contains_key("rule_set"))
            });
        }
    }

    // 3. Add inline fallback rules (same as desktop)
    let inline_rules: Vec<Value> = vec![
        // DNS hijack prevention
        serde_json::json!({"protocol": "dns", "outbound": "dns-out"}),
        // Private IPs → direct
        serde_json::json!({
            "ip_is_private": true,
            "outbound": "direct"
        }),
        // Common private CIDRs → direct
        serde_json::json!({
            "ip_cidr": [
                "10.0.0.0/8", "172.16.0.0/12", "192.168.0.0/16",
                "127.0.0.0/8", "224.0.0.0/4", "240.0.0.0/4",
                "100.64.0.0/10", "198.18.0.0/15", "169.254.0.0/16",
                "fe80::/10", "fc00::/7", "ff00::/8"
            ],
            "outbound": "direct"
        }),
    ];

    if let Some(rules) = route.get_mut("rules") {
        if let Some(arr) = rules.as_array_mut() {
            // Prepend inline rules at the beginning (they take priority)
            let mut new_rules = inline_rules;
            new_rules.append(arr);
            *arr = new_rules;
        }
    }
}

/// Configure the mixed inbound (HTTP/SOCKS proxy port)
pub fn configure_mixed_inbound(
    config: &mut Value,
    port: u16,
    allow_lan: bool,
    _bypass_router: bool,
) {
    let listen = if allow_lan { "0.0.0.0" } else { "127.0.0.1" };

    if let Some(inbounds) = config.get_mut("inbounds") {
        if let Some(arr) = inbounds.as_array_mut() {
            for inbound in arr.iter_mut() {
                if inbound.get("type").and_then(|v| v.as_str()) == Some("mixed") {
                    inbound["listen"] = Value::String(listen.to_string());
                    inbound["listen_port"] = Value::Number(port.into());
                    if !inbound.as_object().map_or(false, |o| o.contains_key("tag")) {
                        inbound["tag"] = Value::String("mixed-in".to_string());
                    }
                }
            }
        }
    }
}

/// Configure the TUN inbound (virtual network adapter)
pub fn configure_tun_inbound(
    config: &mut Value,
    stack: &str,
    interface_name: &str,
    bypass_router: bool,
) {
    if let Some(inbounds) = config.get_mut("inbounds") {
        if let Some(arr) = inbounds.as_array_mut() {
            for inbound in arr.iter_mut() {
                let is_tun = inbound.get("type").and_then(|v| v.as_str()) == Some("tun");
                if !is_tun { continue; }
                inbound["stack"] = Value::String(stack.to_string());
                inbound["interface_name"] = Value::String(interface_name.to_string());
                if bypass_router {
                    if let Some(obj) = inbound.as_object_mut() {
                        let exclude = serde_json::json!([
                            "10.0.0.0/8", "172.16.0.0/12", "192.168.0.0/16",
                            "fc00::/7", "fe80::/10"
                        ]);
                        if let Some(inet4) = obj.get_mut("inet4_address") {
                            inet4["route_exclude_address"] = exclude;
                        }
                    }
                }
                if !inbound.as_object().map_or(false, |o| o.contains_key("tag")) {
                    inbound["tag"] = Value::String("tun-in".to_string());
                }
            }
        }
    }
}

/// Update DHCP/DNS settings
pub fn update_dhcp_settings(config: &mut Value, _use_dhcp: bool, direct_dns_tag: &str) {
    if let Some(dns) = config.get_mut("dns") {
        if let Some(servers) = dns.get_mut("servers") {
            if let Some(arr) = servers.as_array_mut() {
                for server in arr.iter_mut() {
                    if server.get("tag").and_then(|v| v.as_str()) == Some("system") {
                        server["tag"] = Value::String(direct_dns_tag.to_string());
                    }
                }
            }
        }
    }
}

/// Update experimental config (clash API, cache file)
pub fn update_experimental_config(config: &mut Value, secret: &str, cache_path: &str) {
    if let Some(obj) = config.as_object_mut() {
        if !obj.contains_key("experimental") {
            obj.insert("experimental".to_string(), serde_json::json!({}));
        }
        if let Some(exp) = obj.get_mut("experimental") {
            if let Some(eobj) = exp.as_object_mut() {
                eobj.insert("clash_api".to_string(), serde_json::json!({
                    "external_controller": "127.0.0.1:9191",
                    "external_ui": "",
                    "secret": secret,
                    "default_mode": "rule"
                }));
                eobj.insert("cache_file".to_string(), serde_json::json!({
                    "enabled": true,
                    "path": cache_path,
                    "cache_id": "aurorabox-cli",
                    "store_fakeip": false
                }));
            }
        }
    }
}
