use serde_json::Value;

use crate::db::models::{ProxyGroup, ProxyServer};

/// Merge proxy groups into the config.
/// Each group becomes a selector/urltest/chain outbound depending on group_type.
pub fn merge_proxy_groups(
    config: &mut Value,
    groups: &[ProxyGroup],
    _servers: &[ProxyServer],
) -> anyhow::Result<()> {
    if groups.is_empty() {
        return Ok(());
    }

    let outbounds = get_or_create_outbounds_array(config);

    for group in groups {
        let tag = format!("gp-{}-sel", group.identifier);
        let group_tag = match group.group_type.as_str() {
            "auto" => format!("gp-{}-auto", group.identifier),
            "chain" => format!("gp-{}-chain-0", group.identifier),
            _ => tag.clone(),
        };

        let outbound_type = match group.group_type.as_str() {
            "auto" => "urltest",
            "random" => "selector",
            "chain" => "selector", // chain handled separately
            _ => "selector",       // fixed = selector
        };

        let mut group_outbound = serde_json::json!({
            "tag": group_tag,
            "type": outbound_type,
            "outbounds": []
        });

        // Add urltest-specific fields
        if group.group_type == "auto" {
            group_outbound["url"] = Value::String("https://www.gstatic.com/generate_204".to_string());
            group_outbound["interval"] = Value::String("10m0s".to_string());
            group_outbound["tolerance"] = Value::Number(50.into());
        }

        // Add to ExitGateway selector
        if let Some(gateway) = find_exit_gateway_mut(outbounds) {
            if let Some(out_tags) = gateway.get_mut("outbounds") {
                if let Some(arr) = out_tags.as_array_mut() {
                    arr.push(Value::String(group_tag.clone()));
                }
            }
        }

        outbounds.push(group_outbound);
    }

    Ok(())
}

fn get_or_create_outbounds_array(config: &mut Value) -> &mut Vec<Value> {
    if !config.as_object().map_or(false, |o| o.contains_key("outbounds")) {
        if let Some(obj) = config.as_object_mut() {
            obj.insert("outbounds".to_string(), Value::Array(Vec::new()));
        }
    }
    config["outbounds"].as_array_mut().unwrap()
}

fn find_exit_gateway_mut(outbounds: &mut Vec<Value>) -> Option<&mut Value> {
    outbounds
        .iter_mut()
        .find(|ob| ob.get("tag").and_then(|v| v.as_str()) == Some("ExitGateway"))
}
