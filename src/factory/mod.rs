//! Multi-agent factory module for the Autonomous Software Factory.
//!
//! This module implements a hierarchical multi-agent workflow where a Master
//! Orchestrator spawns specialized AI agents (Business Analyst, UI/UX Designer,
//! Developer, Tester, DevOps) that collaborate through phased execution to turn
//! a user's idea into working software.
//!
//! # Architecture
//!
//! - [`roles`]: Agent role definitions with canonical system prompts
//! - [`blackboard`]: Shared state management for inter-agent communication
//! - [`workflow`]: Workflow state machine with phased execution
//! - [`orchestrator_tool`]: `factory_build` tool implementing the `Tool` trait
//! - [`intent`]: Intent classification for dynamic agent spawning
//! - [`progress`]: Real-time progress streaming system
//!
//! # Configuration
//!
//! The factory is opt-in via `[factory]` config section. When `factory.enabled`
//! is false (default), no factory tools are registered and existing single-agent
//! mode is unaffected.

pub mod blackboard;
pub mod orchestrator_tool;
pub mod progress;
pub mod roles;
pub mod workflow;

pub use blackboard::{Artifact, ArtifactEntry, Blackboard};
pub use orchestrator_tool::FactoryOrchestratorTool;
pub use progress::{
    AgentStatus, DeployTarget, FactoryProgressEvent, ProgressBroadcaster, ProgressFormatter,
    ProgressTracker, TestStatus, WorkflowCompletionStatus,
};
pub use roles::{AgentRole, RoleConfig};
pub use workflow::{FactoryWorkflow, WorkflowPhase};
