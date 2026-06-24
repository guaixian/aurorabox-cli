use serde_json::Value;

/// Patch rule_set entries: convert remote CDN downloads to local file:// URLs
/// for rule-sets we have bundled, and remove references to rule-sets we don't have.
pub fn patch_rule_set_cdn(config: &mut Value) {
    let local_rulesets = crate::utils::rule_sets::extract_rule_sets();

    if let Some(route) = config.get_mut("route") {
        // Convert route.rule_set: remote URLs → local file:// URLs
        if let Some(rule_sets) = route.get_mut("rule_set") {
            if let Some(arr) = rule_sets.as_array_mut() {
                // First pass: convert known rule_sets to local, mark unknown for removal
                let mut to_remove = Vec::new();
                for (i, rs) in arr.iter_mut().enumerate() {
                    // Clone tag to avoid borrow conflict
                    let tag = rs.get("tag").and_then(|v| v.as_str()).map(|s| s.to_string());
                    if let Some(ref tag_str) = tag {
                        if let Some(local_url) = local_rulesets.get(tag_str) {
                            if let Some(obj) = rs.as_object_mut() {
                                obj.insert("type".to_string(), Value::String("local".to_string()));
                                obj.insert("path".to_string(), Value::String(local_url.clone()));
                                obj.remove("url");
                                obj.remove("format");
                                obj.remove("download_detour");
                                log::info!("Using local rule-set: {}", tag_str);
                            }
                        } else {
                            to_remove.push(i);
                        }
                    }
                }
                // Remove entries that weren't converted (still remote)
                for i in to_remove.into_iter().rev() {
                    arr.remove(i);
                }
            }
        }

        // Collect available tags for rule filtering
        let available_tags: Vec<String> = local_rulesets.keys().cloned().collect();

        // Clean route rules: keep only those referencing available rule_sets
        if let Some(rules) = route.get_mut("rules") {
            if let Some(arr) = rules.as_array_mut() {
                let mut to_remove = Vec::new();
                for (i, rule) in arr.iter().enumerate() {
                    let keep = match rule.get("rule_set") {
                        Some(Value::String(tag)) => available_tags.iter().any(|t| t == tag),
                        Some(Value::Array(tags)) => tags.iter().any(|v| {
                            v.as_str().map(|t| available_tags.iter().any(|a| a == t)).unwrap_or(false)
                        }),
                        Some(_) => true, // inline rule_set with "rules" key — keep
                        None => true,    // non-rule_set rule — keep
                    };
                    if !keep {
                        to_remove.push(i);
                    }
                }
                for i in to_remove.into_iter().rev() {
                    arr.remove(i);
                }
            }
        }
    }

    // Clean DNS rules referencing unavailable rule_sets
    if let Some(dns) = config.get_mut("dns") {
        if let Some(rules) = dns.get_mut("rules") {
            if let Some(arr) = rules.as_array_mut() {
                let available: Vec<String> = local_rulesets.keys().cloned().collect();
                let mut to_remove = Vec::new();
                for (i, rule) in arr.iter().enumerate() {
                    if let Some(rs) = rule.get("rule_set") {
                        let has_any = rs.as_array().map_or(false, |tags| {
                            tags.iter().any(|v| {
                                v.as_str().map(|t| available.iter().any(|a| a == t)).unwrap_or(false)
                            })
                        });
                        if !has_any {
                            to_remove.push(i);
                        }
                    }
                }
                for i in to_remove.into_iter().rev() {
                    arr.remove(i);
                }
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
                obj.insert("experimental".to_string(), serde_json::json!({}));
            }
            obj.get_mut("experimental")
        });

    if let Some(exp) = experimental {
        if let Some(obj) = exp.as_object_mut() {
            obj.insert("clash_api".to_string(), serde_json::json!({
                "external_controller": "127.0.0.1:9191",
                "external_ui": "",
                "secret": secret,
                "default_mode": "rule"
            }));
            obj.insert("cache_file".to_string(), serde_json::json!({
                "enabled": true,
                "path": cache_path,
                "cache_id": "aurorabox-cli",
                "store_fakeip": false
            }));
        }
    }
}
