use serde_json::Value;

/// Patch rule_set entries: strip remote rule_set references (converted to inline
/// rules during config generation). The template uses rule_set references for
/// CDN delivery; this removes them since the CLI doesn't use CDN rule sets.
pub fn patch_rule_set_cdn(config: &mut Value) {
    // Remove top-level "rule_set" key if it exists
    if let Some(obj) = config.as_object_mut() {
        obj.remove("rule_set");
    }

    // In the route section, remove any "rule_set" entries from rules
    if let Some(route) = config.get_mut("route") {
        if let Some(rules) = route.get_mut("rules") {
            if let Some(arr) = rules.as_array_mut() {
                arr.retain(|rule| {
                    // Keep rules that don't reference remote rule_sets
                    if let Some(rule_set) = rule.get("rule_set") {
                        // Check if it's an inline rule_set (has "rules" array)
                        // or just a tag reference (remote)
                        if let Some(rules_arr) = rule_set.get("rules") {
                            rules_arr.is_array() && !rules_arr.as_array().unwrap().is_empty()
                        } else {
                            // Remote reference - remove
                            false
                        }
                    } else {
                        true
                    }
                });
            }
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
                if let Some(typ) = inbound.get("type").and_then(|v| v.as_str()) {
                    if typ == "mixed" {
                        inbound["listen"] = Value::String(listen.to_string());
                        inbound["listen_port"] = Value::Number(port.into());
                        // Set a default tag if not present
                        if !inbound.as_object().map_or(false, |o| o.contains_key("tag")) {
                            inbound["tag"] = Value::String("mixed-in".to_string());
                        }
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
                let is_tun = inbound
                    .get("type")
                    .and_then(|v| v.as_str())
                    .map(|t| t == "tun")
                    .unwrap_or(false);

                if !is_tun {
                    continue;
                }

                inbound["stack"] = Value::String(stack.to_string());
                inbound["interface_name"] = Value::String(interface_name.to_string());

                // Bypass router: add route_exclude_address for LAN
                if bypass_router {
                    if let Some(obj) = inbound.as_object_mut() {
                        let exclude = serde_json::json!([
                            "10.0.0.0/8",
                            "172.16.0.0/12",
                            "192.168.0.0/16",
                            "fc00::/7",
                            "fe80::/10"
                        ]);
                        if let Some(inet4) = obj.get_mut("inet4_address") {
                            inet4["route_exclude_address"] = exclude;
                        }
                    }
                }

                // Set a default tag if not present
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
                    if let Some(tag) = server.get("tag").and_then(|v| v.as_str()) {
                        if tag == "system" {
                            server["tag"] = Value::String(direct_dns_tag.to_string());
                        }
                    }
                }
            }
        }
    }
}

/// Update experimental config (clash API, cache file)
pub fn update_experimental_config(config: &mut Value, secret: &str, cache_path: &str) {
    let experimental = config
        .as_object_mut()
        .and_then(|obj| {
            if !obj.contains_key("experimental") {
                obj.insert(
                    "experimental".to_string(),
                    serde_json::json!({}),
                );
            }
            obj.get_mut("experimental")
        });

    if let Some(exp) = experimental {
        if let Some(obj) = exp.as_object_mut() {
            let clash_api = serde_json::json!({
                "external_controller": "127.0.0.1:9191",
                "external_ui": "",
                "secret": secret,
                "default_mode": "rule"
            });
            obj.insert("clash_api".to_string(), clash_api);
            let cache = serde_json::json!({
                "enabled": true,
                "path": cache_path,
                "cache_id": "aurorabox-cli",
                "store_fakeip": false
            });
            obj.insert("cache_file".to_string(), cache);
        }
    }
}
