//! ZeroBuild store layer: SQLite-backed persistence for sandbox sessions,
//! project snapshots, and GitHub OAuth tokens.
//!
//! This replaces the Node.js backend's SQLite storage. All data is stored
//! in a single database file at the path configured in `ZerobuildConfig.db_path`.

pub mod session;
pub mod snapshot;
pub mod tokens;

use anyhow::Result;
use rusqlite::Connection;
use std::path::Path;

/// Initialize the ZeroBuild SQLite database and create tables if needed.
pub fn init_db(db_path: &Path) -> Result<Connection> {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let conn = Connection::open(db_path)?;

    conn.execute_batch(
        "PRAGMA journal_mode=WAL;
         PRAGMA synchronous=NORMAL;
         PRAGMA foreign_keys=ON;

         CREATE TABLE IF NOT EXISTS sandbox_session (
             id INTEGER PRIMARY KEY CHECK (id = 1),
             sandbox_id TEXT NOT NULL,
             updated_at TEXT NOT NULL
         );

         CREATE TABLE IF NOT EXISTS snapshots (
             id INTEGER PRIMARY KEY CHECK (id = 1),
             files TEXT NOT NULL,
             project_type TEXT,
             updated_at TEXT NOT NULL
         );

         CREATE TABLE IF NOT EXISTS tokens (
             id INTEGER PRIMARY KEY CHECK (id = 1),
             github_token TEXT,
             github_username TEXT,
             updated_at TEXT NOT NULL
         );",
    )?;

    Ok(conn)
}
