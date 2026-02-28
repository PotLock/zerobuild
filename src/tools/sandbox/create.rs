//! `sandbox_create` tool — create or reset a sandbox (E2B or Docker).

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
        "Create a new sandbox (cloud MicroVM or local Docker container) for running code. \
         Pass reset=true to kill any existing sandbox and start fresh. \
         Returns the sandbox/container ID — all other sandbox_* tools use this automatically. \
         Call this before any file or command operations."
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
                });
            }
        }

        match self
            .client
            .create_sandbox(reset, &self.template, self.timeout_ms)
            .await
        {
            Ok(id) => Ok(ToolResult {
                success: true,
                output: format!(
                    "Sandbox created.\nsandbox_id: {id}\ntemplate: {}\nstatus: running",
                    self.template
                ),
                error: None,
            }),
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Failed to create sandbox: {e}")),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_name() {
        // Use a mock client that always fails (empty client)
        let client = Arc::new(crate::sandbox::e2b::E2bSandboxClient::new(""));
        let tool = SandboxCreateTool::new(client, "code-interpreter-v1", 600_000);
        assert_eq!(tool.name(), TOOL_NAME);
    }
}
