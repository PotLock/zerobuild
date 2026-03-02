//! `factory_build` tool — the entry point for the multi-agent factory workflow.
//!
//! Implements the [`Tool`] trait, registered conditionally when
//! `factory.enabled = true` in the config.

use super::workflow::FactoryWorkflow;
use crate::config::DelegateAgentConfig;
use crate::providers;
use crate::tools::traits::{Tool, ToolResult};
use async_trait::async_trait;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;

/// Tool that orchestrates the full multi-agent factory workflow.
///
/// When invoked, spawns specialized agents (Business Analyst, UI/UX Designer, Developer, Tester, DevOps)
/// through phased execution to turn a user's idea into working software.
///
/// Features:
/// - Intent classification for dynamic agent spawning
/// - Progress streaming for real-time visibility
/// - Cross-phase memory sharing
/// - Optional learning and sub-agent spawning
pub struct FactoryOrchestratorTool {
    max_ping_pong: usize,
    role_overrides: HashMap<String, DelegateAgentConfig>,
    provider_runtime_options: providers::ProviderRuntimeOptions,
    fallback_credential: Option<String>,
    default_provider: String,
    default_model: String,
    parent_tools: Arc<Vec<Arc<dyn Tool>>>,
    multimodal_config: crate::config::MultimodalConfig,
    enable_streaming: bool,
}

impl FactoryOrchestratorTool {
    /// Create a new factory orchestrator tool.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        max_ping_pong: usize,
        role_overrides: HashMap<String, DelegateAgentConfig>,
        provider_runtime_options: providers::ProviderRuntimeOptions,
        fallback_credential: Option<String>,
        default_provider: String,
        default_model: String,
        parent_tools: Arc<Vec<Arc<dyn Tool>>>,
        multimodal_config: crate::config::MultimodalConfig,
        enable_streaming: bool,
    ) -> Self {
        Self {
            max_ping_pong,
            role_overrides,
            provider_runtime_options,
            fallback_credential,
            default_provider,
            default_model,
            parent_tools,
            multimodal_config,
            enable_streaming,
        }
    }
}

#[async_trait]
impl Tool for FactoryOrchestratorTool {
    fn name(&self) -> &str {
        "factory_build"
    }

    fn description(&self) -> &str {
        "Launch the multi-agent factory workflow to build a complete project from an idea. \
         Spawns specialized agents (Business Analyst, UI/UX Designer, Developer, Tester, DevOps) \
         that collaborate through phased execution: analysis → parallel build → integration testing → deployment."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "idea": {
                    "type": "string",
                    "minLength": 1,
                    "description": "The project idea or description to build"
                },
                "project_type": {
                    "type": "string",
                    "enum": ["web", "api", "cli", "library", "mobile"],
                    "description": "Optional project type hint for the factory agents"
                }
            },
            "required": ["idea"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let idea = args
            .get("idea")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .ok_or_else(|| anyhow::anyhow!("Missing 'idea' parameter"))?;

        if idea.is_empty() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("'idea' parameter must not be empty".into()),
                error_hint: None,
            });
        }

        let project_type = args
            .get("project_type")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let full_idea = if project_type.is_empty() {
            idea.to_string()
        } else {
            format!("[Project type: {project_type}]\n\n{idea}")
        };

        let mut workflow = FactoryWorkflow::new(
            full_idea,
            self.max_ping_pong,
            self.role_overrides.clone(),
            self.provider_runtime_options.clone(),
            self.fallback_credential.clone(),
            self.default_provider.clone(),
            self.default_model.clone(),
            self.parent_tools.clone(),
            self.multimodal_config.clone(),
            self.enable_streaming,
        );

        match workflow.run().await {
            Ok(summary) => Ok(ToolResult {
                success: true,
                output: summary,
                error: None,
                error_hint: None,
            }),
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Factory build failed: {e}")),
                error_hint: Some(
                    "Check provider configuration and ensure factory agents \
                     have valid provider/model settings."
                        .into(),
                ),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_name_and_schema() {
        let tool = FactoryOrchestratorTool::new(
            5,
            HashMap::new(),
            providers::ProviderRuntimeOptions::default(),
            None,
            "openrouter".into(),
            "test-model".into(),
            Arc::new(Vec::new()),
            crate::config::MultimodalConfig::default(),
            true,
        );

        assert_eq!(tool.name(), "factory_build");
        let schema = tool.parameters_schema();
        assert!(schema["properties"]["idea"].is_object());
        assert!(schema["properties"]["project_type"].is_object());
        let required = schema["required"].as_array().unwrap();
        assert!(required.contains(&json!("idea")));
    }

    #[test]
    fn description_not_empty() {
        let tool = FactoryOrchestratorTool::new(
            5,
            HashMap::new(),
            providers::ProviderRuntimeOptions::default(),
            None,
            "openrouter".into(),
            "test-model".into(),
            Arc::new(Vec::new()),
            crate::config::MultimodalConfig::default(),
            true,
        );

        assert!(!tool.description().is_empty());
    }

    #[tokio::test]
    async fn empty_idea_rejected() {
        let tool = FactoryOrchestratorTool::new(
            5,
            HashMap::new(),
            providers::ProviderRuntimeOptions::default(),
            None,
            "openrouter".into(),
            "test-model".into(),
            Arc::new(Vec::new()),
            crate::config::MultimodalConfig::default(),
            true,
        );

        let result = tool.execute(json!({"idea": "  "})).await.unwrap();
        assert!(!result.success);
        assert!(result.error.unwrap().contains("must not be empty"));
    }

    #[tokio::test]
    async fn missing_idea_is_error() {
        let tool = FactoryOrchestratorTool::new(
            5,
            HashMap::new(),
            providers::ProviderRuntimeOptions::default(),
            None,
            "openrouter".into(),
            "test-model".into(),
            Arc::new(Vec::new()),
            crate::config::MultimodalConfig::default(),
            true,
        );

        let result = tool.execute(json!({})).await;
        assert!(result.is_err());
    }
}
