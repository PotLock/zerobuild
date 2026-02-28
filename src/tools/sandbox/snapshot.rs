//! `sandbox_save_snapshot` tool — extract files from sandbox to SQLite for persistence.

use crate::sandbox::SandboxClient;
use crate::store;
use crate::tools::traits::{Tool, ToolResult};
use async_trait::async_trait;
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;

const TOOL_NAME: &str = "sandbox_save_snapshot";

pub struct SandboxSaveSnapshotTool {
    client: Arc<dyn SandboxClient>,
    db_path: PathBuf,
}

impl SandboxSaveSnapshotTool {
    pub fn new(client: Arc<dyn SandboxClient>, db_path: impl Into<PathBuf>) -> Self {
        Self {
            client,
            db_path: db_path.into(),
        }
    }
}

#[async_trait]
impl Tool for SandboxSaveSnapshotTool {
    fn name(&self) -> &str {
        TOOL_NAME
    }

    fn description(&self) -> &str {
        "Save a snapshot of project files from the sandbox to local SQLite storage. \
         This allows the project to be restored even if the sandbox expires. \
         Call this after completing major changes to the project. \
         Returns the number of files saved."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "workdir": {
                    "type": "string",
                    "description": "Root directory of the project in the sandbox (e.g. /home/user/project). Default: /home/user/project."
                },
                "project_type": {
                    "type": "string",
                    "description": "Project type hint (e.g. 'nextjs', 'react'). Optional."
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
            });
        }

        let workdir = args["workdir"].as_str().unwrap_or("/home/user/project");
        let project_type = args["project_type"].as_str();

        // Collect snapshot files from the sandbox
        let files = match self.client.collect_snapshot_files(workdir).await {
            Ok(f) => f,
            Err(e) => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(format!("Failed to collect snapshot files: {e}")),
                })
            }
        };

        let files_count = files.len();

        if files_count == 0 {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!(
                    "No files found in {workdir}. Make sure the project has been created."
                )),
            });
        }

        // Open DB and save snapshot
        let conn = match store::init_db(&self.db_path) {
            Ok(c) => c,
            Err(e) => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(format!("Failed to open store DB: {e}")),
                })
            }
        };

        if let Err(e) = store::snapshot::save_snapshot(&conn, &files, project_type) {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Failed to save snapshot: {e}")),
            });
        }

        Ok(ToolResult {
            success: true,
            output: format!("Snapshot saved: {files_count} files from {workdir}"),
            error: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn tool_name() {
        let tmp = TempDir::new().unwrap();
        let client = Arc::new(crate::sandbox::e2b::E2bSandboxClient::new(""));
        assert_eq!(
            SandboxSaveSnapshotTool::new(client, tmp.path().join("test.db")).name(),
            TOOL_NAME
        );
    }
}
