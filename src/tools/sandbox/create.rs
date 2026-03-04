//! `sandbox_create` tool — create or reset a local process sandbox.

use crate::sandbox::SandboxClient;
use crate::tools::traits::{Tool, ToolResult};
use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;

const TOOL_NAME: &str = "sandbox_create";

pub struct SandboxCreateTool {
    client: Arc<dyn SandboxClient>,
    template: String,
    timeout_ms: u64,
}

impl SandboxCreateTool {
    pub fn new(
        client: Arc<dyn SandboxClient>,
        template: impl Into<String>,
        timeout_ms: u64,
    ) -> Self {
        Self {
            client,
            template: template.into(),
            timeout_ms,
        }
    }
}

#[async_trait]
impl Tool for SandboxCreateTool {
    fn name(&self) -> &str {
        TOOL_NAME
    }

    fn description(&self) -> &str {
        "🚀 Create a new SANDBOX (isolated temp directory) for running code. \
         \
         ⚠️ This creates an ISOLATED environment — NOT your local machine. \
         All build operations (npm, npx, node) must run INSIDE this sandbox via `sandbox_run_command`. \
         \
         Pass reset=true to kill any existing sandbox and start fresh. \
         Returns the sandbox ID — all other sandbox_* tools use this automatically. \
         Call this before any file or command operations. \
         \
         ❌ DO NOT use `shell` tool for npm/npx/node — it runs LOCALLY, not in sandbox!"
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "reset": {
                    "type": "boolean",
                    "description": "If true, kill any existing sandbox and create a fresh one. Default: false (resume if possible)."
                }
            },
            "required": []
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let reset = args["reset"].as_bool().unwrap_or(false);

        if !reset {
            if let Some(existing_id) = self.client.current_id() {
                return Ok(ToolResult {
                    success: true,
                    output: format!(
                        "Reusing existing sandbox.\nsandbox_id: {existing_id}\nstatus: running"
                    ),
                    error: None,
                    error_hint: None,
                });
            }
        }

        match self
            .client
            .create_sandbox(reset, &self.template, self.timeout_ms)
            .await
        {
            Ok(id) => {
                // Auto-detect package manager after sandbox creation
                let pm = self.client.detect_package_manager().await;
                // Build tip based on detected package manager
                let tip = if pm == crate::sandbox::PackageManager::Npm {
                    "💡 Tip: Using npm as package manager. Consider installing pnpm for faster installs.".to_string()
                } else {
                    format!(
                        "💡 Tip: Use '{install}' for faster installs instead of 'npm install'",
                        install = pm.install_cmd()
                    )
                };
                Ok(ToolResult {
                    success: true,
                    output: format!(
                        "Sandbox created.\nsandbox_id: {id}\ntemplate: {}\nstatus: running\npackage_manager: {pm}\n\n{tip}",
                        self.template,
                        pm = pm,
                        tip = tip
                    ),
                    error: None,
                    error_hint: None,
                })
            }
            Err(e) => {
                let err_msg = format!("{e}");
                Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(format!("🚨 SANDBOX CREATION FAILED: {err_msg}")),
                    error_hint: Some(
                        "Sandbox creation failed. STOP: Do not proceed with file_write or shell. Fix the sandbox issue first.".to_string(),
                    ),
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_name() {
        // Use a mock client that always fails (empty client)
        let client = Arc::new(crate::sandbox::local::LocalProcessSandboxClient::new());
        let tool = SandboxCreateTool::new(client, "code-interpreter-v1", 600_000);
        assert_eq!(tool.name(), TOOL_NAME);
    }
}
