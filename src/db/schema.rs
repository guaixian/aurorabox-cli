use rusqlite::Connection;

const MIGRATION_1: &str = r#"
CREATE TABLE IF NOT EXISTS subscriptions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    identifier TEXT NOT NULL UNIQUE,
    name TEXT,
    used_traffic INTEGER DEFAULT 0,
    total_traffic INTEGER DEFAULT 1,
    subscription_url TEXT,
    official_website TEXT,
    expire_time INTEGER DEFAULT (strftime('%s', 'now', '+30 days')),
    last_update_time INTEGER DEFAULT (strftime('%s', 'now'))
);
CREATE TABLE IF NOT EXISTS subscription_configs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    identifier TEXT NOT NULL,
    config_content TEXT,
    FOREIGN KEY (identifier) REFERENCES subscriptions(identifier) ON DELETE CASCADE
);
PRAGMA foreign_keys = ON;
"#;

const MIGRATION_2: &str = r#"
CREATE TABLE IF NOT EXISTS proxy_servers (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    identifier TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    server_address TEXT NOT NULL,
    server_port INTEGER NOT NULL,
    password TEXT NOT NULL,
    encryption_method TEXT NOT NULL,
    plugin TEXT DEFAULT '',
    plugin_opts TEXT DEFAULT '',
    is_active INTEGER DEFAULT 0,
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    updated_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_proxy_servers_active ON proxy_servers(is_active) WHERE is_active = 1;
"#;

const MIGRATION_3: &str = r#"
ALTER TABLE proxy_servers ADD COLUMN proxy_type TEXT NOT NULL DEFAULT 'ss';
ALTER TABLE proxy_servers ADD COLUMN username TEXT DEFAULT '';
"#;

const MIGRATION_4: &str = r#"
ALTER TABLE proxy_servers ADD COLUMN vless_uuid TEXT DEFAULT '';
ALTER TABLE proxy_servers ADD COLUMN vless_opts TEXT DEFAULT '';
"#;

const MIGRATION_5: &str = r#"
CREATE TABLE IF NOT EXISTS proxy_groups (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    identifier TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    group_type TEXT NOT NULL DEFAULT 'fixed',
    is_active INTEGER DEFAULT 0,
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    updated_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
);
CREATE TABLE IF NOT EXISTS proxy_group_members (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    group_identifier TEXT NOT NULL,
    server_identifier TEXT NOT NULL,
    sort_order INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (group_identifier) REFERENCES proxy_groups(identifier) ON DELETE CASCADE,
    FOREIGN KEY (server_identifier) REFERENCES proxy_servers(identifier) ON DELETE CASCADE
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_proxy_groups_active ON proxy_groups(is_active) WHERE is_active = 1;
"#;

const MIGRATIONS: &[&str] = &[
    MIGRATION_1,
    MIGRATION_2,
    MIGRATION_3,
    MIGRATION_4,
    MIGRATION_5,
];

/// Apply all migrations to the database
pub fn run_migrations(conn: &Connection) -> anyhow::Result<()> {
    // Create migrations tracking table if it doesn't exist
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS _migrations (
            version INTEGER PRIMARY KEY,
            applied_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
        );"
    )?;

    let current_version: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM _migrations",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    for (i, sql) in MIGRATIONS.iter().enumerate() {
        let version = (i + 1) as i64;
        if version > current_version {
            log::info!("Applying migration {}", version);
            conn.execute_batch(sql)?;
            conn.execute(
                "INSERT INTO _migrations (version) VALUES (?1)",
                rusqlite::params![version],
            )?;
        }
    }

    Ok(())
}
