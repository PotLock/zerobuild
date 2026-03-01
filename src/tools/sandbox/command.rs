//! `sandbox_run_command` tool — execute a shell command in the sandbox.

use crate::sandbox::SandboxClient;
use crate::tools::traits::{Tool, ToolResult};
use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;

const TOOL_NAME: &str = "sandbox_run_command";

pub struct SandboxRunCommandTool {
    client: Arc<dyn SandboxClient>,
}

impl SandboxRunCommandTool {
    pub fn new(client: Arc<dyn SandboxClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl Tool for SandboxRunCommandTool {
    fn name(&self) -> &str {
        TOOL_NAME
    }

    fn description(&self) -> &str {
        "🚀 Run a shell command INSIDE THE SANDBOX (isolated environment). \
         \
         ✅ REQUIRED for: npm install, npx create-next-app, npm run build, npm run dev, npx, node, python \
         \
         ❌ DO NOT use `shell` tool for build operations — it runs locally, not in sandbox! \
         \
         Returns stdout, stderr, and exit_code. \
         Requires an active sandbox (call sandbox_create first)."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "Shell command to execute (e.g. 'npm install', 'npx create-next-app@latest . --typescript --yes')"
                },
                "workdir": {
                    "type": "string",
                    "description": "Working directory for the command. Default: /home/user."
                },
                "timeout_ms": {
                    "type": "integer",
                    "description": "Timeout in milliseconds. Default: 120000 (2 minutes)."
                }
            },
            "required": ["command"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
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

        let command = args["command"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: command"))?;

        if command.trim().is_empty() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("command cannot be empty".to_string()),
                error_hint: None,
            });
        }

        let workdir = args["workdir"].as_str().unwrap_or("/home/user/project");
        let timeout_ms = args["timeout_ms"].as_u64().unwrap_or(300_000);

        match self.client.run_command(command, workdir, timeout_ms).await {
            Ok(output) => {
                let exit_code = output.exit_code;
                let success = exit_code == 0;

                let mut out = format!("exit_code: {exit_code}");
                if !output.stdout.is_empty() {
                    out.push_str(&format!("\n\nstdout:\n{}", output.stdout));
                }
                if !output.stderr.is_empty() {
                    out.push_str(&format!("\n\nstderr:\n{}", output.stderr));
                }

                if success {
                    Ok(ToolResult {
                        success: true,
                        output: out,
                        error: None,
                        error_hint: None,
                    })
                } else {
                    Ok(ToolResult {
                        success: false,
                        output: out,
                        error: Some(format!("Command exited with code {exit_code}")),
                        error_hint: None,
                    })
                }
            }
            Err(e) => {
                let err_msg = format!("{e}");
                // Check if it's a "no active sandbox" error
                let error_hint = if err_msg.contains("No active sandbox") {
                    "🚨 SANDBOX NOT AVAILABLE\n\
                    \n\
                    The sandbox is not active. Possible reasons:\n\
                    1. sandbox_create was not called\n\
                    2. sandbox was killed\n\
                    \n\
                    ✅ What to do:\n\
                    - Call sandbox_create with reset=true to create a new sandbox\n\
                    \n\
                    ❌ DO NOT use file_write or shell as fallback - this will create files on your local machine!".to_string()
                } else {
                    format!("Sandbox command failed: {e}")
                };
                Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(err_msg),
                    error_hint: Some(error_hint),
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
        assert_eq!(SandboxRunCommandTool::new(client).name(), TOOL_NAME);
    }
}
