use clap::{Parser, Subcommand, ValueEnum};

/// AuroraBox CLI - sing-box proxy manager
#[derive(Parser, Debug)]
#[command(
    name = "aurorabox",
    version,
    about = "AuroraBox CLI - A sing-box proxy manager with optional web UI",
    long_about = None
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Start the web UI and proxy engine
    ///
    /// Launches an HTTP server with REST API and embedded frontend UI.
    /// The proxy engine runs in the same process.
    Web {
        /// HTTP listen port
        #[arg(long, default_value = "8080")]
        port: u16,

        /// Host address to bind to
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
    },

    /// Start the sing-box proxy engine
    ///
    /// Generates config and launches sing-box. The CLI will block and
    /// monitor the sing-box process until it exits or receives SIGTERM.
    Start {
        /// Proxy mode: rule, global, or tun
        #[arg(long, default_value = "rule")]
        mode: ProxyModeArg,

        /// Subscription identifier to use for outbounds
        #[arg(long)]
        subscription: Option<String>,

        /// Proxy server or group identifier to activate before starting.
        /// Can be specified multiple times. If a group is active, its
        /// members are used. If a server is active, it becomes the
        /// default outbound.
        #[arg(long = "proxy", short = 'p')]
        proxy_ids: Vec<String>,

        /// Config directory (default: ~/.config/aurorabox)
        #[arg(long)]
        config_dir: Option<String>,

        /// Also start the web server
        #[arg(long)]
        web_server: bool,

        /// Web server port (only with --web-server)
        #[arg(long, default_value = "8080")]
        port: u16,
    },

    /// Stop the running sing-box process
    Stop,

    /// Reload sing-box configuration (sends SIGHUP)
    Reload,

    /// Show engine status
    Status,

    /// Add a subscription or proxy server
    Add {
        #[command(subcommand)]
        source: AddSource,
    },

    /// Import proxy servers from share links
    ///
    /// Accepts ss://, socks5://, http://, vless://, trojan://, hysteria2:// links.
    /// Supports bulk import from stdin, file, or direct link argument.
    Import {
        /// Share link(s) to import. If omitted, reads from stdin.
        /// Can be a single link or multi-line text with multiple links.
        #[arg()]
        links: Vec<String>,

        /// Read links from a file
        #[arg(short, long)]
        file: Option<String>,
    },

    /// List subscriptions, proxies, or groups
    List {
        #[command(subcommand)]
        target: ListTarget,
    },

    /// Remove a subscription, proxy, or group
    Remove {
        #[command(subcommand)]
        target: RemoveTarget,
    },

    /// Generate sing-box config to stdout (dry-run, does not start engine)
    Generate {
        /// Proxy mode: rule, global, or tun
        #[arg(long, default_value = "rule")]
        mode: ProxyModeArg,

        /// Subscription identifier to use for outbounds
        #[arg(long)]
        subscription: Option<String>,

        /// Output file path (default: stdout)
        #[arg(short, long)]
        output: Option<String>,
    },

    /// Download and install sing-box binary
    Install {
        /// Version to download (default: latest 1.13.x)
        #[arg(long)]
        version: Option<String>,

        /// Target directory (default: ~/.local/share/aurorabox/bin/)
        #[arg(long)]
        dir: Option<String>,
    },
}

#[derive(ValueEnum, Clone, Debug)]
pub enum ProxyModeArg {
    /// Rule-based routing (split traffic by rules)
    Rule,
    /// Global proxy (all traffic through proxy)
    Global,
    /// TUN mode (virtual network adapter, requires admin/root)
    Tun,
    /// Alias for tun mode
    Nic,
}

#[derive(Subcommand, Debug)]
pub enum AddSource {
    /// Add a subscription URL
    #[command(name = "sub")]
    Subscription {
        /// Subscription URL
        url: String,

        /// Display name for this subscription
        #[arg(short, long)]
        name: Option<String>,
    },

    /// Add a single proxy server
    #[command(name = "proxy")]
    Proxy {
        #[command(subcommand)]
        proto: ProxyProtocol,
    },
}

#[derive(Subcommand, Debug)]
pub enum ProxyProtocol {
    /// Shadowsocks proxy
    Ss {
        /// Display name
        #[arg(long)]
        name: String,
        /// Server address
        #[arg(long)]
        server: String,
        /// Server port
        #[arg(long)]
        port: u16,
        /// Password
        #[arg(long)]
        password: String,
        /// Encryption method (default: aes-256-gcm)
        #[arg(long, default_value = "aes-256-gcm")]
        method: String,
        /// Plugin name (e.g., obfs, v2ray-plugin)
        #[arg(long)]
        plugin: Option<String>,
        /// Plugin options
        #[arg(long)]
        plugin_opts: Option<String>,
    },

    /// SOCKS5 proxy
    #[command(name = "socks5")]
    Socks5 {
        /// Display name
        #[arg(long)]
        name: String,
        /// Server address
        #[arg(long)]
        server: String,
        /// Server port
        #[arg(long)]
        port: u16,
        /// Username (optional)
        #[arg(long)]
        username: Option<String>,
        /// Password (optional)
        #[arg(long)]
        password: Option<String>,
    },

    /// HTTP proxy
    #[command(name = "http")]
    Http {
        /// Display name
        #[arg(long)]
        name: String,
        /// Server address
        #[arg(long)]
        server: String,
        /// Server port
        #[arg(long)]
        port: u16,
        /// Username (optional)
        #[arg(long)]
        username: Option<String>,
        /// Password (optional)
        #[arg(long)]
        password: Option<String>,
    },

    /// Trojan proxy
    #[command(name = "trojan")]
    Trojan {
        /// Display name
        #[arg(long)]
        name: String,
        /// Server address
        #[arg(long)]
        server: String,
        /// Server port
        #[arg(long)]
        port: u16,
        /// Password
        #[arg(long)]
        password: String,
        /// Advanced options as JSON (sni, alpn, fingerprint, etc.)
        #[arg(long)]
        opts: Option<String>,
    },

    /// VLESS proxy
    #[command(name = "vless")]
    Vless {
        /// Display name
        #[arg(long)]
        name: String,
        /// Server address
        #[arg(long)]
        server: String,
        /// Server port
        #[arg(long)]
        port: u16,
        /// VLESS UUID
        #[arg(long)]
        uuid: String,
        /// Advanced options as JSON (flow, security, transport, reality, etc.)
        #[arg(long)]
        opts: Option<String>,
    },

    /// Hysteria2 proxy
    #[command(name = "hysteria2")]
    Hysteria2 {
        /// Display name
        #[arg(long)]
        name: String,
        /// Server address
        #[arg(long)]
        server: String,
        /// Server port
        #[arg(long)]
        port: u16,
        /// Password
        #[arg(long)]
        password: String,
        /// Advanced options as JSON (sni, obfs, upmbps, downmbps, etc.)
        #[arg(long)]
        opts: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
pub enum ListTarget {
    /// List all subscriptions
    #[command(name = "subscriptions")]
    Subscriptions,

    /// List all proxy servers
    #[command(name = "proxies")]
    Proxies,

    /// List all proxy groups
    #[command(name = "groups")]
    Groups,
}

#[derive(Subcommand, Debug)]
pub enum RemoveTarget {
    /// Remove a subscription by ID
    #[command(name = "subscription")]
    Subscription {
        /// Subscription ID
        id: i64,
    },

    /// Remove a proxy server by ID
    #[command(name = "proxy")]
    Proxy {
        /// Proxy server ID
        id: i64,
    },

    /// Remove a proxy group by ID
    #[command(name = "group")]
    Group {
        /// Proxy group ID
        id: i64,
    },
}
