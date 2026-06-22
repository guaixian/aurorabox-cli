pub mod models;
pub mod queries;
pub mod schema;

use rusqlite::Connection;

/// Get the default database path
pub fn db_path() -> String {
    let config_dir = crate::core::config_dir();
    format!("{}/data.db", config_dir)
}

/// Open or create the database with all migrations applied
pub fn open(path: &str) -> anyhow::Result<Connection> {
    // Ensure parent directory exists
    if let Some(parent) = std::path::Path::new(path).parent() {
        std::fs::create_dir_all(parent)?;
    }

    let conn = Connection::open(path)?;

    // Enable WAL mode for better concurrent access
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;

    // Apply migrations
    schema::run_migrations(&conn)?;

    Ok(conn)
}
