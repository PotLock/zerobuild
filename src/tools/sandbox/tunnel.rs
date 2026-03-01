//! `sandbox_get_public_url` tool — start a Cloudflare Quick Tunnel for a port.

use crate::sandbox::SandboxClient;
use crate::tools::traits::{Tool, ToolResult};
use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;

const TOOL_NAME: &str = "sandbox_get_public_url";

const INSTALL_HINT: &str = "\
Install cloudflared to use public URLs:\n\
  Linux:  curl -fsSL https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-linux-amd64 \
-o /usr/local/bin/cloudflared && chmod +x /usr/local/bin/cloudflared\n\
  macOS:  brew install cloudflare/cloudflare/cloudflared";

pub struct SandboxGetPublicUrlTool {
    client: Arc<dyn SandboxClient>,
}

impl SandboxGetPublicUrlTool {
    pub fn new(client: Arc<dyn SandboxClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl Tool for SandboxGetPublicUrlTool {
    fn name(&self) -> &str {
        TOOL_NAME
    }

    fn description(&self) -> &str {
        "Start a Cloudflare Quick Tunnel and return a public HTTPS URL for the sandbox port. \
         Use this instead of sandbox_get_preview_url when running on a VPS or remote server \
         where localhost is not accessible from the user's device. \
         Requires cloudflared to be installed on the host. \
         The tunnel URL (https://xxx.trycloudflare.com) is publicly accessible from any device \
         with no firewall configuration required. \
         The tunnel is cached — calling this again for the same port returns the same URL instantly. \
         The tunnel is killed automatically when sandbox_kill is called."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "port": {
                    "type": "integer",
                    "description": "Port number to tunnel. Default: 3000.",
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
                    error_hint: None,
                })
            }
        };

        let port = args["port"].as_u64().map(|p| p as u16).unwrap_or(3000);

        match self.client.start_tunnel(port).await {
            Ok(url) => Ok(ToolResult {
                success: true,
                output: format!("Public URL (port {port}): {url}\n(sandbox: {sandbox_id})"),
                error: None,
                error_hint: None,
            }),
            Err(e) => {
                let msg = e.to_string();
                let hint = if msg.contains("cloudflared not found") {
                    Some(INSTALL_HINT.to_string())
                } else {
                    None
                };
                Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(format!("Failed to start tunnel: {msg}")),
                    error_hint: hint,
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
        let client = Arc::new(crate::sandbox::local::LocalProcessSandboxClient::new());
        assert_eq!(SandboxGetPublicUrlTool::new(client).name(), TOOL_NAME);
    }
}
