pub mod merger;
pub mod outbound;
pub mod template;

use serde_json::Value;

/// Get the config directory (~/.config/aurorabox)
pub fn config_dir() -> String {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    format!("{}/.config/aurorabox", home)
}

/// Generate the sing-box config.json as a JSON Value
pub fn generate_config(
    mode: &str,
    tun: bool,
    subscription_id: Option<&str>,
) -> anyhow::Result<Value> {
    let config_type = template::ConfigType::from_mode(mode, tun);
    let mut config = config_type.load_template()?;

    // If a subscription is specified, try to load its config_content from DB
    if let Some(sub_id) = subscription_id {
        let db_path = crate::db::db_path();
        let conn = crate::db::open(&db_path)?;
        if let Some(sub) = crate::db::queries::get_subscription_by_identifier(&conn, sub_id)? {
            if let Some(config_record) =
                crate::db::queries::get_subscription_config(&conn, &sub.identifier)?
            {
                if let Some(content) = config_record.config_content {
                    if let Ok(sub_config) = serde_json::from_str::<Value>(&content) {
                        merger::subscription::merge_subscription_outbounds(
                            &mut config,
                            &sub_config,
                        )?;
                    }
                }
            }
        }
    }

    // Merge manual proxy servers
    let db_path = crate::db::db_path();
    if let Ok(conn) = crate::db::open(&db_path) {
        if let Ok(servers) = crate::db::queries::get_all_proxy_servers(&conn) {
            if !servers.is_empty() {
                merger::servers::merge_manual_servers(&mut config, &servers)?;
            }
        }
        if let Ok(groups) = crate::db::queries::get_all_proxy_groups(&conn) {
            if !groups.is_empty() {
                let servers = crate::db::queries::get_all_proxy_servers(&conn).unwrap_or_default();
                merger::groups::merge_proxy_groups(&mut config, &groups, &servers)?;
            }
        }
    }

    // Ensure auto/ExitGateway have valid outbounds (not empty → sing-box crash)
    ensure_valid_outbounds(&mut config);

    // Apply standard patches
    merger::helper::patch_rule_set_cdn(&mut config);
    merger::helper::configure_mixed_inbound(&mut config, 6789, false, false);
    merger::helper::update_dhcp_settings(&mut config, false, "system");
    merger::helper::update_experimental_config(
        &mut config,
        "aurorabox-cli",
        &format!("{}/cache.db", config_dir()),
    );

    if tun {
        merger::helper::configure_tun_inbound(&mut config, "gvisor", "utun233", false);
    }

    // Set log level
    if let Some(log) = config.get_mut("log") {
        if let Some(obj) = log.as_object_mut() {
            obj.insert("level".to_string(), Value::String("info".to_string()));
            obj.insert("timestamp".to_string(), Value::Bool(true));
        }
    }

    Ok(config)
}

/// Ensure urltest/selector groups have at least one valid outbound.
/// An empty outbounds list causes sing-box to crash with "dependency not found".
fn ensure_valid_outbounds(config: &mut Value) {
    let outbounds = if let Some(arr) = config.get_mut("outbounds") {
        arr.as_array_mut().unwrap()
    } else {
        return;
    };

    // Collect all outbound tags
    let all_tags: Vec<String> = outbounds
        .iter()
        .filter_map(|ob| ob.get("tag").and_then(|v| v.as_str()).map(|s| s.to_string()))
        .collect();

    // First pass: collect types and tags (immutable borrows)
    let mut fixes: Vec<(usize, String)> = Vec::new();
    for (i, ob) in outbounds.iter().enumerate() {
        let otype = ob.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if otype == "selector" || otype == "urltest" {
            let tag = ob.get("tag").and_then(|v| v.as_str()).unwrap_or("?").to_string();
            fixes.push((i, tag));
        }
    }

    // Second pass: apply fixes (mutable borrows)
    for (idx, tag) in fixes {
        let ob = &mut outbounds[idx];
        if let Some(out_tags) = ob.get_mut("outbounds") {
            if let Some(arr) = out_tags.as_array_mut() {
                // Remove references to non-existent outbounds
                arr.retain(|v| {
                    if let Some(ref_str) = v.as_str() {
                        all_tags.iter().any(|t| t == ref_str)
                    } else {
                        true
                    }
                });

                // If still empty, add "direct" as fallback
                if arr.is_empty() {
                    arr.push(Value::String("direct".to_string()));
                    let otype = ob.get("type").and_then(|v| v.as_str()).unwrap_or("?");
                    log::warn!("Outbound '{}' ({}) had no valid outbounds, added 'direct' fallback", tag, otype);
                }
            }
        }
    }
}

/// Generate config and write it to the default config path
/// Returns the path to the written config file
pub fn generate_and_write_config(
    mode: &str,
    tun: bool,
    subscription_id: Option<&str>,
    config_dir_override: Option<&str>,
) -> anyhow::Result<String> {
    let dir = config_dir_override
        .map(|s| s.to_string())
        .unwrap_or_else(config_dir);
    std::fs::create_dir_all(&dir)?;

    let config = generate_config(mode, tun, subscription_id)?;
    let config_path = format!("{}/config.json", dir);
    let json = serde_json::to_string_pretty(&config)?;
    std::fs::write(&config_path, json)?;

    Ok(config_path)
}
