/// Integration tests for aurorabox-cli
use serde_json::Value;

/// Test that all 4 config templates load and are valid JSON
#[test]
fn test_all_templates_load() {
    let templates = vec![
        ("mixed", false),
        ("tun", true),
        ("global", false),
        ("global", true),
    ];

    for (mode, tun) in templates {
        let config = aurorabox_cli::core::generate_config(mode, tun, None);
        assert!(config.is_ok(), "Failed to generate config for mode={}, tun={}", mode, tun);

        let config = config.unwrap();
        // Verify basic structure
        assert!(config.get("log").is_some(), "Missing 'log' section");
        assert!(config.get("dns").is_some(), "Missing 'dns' section");
        assert!(config.get("inbounds").is_some(), "Missing 'inbounds' section");
        assert!(config.get("outbounds").is_some(), "Missing 'outbounds' section");
        assert!(config.get("route").is_some(), "Missing 'route' section");
    }
}

/// Test that generate_config produces valid JSON that can be serialized
#[test]
fn test_generate_mixed_config() {
    let config = aurorabox_cli::core::generate_config("rule", false, None)
        .expect("Should generate mixed config");

    // Verify it can be serialized
    let json_str = serde_json::to_string(&config).expect("Should serialize");
    assert!(!json_str.is_empty());

    // Verify JSON can be parsed back
    let _parsed: Value = serde_json::from_str(&json_str).expect("Should parse back");
}

/// Test that the experimental config section is properly configured
#[test]
fn test_experimental_config() {
    let config = aurorabox_cli::core::generate_config("rule", false, None).unwrap();

    let experimental = config.get("experimental").expect("Should have experimental section");
    assert!(experimental.get("clash_api").is_some(), "Missing clash_api");
    assert!(experimental.get("cache_file").is_some(), "Missing cache_file");
}

/// Test outbound builder for all protocol types via the internal API
#[test]
fn test_outbound_builder_basic() {
    use aurorabox_cli::db::models::ProxyServer;

    let now = chrono::Utc::now().timestamp();

    let server = ProxyServer {
        id: 0,
        identifier: "test-ss-001".to_string(),
        name: "Test SS".to_string(),
        server_address: "example.com".to_string(),
        server_port: 8388,
        password: "testpass".to_string(),
        encryption_method: "aes-256-gcm".to_string(),
        plugin: String::new(),
        plugin_opts: String::new(),
        is_active: 0,
        created_at: now,
        updated_at: now,
        proxy_type: "ss".to_string(),
        username: String::new(),
        vless_uuid: String::new(),
        vless_opts: String::new(),
    };

    let outbound = aurorabox_cli::core::outbound::build_outbound(&server)
        .expect("Should build SS outbound");

    assert_eq!(outbound["type"], "shadowsocks");
    assert_eq!(outbound["server"], "example.com");
    assert_eq!(outbound["server_port"], 8388);
    assert_eq!(outbound["method"], "aes-256-gcm");
    assert_eq!(outbound["password"], "testpass");
}

/// Test SOCKS5 outbound
#[test]
fn test_outbound_builder_socks5() {
    use aurorabox_cli::db::models::ProxyServer;

    let now = chrono::Utc::now().timestamp();
    let server = ProxyServer {
        id: 0,
        identifier: "test-socks-001".to_string(),
        name: "Test SOCKS5".to_string(),
        server_address: "127.0.0.1".to_string(),
        server_port: 1080,
        password: String::new(),
        encryption_method: String::new(),
        plugin: String::new(),
        plugin_opts: String::new(),
        is_active: 0,
        created_at: now,
        updated_at: now,
        proxy_type: "socks5".to_string(),
        username: String::new(),
        vless_uuid: String::new(),
        vless_opts: String::new(),
    };

    let outbound = aurorabox_cli::core::outbound::build_outbound(&server)
        .expect("Should build SOCKS5 outbound");

    assert_eq!(outbound["type"], "socks");
    assert_eq!(outbound["version"], "5");
    assert_eq!(outbound["server"], "127.0.0.1");
    assert_eq!(outbound["server_port"], 1080);
}

/// Test database operations with in-memory SQLite
#[test]
fn test_database_crud() {
    use aurorabox_cli::db::{models::ProxyServer, queries};

    // Use in-memory database for testing
    let conn = rusqlite::Connection::open_in_memory().expect("Should open in-memory DB");
    aurorabox_cli::db::schema::run_migrations(&conn).expect("Should run migrations");

    let now = chrono::Utc::now().timestamp();
    let server = ProxyServer {
        id: 0,
        identifier: "test-crud-001".to_string(),
        name: "Test Server".to_string(),
        server_address: "10.0.0.1".to_string(),
        server_port: 443,
        password: "secret".to_string(),
        encryption_method: "chacha20-ietf-poly1305".to_string(),
        plugin: String::new(),
        plugin_opts: String::new(),
        is_active: 0,
        created_at: now,
        updated_at: now,
        proxy_type: "ss".to_string(),
        username: String::new(),
        vless_uuid: String::new(),
        vless_opts: String::new(),
    };

    // Insert
    queries::insert_proxy_server(&conn, &server).expect("Should insert");
    let all = queries::get_all_proxy_servers(&conn).expect("Should query");
    assert_eq!(all.len(), 1, "Should have 1 server");
    assert_eq!(all[0].name, "Test Server");

    // Delete
    queries::delete_proxy_server(&conn, all[0].id).expect("Should delete");
    let after = queries::get_all_proxy_servers(&conn).expect("Should query after delete");
    assert_eq!(after.len(), 0, "Should have 0 servers after delete");
}
