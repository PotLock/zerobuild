//! Workspace isolation and sandboxing
//!
//! Ensures agents cannot access files outside their workspace
//! and provides different levels of sandboxing.

use super::*;
use std::fs;
use std::path::Path;
use tracing::warn;

/// Types of sandboxing available
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxType {
    /// Full filesystem isolation - agent can only see its workspace
    FullIsolation,
    /// Shared project - agent can read shared project files
    SharedProject,
    /// Read-only shared - agent can read but not write shared files
    ReadOnlyShared,
}

/// Manages isolation for a workspace
pub struct WorkspaceIsolation {
    workspace_paths: WorkspacePaths,
    sandbox_type: SandboxType,
    shared_paths: Vec<PathBuf>,
}

impl WorkspaceIsolation {
    /// Create new isolation manager for a workspace
    pub fn new(workspace_paths: WorkspacePaths, sandbox_type: SandboxType) -> Self {
        Self {
            workspace_paths,
            sandbox_type,
            shared_paths: Vec::new(),
        }
    }

    /// Add a shared path that agents can access
    pub fn with_shared_path(mut self, path: impl AsRef<Path>) -> Self {
        self.shared_paths.push(path.as_ref().to_path_buf());
        self
    }

    /// Check if a path is within the allowed boundaries
    pub fn is_path_allowed(&self, path: &Path) -> bool {
        let canonical = match path.canonicalize() {
            Ok(p) => p,
            Err(_) => return false, // Cannot access non-existent paths
        };

        // Always allow access to workspace
        if let Ok(workspace_root) = self.workspace_paths.root.canonicalize() {
            if canonical.starts_with(&workspace_root) {
                return true;
            }
        }

        // Check shared paths based on sandbox type
        match self.sandbox_type {
            SandboxType::FullIsolation => false,
            SandboxType::SharedProject | SandboxType::ReadOnlyShared => {
                self.shared_paths.iter().any(|shared| {
                    if let Ok(shared_canonical) = shared.canonicalize() {
                        canonical.starts_with(&shared_canonical)
                    } else {
                        false
                    }
                })
            }
        }
    }

    /// Validate a file path is within workspace boundaries
    pub fn validate_file_access(&self, path: &Path, operation: FileOperation) -> Result<()> {
        if !self.is_path_allowed(path) {
            return Err(WorkspaceError::InvalidStructure(format!(
                "Access denied: {:?} is outside workspace boundaries. \
                     Operation: {:?}",
                path, operation
            )));
        }

        // For read-only shared, prevent writes
        if self.sandbox_type == SandboxType::ReadOnlyShared {
            if let Ok(canonical) = path.canonicalize() {
                let in_shared = self.shared_paths.iter().any(|shared| {
                    if let Ok(shared_canonical) = shared.canonicalize() {
                        canonical.starts_with(&shared_canonical)
                    } else {
                        false
                    }
                });

                if in_shared && operation == FileOperation::Write {
                    return Err(WorkspaceError::InvalidStructure(format!(
                        "Write access denied to shared path: {:?}",
                        path
                    )));
                }
            }
        }

        Ok(())
    }

    /// Get the effective sandbox path for a given request
    /// This translates relative paths to absolute paths within the workspace
    pub fn resolve_sandbox_path(&self, relative_path: impl AsRef<Path>) -> PathBuf {
        let relative = relative_path.as_ref();

        // Handle absolute paths (shouldn't happen, but safety check)
        if relative.is_absolute() {
            warn!("Attempted to use absolute path in sandbox: {:?}", relative);
            // Strip leading slash and treat as relative
            let stripped = relative.strip_prefix("/").unwrap_or(relative);
            return self.workspace_paths.sandbox.join(stripped);
        }

        self.workspace_paths.sandbox.join(relative)
    }

    /// Create a sandbox environment for an agent
    pub fn create_sandbox_env(&self) -> SandboxEnvironment {
        SandboxEnvironment {
            workspace_root: self.workspace_paths.root.clone(),
            sandbox_root: self.workspace_paths.sandbox.clone(),
            allowed_paths: vec![self.workspace_paths.root.clone()],
            shared_paths: self.shared_paths.clone(),
        }
    }

    /// Clean up sandbox directory (remove all files)
    pub fn clear_sandbox(&self) -> Result<()> {
        if self.workspace_paths.sandbox.exists() {
            fs::remove_dir_all(&self.workspace_paths.sandbox)?;
            fs::create_dir_all(&self.workspace_paths.sandbox)?;
        }
        Ok(())
    }

    /// Get disk usage statistics for the workspace
    pub fn get_disk_usage(&self) -> Result<DiskUsage> {
        let mut total_size = 0u64;
        let mut file_count = 0u64;

        for entry in walkdir::WalkDir::new(&self.workspace_paths.root) {
            if let Ok(entry) = entry {
                if let Ok(metadata) = entry.metadata() {
                    if metadata.is_file() {
                        total_size += metadata.len();
                        file_count += 1;
                    }
                }
            }
        }

        Ok(DiskUsage {
            total_size_bytes: total_size,
            file_count,
        })
    }

    /// Check if workspace exceeds size limits
    pub fn check_size_limits(&self, max_size: u64) -> Result<bool> {
        let usage = self.get_disk_usage()?;
        Ok(usage.total_size_bytes <= max_size)
    }
}

/// File operations that can be validated
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileOperation {
    Read,
    Write,
    Delete,
    Execute,
}

/// Environment variables and configuration for sandboxed execution
#[derive(Debug, Clone)]
pub struct SandboxEnvironment {
    pub workspace_root: PathBuf,
    pub sandbox_root: PathBuf,
    pub allowed_paths: Vec<PathBuf>,
    pub shared_paths: Vec<PathBuf>,
}

impl SandboxEnvironment {
    /// Convert to environment variables for child processes
    pub fn to_env_vars(&self) -> Vec<(String, String)> {
        vec![
            (
                "ZEROBUILD_WORKSPACE_ROOT".to_string(),
                self.workspace_root.to_string_lossy().to_string(),
            ),
            (
                "ZEROBUILD_SANDBOX_ROOT".to_string(),
                self.sandbox_root.to_string_lossy().to_string(),
            ),
        ]
    }
}

/// Disk usage statistics
#[derive(Debug, Clone, Copy)]
pub struct DiskUsage {
    pub total_size_bytes: u64,
    pub file_count: u64,
}

impl DiskUsage {
    /// Format size in human-readable format
    pub fn format_size(&self) -> String {
        const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
        let mut size = self.total_size_bytes as f64;
        let mut unit_index = 0;

        while size >= 1024.0 && unit_index < UNITS.len() - 1 {
            size /= 1024.0;
            unit_index += 1;
        }

        format!("{:.2} {}", size, UNITS[unit_index])
    }
}

/// Migration tool to move from old /tmp structure to new workspace structure
pub struct WorkspaceMigration;

impl WorkspaceMigration {
    /// Check if migration is needed
    pub fn needs_migration() -> bool {
        std::path::Path::new("/tmp")
            .read_dir()
            .map(|entries| {
                entries.flatten().any(|entry| {
                    entry
                        .file_name()
                        .to_string_lossy()
                        .starts_with("zerobuild-sandbox-")
                })
            })
            .unwrap_or(false)
    }

    /// Perform migration from old /tmp sandbox to new workspace
    pub async fn migrate(manager: &WorkspaceManager) -> Result<Vec<MigrationResult>> {
        let mut results = Vec::new();

        let tmp_dir = std::path::Path::new("/tmp");
        let entries = tmp_dir.read_dir()?;

        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();

            if name_str.starts_with("zerobuild-sandbox-") {
                let old_path = entry.path();

                // Extract UUID from old path
                if let Some(uuid_str) = name_str.strip_prefix("zerobuild-sandbox-") {
                    if let Ok(uuid) = Uuid::parse_str(uuid_str) {
                        // Create new workspace
                        let workspace_id = WorkspaceId {
                            agent_role: "migrated".to_string(),
                            agent_uuid: uuid,
                        };

                        match manager.create_workspace(&workspace_id, "migrated").await {
                            Ok(workspace) => {
                                // Copy files from old to new
                                match Self::migrate_sandbox_contents(
                                    &old_path,
                                    &workspace.paths.sandbox,
                                )
                                .await
                                {
                                    Ok(_) => {
                                        results.push(MigrationResult {
                                            old_path: old_path.clone(),
                                            new_workspace: workspace_id,
                                            success: true,
                                            error: None,
                                        });
                                    }
                                    Err(e) => {
                                        results.push(MigrationResult {
                                            old_path: old_path.clone(),
                                            new_workspace: workspace_id,
                                            success: false,
                                            error: Some(e.to_string()),
                                        });
                                    }
                                }
                            }
                            Err(e) => {
                                results.push(MigrationResult {
                                    old_path: old_path.clone(),
                                    new_workspace: workspace_id,
                                    success: false,
                                    error: Some(e.to_string()),
                                });
                            }
                        }
                    }
                }
            }
        }

        Ok(results)
    }

    async fn migrate_sandbox_contents(from: &Path, to: &Path) -> Result<()> {
        use tokio::fs;

        // Create destination if needed
        fs::create_dir_all(to).await?;

        // Copy all contents recursively
        let mut entries = fs::read_dir(from).await?;

        while let Some(entry) = entries.next_entry().await? {
            let src = entry.path();
            let dst = to.join(entry.file_name());

            let metadata = entry.metadata().await?;

            if metadata.is_dir() {
                Box::pin(Self::migrate_sandbox_contents(&src, &dst)).await?;
            } else {
                fs::copy(&src, &dst).await?;
            }
        }

        Ok(())
    }
}

/// Result of a migration operation
#[derive(Debug, Clone)]
pub struct MigrationResult {
    pub old_path: PathBuf,
    pub new_workspace: WorkspaceId,
    pub success: bool,
    pub error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_path_validation() {
        let temp = TempDir::new().unwrap();
        let paths = WorkspacePaths::new(temp.path(), &WorkspaceId::new("test"));

        // Create workspace structure
        std::fs::create_dir_all(&paths.root).unwrap();
        std::fs::create_dir_all(&paths.sandbox).unwrap();

        let isolation = WorkspaceIsolation::new(paths, SandboxType::FullIsolation);

        // Path within workspace should be allowed
        let valid_path = isolation.resolve_sandbox_path("test.txt");
        assert!(isolation.is_path_allowed(&valid_path));

        // Path outside workspace should be denied
        assert!(!isolation.is_path_allowed(Path::new("/etc/passwd")));
    }

    #[test]
    fn test_disk_usage() {
        let temp = TempDir::new().unwrap();
        let paths = WorkspacePaths::new(temp.path(), &WorkspaceId::new("test"));

        std::fs::create_dir_all(&paths.root).unwrap();

        // Create some test files
        std::fs::write(paths.root.join("file1.txt"), "Hello").unwrap();
        std::fs::write(paths.root.join("file2.txt"), "World!!!").unwrap();

        let isolation = WorkspaceIsolation::new(paths, SandboxType::FullIsolation);
        let usage = isolation.get_disk_usage().unwrap();

        assert_eq!(usage.file_count, 2);
        assert!(usage.total_size_bytes > 0);
    }
}
