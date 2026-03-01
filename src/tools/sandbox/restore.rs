//! `sandbox_restore_snapshot` tool — restore project files from SQLite snapshot into sandbox.

use crate::sandbox::SandboxClient;
use crate::store;
use crate::tools::traits::{Tool, ToolResult};
use async_trait::async_trait;
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;

const TOOL_NAME: &str = "sandbox_restore_snapshot";

pub struct SandboxRestoreSnapshotTool {
    client: Arc<dyn SandboxClient>,
    db_path: PathBuf,
}

impl SandboxRestoreSnapshotTool {
    pub fn new(client: Arc<dyn SandboxClient>, db_path: impl Into<PathBuf>) -> Self {
        Self {
            client,
            db_path: db_path.into(),
        }
    }
}

#[async_trait]
impl Tool for SandboxRestoreSnapshotTool {
    fn name(&self) -> &str {
        TOOL_NAME
    }

    fn description(&self) -> &str {
        "Restore project files from the last saved SQLite snapshot into the active sandbox. \
         Use this after sandbox_create when resuming work on a previously built project. \
         Writes every file from the snapshot back into the sandbox under the given workdir. \
         Returns the number of files restored."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "workdir": {
                    "type": "string",
                    "description": "Project root relative to sandbox root (e.g. 'project'). Default: 'project'. Must match the workdir used when the snapshot was saved."
                }
            },
            "required": []
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        if let Err(e) = self.client.require_id() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(e),
                error_hint: Some(
                    "Call sandbox_create before sandbox_restore_snapshot.".to_string(),
                ),
            });
        }

        let workdir = args["workdir"].as_str().unwrap_or("project");

        let conn = match store::init_db(&self.db_path) {
            Ok(c) => c,
            Err(e) => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(format!("Failed to open store DB: {e}")),
                    error_hint: None,
                })
            }
        };

        let snapshot = match store::snapshot::load_snapshot(&conn) {
            Ok(Some(s)) => s,
            Ok(None) => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some("No snapshot found. Run sandbox_save_snapshot first.".to_string()),
                    error_hint: None,
                })
            }
            Err(e) => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(format!("Failed to load snapshot: {e}")),
                    error_hint: None,
                })
            }
        };

        let (files, project_type) = snapshot;
        let total = files.len();
        let mut failed: Vec<String> = Vec::new();

        for (rel_path, content) in &files {
            // Restore under workdir — e.g. "project/src/pages/index.tsx"
            let dest = format!("{workdir}/{rel_path}");
            if let Err(e) = self.client.write_file(&dest, content).await {
                tracing::warn!("Failed to restore {dest}: {e}");
                failed.push(dest);
            }
        }

        let restored = total - failed.len();

        if failed.is_empty() {
            Ok(ToolResult {
                success: true,
                output: format!(
                    "Snapshot restored: {restored}/{total} files written to '{workdir}' (project_type: {}).",
                    project_type.as_deref().unwrap_or("unknown")
                ),
                error: None,
                error_hint: None,
            })
        } else {
            Ok(ToolResult {
                success: false,
                output: format!(
                    "Restored {restored}/{total} files. {} failed.",
                    failed.len()
                ),
                error: Some(format!("Failed to restore: {}", failed.join(", "))),
                error_hint: None,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn tool_name() {
        let tmp = TempDir::new().unwrap();
        let client = Arc::new(crate::sandbox::local::LocalProcessSandboxClient::new());
        assert_eq!(
            SandboxRestoreSnapshotTool::new(client, tmp.path().join("test.db")).name(),
            TOOL_NAME
        );
    }
}
