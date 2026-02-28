//! Sandbox session persistence: track the current E2B sandbox_id so it can be
//! resumed across agent restarts.

use anyhow::Result;
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};

/// Load the persisted sandbox_id, if any.
pub fn load_sandbox_id(conn: &Connection) -> Result<Option<String>> {
    let mut stmt = conn.prepare(
        "SELECT sandbox_id FROM sandbox_session WHERE id = 1",
    )?;

    let result = stmt.query_row([], |row| row.get(0)).optional()?;
    Ok(result)
}

/// Persist the current sandbox_id (upsert — always row id=1).
pub fn save_sandbox_id(conn: &Connection, sandbox_id: &str) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO sandbox_session (id, sandbox_id, updated_at)
         VALUES (1, ?1, ?2)
         ON CONFLICT(id) DO UPDATE SET
             sandbox_id = excluded.sandbox_id,
             updated_at = excluded.updated_at",
        params![sandbox_id, now],
    )?;
    Ok(())
}

/// Clear the persisted sandbox_id (sandbox was killed or reset).
pub fn clear_sandbox_id(conn: &Connection) -> Result<()> {
    conn.execute("DELETE FROM sandbox_session WHERE id = 1", [])?;
    Ok(())
}
