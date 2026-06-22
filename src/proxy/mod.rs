pub mod manager;
pub mod monitor;
pub mod process;
pub mod readiness;

use crate::cli::ProxyModeArg;

/// Proxy mode - simplified from the original AuroraBox engine modes.
/// Maps to how sing-box is launched and whether system proxy is set.
#[derive(Clone, Debug, PartialEq)]
pub enum ProxyMode {
    /// sing-box runs as a sidecar, system proxy is NOT set
    Manual,
    /// sing-box runs as a sidecar, system proxy IS set
    System,
    /// TUN mode: sing-box creates a virtual network adapter
    Tun,
}

impl From<ProxyModeArg> for ProxyMode {
    fn from(arg: ProxyModeArg) -> Self {
        match arg {
            ProxyModeArg::Rule => ProxyMode::System,
            ProxyModeArg::Global => ProxyMode::System,
            ProxyModeArg::Tun => ProxyMode::Tun,
            ProxyModeArg::Nic => ProxyMode::Tun,
        }
    }
}

impl ProxyModeArg {
    /// Convert to a mode string for config generation
    pub fn to_mode_str(&self) -> &str {
        match self {
            ProxyModeArg::Rule => "rule",
            ProxyModeArg::Global => "global",
            ProxyModeArg::Tun => "tun",
            ProxyModeArg::Nic => "tun",
        }
    }

    /// Whether this mode uses TUN
    pub fn is_tun(&self) -> bool {
        matches!(self, ProxyModeArg::Tun | ProxyModeArg::Nic)
    }
}
