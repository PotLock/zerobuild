//! Project snapshot persistence: save/load source files as a JSON map
//! `{"/path": "content"}` so the project can be restored after sandbox expiry.

use anyhow::Result;
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::HashMap;

/// Load the last saved project snapshot.
/// Returns `None` if no snapshot has been saved yet.
pub fn load_snapshot(
    conn: &Connection,
) -> Result<Option<(HashMap<String, String>, Option<String>)>> {
    let mut stmt = conn.prepare(
        "SELECT files, project_type FROM snapshots WHERE id = 1",
    )?;

    let result = stmt
        .query_row([], |row| {
            let files_json: String = row.get(0)?;
            let project_type: Option<String> = row.get(1)?;
            Ok((files_json, project_type))
        })
        .optional()?;

    match result {
        None => Ok(None),
        Some((files_json, project_type)) => {
            let files: HashMap<String, String> =
                serde_json::from_str(&files_json)
                    .map_err(|e| anyhow::anyhow!("Failed to deserialize snapshot: {e}"))?;
            Ok(Some((files, project_type)))
        }
    }
}

/// Persist project files as a snapshot (upsert — always row id=1).
pub fn save_snapshot(
    conn: &Connection,
    files: &HashMap<String, String>,
    project_type: Option<&str>,
) -> Result<usize> {
    let files_json = serde_json::to_string(files)
        .map_err(|e| anyhow::anyhow!("Failed to serialize snapshot: {e}"))?;
    let now = Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO snapshots (id, files, project_type, updated_at)
         VALUES (1, ?1, ?2, ?3)
         ON CONFLICT(id) DO UPDATE SET
             files = excluded.files,
             project_type = excluded.project_type,
             updated_at = excluded.updated_at",
        params![files_json, project_type, now],
    )?;

    Ok(files.len())
}
