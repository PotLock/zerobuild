//! `sandbox_get_preview_url` tool — get the preview URL for a port.

use crate::sandbox::SandboxClient;
use crate::tools::traits::{Tool, ToolResult};
use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;

const TOOL_NAME: &str = "sandbox_get_preview_url";

pub struct SandboxGetPreviewUrlTool {
    client: Arc<dyn SandboxClient>,
}

impl SandboxGetPreviewUrlTool {
    pub fn new(client: Arc<dyn SandboxClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl Tool for SandboxGetPreviewUrlTool {
    fn name(&self) -> &str {
        TOOL_NAME
    }

    fn description(&self) -> &str {
        "Get the public preview URL for a port exposed in the sandbox. \
         Use port 3000 for Next.js dev server. \
         Call this after starting the dev server with sandbox_run_command. \
         Returns a URL that can be shared with the user. \
         Requires an active sandbox. \
         Note: With Docker provider, the URL is http://localhost:{port} (local access only)."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "port": {
                    "type": "integer",
                    "description": "Port number to get the preview URL for. Default: 3000.",
                    "default": 3000
                }
            },
            "required": []
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let sandbox_id = match self.client.require_id() {
            Ok(id) => id,
            Err(e) => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(e),
                })
            }
        };

        let port = args["port"].as_u64().map(|p| p as u16).unwrap_or(3000);

        match self.client.get_preview_url(port).await {
            Ok(url) => Ok(ToolResult {
                success: true,
                output: format!("Preview URL (port {port}): {url}\n(sandbox: {sandbox_id})"),
                error: None,
            }),
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!(
                    "Failed to get preview URL: {e}\nMake sure the dev server is running on port {port}."
                )),
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
        assert_eq!(SandboxGetPreviewUrlTool::new(client).name(), TOOL_NAME);
    }
}
