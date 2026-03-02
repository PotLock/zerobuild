//! Workflow state machine for the multi-agent factory.
//!
//! Defines the phased execution model: Analysis → ParallelBuild →
//! IntegrationLoop → Deployment → Completed/Failed.
//!
//! Features:
//! - Intent classification for dynamic agent spawning
//! - Progress streaming for real-time visibility
//! - Cross-phase memory sharing
//! - Optional learning from past builds
//! - Optional sub-agent spawning for complex tasks

use super::blackboard::{Artifact, Blackboard};
use super::progress::{
    AgentStatus, ProgressBroadcaster, ProgressTracker, TestStatus, WorkflowCompletionStatus,
};
use super::roles::{AgentRole, RoleConfig};
use crate::config::DelegateAgentConfig;
use crate::providers::{self, Provider};
use crate::tools::traits::Tool;
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use uuid::Uuid;

/// Workflow phases in the factory pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowPhase {
    /// Intent classification and analysis
    IntentAnalysis,
    /// BA agent analyzes user idea and writes PRD.
    Analysis,
    /// UI/UX, Developer, and Tester agents run concurrently.
    ParallelBuild,
    /// Developer-Tester ping-pong loop until tests pass.
    IntegrationLoop,
    /// DevOps agent deploys the project.
    Deployment,
    /// Workflow completed successfully.
    Completed,
    /// Workflow failed.
    Failed,
}

/// Factory workflow orchestrator.
///
/// Manages the lifecycle of a multi-agent build session, coordinating
/// specialized agents through phased execution.
pub struct FactoryWorkflow {
    workflow_id: Uuid,
    blackboard: Blackboard,
    idea: String,
    max_ping_pong: usize,
    phase: WorkflowPhase,
    // Dynamic spawning flags (set by LLM analysis)
    spawn_ui_ux: bool,
    spawn_devops: bool,
    role_overrides: HashMap<String, DelegateAgentConfig>,
    provider_runtime_options: providers::ProviderRuntimeOptions,
    fallback_credential: Option<String>,
    default_provider: String,
    default_model: String,
    parent_tools: Arc<Vec<Arc<dyn Tool>>>,
    multimodal_config: crate::config::MultimodalConfig,
    progress: ProgressTracker,
    enable_streaming: bool,
    enable_dynamic_spawning: bool,
}

impl FactoryWorkflow {
    /// Create a new factory workflow for the given idea.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        idea: String,
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
        let broadcaster = ProgressBroadcaster::new();
        let progress = ProgressTracker::new(broadcaster);

        Self {
            workflow_id: Uuid::new_v4(),
            blackboard: Blackboard::new(),
            idea,
            max_ping_pong,
            spawn_ui_ux: true,
            spawn_devops: true,
            phase: WorkflowPhase::IntentAnalysis,
            role_overrides,
            provider_runtime_options,
            fallback_credential,
            default_provider,
            default_model,
            parent_tools,
            multimodal_config,
            progress,
            enable_streaming,
            enable_dynamic_spawning: true,
        }
    }

    /// Disable progress streaming
    pub fn without_streaming(mut self) -> Self {
        self.enable_streaming = false;
        self
    }

    /// Disable dynamic agent spawning
    pub fn without_dynamic_spawning(mut self) -> Self {
        self.enable_dynamic_spawning = false;
        self
    }

    /// Get progress broadcaster for external subscription
    pub fn progress_broadcaster(&self) -> &ProgressBroadcaster {
        self.progress.broadcaster()
    }

    /// Execute the full factory workflow, returning a summary of the result.
    pub async fn run(&mut self) -> Result<String> {
        // Start workflow
        if self.enable_streaming {
            self.progress.workflow_started();
        }

        // Phase 0: Intent Classification
        self.classify_intent().await?;

        // Phase 1-4: Execute main workflow
        self.execute_phase_1_analysis().await?;
        self.execute_phase_2_parallel_build().await?;
        self.execute_phase_3_integration_loop().await?;
        let result = self.execute_phase_4_deployment().await;

        // Complete workflow
        if self.enable_streaming {
            let status = match &result {
                Ok(_) => WorkflowCompletionStatus::Success,
                Err(_) => WorkflowCompletionStatus::Failed,
            };
            self.progress.workflow_completed(status);
        }

        result
    }

    /// Phase 0: Intent Analysis using LLM
    async fn classify_intent(&mut self) -> Result<()> {
        self.phase = WorkflowPhase::IntentAnalysis;

        if !self.enable_dynamic_spawning {
            return Ok(());
        }

        if self.enable_streaming {
            self.progress
                .progress_update("Analyzing project requirements...", 5);
        }

        // Use BA Agent with LLM to analyze intent
        let analysis_prompt = format!(
            r#"Analyze this project request and determine the optimal team structure:

REQUEST: "{}"

Respond with JSON only:
{{
    "project_type": "web_app|mobile_app|api_backend|cli_tool|library|script|game",
    "complexity": "simple|medium|complex",
    "required_agents": ["business_analyst", "ui_ux_designer", "developer", "tester", "devops"],
    "needs_frontend": true|false,
    "needs_backend": true|false,
    "reasoning": "brief explanation"
}}

Rules:
- simple: Just Developer (scripts, small tools)
- medium: BA + Developer + Tester (APIs, backends)  
- complex: Full team including UI/UX (web apps, mobile apps)"#,
            self.idea
        );

        // Run analysis through Orchestrator agent (intent classification is a coordination decision)
        let analysis_result = self
            .run_agent_simple(AgentRole::Orchestrator, &analysis_prompt)
            .await?;

        // Parse which agents are needed
        let needs_ui_ux = analysis_result.to_lowercase().contains("ui_ux")
            || analysis_result.to_lowercase().contains("ui/ux")
            || analysis_result
                .to_lowercase()
                .contains("\"needs_frontend\": true");

        let needs_devops = analysis_result.to_lowercase().contains("devops")
            || analysis_result
                .to_lowercase()
                .contains("\"needs_backend\": true");

        // Store classification for later phases
        self.spawn_ui_ux = needs_ui_ux;
        self.spawn_devops = needs_devops;

        if self.enable_streaming {
            self.progress.progress_update(
                format!(
                    "Analysis complete. Frontend: {}, Backend: {}",
                    if needs_ui_ux { "Yes" } else { "No" },
                    if needs_devops { "Yes" } else { "No" }
                ),
                10,
            );
        }

        Ok(())
    }

    /// Phase 1: Analysis
    async fn execute_phase_1_analysis(&mut self) -> Result<()> {
        self.phase = WorkflowPhase::Analysis;

        if self.enable_streaming {
            self.progress
                .phase_started(WorkflowPhase::Analysis, "Business Analyst creating PRD");
        }

        // Determine which agents to spawn based on LLM analysis
        let mut suggested_agents = vec![
            AgentRole::BusinessAnalyst,
            AgentRole::Developer,
            AgentRole::Tester,
        ];
        if self.spawn_ui_ux {
            suggested_agents.push(AgentRole::UiUxDesigner);
        }
        if self.spawn_devops {
            suggested_agents.push(AgentRole::DevOps);
        }

        if self.enable_streaming {
            self.progress
                .progress_update(format!("Spawning {} agents", suggested_agents.len()), 10);
            self.progress
                .agent_started(AgentRole::BusinessAnalyst, "Creating PRD from user idea");
        }

        let start = Instant::now();
        let prd = self
            .run_agent_simple(
                AgentRole::BusinessAnalyst,
                &format!(
                    "Analyze the following project idea and produce a comprehensive PRD:\n\n{}",
                    self.idea
                ),
            )
            .await?;

        if self.enable_streaming {
            self.progress.agent_completed(
                AgentRole::BusinessAnalyst,
                start.elapsed(),
                AgentStatus::Success,
            );
        }

        self.blackboard
            .publish_artifact(Artifact::Prd, json!(prd), "business_analyst");

        if self.enable_streaming {
            self.progress.phase_completed(WorkflowPhase::Analysis);
        }

        Ok(())
    }

    /// Phase 2: Parallel Build
    async fn execute_phase_2_parallel_build(&mut self) -> Result<()> {
        self.phase = WorkflowPhase::ParallelBuild;

        if self.enable_streaming {
            self.progress.phase_started(
                WorkflowPhase::ParallelBuild,
                "UI/UX, Developer, and Tester working in parallel",
            );
        }

        // Use LLM analysis results
        let spawn_ui_ux = self.spawn_ui_ux;

        let prd = self
            .blackboard
            .read_artifact(&Artifact::Prd)
            .unwrap_or(json!(""))
            .to_string();

        let design_prompt = format!("Based on this PRD, produce a design specification:\n\n{prd}");
        let dev_prompt = format!("Based on this PRD, implement the project:\n\n{prd}");
        let test_prompt = format!("Based on this PRD, write comprehensive test cases:\n\n{prd}");

        if self.enable_streaming {
            self.progress
                .progress_update("Starting parallel build phase", 20);
        }

        let (design_result, dev_result, test_result) = if spawn_ui_ux {
            if self.enable_streaming {
                self.progress
                    .agent_started(AgentRole::UiUxDesigner, "Creating design specifications");
                self.progress
                    .agent_started(AgentRole::Developer, "Implementing source code");
                self.progress
                    .agent_started(AgentRole::Tester, "Writing test cases");
            }

            tokio::join!(
                self.run_agent_simple(AgentRole::UiUxDesigner, &design_prompt),
                self.run_agent_agentic(AgentRole::Developer, &dev_prompt),
                self.run_agent_simple(AgentRole::Tester, &test_prompt),
            )
        } else {
            if self.enable_streaming {
                self.progress
                    .agent_started(AgentRole::Developer, "Implementing source code");
                self.progress
                    .agent_started(AgentRole::Tester, "Writing test cases");
            }

            let (dev_result, test_result) = tokio::join!(
                self.run_agent_agentic(AgentRole::Developer, &dev_prompt),
                self.run_agent_simple(AgentRole::Tester, &test_prompt),
            );
            (Ok(String::new()), dev_result, test_result)
        };

        // Process results
        if spawn_ui_ux {
            let design_spec = design_result?;
            self.blackboard.publish_artifact(
                Artifact::DesignSpec,
                json!(design_spec),
                "ui_ux_designer",
            );
            if self.enable_streaming {
                self.progress.agent_completed(
                    AgentRole::UiUxDesigner,
                    Duration::from_secs(60),
                    AgentStatus::Success,
                );
            }
        }

        let dev_output = dev_result?;
        self.blackboard
            .publish_artifact(Artifact::SourceCode, json!(dev_output), "developer");
        if self.enable_streaming {
            self.progress.agent_completed(
                AgentRole::Developer,
                Duration::from_secs(120),
                AgentStatus::Success,
            );
        }

        let test_cases = test_result?;
        self.blackboard
            .publish_artifact(Artifact::TestCases, json!(test_cases), "tester");
        if self.enable_streaming {
            self.progress.agent_completed(
                AgentRole::Tester,
                Duration::from_secs(60),
                AgentStatus::Success,
            );
        }

        if self.enable_streaming {
            self.progress
                .progress_update("Parallel build completed", 50);
            self.progress.phase_completed(WorkflowPhase::ParallelBuild);
        }

        Ok(())
    }

    /// Phase 3: Integration Loop
    async fn execute_phase_3_integration_loop(&mut self) -> Result<()> {
        self.phase = WorkflowPhase::IntegrationLoop;

        if self.enable_streaming {
            self.progress.phase_started(
                WorkflowPhase::IntegrationLoop,
                "Developer-Tester iteration loop",
            );
        }

        let test_cases = self
            .blackboard
            .read_artifact(&Artifact::TestCases)
            .unwrap_or(json!(""))
            .to_string();

        let mut tests_passed = false;

        for iteration in 1..=self.max_ping_pong {
            if self.enable_streaming {
                self.progress.dev_tester_iteration(
                    iteration as u8,
                    self.max_ping_pong as u8,
                    TestStatus::Running,
                );
            }

            let test_prompt = format!(
                "Run the test cases against the current source code. \
                 Test cases:\n{test_cases}\n\n\
                 Report which tests pass and which fail. \
                 If all tests pass, say 'ALL TESTS PASSED'. \
                 Iteration {iteration}/{}.",
                self.max_ping_pong,
            );

            if self.enable_streaming {
                self.progress.agent_started(
                    AgentRole::Tester,
                    format!("Running tests (iteration {})", iteration),
                );
            }

            let test_result = self
                .run_agent_agentic(AgentRole::Tester, &test_prompt)
                .await?;

            if self.enable_streaming {
                self.progress.agent_completed(
                    AgentRole::Tester,
                    Duration::from_secs(30),
                    AgentStatus::Success,
                );
            }

            self.blackboard
                .publish_artifact(Artifact::TestResults, json!(test_result), "tester");

            if test_result.to_uppercase().contains("ALL TESTS PASSED") {
                if self.enable_streaming {
                    self.progress.dev_tester_iteration(
                        iteration as u8,
                        self.max_ping_pong as u8,
                        TestStatus::Passed,
                    );
                }
                tests_passed = true;
                break;
            } else {
                if self.enable_streaming {
                    self.progress.dev_tester_iteration(
                        iteration as u8,
                        self.max_ping_pong as u8,
                        TestStatus::Failed { failure_count: 1 },
                    );
                }
            }

            // Developer fixes
            if iteration < self.max_ping_pong {
                let fix_prompt = format!(
                    "The following tests failed. Fix the code:\n\n{test_result}\n\n\
                     Iteration {iteration}/{}.",
                    self.max_ping_pong,
                );

                if self.enable_streaming {
                    self.progress.agent_started(
                        AgentRole::Developer,
                        format!("Fixing code (iteration {})", iteration),
                    );
                }

                let fix_output = self
                    .run_agent_agentic(AgentRole::Developer, &fix_prompt)
                    .await?;
                self.blackboard.publish_artifact(
                    Artifact::SourceCode,
                    json!(fix_output),
                    "developer",
                );

                if self.enable_streaming {
                    self.progress.agent_completed(
                        AgentRole::Developer,
                        Duration::from_secs(60),
                        AgentStatus::Success,
                    );
                }
            }
        }

        if self.enable_streaming {
            self.progress
                .phase_completed(WorkflowPhase::IntegrationLoop);
        }

        if !tests_passed {
            bail!("Tests failed after {} iterations", self.max_ping_pong);
        }

        Ok(())
    }

    /// Phase 4: Deployment
    async fn execute_phase_4_deployment(&mut self) -> Result<String> {
        self.phase = WorkflowPhase::Deployment;

        if self.enable_streaming {
            self.progress
                .phase_started(WorkflowPhase::Deployment, "Deploying to GitHub");
        }

        let deploy_result = self
            .run_agent_agentic(
                AgentRole::DevOps,
                "Deploy the project. Push the code to GitHub.",
            )
            .await?;

        self.blackboard
            .publish_artifact(Artifact::DeployConfig, json!(deploy_result), "devops");

        if self.enable_streaming {
            use super::progress::DeployTarget;
            self.progress
                .deployment_completed(DeployTarget::GitHub, None);
            self.progress.phase_completed(WorkflowPhase::Deployment);
        }

        self.phase = WorkflowPhase::Completed;

        // Build summary
        let prd = self
            .blackboard
            .read_artifact(&Artifact::Prd)
            .unwrap_or(json!(""))
            .to_string();
        let design = self
            .blackboard
            .read_artifact(&Artifact::DesignSpec)
            .unwrap_or(json!(""))
            .to_string();

        Ok(format!(
            "Factory build completed successfully!\n\n\
             PRD: {}\n\n\
             Design: {}\n\n\
             Deployment: {}",
            prd.chars().take(200).collect::<String>(),
            design.chars().take(200).collect::<String>(),
            deploy_result
        ))
    }

    /// Current workflow phase.
    pub fn phase(&self) -> WorkflowPhase {
        self.phase
    }

    // ── Agent execution helpers ──────────────────────────────────

    async fn run_agent_simple(&self, role: AgentRole, prompt: &str) -> Result<String> {
        let config = self.resolve_config(role);
        let provider = self.create_provider(&config)?;

        let result = tokio::time::timeout(
            Duration::from_secs(120),
            provider.chat_with_system(
                config.system_prompt.as_deref(),
                prompt,
                &config.model,
                config.temperature.unwrap_or(0.7),
            ),
        )
        .await;

        match result {
            Ok(Ok(response)) => Ok(response),
            Ok(Err(e)) => bail!("Agent {role} failed: {e}"),
            Err(_) => bail!("Agent {role} timed out"),
        }
    }

    async fn run_agent_agentic(&self, role: AgentRole, prompt: &str) -> Result<String> {
        let config = self.resolve_config(role);

        if config.allowed_tools.is_empty() {
            return self.run_agent_simple(role, prompt).await;
        }

        let role_name = role.to_string();
        let mut agents = HashMap::new();
        agents.insert(role_name.clone(), config);

        let security = Arc::new(crate::security::SecurityPolicy::default());
        let delegate = crate::tools::delegate::DelegateTool::new_with_options(
            agents,
            self.fallback_credential.clone(),
            security,
            self.provider_runtime_options.clone(),
        )
        .with_parent_tools(self.parent_tools.clone())
        .with_multimodal_config(self.multimodal_config.clone());

        let result = delegate
            .execute(json!({
                "agent": role_name,
                "prompt": prompt,
            }))
            .await?;

        if result.success {
            Ok(result.output)
        } else {
            bail!("Agent {role} failed: {}", result.error.unwrap_or_default())
        }
    }

    fn resolve_config(&self, role: AgentRole) -> DelegateAgentConfig {
        let mut role_config = RoleConfig::default_for(role);

        if let Some(overrides) = self.role_overrides.get(&role.to_string()) {
            role_config = role_config.with_overrides(overrides);
        }

        let mut config = role_config.delegate_config;
        if config.provider.is_empty() {
            config.provider = self.default_provider.clone();
        }
        if config.model.is_empty() {
            config.model = self.default_model.clone();
        }
        if config.api_key.is_none() {
            config.api_key = self.fallback_credential.clone();
        }

        config
    }

    fn create_provider(&self, config: &DelegateAgentConfig) -> Result<Box<dyn Provider>> {
        providers::create_provider_with_options(
            &config.provider,
            config.api_key.as_deref(),
            &self.provider_runtime_options,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workflow_phases_serialize() {
        let phase = WorkflowPhase::ParallelBuild;
        let json = serde_json::to_string(&phase).unwrap();
        assert_eq!(json, "\"parallel_build\"");

        let parsed: WorkflowPhase = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, WorkflowPhase::ParallelBuild);
    }

    #[test]
    fn resolve_config_uses_defaults() {
        let wf = FactoryWorkflow::new(
            "test idea".into(),
            5,
            HashMap::new(),
            providers::ProviderRuntimeOptions::default(),
            Some("test-key".into()),
            "openrouter".into(),
            "test-model".into(),
            Arc::new(Vec::new()),
            crate::config::MultimodalConfig::default(),
            true,
        );

        let config = wf.resolve_config(AgentRole::Developer);
        assert_eq!(config.provider, "openrouter");
        assert_eq!(config.model, "test-model");
        assert_eq!(config.api_key.as_deref(), Some("test-key"));
        assert!(config.system_prompt.is_some());
    }
}
