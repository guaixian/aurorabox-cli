use serde_json::Value;

/// Merge subscription outbounds into the config.
/// The subscription config typically contains outbounds that should be
/// wired into the ExitGateway selector.
pub fn merge_subscription_outbounds(
    config: &mut Value,
    subscription_config: &Value,
) -> anyhow::Result<()> {
    // Extract outbounds from subscription config
    let sub_outbounds = subscription_config
        .get("outbounds")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    if sub_outbounds.is_empty() {
        log::warn!("Subscription config has no outbounds");
        return Ok(());
    }

    // Get or create the outbounds array in the config
    let outbounds = if let Some(arr) = config.get_mut("outbounds").and_then(|v| v.as_array_mut()) {
        arr
    } else {
        if let Some(obj) = config.as_object_mut() {
            obj.insert("outbounds".to_string(), Value::Array(Vec::new()));
        }
        config["outbounds"].as_array_mut().unwrap()
    };

    // Collect the tags of subscription outbounds
    let mut sub_tags: Vec<String> = Vec::new();

    for outbound in &sub_outbounds {
        if let Some(tag) = outbound.get("tag").and_then(|v| v.as_str()) {
            sub_tags.push(tag.to_string());
        }
        outbounds.push(outbound.clone());
    }

    // Wire subscription outbounds into ExitGateway selector
    if !sub_tags.is_empty() {
        if let Some(gateway) = outbounds
            .iter_mut()
            .find(|ob| ob.get("tag").and_then(|v| v.as_str()) == Some("ExitGateway"))
        {
            if let Some(gateway_outbounds) = gateway.get_mut("outbounds") {
                if let Some(arr) = gateway_outbounds.as_array_mut() {
                    for tag in &sub_tags {
                        arr.push(Value::String(tag.clone()));
                    }
                }
            }
        }

        // Also wire into the auto urltest group
        if let Some(auto) = outbounds
            .iter_mut()
            .find(|ob| ob.get("tag").and_then(|v| v.as_str()) == Some("auto"))
        {
            if let Some(auto_outbounds) = auto.get_mut("outbounds") {
                if let Some(arr) = auto_outbounds.as_array_mut() {
                    for tag in &sub_tags {
                        arr.push(Value::String(tag.clone()));
                    }
                }
            }
        }
    }

    Ok(())
}
