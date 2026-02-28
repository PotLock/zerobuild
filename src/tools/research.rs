//! Research tool — allows Master Agent to perform research queries.
//!
//! This tool enables the agent to search for information, documentation,
//! best practices, or examples before making recommendations to users.

use super::traits::{Tool, ToolResult};
use async_trait::async_trait;
use serde_json::json;

const TOOL_NAME: &str = "research";
const DEFAULT_MAX_RESULTS: usize = 5;

pub struct ResearchTool {
    web_search_enabled: bool,
}

impl ResearchTool {
    pub fn new() -> Self {
        Self {
            web_search_enabled: true,
        }
    }

    pub fn with_web_search(mut self, enabled: bool) -> Self {
        self.web_search_enabled = enabled;
        self
    }

    fn perform_research(&self, query: &str, context: &str, _max_results: usize) -> String {
        // For now, this returns a structured research request that the LLM
        // can use to inform its response. In a full implementation, this could
        // integrate with web search APIs, documentation databases, etc.
        
        let mut result = format!("Research Query: {}\n", query);
        
        if !context.is_empty() {
            result.push_str(&format!("Context: {}\n", context));
        }
        
        result.push_str("\nResearch Guidelines:\n");
        result.push_str("- Consider best practices and industry standards\n");
        result.push_str("- Look for recent examples and documentation\n");
        result.push_str("- Evaluate multiple approaches before recommending\n");
        result.push_str("- Prioritize official documentation and reliable sources\n");
        
        if self.web_search_enabled {
            result.push_str("\n(Web search would be performed here in full implementation)\n");
        }
        
        result
    }
}

#[async_trait]
impl Tool for ResearchTool {
    fn name(&self) -> &str {
        TOOL_NAME
    }

    fn description(&self) -> &str {
        "Research a topic to gather information before making recommendations. \
         Use this when you need to verify best practices, find documentation, \
         or gather context about technologies, frameworks, or approaches. \
         This helps provide accurate, up-to-date advice to users."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The research query or topic to investigate"
                },
                "context": {
                    "type": "string",
                    "description": "Additional context about why this research is needed",
                    "default": ""
                },
                "max_results": {
                    "type": "integer",
                    "description": "Maximum number of results to consider",
                    "default": 5,
                    "minimum": 1,
                    "maximum": 10
                },
                "focus_areas": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Specific areas to focus research on (e.g., ['security', 'performance', 'examples'])",
                    "default": []
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let query = args["query"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: query"))?;

        let context = args["context"].as_str().unwrap_or("");
        let max_results = args["max_results"]
            .as_u64()
            .map(|n| n as usize)
            .unwrap_or(DEFAULT_MAX_RESULTS);

        let research_output = self.perform_research(query, context, max_results);

        Ok(ToolResult {
            success: true,
            output: research_output,
            error: None,
        })
    }
}

impl Default for ResearchTool {
    fn default() -> Self {
        Self::new()
    }
}
