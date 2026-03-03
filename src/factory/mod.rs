//! Multi-agent factory module for the Autonomous Software Factory.
//!
//! This module implements a hierarchical multi-agent workflow where a Master
//! Orchestrator spawns specialized AI agents (Business Analyst, UI/UX Designer,
//! Developer, Tester, DevOps) that collaborate through phased execution to turn
//! a user's idea into working software.
//!
//! # Architecture
//!
//! ## Core Modules
//! - [`roles`]: Agent role definitions with canonical system prompts
//! - [`blackboard`]: Shared state management for inter-agent communication
//! - [`workflow`]: Workflow state machine with phased execution
//! - [`orchestrator_tool`]: `factory_build` tool implementing the `Tool` trait
//! - [`progress`]: Real-time progress streaming system
//!
//! ## Workspace Isolation Modules (Phase B)
//! - [`workspace`]: Per-agent workspace management with isolation
//!
//! ## Inter-Agent Communication (Phase C)
//! - [`protocol`]: Inter-agent communication protocol (IACP)
//!
//! ## Agent Pool Management (Phase F)
//! - [`pool`]: Agent pool management with warm/cold states
//!
//! # Configuration
//!
//! The factory is enabled by default via `[factory]` config section. When `factory.enabled`
//! is true, the `factory_build` tool is available and agents can autonomously decide
//! when to use it based on task complexity.

pub mod blackboard;
pub mod orchestrator_tool;
pub mod pool;
pub mod progress;
pub mod protocol;
pub mod roles;
pub mod workflow;
pub mod workspace;

pub use blackboard::{Artifact, ArtifactEntry, Blackboard};
pub use orchestrator_tool::FactoryOrchestratorTool;
pub use pool::{
    AgentInstance, AgentInstanceId, AgentPool, AgentState as PoolAgentState, PoolConfig, PoolStats,
};
pub use progress::{
    AgentStatus, DeployTarget, FactoryProgressEvent, ProgressBroadcaster, ProgressFormatter,
    ProgressTracker, TestStatus, WorkflowCompletionStatus,
};
pub use protocol::{
    AgentMessage, MessageBus, MessageContent, MessageHandler, MessageHandlerRegistry,
    MessageHeader, MessageId, MessagePriority, ProtocolError,
};
pub use roles::{AgentRole, RoleConfig};
pub use workflow::{FactoryWorkflow, WorkflowPhase};
pub use workspace::{
    AgentConfig, AgentWorkspace, WorkspaceConfig, WorkspaceId, WorkspaceManager, WorkspacePaths,
};
