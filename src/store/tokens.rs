//! GitHub connector - OAuth token persistence.
//!
//! Stores GitHub connector tokens in the local SQLite database.
//! The database should only be accessible to the local user running ZeroBuild.

use anyhow::Result;
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};

/// Stored GitHub credentials.
#[derive(Debug, Clone)]
pub struct GitHubToken {
    pub token: String,
    pub username: Option<String>,
}

/// Load the stored GitHub token, if any.
pub fn load_github_token(conn: &Connection) -> Result<Option<GitHubToken>> {
    let mut stmt = conn.prepare(
        "SELECT github_token, github_username FROM tokens WHERE id = 1",
    )?;

    let result = stmt
        .query_row([], |row| {
            let token: Option<String> = row.get(0)?;
            let username: Option<String> = row.get(1)?;
            Ok((token, username))
        })
        .optional()?;

    match result {
        None => Ok(None),
        Some((None, _)) => Ok(None),
        Some((Some(token), username)) => Ok(Some(GitHubToken { token, username })),
    }
}

/// Persist a GitHub token (upsert — always row id=1).
pub fn save_github_token(
    conn: &Connection,
    token: &str,
    username: Option<&str>,
) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO tokens (id, github_token, github_username, updated_at)
         VALUES (1, ?1, ?2, ?3)
         ON CONFLICT(id) DO UPDATE SET
             github_token = excluded.github_token,
             github_username = excluded.github_username,
             updated_at = excluded.updated_at",
        params![token, username, now],
    )?;
    Ok(())
}

/// Clear the stored GitHub token.
pub fn clear_github_token(conn: &Connection) -> Result<()> {
    conn.execute(
        "UPDATE tokens SET github_token = NULL, github_username = NULL WHERE id = 1",
        [],
    )?;
    Ok(())
}
