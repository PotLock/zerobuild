//! Product Advisor Tool — Generate improvement suggestions for a project.
//!
//! This tool analyzes the current project context and generates structured
//! improvement recommendations. It does NOT make its own LLM calls; instead
//! it formats a structured context block that the main agent LLM uses to
//! generate recommendations.

use crate::security::SecurityPolicy;
use crate::tools::traits::{Tool, ToolResult};
use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;

/// Focus area for improvement suggestions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusArea {
    Ux,
    Performance,
    Features,
    Security,
    Monetization,
    All,
}

impl FocusArea {
    fn from_str(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "ux" | "user_experience" | "user experience" => Some(Self::Ux),
            "performance" | "speed" => Some(Self::Performance),
            "features" | "feature" => Some(Self::Features),
            "security" | "secure" => Some(Self::Security),
            "monetization" | "monetize" | "revenue" => Some(Self::Monetization),
            "all" | "" => Some(Self::All),
            _ => None,
        }
    }

    fn as_str(&self) -> &'static str {
        match self {
            Self::Ux => "ux",
            Self::Performance => "performance",
            Self::Features => "features",
            Self::Security => "security",
            Self::Monetization => "monetization",
            Self::All => "all",
        }
    }
}

/// Product Advisor Tool — generates improvement suggestions
pub struct ProductAdvisorTool {
    security: Arc<SecurityPolicy>,
}

impl ProductAdvisorTool {
    /// Create a new ProductAdvisorTool
    pub fn new(security: Arc<SecurityPolicy>) -> Self {
        Self { security }
    }

    /// Format the output according to the standard template
    fn format_suggestions(&self, project_name: &str, focus: FocusArea) -> String {
        let focus_text = match focus {
            FocusArea::Ux => "User Experience",
            FocusArea::Performance => "Performance",
            FocusArea::Features => "Features",
            FocusArea::Security => "Security",
            FocusArea::Monetization => "Monetization",
            FocusArea::All => "All Areas",
        };

        format!(
            "💡 IMPROVEMENT SUGGESTIONS — {}\n\
             ═══════════════════════════════════════════\n\
             Focus: {}\n\n\
             🔴 HIGH PRIORITY:\n\
             {{high_priority_suggestions}}\n\n\
             🟡 MEDIUM PRIORITY:\n\
             {{medium_priority_suggestions}}\n\n\
             🔵 LONG-TERM:\n\
             {{long_term_suggestions}}\n\n\
             Which improvement would you like to start with?",
            project_name, focus_text
        )
    }

    /// Build the prompt context for the LLM to generate suggestions
    fn build_prompt_context(
        &self,
        project_name: &str,
        description: &str,
        current_features: &[String],
        target_users: Option<&str>,
        focus: FocusArea,
    ) -> String {
        let features_list = if current_features.is_empty() {
            "None specified".to_string()
        } else {
            current_features
                .iter()
                .map(|f| format!("  • {f}"))
                .collect::<Vec<_>>()
                .join("\n")
        };

        let target_users_text = target_users
            .map(|u| format!("\nTarget Users: {u}"))
            .unwrap_or_default();

        let focus_guidance = match focus {
            FocusArea::Ux => "Focus on user interface improvements, usability enhancements, and design refinements.",
            FocusArea::Performance => "Focus on speed optimizations, loading times, and resource efficiency.",
            FocusArea::Features => "Focus on new functionality, user requests, and product capabilities.",
            FocusArea::Security => "Focus on data protection, authentication, and vulnerability prevention.",
            FocusArea::Monetization => "Focus on revenue generation, pricing strategies, and business models.",
            FocusArea::All => "Provide balanced recommendations across UX, performance, features, security, and monetization.",
        };

        format!(
            "Analyze the following project and generate improvement suggestions.\n\n\
             PROJECT CONTEXT\n\
             ═════════════════\n\
             Name: {project_name}\n\
             Description: {description}{target_users_text}\n\n\
             Current Features:\n{features_list}\n\n\
             {focus_guidance}\n\n\
             INSTRUCTIONS\n\
             ═════════════\n\
             Provide 3-6 specific, actionable recommendations organized by priority:\n\n\
             HIGH PRIORITY (1-2 items):\n\
             - Critical improvements that should be done immediately\n\
             - High impact, often low effort\n\n\
             MEDIUM PRIORITY (1-2 items):\n\
             - Valuable enhancements for the near term\n\
             - Moderate effort but clear benefit\n\n\
             LONG-TERM (1-2 items):\n\
             - Strategic improvements for future iterations\n\
             - May require more planning or resources\n\n\
             Format each suggestion as a single bullet point (•). Be specific and concrete."
        )
    }
}

#[async_trait]
impl Tool for ProductAdvisorTool {
    fn name(&self) -> &str {
        "product_advisor"
    }

    fn description(&self) -> &str {
        "Generate improvement suggestions for a web project. Analyzes the current \
         project context and returns structured recommendations. This tool formats \
         the context; the LLM generates the actual suggestions based on the prompt.\n\
         Use this after completing a build or deployment to suggest next steps."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "project_name": {
                    "type": "string",
                    "description": "Name of the project being analyzed"
                },
                "description": {
                    "type": "string",
                    "description": "Brief description of the project and its purpose"
                },
                "current_features": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "List of features currently implemented"
                },
                "target_users": {
                    "type": "string",
                    "description": "Description of the target user audience (optional)"
                },
                "focus": {
                    "type": "string",
                    "enum": ["ux", "performance", "features", "security", "monetization", "all"],
                    "description": "Area to focus recommendations on (default: all)"
                }
            },
            "required": ["project_name", "description", "current_features"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        // Extract required parameters
        let project_name = args
            .get("project_name")
            .and_then(|v| v.as_str())
            .unwrap_or("Untitled Project");

        let description = args
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let current_features = args
            .get("current_features")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let target_users = args.get("target_users").and_then(|v| v.as_str());

        let focus = args
            .get("focus")
            .and_then(|v| v.as_str())
            .and_then(FocusArea::from_str)
            .unwrap_or(FocusArea::All);

        // Build the prompt context that the LLM will use
        let prompt_context = self.build_prompt_context(
            project_name,
            description,
            &current_features,
            target_users,
            focus,
        );

        // Format the template output
        let template = self.format_suggestions(project_name, focus);

        // Combine prompt context with template for LLM processing
        let output = format!(
            "{template}\n\n\
             ---\n\n\
             [AGENT INSTRUCTIONS: Use the following context to fill in the suggestions above]\n\n\
             {prompt_context}"
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
    use crate::security::SecurityPolicy;

    fn default_tool() -> ProductAdvisorTool {
        ProductAdvisorTool::new(Arc::new(SecurityPolicy::default()))
    }

    #[test]
    fn tool_name_and_schema() {
        let tool = default_tool();
        assert_eq!(tool.name(), "product_advisor");

        let schema = tool.parameters_schema();
        assert!(schema["properties"]["project_name"].is_object());
        assert!(schema["properties"]["description"].is_object());
        assert!(schema["properties"]["current_features"].is_object());
        assert!(schema["properties"]["target_users"].is_object());
        assert!(schema["properties"]["focus"].is_object());
    }

    #[tokio::test]
    async fn execute_with_minimal_params() {
        let tool = default_tool();

        let r = tool
            .execute(json!({
                "project_name": "Test Project",
                "description": "A test project",
                "current_features": ["Home page", "Contact form"]
            }))
            .await
            .unwrap();

        assert!(r.success);
        assert!(r.output.contains("IMPROVEMENT SUGGESTIONS"));
        assert!(r.output.contains("Test Project"));
        assert!(r.output.contains("HIGH PRIORITY"));
        assert!(r.output.contains("MEDIUM PRIORITY"));
        assert!(r.output.contains("LONG-TERM"));
        assert!(r
            .output
            .contains("Which improvement would you like to start with?"));
    }

    #[tokio::test]
    async fn execute_with_focus() {
        let tool = default_tool();

        let r = tool
            .execute(json!({
                "project_name": "E-commerce Site",
                "description": "Online store",
                "current_features": ["Product listing", "Cart"],
                "focus": "security"
            }))
            .await
            .unwrap();

        assert!(r.success);
        assert!(r.output.contains("Focus: Security"));
    }

    #[tokio::test]
    async fn execute_with_target_users() {
        let tool = default_tool();

        let r = tool
            .execute(json!({
                "project_name": "Learning Platform",
                "description": "Online courses",
                "current_features": ["Video player", "Quizzes"],
                "target_users": "College students aged 18-25"
            }))
            .await
            .unwrap();

        assert!(r.success);
        assert!(r.output.contains("College students aged 18-25"));
    }

    #[tokio::test]
    async fn execute_with_empty_features() {
        let tool = default_tool();

        let r = tool
            .execute(json!({
                "project_name": "New Startup",
                "description": "Just getting started",
                "current_features": []
            }))
            .await
            .unwrap();

        assert!(r.success);
        assert!(r.output.contains("IMPROVEMENT SUGGESTIONS"));
    }

    #[test]
    fn focus_area_from_str() {
        assert_eq!(FocusArea::from_str("ux"), Some(FocusArea::Ux));
        assert_eq!(
            FocusArea::from_str("performance"),
            Some(FocusArea::Performance)
        );
        assert_eq!(FocusArea::from_str("features"), Some(FocusArea::Features));
        assert_eq!(FocusArea::from_str("security"), Some(FocusArea::Security));
        assert_eq!(
            FocusArea::from_str("monetization"),
            Some(FocusArea::Monetization)
        );
        assert_eq!(FocusArea::from_str("all"), Some(FocusArea::All));
        assert_eq!(FocusArea::from_str(""), Some(FocusArea::All));
        assert_eq!(FocusArea::from_str("invalid"), None);
    }

    #[test]
    fn focus_area_as_str() {
        assert_eq!(FocusArea::Ux.as_str(), "ux");
        assert_eq!(FocusArea::Performance.as_str(), "performance");
        assert_eq!(FocusArea::Features.as_str(), "features");
        assert_eq!(FocusArea::Security.as_str(), "security");
        assert_eq!(FocusArea::Monetization.as_str(), "monetization");
        assert_eq!(FocusArea::All.as_str(), "all");
    }
}
