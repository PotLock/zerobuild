//! Model advisor — recommends optimal models based on task type.
//!
//! This module analyzes user requests and recommends appropriate models
//! for different tasks (coding, web dev, cron jobs, reasoning, etc.).
//! It integrates with the model routing system to persist user preferences.

use crate::config::{ClassificationRule, Config, ModelRouteConfig, QueryClassificationConfig};
use std::collections::HashMap;

/// Task type detected from user message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskType {
    /// General chat, Q&A
    Chat,
    /// Code writing, debugging, refactoring
    Coding,
    /// Web development (frontend/backend/fullstack)
    WebDev,
    /// System administration, cron jobs, shell scripts
    SystemAdmin,
    /// Complex reasoning, analysis, planning
    Reasoning,
    /// Creative writing, content generation
    Creative,
}

impl TaskType {
    /// Get a human-readable description.
    pub fn description(&self) -> &'static str {
        match self {
            TaskType::Chat => "general conversation",
            TaskType::Coding => "software development",
            TaskType::WebDev => "web development",
            TaskType::SystemAdmin => "system administration",
            TaskType::Reasoning => "complex reasoning",
            TaskType::Creative => "creative writing",
        }
    }

    /// Get the recommended model route hint.
    pub fn hint(&self) -> &'static str {
        match self {
            TaskType::Chat => "fast",
            TaskType::Coding => "code",
            TaskType::WebDev => "code",
            TaskType::SystemAdmin => "reasoning",
            TaskType::Reasoning => "reasoning",
            TaskType::Creative => "creative",
        }
    }

    /// Get explanation of why this model type is recommended.
    pub fn recommendation_reason(&self) -> &'static str {
        match self {
            TaskType::Chat => "Fast, cost-effective models work well for casual conversation",
            TaskType::Coding => "Code-specialized models excel at programming tasks, debugging, and understanding context",
            TaskType::WebDev => "Web development benefits from models strong in code generation and framework knowledge",
            TaskType::SystemAdmin => "System tasks require careful reasoning and safety awareness",
            TaskType::Reasoning => "Complex tasks benefit from models with strong reasoning capabilities",
            TaskType::Creative => "Creative tasks need models with good language fluency and imagination",
        }
    }
}

/// Detect task type from user message.
pub fn detect_task_type(message: &str) -> Option<TaskType> {
    let lower = message.to_lowercase();
    
    // Coding patterns
    let coding_keywords = [
        "code", "function", "class", "api", "debug", "error", "bug", "fix",
        "refactor", "implement", "library", "module", "package", "import",
        "typescript", "javascript", "python", "rust", "go", "java", "cpp",
        "algorithm", "data structure", "compile", "build", "test",
    ];
    
    // Web dev patterns
    let web_keywords = [
        "website", "web app", "frontend", "backend", "fullstack",
        "react", "vue", "angular", "svelte", "nextjs", "nuxt",
        "html", "css", "dom", "browser", "responsive", "ui", "ux",
        "component", "page", "route", "api endpoint", "database",
        "deploy", "hosting", "vercel", "netlify", "server",
    ];
    
    // System admin patterns
    let sysadmin_keywords = [
        "cron", "schedule", "systemd", "service", "daemon",
        "shell script", "bash", "zsh", "automation", "backup",
        "monitoring", "log", "server setup", "install", "configure",
        "nginx", "apache", "docker", "container", "kubernetes",
    ];
    
    // Reasoning patterns
    let reasoning_keywords = [
        "analyze", "explain", "why", "how does", "compare", "evaluate",
        "architecture", "design pattern", "strategy", "planning",
        "complex", "performance", "optimize", "trade-off", "decision",
    ];
    
    // Check for web dev first (more specific than coding)
    if web_keywords.iter().any(|kw| lower.contains(kw)) {
        return Some(TaskType::WebDev);
    }
    
    // Check for system admin
    if sysadmin_keywords.iter().any(|kw| lower.contains(kw)) {
        return Some(TaskType::SystemAdmin);
    }
    
    // Check for coding
    if coding_keywords.iter().any(|kw| lower.contains(kw)) {
        return Some(TaskType::Coding);
    }
    
    // Check for reasoning
    if reasoning_keywords.iter().any(|kw| lower.contains(kw)) {
        return Some(TaskType::Reasoning);
    }
    
    // Creative patterns
    let creative_keywords = [
        "write", "story", "poem", "email", "letter", "content",
        "blog post", "article", "creative", "marketing", "copy",
    ];
    if creative_keywords.iter().any(|kw| lower.contains(kw)) {
        return Some(TaskType::Creative);
    }
    
    None
}

/// Get recommended models for a task type.
/// Returns vec of (provider, model, description) tuples.
pub fn get_recommended_models(task: TaskType) -> Vec<(&'static str, &'static str, &'static str)> {
    match task {
        TaskType::Coding | TaskType::WebDev => vec![
            ("anthropic", "claude-sonnet-4-20250514", "Claude Sonnet 4 — Excellent at coding, fast"),
            ("openrouter", "anthropic/claude-sonnet-4", "Claude Sonnet 4 via OpenRouter"),
            ("openrouter", "openai/gpt-4o", "GPT-4o — Strong coding capabilities"),
            ("kimi", "kimi-k2", "Kimi K2 — Specialized for Chinese/English code"),
        ],
        TaskType::SystemAdmin => vec![
            ("anthropic", "claude-sonnet-4-20250514", "Claude Sonnet 4 — Careful with system commands"),
            ("openrouter", "anthropic/claude-opus-4", "Claude Opus 4 — Best reasoning for complex systems"),
        ],
        TaskType::Reasoning => vec![
            ("anthropic", "claude-opus-4-20250514", "Claude Opus 4 — Best reasoning capabilities"),
            ("openrouter", "openai/o1-preview", "o1 — Strong reasoning and planning"),
            ("openrouter", "anthropic/claude-sonnet-4", "Claude Sonnet 4 — Good balance"),
        ],
        TaskType::Creative => vec![
            ("anthropic", "claude-sonnet-4-20250514", "Claude Sonnet 4 — Natural writing style"),
            ("openrouter", "openai/gpt-4o", "GPT-4o — Creative and fluent"),
        ],
        TaskType::Chat => vec![
            ("openrouter", "anthropic/claude-haiku", "Claude Haiku — Fast and cheap"),
            ("openrouter", "google/gemini-flash", "Gemini Flash — Very fast responses"),
        ],
    }
}

/// Generate recommendation message for user.
pub fn generate_recommendation(
    current_model: &str,
    task: TaskType,
    has_routing: bool,
) -> String {
    let reason = task.recommendation_reason();
    let hint = task.hint();
    
    let mut msg = format!(
        "💡 **Model Recommendation**\n\n\
        Detected task type: **{}**\n\
        Current model: `{}`\n\n\
        Recommendation: {}\n\n",
        task.description(),
        current_model,
        reason
    );
    
    if has_routing {
        msg.push_str(&format!(
            "You can use `model_routing_config` to set up automatic routing for '{}' tasks.\n\n",
            task.description()
        ));
    } else {
        msg.push_str(&format!(
            "Consider switching to a model optimized for {}.\n\
            Use `model_routing_config` to set up automatic routing for '{}' tasks.\n\n",
            task.description(),
            hint
        ));
    }
    
    msg.push_str("Recommended models:\n");
    for (i, (provider, model, desc)) in get_recommended_models(task).iter().enumerate() {
        msg.push_str(&format!("  {}. {}: `{}` — {}\n", i + 1, provider, model, desc));
    }
    
    msg.push_str("\nWant to switch? Just say 'switch to model N' or 'set up routing for code tasks'");
    
    msg
}

/// Check if current model is already good for the task.
pub fn is_model_suitable(current_model: &str, task: TaskType) -> bool {
    let model_lower = current_model.to_lowercase();
    
    match task {
        TaskType::Coding | TaskType::WebDev => {
            model_lower.contains("sonnet") || 
            model_lower.contains("gpt-4") || 
            model_lower.contains("code") ||
            model_lower.contains("kimi")
        }
        TaskType::Reasoning | TaskType::SystemAdmin => {
            model_lower.contains("opus") || 
            model_lower.contains("o1") || 
            model_lower.contains("reasoning") ||
            model_lower.contains("sonnet")
        }
        TaskType::Creative => {
            model_lower.contains("gpt-4") || 
            model_lower.contains("claude") ||
            model_lower.contains("creative")
        }
        TaskType::Chat => true, // Any model works for chat
    }
}

/// Create default classification rules for task-based routing.
pub fn default_classification_rules() -> Vec<ClassificationRule> {
    vec![
        ClassificationRule {
            hint: "code".to_string(),
            keywords: vec![
                "code".to_string(), "programming".to_string(), "debug".to_string(), 
                "function".to_string(), "api".to_string(), 
                "javascript".to_string(), "typescript".to_string(), 
                "python".to_string(), "rust".to_string(), "react".to_string(), "vue".to_string()
            ],
            patterns: vec!["```".to_string(), "fn ".to_string(), "class ".to_string()],
            min_length: None,
            max_length: None,
            priority: 10,
        },
        ClassificationRule {
            hint: "reasoning".to_string(),
            keywords: vec![
                "analyze".to_string(), "explain why".to_string(), 
                "how does".to_string(), "architecture".to_string(),
                "design pattern".to_string(), "optimize".to_string(), "performance".to_string()
            ],
            patterns: vec![],
            min_length: Some(50),
            max_length: None,
            priority: 8,
        },
        ClassificationRule {
            hint: "fast".to_string(),
            keywords: vec![
                "hello".to_string(), "hi".to_string(), "thanks".to_string(), 
                "bye".to_string(), "quick question".to_string()
            ],
            patterns: vec![],
            min_length: None,
            max_length: Some(50),
            priority: 5,
        },
    ]
}

/// Create default model routes for common tasks.
pub fn default_model_routes() -> Vec<ModelRouteConfig> {
    vec![
        ModelRouteConfig {
            hint: "code".to_string(),
            provider: "anthropic".to_string(),
            model: "claude-sonnet-4-20250514".to_string(),
            api_key: None,
        },
        ModelRouteConfig {
            hint: "reasoning".to_string(),
            provider: "openrouter".to_string(),
            model: "anthropic/claude-opus-4".to_string(),
            api_key: None,
        },
        ModelRouteConfig {
            hint: "fast".to_string(),
            provider: "openrouter".to_string(),
            model: "anthropic/claude-haiku".to_string(),
            api_key: None,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_coding_task() {
        assert_eq!(
            detect_task_type("Write a function to sort an array"),
            Some(TaskType::Coding)
        );
        assert_eq!(
            detect_task_type("Debug this Python error"),
            Some(TaskType::Coding)
        );
    }

    #[test]
    fn detect_web_dev_task() {
        assert_eq!(
            detect_task_type("Create a React component"),
            Some(TaskType::WebDev)
        );
        assert_eq!(
            detect_task_type("Build a website with Next.js"),
            Some(TaskType::WebDev)
        );
    }

    #[test]
    fn detect_system_admin_task() {
        assert_eq!(
            detect_task_type("Set up a cron job to backup files"),
            Some(TaskType::SystemAdmin)
        );
        assert_eq!(
            detect_task_type("Write a systemd service"),
            Some(TaskType::SystemAdmin)
        );
    }

    #[test]
    fn detect_reasoning_task() {
        assert_eq!(
            detect_task_type("Explain why this architecture pattern is better"),
            Some(TaskType::Reasoning)
        );
    }

    #[test]
    fn chat_not_detected() {
        // Generic chat shouldn't be detected as a specific task
        assert_eq!(detect_task_type("Hello, how are you?"), None);
        assert_eq!(detect_task_type("Thanks for the help!"), None);
    }
}
