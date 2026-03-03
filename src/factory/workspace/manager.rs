//! Workspace lifecycle management
//!
//! Handles creation, initialization, loading, and cleanup of agent workspaces.

use super::*;
use std::fs;
use std::path::Path;
use tracing::info;

/// Manages the lifecycle of agent workspaces
pub struct WorkspaceManager {
    base_path: PathBuf,
    config: WorkspaceConfig,
}

/// Configuration for workspace management
#[derive(Debug, Clone)]
pub struct WorkspaceConfig {
    /// Root directory for all workspaces
    pub workspace_root: PathBuf,
    /// Whether to preserve workspaces after agent termination
    pub preserve_workspaces: bool,
    /// Archive workspaces older than this duration
    pub archive_after: Option<std::time::Duration>,
    /// Maximum disk space per workspace (bytes)
    pub max_workspace_size: Option<u64>,
    /// Default identity template for new agents
    pub default_identity_template: String,
}

impl Default for WorkspaceConfig {
    fn default() -> Self {
        // Use directories crate to get home directory
        let home = directories::BaseDirs::new()
            .map(|dirs| dirs.home_dir().to_path_buf())
            .unwrap_or_else(|| PathBuf::from("/tmp"));

        Self {
            workspace_root: home.join(".zerobuild").join("workspaces"),
            preserve_workspaces: true,
            archive_after: Some(std::time::Duration::from_secs(7 * 24 * 60 * 60)), // 7 days
            max_workspace_size: Some(1024 * 1024 * 1024),                          // 1GB
            default_identity_template: include_str!("../../../templates/default_identity.md")
                .to_string(),
        }
    }
}

impl WorkspaceManager {
    /// Create a new workspace manager with the given configuration
    pub fn new(config: WorkspaceConfig) -> Result<Self> {
        // Ensure workspace root exists
        fs::create_dir_all(&config.workspace_root)?;

        Ok(Self {
            base_path: config.workspace_root.clone(),
            config,
        })
    }

    /// Create a workspace manager with a custom root path (for testing)
    pub fn with_root(path: PathBuf) -> Result<Self> {
        let config = WorkspaceConfig {
            workspace_root: path,
            ..Default::default()
        };
        Self::new(config)
    }

    /// Create a new workspace for an agent
    pub async fn create_workspace(
        &self,
        workspace_id: &WorkspaceId,
        role: &str,
    ) -> Result<AgentWorkspace> {
        let paths = WorkspacePaths::new(&self.base_path, workspace_id);

        // Check if workspace already exists
        if paths.root.exists() {
            return Err(WorkspaceError::AlreadyExists(paths.root));
        }

        // Create directory structure
        fs::create_dir_all(&paths.root)?;
        fs::create_dir_all(&paths.agent_config)?;
        fs::create_dir_all(&paths.skills)?;
        fs::create_dir_all(&paths.memory)?;
        fs::create_dir_all(&paths.sandbox)?;
        fs::create_dir_all(paths.memory.join("conversations"))?;
        fs::create_dir_all(paths.memory.join("embeddings"))?;

        // Create initial identity.md
        let identity_path = paths.agent_config.join("identity.md");
        let identity_content = self.generate_identity(role);
        fs::write(&identity_path, identity_content)?;

        // Create initial state.json
        let state = AgentState::default();
        let state_json = serde_json::to_string_pretty(&state)?;
        fs::write(&paths.state, state_json)?;

        // Copy default skills for this role
        self.initialize_skills(&paths.skills, role).await?;

        info!("Created workspace for {} at {:?}", workspace_id, paths.root);

        Ok(AgentWorkspace {
            id: workspace_id.clone(),
            paths,
            config: AgentConfig::default(),
        })
    }

    /// Load an existing workspace
    pub fn load_workspace(&self, workspace_id: &WorkspaceId) -> Result<AgentWorkspace> {
        let paths = WorkspacePaths::new(&self.base_path, workspace_id);

        if !paths.root.exists() {
            return Err(WorkspaceError::NotFound(paths.root));
        }

        // Validate workspace structure
        self.validate_workspace_structure(&paths)?;

        // Load agent config
        let config = self.load_agent_config(&paths)?;

        Ok(AgentWorkspace {
            id: workspace_id.clone(),
            paths,
            config,
        })
    }

    /// Archive a workspace (compress and move to archive directory)
    pub async fn archive_workspace(&self, workspace_id: &WorkspaceId) -> Result<PathBuf> {
        let paths = WorkspacePaths::new(&self.base_path, workspace_id);

        if !paths.root.exists() {
            return Err(WorkspaceError::NotFound(paths.root.clone()));
        }

        let archive_dir = self.base_path.join("archive");
        fs::create_dir_all(&archive_dir)?;

        let archive_path = archive_dir.join(format!(
            "{}-{}.tar.gz",
            workspace_id.agent_role,
            chrono::Utc::now().format("%Y%m%d-%H%M%S")
        ));

        // Create tar.gz archive
        self.create_archive(&paths.root, &archive_path).await?;

        // Remove original workspace
        fs::remove_dir_all(&paths.root)?;

        info!("Archived workspace {} to {:?}", workspace_id, archive_path);

        Ok(archive_path)
    }

    /// Delete a workspace permanently
    pub fn delete_workspace(&self, workspace_id: &WorkspaceId) -> Result<()> {
        let paths = WorkspacePaths::new(&self.base_path, workspace_id);

        if paths.root.exists() {
            fs::remove_dir_all(&paths.root)?;
            info!("Deleted workspace {}", workspace_id);
        }

        Ok(())
    }

    /// List all workspaces
    pub fn list_workspaces(&self) -> Result<Vec<WorkspaceId>> {
        let mut workspaces = Vec::new();

        if let Ok(entries) = fs::read_dir(&self.base_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        // Parse workspace directory name (format: "role-uuid")
                        if let Some((role, uuid_str)) = name.rsplit_once('-') {
                            if let Ok(uuid) = Uuid::parse_str(uuid_str) {
                                workspaces.push(WorkspaceId {
                                    agent_role: role.to_string(),
                                    agent_uuid: uuid,
                                });
                            }
                        }
                    }
                }
            }
        }

        Ok(workspaces)
    }

    /// Clean up old workspaces based on archive policy
    pub async fn cleanup_old_workspaces(&self) -> Result<usize> {
        let mut cleaned = 0;

        if let Some(archive_after) = self.config.archive_after {
            let cutoff = std::time::SystemTime::now() - archive_after;

            for workspace_id in self.list_workspaces()? {
                let paths = WorkspacePaths::new(&self.base_path, &workspace_id);

                if let Ok(metadata) = fs::metadata(&paths.root) {
                    if let Ok(modified) = metadata.modified() {
                        if modified < cutoff {
                            self.archive_workspace(&workspace_id).await?;
                            cleaned += 1;
                        }
                    }
                }
            }
        }

        Ok(cleaned)
    }

    // Helper methods

    fn generate_identity(&self, role: &str) -> String {
        // Use role-specific template if available, otherwise default
        let template = self
            .get_role_template(role)
            .unwrap_or_else(|| self.config.default_identity_template.clone());

        template.replace("{{ROLE}}", role)
    }

    fn get_role_template(&self, role: &str) -> Option<String> {
        let template_path = self
            .base_path
            .join("../templates")
            .join(format!("{}_identity.md", role));

        fs::read_to_string(template_path).ok()
    }

    async fn initialize_skills(&self, skills_dir: &Path, role: &str) -> Result<()> {
        // Copy default skills for this role
        let default_skills_dir = self
            .base_path
            .join("../templates")
            .join("skills")
            .join(role);

        if default_skills_dir.exists() {
            self.copy_dir_contents(&default_skills_dir, skills_dir)
                .await?;
        }

        Ok(())
    }

    async fn copy_dir_contents(&self, from: &Path, to: &Path) -> Result<()> {
        for entry in walkdir::WalkDir::new(from) {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() {
                let relative = path
                    .strip_prefix(from)
                    .map_err(|e| WorkspaceError::InvalidPath(e.to_string()))?;
                let dest = to.join(relative);

                if let Some(parent) = dest.parent() {
                    fs::create_dir_all(parent)?;
                }

                fs::copy(path, dest)?;
            }
        }

        Ok(())
    }

    fn validate_workspace_structure(&self, paths: &WorkspacePaths) -> Result<()> {
        let required = vec![
            &paths.agent_config,
            &paths.skills,
            &paths.memory,
            &paths.sandbox,
        ];

        for dir in required {
            if !dir.exists() {
                return Err(WorkspaceError::InvalidStructure(format!(
                    "Missing required directory: {:?}",
                    dir
                )));
            }
        }

        Ok(())
    }

    fn load_agent_config(&self, paths: &WorkspacePaths) -> Result<AgentConfig> {
        // Load identity
        let identity = fs::read_to_string(paths.agent_config.join("identity.md"))?;

        // Load skills
        let mut skills = Vec::new();
        if let Ok(entries) = fs::read_dir(&paths.skills) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map_or(false, |ext| ext == "md") {
                    let content = fs::read_to_string(&path)?;
                    let name = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("unknown")
                        .to_string();

                    skills.push(Skill {
                        name,
                        content,
                        file_path: path,
                    });
                }
            }
        }

        // Load memory (conversations and embeddings)
        let mut conversations = Vec::new();
        let mut embeddings = Vec::new();

        // Load conversations from memory/conversations/
        let conversations_dir = paths.memory.join("conversations");
        if conversations_dir.exists() {
            for entry in std::fs::read_dir(&conversations_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().map_or(false, |e| e == "json") {
                    let content = std::fs::read_to_string(&path)?;
                    if let Ok(conv) = serde_json::from_str::<Conversation>(&content) {
                        conversations.push(conv);
                    }
                }
            }
        }

        // Load embeddings from memory/embeddings/
        let embeddings_dir = paths.memory.join("embeddings");
        if embeddings_dir.exists() {
            for entry in std::fs::read_dir(&embeddings_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().map_or(false, |e| e == "json") {
                    let content = std::fs::read_to_string(&path)?;
                    if let Ok(emb) = serde_json::from_str::<Embedding>(&content) {
                        embeddings.push(emb);
                    }
                }
            }
        }

        let memory = AgentMemory {
            conversations,
            embeddings,
        };

        Ok(AgentConfig {
            identity,
            skills,
            memory,
        })
    }

    async fn create_archive(&self, source: &Path, dest: &Path) -> Result<()> {
        // Use tar command for now - can be replaced with pure Rust implementation
        let output = tokio::process::Command::new("tar")
            .args([
                "-czf",
                dest.to_str().unwrap(),
                "-C",
                source.parent().unwrap().to_str().unwrap(),
                source.file_name().unwrap().to_str().unwrap(),
            ])
            .output()
            .await
            .map_err(|e| WorkspaceError::Io(e.into()))?;

        if !output.status.success() {
            return Err(WorkspaceError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("tar failed: {}", String::from_utf8_lossy(&output.stderr)),
            )));
        }

        Ok(())
    }
}

/// Represents an active agent workspace
pub struct AgentWorkspace {
    pub id: WorkspaceId,
    pub paths: WorkspacePaths,
    pub config: AgentConfig,
}

impl AgentWorkspace {
    /// Get the sandbox path for this workspace
    pub fn sandbox_path(&self) -> &Path {
        &self.paths.sandbox
    }

    /// Save agent state to disk
    pub fn save_state(&self, state: &AgentState) -> Result<()> {
        let state_json = serde_json::to_string_pretty(state)?;
        fs::write(&self.paths.state, state_json)?;
        Ok(())
    }

    /// Load agent state from disk
    pub fn load_state(&self) -> Result<AgentState> {
        let state_json = fs::read_to_string(&self.paths.state)?;
        let state = serde_json::from_str(&state_json)?;
        Ok(state)
    }
}

/// Serializable agent state
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct AgentState {
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_active: Option<chrono::DateTime<chrono::Utc>>,
    pub task_count: u64,
    pub status: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_create_and_load_workspace() {
        let temp = TempDir::new().unwrap();
        let config = WorkspaceConfig {
            workspace_root: temp.path().to_path_buf(),
            ..Default::default()
        };

        let manager = WorkspaceManager::new(config).unwrap();
        let workspace_id = WorkspaceId::new("developer");

        // Create workspace
        let workspace = manager
            .create_workspace(&workspace_id, "developer")
            .await
            .unwrap();

        assert!(workspace.paths.root.exists());
        assert!(workspace.paths.sandbox.exists());
        assert!(workspace.paths.agent_config.join("identity.md").exists());

        // Load workspace
        let loaded = manager.load_workspace(&workspace_id).unwrap();
        assert_eq!(loaded.id, workspace_id);
    }

    #[tokio::test]
    async fn test_delete_workspace() {
        let temp = TempDir::new().unwrap();
        let config = WorkspaceConfig {
            workspace_root: temp.path().to_path_buf(),
            ..Default::default()
        };

        let manager = WorkspaceManager::new(config).unwrap();
        let workspace_id = WorkspaceId::new("tester");

        manager
            .create_workspace(&workspace_id, "tester")
            .await
            .unwrap();
        assert!(manager.load_workspace(&workspace_id).is_ok());

        manager.delete_workspace(&workspace_id).unwrap();
        assert!(manager.load_workspace(&workspace_id).is_err());
    }
}
