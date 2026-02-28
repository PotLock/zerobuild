//! Think tool — allows Master Agent to reason through complex problems step by step.
//!
//! This tool enables structured thinking for the agent, helping it work through
//! complex planning, debugging, or decision-making tasks before taking action.

use super::traits::{Tool, ToolResult};
use async_trait::async_trait;
use serde_json::json;

const TOOL_NAME: &str = "think";

pub struct ThinkTool;

impl ThinkTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for ThinkTool {
    fn name(&self) -> &str {
        TOOL_NAME
    }

    fn description(&self) -> &str {
        "Use this tool to think through complex problems step by step. \
         This helps with structured reasoning, planning, debugging, or decision-making. \
         Provide your thoughts in the 'thoughts' parameter. \
         The content will be returned to you to help maintain context."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "thoughts": {
                    "type": "string",
                    "description": "Your detailed thoughts, reasoning, or analysis. Be thorough and explicit."
                },
                "reasoning_type": {
                    "type": "string",
                    "description": "Type of reasoning being performed",
                    "enum": ["planning", "debugging", "analysis", "decision", "reflection"],
                    "default": "analysis"
                }
            },
            "required": ["thoughts"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let thoughts = args["thoughts"]
            .as_str()
            .unwrap_or("No thoughts provided");
        
        let reasoning_type = args["reasoning_type"]
            .as_str()
            .unwrap_or("analysis");

        let output = format!(
            "[{}] Thinking through the problem:\n\n{}",
            reasoning_type.to_uppercase(),
            thoughts
        );

        Ok(ToolResult {
            success: true,
            output,
            error: None,
        })
    }
}

impl Default for ThinkTool {
    fn default() -> Self {
        Self::new()
    }
}
