use rusqlite::{params, Connection};

use super::models::*;

// ============================================================
// Subscriptions
// ============================================================

pub fn get_all_subscriptions(conn: &Connection) -> anyhow::Result<Vec<Subscription>> {
    let mut stmt = conn.prepare(
        "SELECT id, identifier, name, used_traffic, total_traffic,
                subscription_url, official_website, expire_time, last_update_time
         FROM subscriptions ORDER BY id DESC"
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(Subscription {
            id: row.get(0)?,
            identifier: row.get(1)?,
            name: row.get(2)?,
            used_traffic: row.get(3)?,
            total_traffic: row.get(4)?,
            subscription_url: row.get(5)?,
            official_website: row.get(6)?,
            expire_time: row.get(7)?,
            last_update_time: row.get(8)?,
        })
    })?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

pub fn get_subscription_by_identifier(
    conn: &Connection,
    identifier: &str,
) -> anyhow::Result<Option<Subscription>> {
    let mut stmt = conn.prepare(
        "SELECT id, identifier, name, used_traffic, total_traffic,
                subscription_url, official_website, expire_time, last_update_time
         FROM subscriptions WHERE identifier = ?1"
    )?;
    let mut rows = stmt.query_map(params![identifier], |row| {
        Ok(Subscription {
            id: row.get(0)?,
            identifier: row.get(1)?,
            name: row.get(2)?,
            used_traffic: row.get(3)?,
            total_traffic: row.get(4)?,
            subscription_url: row.get(5)?,
            official_website: row.get(6)?,
            expire_time: row.get(7)?,
            last_update_time: row.get(8)?,
        })
    })?;
    match rows.next() {
        Some(row) => Ok(Some(row?)),
        None => Ok(None),
    }
}

pub fn get_subscription_config(
    conn: &Connection,
    identifier: &str,
) -> anyhow::Result<Option<SubscriptionConfig>> {
    let mut stmt = conn.prepare(
        "SELECT id, identifier, config_content FROM subscription_configs WHERE identifier = ?1"
    )?;
    let mut rows = stmt.query_map(params![identifier], |row| {
        Ok(SubscriptionConfig {
            id: row.get(0)?,
            identifier: row.get(1)?,
            config_content: row.get(2)?,
        })
    })?;
    match rows.next() {
        Some(row) => Ok(Some(row?)),
        None => Ok(None),
    }
}

pub fn insert_subscription(
    conn: &Connection,
    identifier: &str,
    url: &str,
    name: &str,
) -> anyhow::Result<()> {
    conn.execute(
        "INSERT INTO subscriptions (identifier, name, subscription_url)
         VALUES (?1, ?2, ?3)",
        params![identifier, name, url],
    )?;
    Ok(())
}

pub fn update_subscription_config(
    conn: &Connection,
    identifier: &str,
    config_content: &str,
) -> anyhow::Result<()> {
    conn.execute(
        "INSERT INTO subscription_configs (identifier, config_content)
         VALUES (?1, ?2)
         ON CONFLICT(identifier) DO UPDATE SET config_content = excluded.config_content",
        params![identifier, config_content],
    )?;
    // Update last_update_time on the subscription
    conn.execute(
        "UPDATE subscriptions SET last_update_time = strftime('%s', 'now') WHERE identifier = ?1",
        params![identifier],
    )?;
    Ok(())
}

pub fn delete_subscription(conn: &Connection, id: i64) -> anyhow::Result<()> {
    conn.execute("DELETE FROM subscriptions WHERE id = ?1", params![id])?;
    Ok(())
}

// ============================================================
// Proxy Servers
// ============================================================

pub fn get_all_proxy_servers(conn: &Connection) -> anyhow::Result<Vec<ProxyServer>> {
    let mut stmt = conn.prepare(
        "SELECT id, identifier, name, server_address, server_port,
                password, encryption_method, plugin, plugin_opts,
                is_active, created_at, updated_at, proxy_type,
                username, vless_uuid, vless_opts
         FROM proxy_servers ORDER BY is_active DESC, id DESC"
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(ProxyServer {
            id: row.get(0)?,
            identifier: row.get(1)?,
            name: row.get(2)?,
            server_address: row.get(3)?,
            server_port: row.get(4)?,
            password: row.get(5)?,
            encryption_method: row.get(6)?,
            plugin: row.get(7)?,
            plugin_opts: row.get(8)?,
            is_active: row.get(9)?,
            created_at: row.get(10)?,
            updated_at: row.get(11)?,
            proxy_type: row.get(12)?,
            username: row.get(13)?,
            vless_uuid: row.get(14)?,
            vless_opts: row.get(15)?,
        })
    })?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

pub fn insert_proxy_server(conn: &Connection, server: &ProxyServer) -> anyhow::Result<()> {
    conn.execute(
        "INSERT INTO proxy_servers
         (identifier, name, server_address, server_port, password,
          encryption_method, plugin, plugin_opts, is_active,
          created_at, updated_at, proxy_type, username, vless_uuid, vless_opts)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
        params![
            server.identifier,
            server.name,
            server.server_address,
            server.server_port,
            server.password,
            server.encryption_method,
            server.plugin,
            server.plugin_opts,
            server.is_active,
            server.created_at,
            server.updated_at,
            server.proxy_type,
            server.username,
            server.vless_uuid,
            server.vless_opts,
        ],
    )?;
    Ok(())
}

pub fn update_proxy_server(
    conn: &Connection,
    identifier: &str,
    server: &ProxyServer,
) -> anyhow::Result<()> {
    conn.execute(
        "UPDATE proxy_servers SET
            name = ?1, server_address = ?2, server_port = ?3,
            password = ?4, encryption_method = ?5, plugin = ?6,
            plugin_opts = ?7, proxy_type = ?8, username = ?9,
            vless_uuid = ?10, vless_opts = ?11,
            updated_at = strftime('%s', 'now')
         WHERE identifier = ?12",
        params![
            server.name,
            server.server_address,
            server.server_port,
            server.password,
            server.encryption_method,
            server.plugin,
            server.plugin_opts,
            server.proxy_type,
            server.username,
            server.vless_uuid,
            server.vless_opts,
            identifier,
        ],
    )?;
    Ok(())
}

pub fn delete_proxy_server(conn: &Connection, id: i64) -> anyhow::Result<()> {
    conn.execute("DELETE FROM proxy_servers WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn set_active_proxy_server(conn: &Connection, identifier: &str) -> anyhow::Result<()> {
    // Deactivate all first (unique index constraint)
    conn.execute("UPDATE proxy_servers SET is_active = 0 WHERE is_active = 1", [])?;
    conn.execute(
        "UPDATE proxy_servers SET is_active = 1 WHERE identifier = ?1",
        params![identifier],
    )?;
    Ok(())
}

pub fn get_proxy_server_by_identifier(
    conn: &Connection,
    identifier: &str,
) -> anyhow::Result<Option<ProxyServer>> {
    let mut stmt = conn.prepare(
        "SELECT id, identifier, name, server_address, server_port,
                password, encryption_method, plugin, plugin_opts,
                is_active, created_at, updated_at, proxy_type,
                username, vless_uuid, vless_opts
         FROM proxy_servers WHERE identifier = ?1"
    )?;
    let mut rows = stmt.query_map(params![identifier], |row| {
        Ok(ProxyServer {
            id: row.get(0)?,
            identifier: row.get(1)?,
            name: row.get(2)?,
            server_address: row.get(3)?,
            server_port: row.get(4)?,
            password: row.get(5)?,
            encryption_method: row.get(6)?,
            plugin: row.get(7)?,
            plugin_opts: row.get(8)?,
            is_active: row.get(9)?,
            created_at: row.get(10)?,
            updated_at: row.get(11)?,
            proxy_type: row.get(12)?,
            username: row.get(13)?,
            vless_uuid: row.get(14)?,
            vless_opts: row.get(15)?,
        })
    })?;
    match rows.next() {
        Some(row) => Ok(Some(row?)),
        None => Ok(None),
    }
}

pub fn get_active_proxy_server(conn: &Connection) -> anyhow::Result<Option<ProxyServer>> {
    let mut stmt = conn.prepare(
        "SELECT id, identifier, name, server_address, server_port,
                password, encryption_method, plugin, plugin_opts,
                is_active, created_at, updated_at, proxy_type,
                username, vless_uuid, vless_opts
         FROM proxy_servers WHERE is_active = 1 LIMIT 1"
    )?;
    let mut rows = stmt.query_map([], |row| {
        Ok(ProxyServer {
            id: row.get(0)?,
            identifier: row.get(1)?,
            name: row.get(2)?,
            server_address: row.get(3)?,
            server_port: row.get(4)?,
            password: row.get(5)?,
            encryption_method: row.get(6)?,
            plugin: row.get(7)?,
            plugin_opts: row.get(8)?,
            is_active: row.get(9)?,
            created_at: row.get(10)?,
            updated_at: row.get(11)?,
            proxy_type: row.get(12)?,
            username: row.get(13)?,
            vless_uuid: row.get(14)?,
            vless_opts: row.get(15)?,
        })
    })?;
    match rows.next() {
        Some(row) => Ok(Some(row?)),
        None => Ok(None),
    }
}

// ============================================================
// Proxy Groups
// ============================================================

pub fn get_all_proxy_groups(conn: &Connection) -> anyhow::Result<Vec<ProxyGroup>> {
    let mut stmt = conn.prepare(
        "SELECT id, identifier, name, group_type, is_active, created_at, updated_at
         FROM proxy_groups ORDER BY is_active DESC, id DESC"
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(ProxyGroup {
            id: row.get(0)?,
            identifier: row.get(1)?,
            name: row.get(2)?,
            group_type: row.get(3)?,
            is_active: row.get(4)?,
            created_at: row.get(5)?,
            updated_at: row.get(6)?,
        })
    })?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

pub fn insert_proxy_group(
    conn: &Connection,
    name: &str,
    group_type: &str,
) -> anyhow::Result<String> {
    let identifier = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO proxy_groups (identifier, name, group_type) VALUES (?1, ?2, ?3)",
        params![identifier, name, group_type],
    )?;
    Ok(identifier)
}

pub fn get_proxy_group_by_identifier(
    conn: &Connection,
    identifier: &str,
) -> anyhow::Result<Option<ProxyGroup>> {
    let mut stmt = conn.prepare(
        "SELECT id, identifier, name, group_type, is_active, created_at, updated_at
         FROM proxy_groups WHERE identifier = ?1"
    )?;
    let mut rows = stmt.query_map(params![identifier], |row| {
        Ok(ProxyGroup {
            id: row.get(0)?,
            identifier: row.get(1)?,
            name: row.get(2)?,
            group_type: row.get(3)?,
            is_active: row.get(4)?,
            created_at: row.get(5)?,
            updated_at: row.get(6)?,
        })
    })?;
    match rows.next() {
        Some(row) => Ok(Some(row?)),
        None => Ok(None),
    }
}

pub fn delete_proxy_group(conn: &Connection, id: i64) -> anyhow::Result<()> {
    conn.execute("DELETE FROM proxy_groups WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn set_active_proxy_group(conn: &Connection, identifier: &str) -> anyhow::Result<()> {
    conn.execute("UPDATE proxy_groups SET is_active = 0 WHERE is_active = 1", [])?;
    conn.execute(
        "UPDATE proxy_groups SET is_active = 1 WHERE identifier = ?1",
        params![identifier],
    )?;
    Ok(())
}

pub fn get_active_proxy_group(conn: &Connection) -> anyhow::Result<Option<ProxyGroup>> {
    let mut stmt = conn.prepare(
        "SELECT id, identifier, name, group_type, is_active, created_at, updated_at
         FROM proxy_groups WHERE is_active = 1 LIMIT 1"
    )?;
    let mut rows = stmt.query_map([], |row| {
        Ok(ProxyGroup {
            id: row.get(0)?,
            identifier: row.get(1)?,
            name: row.get(2)?,
            group_type: row.get(3)?,
            is_active: row.get(4)?,
            created_at: row.get(5)?,
            updated_at: row.get(6)?,
        })
    })?;
    match rows.next() {
        Some(row) => Ok(Some(row?)),
        None => Ok(None),
    }
}

pub fn get_group_members(
    conn: &Connection,
    group_identifier: &str,
) -> anyhow::Result<Vec<ProxyGroupMember>> {
    let mut stmt = conn.prepare(
        "SELECT id, group_identifier, server_identifier, sort_order
         FROM proxy_group_members
         WHERE group_identifier = ?1
         ORDER BY sort_order ASC"
    )?;
    let rows = stmt.query_map(params![group_identifier], |row| {
        Ok(ProxyGroupMember {
            id: row.get(0)?,
            group_identifier: row.get(1)?,
            server_identifier: row.get(2)?,
            sort_order: row.get(3)?,
        })
    })?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

pub fn add_group_member(
    conn: &Connection,
    group_identifier: &str,
    server_identifier: &str,
    sort_order: i64,
) -> anyhow::Result<()> {
    conn.execute(
        "INSERT INTO proxy_group_members (group_identifier, server_identifier, sort_order)
         VALUES (?1, ?2, ?3)",
        params![group_identifier, server_identifier, sort_order],
    )?;
    Ok(())
}

pub fn remove_group_member(
    conn: &Connection,
    group_identifier: &str,
    server_identifier: &str,
) -> anyhow::Result<()> {
    conn.execute(
        "DELETE FROM proxy_group_members
         WHERE group_identifier = ?1 AND server_identifier = ?2",
        params![group_identifier, server_identifier],
    )?;
    Ok(())
}
