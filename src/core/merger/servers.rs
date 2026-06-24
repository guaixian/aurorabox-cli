use serde_json::Value;

use crate::core::outbound;
use crate::db::models::ProxyServer;

/// Merge manual proxy servers into the config as sing-box outbounds.
/// This creates outbound entries for each server and wires them into
/// the ExitGateway selector and urltest group.
pub fn merge_manual_servers(config: &mut Value, servers: &[ProxyServer]) -> anyhow::Result<()> {
    if servers.is_empty() {
        return Ok(());
    }

    let outbounds = get_or_create_outbounds_array(config);
    let existing_len = outbounds.len();

    // Find the ExitGateway selector and auto urltest to add servers to
    let exit_gateway_tags = find_or_create_exit_tags(outbounds);

    // Build outbounds for each server
    for server in servers {
        let mut ob = outbound::build_outbound(server)?;

        // Use the tag from build_outbound (e.g., "ss-abc12345", "socks5-def67890")
        let tag = ob
            .get("tag")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("manual-{}", server.identifier));

        outbounds.push(ob);

        // Add to ExitGateway selector
        if let Some(gateway_idx) = exit_gateway_tags.0 {
            if let Some(gateway) = outbounds.get_mut(gateway_idx) {
                if let Some(out_tags) = gateway.get_mut("outbounds") {
                    if let Some(arr) = out_tags.as_array_mut() {
                        arr.push(Value::String(tag.clone()));
                    }
                }
            }
        }

        // Add to urltest group
        if let Some(urltest_idx) = exit_gateway_tags.1 {
            if let Some(ut) = outbounds.get_mut(urltest_idx) {
                if let Some(out_tags) = ut.get_mut("outbounds") {
                    if let Some(arr) = out_tags.as_array_mut() {
                        arr.push(Value::String(tag.clone()));
                    }
                }
            }
        }
    }

    // If this is the first time adding manual servers, add them as the default
    // selection in ExitGateway
    if existing_len == 0 {
        // Default behavior: first active server is the gateway default
    }

    Ok(())
}

/// Get or create the outbounds array in config
fn get_or_create_outbounds_array(config: &mut Value) -> &mut Vec<Value> {
    if !config.as_object().map_or(false, |o| o.contains_key("outbounds")) {
        if let Some(obj) = config.as_object_mut() {
            obj.insert("outbounds".to_string(), Value::Array(Vec::new()));
        }
    }
    config["outbounds"].as_array_mut().unwrap()
}

/// Find or create the ExitGateway selector and auto urltest tags
/// Returns (gateway_index, urltest_index)
fn find_or_create_exit_tags(outbounds: &mut Vec<Value>) -> (Option<usize>, Option<usize>) {
    let mut gateway_idx = None;
    let mut urltest_idx = None;

    for (i, ob) in outbounds.iter().enumerate() {
        if let Some(tag) = ob.get("tag").and_then(|v| v.as_str()) {
            if tag == "ExitGateway" {
                gateway_idx = Some(i);
            }
            if tag == "auto" {
                urltest_idx = Some(i);
            }
        }
    }

    // Create ExitGateway if not found
    if gateway_idx.is_none() {
        let gateway = serde_json::json!({
            "tag": "ExitGateway",
            "type": "selector",
            "outbounds": ["auto"]
        });
        outbounds.push(gateway);
        gateway_idx = Some(outbounds.len() - 1);
    }

    // Create auto urltest if not found
    if urltest_idx.is_none() {
        let auto = serde_json::json!({
            "tag": "auto",
            "type": "urltest",
            "outbounds": [],
            "url": "https://www.gstatic.com/generate_204",
            "interval": "10m0s",
            "tolerance": 50
        });
        outbounds.push(auto);
        urltest_idx = Some(outbounds.len() - 1);
    }

    (gateway_idx, urltest_idx)
}
