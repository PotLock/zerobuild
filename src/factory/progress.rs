//! Progress Streaming System for ZeroBuild
//!
//! Provides real-time visibility into factory workflow execution.

use crate::factory::blackboard::Artifact;
use crate::factory::roles::AgentRole;
use crate::factory::workflow::WorkflowPhase;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::Duration;
use tokio::sync::broadcast;

/// Progress events emitted during factory execution
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FactoryProgressEvent {
    /// Workflow started
    WorkflowStarted {
        workflow_id: String,
        timestamp: chrono::DateTime<chrono::Utc>,
    },

    /// Phase transition
    PhaseStarted {
        phase: WorkflowPhase,
        timestamp: chrono::DateTime<chrono::Utc>,
        message: String,
    },

    /// Phase completed
    PhaseCompleted {
        phase: WorkflowPhase,
        timestamp: chrono::DateTime<chrono::Utc>,
        duration_secs: u64,
    },

    /// Agent spawned
    AgentSpawned {
        role: AgentRole,
        agent_id: String,
        timestamp: chrono::DateTime<chrono::Utc>,
    },

    /// Agent started working
    AgentStarted {
        role: AgentRole,
        timestamp: chrono::DateTime<chrono::Utc>,
        task_description: String,
    },

    /// Agent completed
    AgentCompleted {
        role: AgentRole,
        timestamp: chrono::DateTime<chrono::Utc>,
        duration_secs: u64,
        status: AgentStatus,
    },

    /// Artifact published to blackboard
    ArtifactPublished {
        artifact: Artifact,
        by: AgentRole,
        timestamp: chrono::DateTime<chrono::Utc>,
    },

    /// Progress update (percentage or status)
    ProgressUpdate {
        phase: WorkflowPhase,
        message: String,
        percent_complete: u8,
        timestamp: chrono::DateTime<chrono::Utc>,
    },

    /// Dev-Tester iteration
    DevTesterIteration {
        iteration: u8,
        max_iterations: u8,
        status: TestStatus,
        timestamp: chrono::DateTime<chrono::Utc>,
    },

    /// Deployment started
    DeploymentStarted {
        target: DeployTarget,
        timestamp: chrono::DateTime<chrono::Utc>,
    },

    /// Deployment completed
    DeploymentCompleted {
        target: DeployTarget,
        url: Option<String>,
        timestamp: chrono::DateTime<chrono::Utc>,
    },

    /// Workflow completed
    WorkflowCompleted {
        workflow_id: String,
        status: WorkflowCompletionStatus,
        total_duration_secs: u64,
        timestamp: chrono::DateTime<chrono::Utc>,
    },

    /// Error occurred
    Error {
        phase: WorkflowPhase,
        agent: Option<AgentRole>,
        message: String,
        timestamp: chrono::DateTime<chrono::Utc>,
    },

    /// Warning
    Warning {
        phase: WorkflowPhase,
        message: String,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
}

/// Agent execution status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    Success,
    PartialSuccess,
    Failed,
    Timeout,
}

/// Test execution status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TestStatus {
    Running,
    Passed,
    Failed { failure_count: u32 },
    Error,
}

/// Deployment target
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeployTarget {
    GitHub,
    Vercel,
    Netlify,
    Custom { name: String },
}

impl fmt::Display for DeployTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DeployTarget::GitHub => write!(f, "GitHub"),
            DeployTarget::Vercel => write!(f, "Vercel"),
            DeployTarget::Netlify => write!(f, "Netlify"),
            DeployTarget::Custom { name } => write!(f, "{}", name),
        }
    }
}

/// Workflow completion status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowCompletionStatus {
    Success,
    SuccessWithWarnings,
    Failed,
    Cancelled,
}

/// Progress broadcaster - sends events to all subscribers
#[derive(Debug, Clone)]
pub struct ProgressBroadcaster {
    sender: broadcast::Sender<FactoryProgressEvent>,
}

impl ProgressBroadcaster {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(100);
        Self { sender }
    }

    /// Broadcast an event to all subscribers
    pub fn broadcast(&self, event: FactoryProgressEvent) {
        let _ = self.sender.send(event);
    }

    /// Subscribe to progress events
    pub fn subscribe(&self) -> broadcast::Receiver<FactoryProgressEvent> {
        self.sender.subscribe()
    }

    /// Get number of active subscribers
    pub fn subscriber_count(&self) -> usize {
        self.sender.receiver_count()
    }
}

impl Default for ProgressBroadcaster {
    fn default() -> Self {
        Self::new()
    }
}

/// Progress tracker - tracks execution state and emits events
pub struct ProgressTracker {
    broadcaster: ProgressBroadcaster,
    workflow_id: String,
    start_time: Option<chrono::DateTime<chrono::Utc>>,
    current_phase: Option<WorkflowPhase>,
    phase_start_time: Option<chrono::DateTime<chrono::Utc>>,
}

impl ProgressTracker {
    pub fn new(broadcaster: ProgressBroadcaster) -> Self {
        Self {
            broadcaster,
            workflow_id: uuid::Uuid::new_v4().to_string(),
            start_time: None,
            current_phase: None,
            phase_start_time: None,
        }
    }

    /// Start workflow tracking
    pub fn workflow_started(&mut self) {
        let now = chrono::Utc::now();
        self.start_time = Some(now);
        self.broadcaster
            .broadcast(FactoryProgressEvent::WorkflowStarted {
                workflow_id: self.workflow_id.clone(),
                timestamp: now,
            });
    }

    /// Track phase start
    pub fn phase_started(&mut self, phase: WorkflowPhase, message: impl Into<String>) {
        let now = chrono::Utc::now();
        self.current_phase = Some(phase);
        self.phase_start_time = Some(now);
        self.broadcaster
            .broadcast(FactoryProgressEvent::PhaseStarted {
                phase,
                timestamp: now,
                message: message.into(),
            });
    }

    /// Track phase completion
    pub fn phase_completed(&mut self, phase: WorkflowPhase) {
        let now = chrono::Utc::now();
        let duration = self
            .phase_start_time
            .map(|start| (now - start).num_seconds() as u64)
            .unwrap_or(0);

        self.broadcaster
            .broadcast(FactoryProgressEvent::PhaseCompleted {
                phase,
                timestamp: now,
                duration_secs: duration,
            });
    }

    /// Track agent spawn
    pub fn agent_spawned(&self, role: AgentRole, agent_id: String) {
        self.broadcaster
            .broadcast(FactoryProgressEvent::AgentSpawned {
                role,
                agent_id,
                timestamp: chrono::Utc::now(),
            });
    }

    /// Track agent started
    pub fn agent_started(&self, role: AgentRole, task_description: impl Into<String>) {
        self.broadcaster
            .broadcast(FactoryProgressEvent::AgentStarted {
                role,
                timestamp: chrono::Utc::now(),
                task_description: task_description.into(),
            });
    }

    /// Track agent completion
    pub fn agent_completed(&self, role: AgentRole, duration: Duration, status: AgentStatus) {
        self.broadcaster
            .broadcast(FactoryProgressEvent::AgentCompleted {
                role,
                timestamp: chrono::Utc::now(),
                duration_secs: duration.as_secs(),
                status,
            });
    }

    /// Track artifact publication
    pub fn artifact_published(&self, artifact: Artifact, by: AgentRole) {
        self.broadcaster
            .broadcast(FactoryProgressEvent::ArtifactPublished {
                artifact,
                by,
                timestamp: chrono::Utc::now(),
            });
    }

    /// Update progress percentage
    pub fn progress_update(&self, message: impl Into<String>, percent: u8) {
        if let Some(phase) = self.current_phase {
            self.broadcaster
                .broadcast(FactoryProgressEvent::ProgressUpdate {
                    phase,
                    message: message.into(),
                    percent_complete: percent.min(100),
                    timestamp: chrono::Utc::now(),
                });
        }
    }

    /// Track dev-tester iteration
    pub fn dev_tester_iteration(&self, iteration: u8, max: u8, status: TestStatus) {
        self.broadcaster
            .broadcast(FactoryProgressEvent::DevTesterIteration {
                iteration,
                max_iterations: max,
                status,
                timestamp: chrono::Utc::now(),
            });
    }

    /// Track deployment start
    pub fn deployment_started(&self, target: DeployTarget) {
        self.broadcaster
            .broadcast(FactoryProgressEvent::DeploymentStarted {
                target,
                timestamp: chrono::Utc::now(),
            });
    }

    /// Track deployment completion
    pub fn deployment_completed(&self, target: DeployTarget, url: Option<String>) {
        self.broadcaster
            .broadcast(FactoryProgressEvent::DeploymentCompleted {
                target,
                url,
                timestamp: chrono::Utc::now(),
            });
    }

    /// Track workflow completion
    pub fn workflow_completed(&self, status: WorkflowCompletionStatus) {
        let now = chrono::Utc::now();
        let total_duration = self
            .start_time
            .map(|start| (now - start).num_seconds() as u64)
            .unwrap_or(0);

        self.broadcaster
            .broadcast(FactoryProgressEvent::WorkflowCompleted {
                workflow_id: self.workflow_id.clone(),
                status,
                total_duration_secs: total_duration,
                timestamp: now,
            });
    }

    /// Track error
    pub fn error(&self, message: impl Into<String>, agent: Option<AgentRole>) {
        if let Some(phase) = self.current_phase {
            self.broadcaster.broadcast(FactoryProgressEvent::Error {
                phase,
                agent,
                message: message.into(),
                timestamp: chrono::Utc::now(),
            });
        }
    }

    /// Track warning
    pub fn warning(&self, message: impl Into<String>) {
        if let Some(phase) = self.current_phase {
            self.broadcaster.broadcast(FactoryProgressEvent::Warning {
                phase,
                message: message.into(),
                timestamp: chrono::Utc::now(),
            });
        }
    }

    /// Get broadcaster for external use
    pub fn broadcaster(&self) -> &ProgressBroadcaster {
        &self.broadcaster
    }
}

/// Progress formatter for human-readable output
pub struct ProgressFormatter;

impl ProgressFormatter {
    /// Format event for display
    pub fn format(event: &FactoryProgressEvent) -> String {
        match event {
            FactoryProgressEvent::WorkflowStarted { workflow_id, .. } => {
                format!("🏭 Factory workflow started (ID: {})", workflow_id)
            }
            FactoryProgressEvent::PhaseStarted { phase, message, .. } => {
                let icon = match phase {
                    WorkflowPhase::IntentAnalysis => "🔍",
                    WorkflowPhase::Analysis => "📋",
                    WorkflowPhase::ParallelBuild => "🔨",
                    WorkflowPhase::IntegrationLoop => "🔄",
                    WorkflowPhase::Deployment => "🚀",
                    WorkflowPhase::Completed => "✅",
                    WorkflowPhase::Failed => "❌",
                };
                format!("{} Phase: {} - {}", icon, phase_to_string(phase), message)
            }
            FactoryProgressEvent::PhaseCompleted {
                phase,
                duration_secs,
                ..
            } => {
                format!(
                    "✓ Phase {} completed in {}s",
                    phase_to_string(phase),
                    duration_secs
                )
            }
            FactoryProgressEvent::AgentSpawned { role, .. } => {
                format!("  👤 Spawned {:?} agent", role)
            }
            FactoryProgressEvent::AgentStarted {
                role,
                task_description,
                ..
            } => {
                format!("  ▶️  {:?}: {}", role, task_description)
            }
            FactoryProgressEvent::AgentCompleted { role, status, .. } => {
                let icon = match status {
                    AgentStatus::Success => "✓",
                    AgentStatus::PartialSuccess => "~",
                    AgentStatus::Failed => "✗",
                    AgentStatus::Timeout => "⏱",
                };
                format!("  {} {:?} completed", icon, role)
            }
            FactoryProgressEvent::ArtifactPublished { artifact, by, .. } => {
                format!("  📝 {:?} published by {:?}", artifact, by)
            }
            FactoryProgressEvent::ProgressUpdate {
                message,
                percent_complete,
                ..
            } => {
                format!("  {}% - {}", percent_complete, message)
            }
            FactoryProgressEvent::DevTesterIteration {
                iteration,
                max_iterations,
                status,
                ..
            } => {
                let status_icon = match status {
                    TestStatus::Running => "🔄",
                    TestStatus::Passed => "✅",
                    TestStatus::Failed { .. } => "❌",
                    TestStatus::Error => "⚠️",
                };
                format!(
                    "  {} Dev-Tester iteration {}/{}",
                    status_icon, iteration, max_iterations
                )
            }
            FactoryProgressEvent::DeploymentStarted { target, .. } => {
                format!("  🚀 Deploying to {}...", target)
            }
            FactoryProgressEvent::DeploymentCompleted { target, url, .. } => {
                if let Some(url) = url {
                    format!("  ✅ Deployed to {}: {}", target, url)
                } else {
                    format!("  ✅ Deployed to {}", target)
                }
            }
            FactoryProgressEvent::WorkflowCompleted {
                status,
                total_duration_secs,
                ..
            } => {
                let icon = match status {
                    WorkflowCompletionStatus::Success => "🎉",
                    WorkflowCompletionStatus::SuccessWithWarnings => "⚠️",
                    WorkflowCompletionStatus::Failed => "❌",
                    WorkflowCompletionStatus::Cancelled => "🚫",
                };
                format!(
                    "{} Workflow completed in {}s (status: {:?})",
                    icon, total_duration_secs, status
                )
            }
            FactoryProgressEvent::Error { message, agent, .. } => {
                if let Some(agent) = agent {
                    format!("  ❌ Error in {:?}: {}", agent, message)
                } else {
                    format!("  ❌ Error: {}", message)
                }
            }
            FactoryProgressEvent::Warning { message, .. } => {
                format!("  ⚠️  Warning: {}", message)
            }
        }
    }
}

fn phase_to_string(phase: &WorkflowPhase) -> &'static str {
    match phase {
        WorkflowPhase::IntentAnalysis => "Intent Analysis",
        WorkflowPhase::Analysis => "Analysis",
        WorkflowPhase::ParallelBuild => "Parallel Build",
        WorkflowPhase::IntegrationLoop => "Integration Loop",
        WorkflowPhase::Deployment => "Deployment",
        WorkflowPhase::Completed => "Completed",
        WorkflowPhase::Failed => "Failed",
    }
}

/// Async progress consumer that prints to stdout
pub async fn print_progress_events(mut rx: broadcast::Receiver<FactoryProgressEvent>) {
    loop {
        match rx.recv().await {
            Ok(event) => println!("{}", ProgressFormatter::format(&event)),
            Err(broadcast::error::RecvError::Closed) => break,
            Err(broadcast::error::RecvError::Lagged(n)) => {
                eprintln!("Warning: Lagged behind {} progress events", n);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn broadcaster_creation() {
        let broadcaster = ProgressBroadcaster::new();
        assert_eq!(broadcaster.subscriber_count(), 0);
    }

    #[test]
    fn broadcast_and_receive() {
        let broadcaster = ProgressBroadcaster::new();
        let mut rx = broadcaster.subscribe();

        broadcaster.broadcast(FactoryProgressEvent::WorkflowStarted {
            workflow_id: uuid::Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now(),
        });

        let event = rx.try_recv();
        assert!(event.is_ok());
    }

    #[test]
    fn multiple_subscribers() {
        let broadcaster = ProgressBroadcaster::new();
        let mut rx1 = broadcaster.subscribe();
        let mut rx2 = broadcaster.subscribe();

        broadcaster.broadcast(FactoryProgressEvent::WorkflowStarted {
            workflow_id: uuid::Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now(),
        });

        assert!(rx1.try_recv().is_ok());
        assert!(rx2.try_recv().is_ok());
    }

    #[test]
    fn progress_formatter_workflow_started() {
        let event = FactoryProgressEvent::WorkflowStarted {
            workflow_id: uuid::Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now(),
        };
        let output = ProgressFormatter::format(&event);
        assert!(output.contains("Factory workflow started"));
    }

    #[test]
    fn progress_formatter_agent_completed() {
        let event = FactoryProgressEvent::AgentCompleted {
            role: AgentRole::Developer,
            timestamp: chrono::Utc::now(),
            duration_secs: 30,
            status: AgentStatus::Success,
        };
        let output = ProgressFormatter::format(&event);
        assert!(output.contains("Developer"));
        assert!(output.contains("completed"));
    }

    #[tokio::test]
    async fn tracker_workflow_lifecycle() {
        let broadcaster = ProgressBroadcaster::new();
        let mut tracker = ProgressTracker::new(broadcaster.clone());
        let mut rx = broadcaster.subscribe();

        tracker.workflow_started();
        assert!(matches!(
            rx.try_recv().unwrap(),
            FactoryProgressEvent::WorkflowStarted { .. }
        ));

        tracker.phase_started(WorkflowPhase::Analysis, "Creating PRD");
        assert!(matches!(
            rx.try_recv().unwrap(),
            FactoryProgressEvent::PhaseStarted { .. }
        ));

        tracker.agent_spawned(AgentRole::BusinessAnalyst, uuid::Uuid::new_v4().to_string());
        assert!(matches!(
            rx.try_recv().unwrap(),
            FactoryProgressEvent::AgentSpawned { .. }
        ));
    }
}
