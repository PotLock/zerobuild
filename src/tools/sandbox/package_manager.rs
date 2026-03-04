//! `sandbox_get_package_manager` tool — get the detected package manager for the sandbox.

use crate::sandbox::{PackageManager, SandboxClient};
use crate::tools::traits::{Tool, ToolResult};
use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;

const TOOL_NAME: &str = "sandbox_get_package_manager";

pub struct SandboxGetPackageManagerTool {
    client: Arc<dyn SandboxClient>,
}

impl SandboxGetPackageManagerTool {
    pub fn new(client: Arc<dyn SandboxClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl Tool for SandboxGetPackageManagerTool {
    fn name(&self) -> &str {
        TOOL_NAME
    }

    fn description(&self) -> &str {
        "📦 Get the detected package manager for the sandbox. \
         \
         Returns the best available package manager in priority order: pnpm > yarn > npm. \
         Use this to determine which package manager to use for install commands. \
         \
         Example: If this returns 'pnpm', use 'pnpm install' instead of 'npm install'."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    async fn execute(&self, _args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let _sandbox_id = match self.client.require_id() {
            Ok(id) => id,
            Err(e) => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(e),
                    error_hint: None,
                })
            }
        };

        // Actively detect package manager instead of using cached state
        let pm = self.client.detect_package_manager().await;
        
        let output = format!(
            "Detected package manager: {pm}\n\
             Install command: {install}\n\
             Add command: {add}\n\
             Run command: {run}",
            pm = pm,
            install = pm.install_cmd(),
            add = pm.add_cmd(),
            run = pm.run_cmd()
        );

        Ok(ToolResult {
            success: true,
            output,
            error: None,
            error_hint: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_name() {
        let client = Arc::new(crate::sandbox::local::LocalProcessSandboxClient::new());
        let tool = SandboxGetPackageManagerTool::new(client);
        assert_eq!(tool.name(), TOOL_NAME);
    }
}
