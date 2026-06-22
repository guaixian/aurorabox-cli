use serde_json::Value;

/// The four sing-box config template types
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ConfigType {
    /// Mixed inbound + rule-based routing
    Mixed,
    /// TUN inbound + rule-based routing
    Tun,
    /// Mixed inbound + global routing (no split)
    MixedGlobal,
    /// TUN inbound + global routing (no split)
    TunGlobal,
}

impl ConfigType {
    /// Map a mode string and tun flag to the appropriate template type
    pub fn from_mode(mode: &str, tun: bool) -> Self {
        match (mode, tun) {
            ("rule", false) | ("rules", false) => ConfigType::Mixed,
            ("rule", true) | ("rules", true) => ConfigType::Tun,
            ("global", false) => ConfigType::MixedGlobal,
            ("global", true) => ConfigType::TunGlobal,
            ("tun", _) | ("nic", _) => {
                // TUN/NIC mode defaults to rule-based TUN
                ConfigType::Tun
            }
            _ => ConfigType::Mixed,
        }
    }

    /// Load the embedded template JSON
    pub fn load_template(&self) -> anyhow::Result<Value> {
        let json_str = self.template_str();
        let value: Value = serde_json::from_str(json_str)?;
        Ok(value)
    }

    /// Get the raw JSON string for this template
    fn template_str(&self) -> &'static str {
        match self {
            ConfigType::Mixed => include_str!("templates/mixed.json"),
            ConfigType::Tun => include_str!("templates/tun.json"),
            ConfigType::MixedGlobal => include_str!("templates/mixed-global.json"),
            ConfigType::TunGlobal => include_str!("templates/tun-global.json"),
        }
    }
}
