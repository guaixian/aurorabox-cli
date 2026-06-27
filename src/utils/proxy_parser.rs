//! Proxy share-link parser. Supports all 6 protocols:
//! - ss://     (SIP002, legacy base64, plain-text, JSON userinfo)
//! - socks5://
//! - http://
//! - vless://
//! - trojan://
//! - hysteria2:// / hy2://

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

/// Parsed proxy server from a share link
#[derive(Debug, Clone)]
pub struct ParsedProxy {
    pub name: String,
    pub server: String,
    pub port: u16,
    pub password: String,
    pub method: String,
    pub plugin: String,
    pub plugin_opts: String,
    pub proxy_type: String,
    pub username: String,
    pub vless_uuid: String,
    pub vless_opts: String,
}

// ============================================================
// Supported methods
// ============================================================

const SUPPORTED_METHODS: &[&str] = &[
    "aes-256-gcm",
    "aes-128-gcm",
    "chacha20-ietf-poly1305",
    "xchacha20-ietf-poly1305",
    "2022-blake3-aes-128-gcm",
    "2022-blake3-aes-256-gcm",
    "2022-blake3-chacha20-poly1305",
    "none",
];

fn is_supported_method(m: &str) -> bool {
    SUPPORTED_METHODS.contains(&m)
}

// ============================================================
// Base64 helpers
// ============================================================

fn try_base64_decode(s: &str) -> Option<String> {
    if s.is_empty() {
        return None;
    }
    let normalized = s.replace('-', "+").replace('_', "/");
    let padded = match normalized.len() % 4 {
        0 => normalized,
        n => normalized + &"=".repeat(4 - n),
    };
    match BASE64.decode(&padded) {
        Ok(bytes) => String::from_utf8(bytes).ok(),
        Err(_) => None,
    }
}

// ============================================================
// Host parsing: server:port or [ipv6]:port
// ============================================================

fn parse_host_port(addr: &str) -> Option<(String, u16)> {
    if addr.is_empty() {
        return None;
    }

    let (server, port_str) = if addr.starts_with('[') {
        // IPv6: [::1]:443
        let end = addr.find(']')?;
        let srv = &addr[1..end];
        let rest = &addr[end + 1..];
        let port = if rest.starts_with(':') {
            &rest[1..]
        } else {
            return None;
        };
        (srv.to_string(), port.to_string())
    } else {
        // IPv4 or hostname: host:port
        match addr.rfind(':') {
            Some(idx) => {
                let srv = &addr[..idx];
                let port = &addr[idx + 1..];
                (srv.to_string(), port.to_string())
            }
            None => return None,
        }
    };

    let port: u16 = port_str.parse().ok()?;
    if port == 0 {
        return None;
    }
    Some((server, port))
}

// ============================================================
// Userinfo parsing for SS: method:password[:plugin[:plugin_opts]]
// ============================================================

struct SSUserinfo {
    method: String,
    password: String,
    plugin: String,
    plugin_opts: String,
}

fn parse_ss_userinfo(userinfo: &str) -> Option<SSUserinfo> {
    if userinfo.is_empty() {
        return None;
    }

    // Try JSON userinfo
    if userinfo.starts_with('{') {
        if let Ok(obj) = serde_json::from_str::<serde_json::Value>(userinfo) {
            let method = obj
                .get("method")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_lowercase();
            let password = obj
                .get("password")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let plugin = obj
                .get("plugin")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let plugin_opts = obj
                .get("plugin_opts")
                .or_else(|| obj.get("pluginOpts"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if is_supported_method(&method) {
                return Some(SSUserinfo {
                    method,
                    password,
                    plugin,
                    plugin_opts,
                });
            }
        }
        return None;
    }

    let parts: Vec<&str> = userinfo.split(':').collect();
    if parts.len() < 2 {
        return None;
    }

    let method = parts[0].to_lowercase();
    if !is_supported_method(&method) {
        return None;
    }

    let is_2022 = method.starts_with("2022-");
    let password = if is_2022 {
        // 2022 ciphers: parts[2] is uPSK, not plugin
        parts[1..].join(":")
    } else {
        parts[1].to_string()
    };
    let plugin = if is_2022 || parts.len() < 3 {
        String::new()
    } else {
        parts[2].to_string()
    };
    let plugin_opts = if is_2022 || parts.len() < 4 {
        String::new()
    } else {
        parts[3..].join(":")
    };

    Some(SSUserinfo {
        method,
        password,
        plugin,
        plugin_opts,
    })
}

// ============================================================
// ss:// parser
// ============================================================

pub fn parse_ss_link(link: &str) -> Option<ParsedProxy> {
    let trimmed = link.trim();
    let raw = trimmed.strip_prefix("ss://")?;
    if raw.is_empty() {
        return None;
    }

    let (pre_fragment, fragment) = split_fragment(raw);

    // Legacy: entire payload is base64 (no @ sign)
    if !pre_fragment.contains('@') {
        if let Some(decoded) = try_base64_decode(pre_fragment) {
            return parse_decoded_ss_payload(&decoded, fragment);
        }
        return None;
    }

    // SIP002: userinfo@host:port
    let at_idx = pre_fragment.rfind('@')?;
    let userinfo_raw = &pre_fragment[..at_idx];
    let host_raw = &pre_fragment[at_idx + 1..];

    let (host_part, name_from_host) = split_fragment(host_raw);
    let name = if name_from_host.is_empty() {
        fragment.to_string()
    } else {
        name_from_host.to_string()
    };

    let (server, port) = parse_host_port(host_part)?;
    let userinfo = try_base64_decode(userinfo_raw).unwrap_or_else(|| userinfo_raw.to_string());
    let userinfo = userinfo.strip_prefix("ss://").unwrap_or(&userinfo);
    let ui = parse_ss_userinfo(userinfo)?;

    let name = if name.is_empty() {
        format!("{}:{}", server, port)
    } else {
        decode_uri_component(&name)
    };

    Some(ParsedProxy {
        name,
        server,
        port,
        password: ui.password,
        method: ui.method,
        plugin: ui.plugin,
        plugin_opts: ui.plugin_opts,
        proxy_type: "ss".to_string(),
        username: String::new(),
        vless_uuid: String::new(),
        vless_opts: String::new(),
    })
}

fn parse_decoded_ss_payload(decoded: &str, fragment: &str) -> Option<ParsedProxy> {
    let payload = decoded.strip_prefix("ss://").unwrap_or(decoded);
    let at_idx = payload.rfind('@')?;
    let userinfo = &payload[..at_idx];
    let mut host_part = &payload[at_idx + 1..];
    let mut name = String::new();

    if let Some(hash_idx) = host_part.find('#') {
        if fragment.is_empty() {
            name = host_part[hash_idx + 1..].to_string();
        }
        host_part = &host_part[..hash_idx];
    }

    if !fragment.is_empty() && name.is_empty() {
        name = fragment.to_string();
    }

    let (server, port) = parse_host_port(host_part)?;
    let ui = parse_ss_userinfo(userinfo)?;

    let name = if name.is_empty() {
        format!("{}:{}", server, port)
    } else {
        decode_uri_component(&name)
    };

    Some(ParsedProxy {
        name,
        server,
        port,
        password: ui.password,
        method: ui.method,
        plugin: ui.plugin,
        plugin_opts: ui.plugin_opts,
        proxy_type: "ss".to_string(),
        username: String::new(),
        vless_uuid: String::new(),
        vless_opts: String::new(),
    })
}

// ============================================================
// Simple proxy parsers: socks5:// and http://
// ============================================================

pub fn parse_socks5_link(link: &str) -> Option<ParsedProxy> {
    let trimmed = link.trim();
    let raw = trimmed.strip_prefix("socks5://")?;
    parse_simple_proxy_link(raw, "socks5")
}

pub fn parse_http_link(link: &str) -> Option<ParsedProxy> {
    let trimmed = link.trim();
    let raw = trimmed.strip_prefix("http://")?;
    parse_simple_proxy_link(raw, "http")
}

fn parse_simple_proxy_link(raw: &str, proxy_type: &str) -> Option<ParsedProxy> {
    let (remaining, name) = split_fragment(raw);

    let (username, password, host_str) = if let Some(at_idx) = remaining.rfind('@') {
        let userinfo = &remaining[..at_idx];
        let host = &remaining[at_idx + 1..];
        if let Some(colon_idx) = userinfo.find(':') {
            (
                decode_uri_component(&userinfo[..colon_idx]),
                decode_uri_component(&userinfo[colon_idx + 1..]),
                host,
            )
        } else {
            (decode_uri_component(userinfo), String::new(), host)
        }
    } else {
        (String::new(), String::new(), remaining)
    };

    let (server, port) = parse_host_port(host_str)?;
    let name = if name.is_empty() {
        format!("{}:{}", server, port)
    } else {
        decode_uri_component(&name)
    };

    Some(ParsedProxy {
        name,
        server,
        port,
        password,
        method: String::new(),
        plugin: String::new(),
        plugin_opts: String::new(),
        proxy_type: proxy_type.to_string(),
        username,
        vless_uuid: String::new(),
        vless_opts: String::new(),
    })
}

// ============================================================
// vless://, trojan://, hysteria2:// parsers
// ============================================================

pub fn parse_vless_link(link: &str) -> Option<ParsedProxy> {
    let trimmed = link.trim();
    let rest = trimmed.strip_prefix("vless://")?;
    let at_idx = rest.rfind('@')?;
    let uuid = &rest[..at_idx];
    if uuid.trim().is_empty() {
        return None;
    }
    parse_vless_style_host(&rest[at_idx + 1..], uuid, "vless")
}

pub fn parse_trojan_link(link: &str) -> Option<ParsedProxy> {
    let trimmed = link.trim();
    let rest = trimmed.strip_prefix("trojan://")?;
    let at_idx = rest.rfind('@')?;
    let password = &rest[..at_idx];
    if password.trim().is_empty() {
        return None;
    }
    parse_vless_style_host(&rest[at_idx + 1..], password, "trojan")
}

pub fn parse_hysteria2_link(link: &str) -> Option<ParsedProxy> {
    let trimmed = link.trim();

    let rest = if let Some(r) = trimmed.strip_prefix("hysteria2://") {
        r
    } else if let Some(r) = trimmed.strip_prefix("hy2://") {
        r
    } else {
        return None;
    };

    let at_idx = rest.rfind('@')?;

    let userpass = &rest[..at_idx];
    let (username, password) = if let Some(colon_idx) = userpass.find(':') {
        (
            decode_uri_component(&userpass[..colon_idx]),
            userpass[colon_idx + 1..].to_string(),
        )
    } else {
        (String::new(), userpass.to_string())
    };

    if password.trim().is_empty() {
        return None;
    }

    let mut result = parse_vless_style_host(&rest[at_idx + 1..], &password, "hysteria2")?;
    result.username = username;
    result.password = password;
    Some(result)
}

fn parse_vless_style_host(
    host_full: &str,
    credential: &str,
    proxy_type: &str,
) -> Option<ParsedProxy> {
    let (addr_part, name) = split_fragment(host_full);

    let (addr, query_str) = if let Some(q_idx) = addr_part.find('?') {
        (&addr_part[..q_idx], &addr_part[q_idx + 1..])
    } else {
        (addr_part, "")
    };

    let (server, port) = parse_host_port(addr)?;

    // Parse query params
    let mut opts = serde_json::Map::new();
    if !query_str.is_empty() {
        for pair in query_str.split('&') {
            if let Some(eq) = pair.find('=') {
                let k = &pair[..eq];
                let v = &pair[eq + 1..];
                opts.insert(k.to_string(), serde_json::Value::String(v.to_string()));
            }
        }
    }

    let vless_opts = if opts.is_empty() {
        String::new()
    } else {
        serde_json::to_string(&opts).unwrap_or_default()
    };

    let name = if name.is_empty() {
        format!("{}:{}", server, port)
    } else {
        decode_uri_component(&name)
    };

    Some(ParsedProxy {
        name,
        server,
        port,
        password: if proxy_type == "trojan" {
            credential.to_string()
        } else {
            String::new()
        },
        method: String::new(),
        plugin: String::new(),
        plugin_opts: String::new(),
        proxy_type: proxy_type.to_string(),
        username: String::new(),
        vless_uuid: if proxy_type == "vless" {
            credential.to_string()
        } else {
            String::new()
        },
        vless_opts,
    })
}

// ============================================================
// Auto-detect and batch parse
// ============================================================

/// Auto-detect protocol from a share link and parse it
pub fn parse_proxy_link(link: &str) -> Option<ParsedProxy> {
    let trimmed = link.trim();
    if trimmed.starts_with("hysteria2://") || trimmed.starts_with("hy2://") {
        parse_hysteria2_link(trimmed)
    } else if trimmed.starts_with("trojan://") {
        parse_trojan_link(trimmed)
    } else if trimmed.starts_with("vless://") {
        parse_vless_link(trimmed)
    } else if trimmed.starts_with("ss://") {
        parse_ss_link(trimmed)
    } else if trimmed.starts_with("socks5://") {
        parse_socks5_link(trimmed)
    } else if trimmed.starts_with("http://") && !trimmed.starts_with("https://") {
        parse_http_link(trimmed)
    } else {
        None
    }
}

/// Parse a block of text into ParsedProxy array.
/// Auto-detects protocol per line. Deduplicates by server:port.
pub fn parse_share_links(text: &str) -> Vec<ParsedProxy> {
    let mut results: Vec<ParsedProxy> = Vec::new();

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(parsed) = parse_proxy_link(trimmed) {
            let is_dup = results
                .iter()
                .any(|r| r.server == parsed.server && r.port == parsed.port);
            if !is_dup {
                results.push(parsed);
            }
        }
    }

    results
}

// ============================================================
// Helpers
// ============================================================

fn split_fragment(s: &str) -> (&str, &str) {
    match s.find('#') {
        Some(idx) => (&s[..idx], &s[idx + 1..]),
        None => (s, ""),
    }
}

fn decode_uri_component(s: &str) -> String {
    urlencoding::decode(s)
        .map(|cow| cow.into_owned())
        .unwrap_or_else(|_| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_socks5() {
        let link = "socks5://127.0.0.1:1080#Test Proxy";
        let parsed = parse_proxy_link(link).expect("Should parse");
        assert_eq!(parsed.proxy_type, "socks5");
        assert_eq!(parsed.server, "127.0.0.1");
        assert_eq!(parsed.port, 1080);
        assert_eq!(parsed.name, "Test Proxy");
    }

    #[test]
    fn test_parse_ss_sip002() {
        let link = "ss://YWVzLTI1Ni1nY206dGVzdHBhc3M@example.com:8388#SS Node";
        let parsed = parse_proxy_link(link).expect("Should parse");
        assert_eq!(parsed.proxy_type, "ss");
        assert_eq!(parsed.server, "example.com");
        assert_eq!(parsed.port, 8388);
        assert_eq!(parsed.method, "aes-256-gcm");
        assert_eq!(parsed.password, "testpass");
    }

    #[test]
    fn test_parse_vless() {
        let link = "vless://my-uuid-here@server.com:443?security=reality&sni=example.com&type=grpc&serviceName=test#VLESS Node";
        let parsed = parse_proxy_link(link).expect("Should parse");
        assert_eq!(parsed.proxy_type, "vless");
        assert_eq!(parsed.vless_uuid, "my-uuid-here");
        assert_eq!(parsed.server, "server.com");
        assert_eq!(parsed.port, 443);
        assert_eq!(parsed.name, "VLESS Node");
    }

    #[test]
    fn test_parse_trojan() {
        let link = "trojan://password123@trojan.example.com:443?sni=example.com#Trojan";
        let parsed = parse_proxy_link(link).expect("Should parse");
        assert_eq!(parsed.proxy_type, "trojan");
        assert_eq!(parsed.password, "password123");
    }

    #[test]
    fn test_parse_hysteria2() {
        let link = "hysteria2://pass@node.com:443?sni=example.com&upmbps=100&downmbps=200#HY2";
        let parsed = parse_proxy_link(link).expect("Should parse");
        assert_eq!(parsed.proxy_type, "hysteria2");
        assert_eq!(parsed.server, "node.com");
        assert_eq!(parsed.port, 443);
    }

    #[test]
    fn test_batch_import() {
        let text = "socks5://127.0.0.1:1080#Local\nss://YWVzLTI1Ni1nY206dGVzdA@example.com:8388#Remote";
        let results = parse_share_links(text);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].proxy_type, "socks5");
        assert_eq!(results[1].proxy_type, "ss");
    }
}
