//! Workspace management for per-agent isolation
//!
//! This module provides isolated workspaces for each agent, enabling:
//! - Per-agent file isolation (no accidental overwrites)
//! - Per-agent identity, skills, and memory
//! - Independent rollback capability
//! - Better debugging and audit trails

pub mod isolation;
pub mod manager;

pub use isolation::{FileOperation, SandboxType, WorkspaceIsolation, WorkspaceMigration};
pub use manager::{AgentWorkspace, WorkspaceConfig, WorkspaceManager};

use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::factory::roles::AgentRole;

/// Unique identifier for an agent workspace
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct WorkspaceId {
    pub agent_role: String,
    pub agent_uuid: Uuid,
}

impl WorkspaceId {
    pub fn new(agent_role: impl Into<String>) -> Self {
        Self {
            agent_role: agent_role.into(),
            agent_uuid: Uuid::new_v4(),
        }
    }

    pub fn directory_name(&self) -> String {
        format!("{}-{}", self.agent_role, self.agent_uuid)
    }
}

impl std::fmt::Display for WorkspaceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}-{}", self.agent_role, self.agent_uuid)
    }
}

/// Paths within an agent workspace
#[derive(Debug, Clone)]
pub struct WorkspacePaths {
    pub root: PathBuf,
    pub agent_config: PathBuf,
    pub skills: PathBuf,
    pub memory: PathBuf,
    pub sandbox: PathBuf,
    pub state: PathBuf,
}

impl WorkspacePaths {
    pub fn new(base: impl AsRef<Path>, workspace_id: &WorkspaceId) -> Self {
        let root = base.as_ref().join(workspace_id.directory_name());

        Self {
            agent_config: root.join(".agent"),
            skills: root.join(".agent").join("skills"),
            memory: root.join(".agent").join("memory"),
            sandbox: root.join("sandbox"),
            state: root.join(".agent").join("state.json"),
            root,
        }
    }
}

/// Configuration for agent identity and capabilities
#[derive(Debug, Clone)]
pub struct AgentConfig {
    pub identity: String,    // Content of identity.md
    pub skills: Vec<Skill>,  // Loaded from skills/
    pub memory: AgentMemory, // Conversation history
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            identity: "Default Agent".to_string(),
            skills: Vec::new(),
            memory: AgentMemory::default(),
        }
    }
}

impl AgentConfig {
    /// Create a default config for a specific role
    pub fn for_role(role: AgentRole) -> Self {
        Self {
            identity: format!("{:?} Agent", role),
            skills: Vec::new(),
            memory: AgentMemory::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Skill {
    pub name: String,
    pub content: String,
    pub file_path: PathBuf,
}

#[derive(Debug, Clone, Default)]
pub struct AgentMemory {
    pub conversations: Vec<Conversation>,
    pub embeddings: Vec<Embedding>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Conversation {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Embedding {
    pub id: String,
    pub vector: Vec<f32>,
    pub metadata: serde_json::Value,
}

/// Errors that can occur in workspace operations
#[derive(thiserror::Error, Debug)]
pub enum WorkspaceError {
    #[error("Workspace already exists: {0}")]
    AlreadyExists(PathBuf),

    #[error("Workspace not found: {0}")]
    NotFound(PathBuf),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Directory traversal error: {0}")]
    WalkDir(#[from] walkdir::Error),

    #[error("Invalid path: {0}")]
    InvalidPath(String),

    #[error("Invalid workspace structure: {0}")]
    InvalidStructure(String),
}

pub type Result<T> = std::result::Result<T, WorkspaceError>;

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_workspace_id_directory_name() {
        let id = WorkspaceId::new("developer");
        let name = id.directory_name();
        assert!(name.starts_with("developer-"));
        assert_eq!(name.len(), "developer-".len() + 36); // UUID length
    }

    #[test]
    fn test_workspace_paths() {
        let temp = TempDir::new().unwrap();
        let id = WorkspaceId::new("tester");
        let paths = WorkspacePaths::new(temp.path(), &id);

        assert!(paths.root.to_string_lossy().contains("tester-"));
        assert_eq!(paths.agent_config, paths.root.join(".agent"));
        assert_eq!(paths.skills, paths.root.join(".agent").join("skills"));
        assert_eq!(paths.sandbox, paths.root.join("sandbox"));
    }
}
