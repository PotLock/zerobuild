//! Sandbox file operation tools: write_file, read_file, list_files.

use crate::sandbox::SandboxClient;
use crate::tools::traits::{Tool, ToolResult};
use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;

// ── sandbox_write_file ────────────────────────────────────────────────────────────

pub struct SandboxWriteFileTool {
    client: Arc<dyn SandboxClient>,
}

impl SandboxWriteFileTool {
    pub fn new(client: Arc<dyn SandboxClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl Tool for SandboxWriteFileTool {
    fn name(&self) -> &str {
        "sandbox_write_file"
    }

    fn description(&self) -> &str {
        "Write content to a file in the sandbox. Creates the file and any parent directories. \
         Use this to create or update source files (e.g. page.tsx, layout.tsx, globals.css). \
         Requires an active sandbox."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Absolute file path in the sandbox (e.g. /home/user/project/src/app/page.tsx)"
                },
                "content": {
                    "type": "string",
                    "description": "Full file content to write"
                }
            },
            "required": ["path", "content"]
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

        let path = args["path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: path"))?;

        let content = args["content"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: content"))?;

        match self.client.write_file(path, content).await {
            Ok(()) => Ok(ToolResult {
                success: true,
                output: format!("File written: {path}"),
                error: None,
            }),
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Failed to write file: {e}")),
            }),
        }
    }
}

// ── sandbox_read_file ─────────────────────────────────────────────────────────────

pub struct SandboxReadFileTool {
    client: Arc<dyn SandboxClient>,
}

impl SandboxReadFileTool {
    pub fn new(client: Arc<dyn SandboxClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl Tool for SandboxReadFileTool {
    fn name(&self) -> &str {
        "sandbox_read_file"
    }

    fn description(&self) -> &str {
        "Read the content of a file from the sandbox. \
         Returns the file content as a string. \
         Requires an active sandbox."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Absolute file path to read from the sandbox"
                }
            },
            "required": ["path"]
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

        let path = args["path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: path"))?;

        match self.client.read_file(path).await {
            Ok(content) => Ok(ToolResult {
                success: true,
                output: content,
                error: None,
            }),
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Failed to read file: {e}")),
            }),
        }
    }
}

// ── sandbox_list_files ────────────────────────────────────────────────────────────

pub struct SandboxListFilesTool {
    client: Arc<dyn SandboxClient>,
}

impl SandboxListFilesTool {
    pub fn new(client: Arc<dyn SandboxClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl Tool for SandboxListFilesTool {
    fn name(&self) -> &str {
        "sandbox_list_files"
    }

    fn description(&self) -> &str {
        "List files and directories at a path in the sandbox. \
         Returns a list of entries with names and types. \
         Requires an active sandbox."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Directory path to list (e.g. /home/user/project/src)"
                }
            },
            "required": ["path"]
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

        let path = args["path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: path"))?;

        match self.client.list_files(path).await {
            Ok(listing) => Ok(ToolResult {
                success: true,
                output: listing,
                error: None,
            }),
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Failed to list files: {e}")),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_names() {
        let client = Arc::new(crate::sandbox::e2b::E2bSandboxClient::new(""));
        assert_eq!(SandboxWriteFileTool::new(client.clone()).name(), "sandbox_write_file");
        assert_eq!(SandboxReadFileTool::new(client.clone()).name(), "sandbox_read_file");
        assert_eq!(SandboxListFilesTool::new(client).name(), "sandbox_list_files");
    }
}
