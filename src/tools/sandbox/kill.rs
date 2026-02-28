//! `sandbox_kill` tool — terminate the current sandbox.

use crate::sandbox::SandboxClient;
use crate::tools::traits::{Tool, ToolResult};
use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;

const TOOL_NAME: &str = "sandbox_kill";

pub struct SandboxKillTool {
    client: Arc<dyn SandboxClient>,
}

impl SandboxKillTool {
    pub fn new(client: Arc<dyn SandboxClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl Tool for SandboxKillTool {
    fn name(&self) -> &str {
        TOOL_NAME
    }

    fn description(&self) -> &str {
        "Terminate the current sandbox. Frees resources and clears the active sandbox_id. \
         Any unsaved work will be lost — call sandbox_save_snapshot first if needed. \
         Only call this when explicitly requested or when starting completely fresh."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    async fn execute(&self, _args: serde_json::Value) -> anyhow::Result<ToolResult> {
        // Check if there's an active sandbox
        if self.client.current_id().is_none() {
            return Ok(ToolResult {
                success: true,
                output: "No active sandbox to kill.".to_string(),
                error: None,
            });
        }

        match self.client.kill_sandbox().await {
            Ok(msg) => Ok(ToolResult {
                success: true,
                output: msg,
                error: None,
            }),
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Failed to kill sandbox: {e}")),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_name() {
        let client = Arc::new(crate::sandbox::e2b::E2bSandboxClient::new(""));
        assert_eq!(SandboxKillTool::new(client).name(), TOOL_NAME);
    }
}
