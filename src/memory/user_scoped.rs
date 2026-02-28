//! User-Scoped Memory
//!
//! Provides memory isolation per user while maintaining a shared global memory
//! for system-wide knowledge (tools, skills, patterns).
//!
//! Architecture:
//! - Each user gets their own memory instance (SQLite/Markdown)
//! - Global memory for shared data (skills, tools, system patterns)
//! - Automatic user memory creation on first access
//! - LRU cache to limit memory usage

use super::traits::{Memory, MemoryCategory, MemoryEntry};
use super::{create_memory, MemoryConfig};
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Memory scope for operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryScope {
    /// User-specific memory only
    User,
    /// Global/shared memory only
    Global,
    /// Both user and global (default)
    Both,
}

/// User-scoped memory manager
pub struct UserScopedMemory {
    /// Base workspace directory
    workspace_dir: PathBuf,
    /// Memory configuration template
    config_template: MemoryConfig,
    /// Global shared memory
    global_memory: Arc<dyn Memory>,
    /// Per-user memory instances (user_id -> memory)
    user_memories: Mutex<HashMap<String, Arc<dyn Memory>>>,
    /// API key for embedding (if needed)
    api_key: Option<String>,
}

impl UserScopedMemory {
    /// Create new user-scoped memory manager
    pub fn new(
        workspace_dir: &Path,
        config: &MemoryConfig,
        api_key: Option<&str>,
    ) -> anyhow::Result<Self> {
        // Create global memory directory
        let global_dir = workspace_dir.join("memory").join("global");
        std::fs::create_dir_all(&global_dir)?;

        // Create global memory instance
        let global_config = MemoryConfig {
            backend: config.backend.clone(),
            ..config.clone()
        };
        let global_memory = create_memory(&global_config, &global_dir, api_key)?;

        Ok(Self {
            workspace_dir: workspace_dir.to_path_buf(),
            config_template: config.clone(),
            global_memory: Arc::from(global_memory),
            user_memories: Mutex::new(HashMap::new()),
            api_key: api_key.map(String::from),
        })
    }

    /// Get or create user-specific memory (public for access)
    pub fn get_user_memory(&self, user_id: &str) -> anyhow::Result<Arc<dyn Memory>> {
        // Check cache first
        {
            let memories = self.user_memories.lock().unwrap();
            if let Some(memory) = memories.get(user_id) {
                return Ok(memory.clone());
            }
        }

        // Create new memory for this user
        let user_dir = self
            .workspace_dir
            .join("memory")
            .join(format!("user_{}", user_id));
        std::fs::create_dir_all(&user_dir)?;

        let user_config = MemoryConfig {
            backend: self.config_template.backend.clone(),
            ..self.config_template.clone()
        };
        
        let user_memory: Arc<dyn Memory> = Arc::from(
            create_memory(&user_config, &user_dir, self.api_key.as_deref())?
        );

        // Store in cache
        {
            let mut memories = self.user_memories.lock().unwrap();
            memories.insert(user_id.to_string(), user_memory.clone());
        }

        tracing::info!("Created user-scoped memory for user_id={}", user_id);
        Ok(user_memory)
    }

    /// Store with explicit user_id
    pub async fn store_for_user(
        &self,
        user_id: &str,
        key: &str,
        content: &str,
        category: MemoryCategory,
        session_id: Option<&str>,
    ) -> anyhow::Result<()> {
        let memory = self.get_user_memory(user_id)?;
        memory.store(key, content, category, session_id).await
    }

    /// Recall with explicit user_id
    pub async fn recall_for_user(
        &self,
        user_id: &str,
        query: &str,
        limit: usize,
        session_id: Option<&str>,
        scope: MemoryScope,
    ) -> anyhow::Result<Vec<MemoryEntry>> {
        let mut results = Vec::new();

        // Get from user memory
        if scope == MemoryScope::User || scope == MemoryScope::Both {
            match self.get_user_memory(user_id) {
                Ok(memory) => {
                    let user_results = memory.recall(query, limit, session_id).await?;
                    results.extend(user_results);
                }
                Err(e) => {
                    tracing::warn!("Failed to get user memory for {}: {}", user_id, e);
                }
            }
        }

        // Get from global memory
        if scope == MemoryScope::Global || scope == MemoryScope::Both {
            let global_results = self.global_memory.recall(query, limit, session_id).await?;
            results.extend(global_results);
        }

        // Sort by score and limit
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(limit);

        Ok(results)
    }

    /// Get from user memory
    pub async fn get_for_user(
        &self,
        user_id: &str,
        key: &str,
    ) -> anyhow::Result<Option<MemoryEntry>> {
        let memory = self.get_user_memory(user_id)?;
        memory.get(key).await
    }

    /// List from user memory
    pub async fn list_for_user(
        &self,
        user_id: &str,
        category: Option<&MemoryCategory>,
        session_id: Option<&str>,
    ) -> anyhow::Result<Vec<MemoryEntry>> {
        let memory = self.get_user_memory(user_id)?;
        memory.list(category, session_id).await
    }

    /// Forget from user memory
    pub async fn forget_for_user(
        &self,
        user_id: &str,
        key: &str,
    ) -> anyhow::Result<bool> {
        let memory = self.get_user_memory(user_id)?;
        memory.forget(key).await
    }

    /// Count user memories
    pub async fn count_for_user(&self, user_id: &str) -> anyhow::Result<usize> {
        let memory = self.get_user_memory(user_id)?;
        memory.count().await
    }

    /// Get global memory (for system operations)
    pub fn global(&self) -> Arc<dyn Memory> {
        self.global_memory.clone()
    }

    /// Clear user from cache (for cleanup)
    pub fn clear_user_cache(&self, user_id: &str) {
        let mut memories = self.user_memories.lock().unwrap();
        memories.remove(user_id);
    }

    /// List active users in cache
    pub fn active_users(&self) -> Vec<String> {
        let memories = self.user_memories.lock().unwrap();
        memories.keys().cloned().collect()
    }

    /// Store to global memory (system-wide)
    pub async fn store_global(
        &self,
        key: &str,
        content: &str,
        category: MemoryCategory,
        session_id: Option<&str>,
    ) -> anyhow::Result<()> {
        self.global_memory
            .store(key, content, category, session_id)
            .await
    }

    /// Get user memory directory path
    pub fn user_memory_path(&self, user_id: &str) -> PathBuf {
        self.workspace_dir
            .join("memory")
            .join(format!("user_{}", user_id))
    }

    /// Check if user has memory
    pub fn user_exists(&self, user_id: &str) -> bool {
        let memories = self.user_memories.lock().unwrap();
        memories.contains_key(user_id) || self.user_memory_path(user_id).exists()
    }
}

// Implement Memory trait for UserScopedMemory (delegates to global memory)
// This allows backward compatibility with code expecting Arc<dyn Memory>
#[async_trait]
impl Memory for UserScopedMemory {
    fn name(&self) -> &str {
        "user_scoped"
    }

    async fn store(
        &self,
        key: &str,
        content: &str,
        category: MemoryCategory,
        session_id: Option<&str>,
    ) -> anyhow::Result<()> {
        self.global_memory.store(key, content, category, session_id).await
    }

    async fn recall(
        &self,
        query: &str,
        limit: usize,
        session_id: Option<&str>,
    ) -> anyhow::Result<Vec<MemoryEntry>> {
        self.global_memory.recall(query, limit, session_id).await
    }

    async fn get(&self, key: &str) -> anyhow::Result<Option<MemoryEntry>> {
        self.global_memory.get(key).await
    }

    async fn list(
        &self,
        category: Option<&MemoryCategory>,
        session_id: Option<&str>,
    ) -> anyhow::Result<Vec<MemoryEntry>> {
        self.global_memory.list(category, session_id).await
    }

    async fn forget(&self, key: &str) -> anyhow::Result<bool> {
        self.global_memory.forget(key).await
    }

    async fn count(&self) -> anyhow::Result<usize> {
        self.global_memory.count().await
    }

    async fn health_check(&self) -> bool {
        self.global_memory.health_check().await
    }
}

/// Adapter to make UserScopedMemory work as standard Memory trait
/// This requires the user_id to be provided at construction time
pub struct UserMemorySession {
    user_id: String,
    scoped: Arc<UserScopedMemory>,
}

impl UserMemorySession {
    pub fn new(user_id: String, scoped: Arc<UserScopedMemory>) -> Self {
        Self { user_id, scoped }
    }
}

#[async_trait]
impl Memory for UserMemorySession {
    fn name(&self) -> &str {
        "user_scoped"
    }

    async fn store(
        &self,
        key: &str,
        content: &str,
        category: MemoryCategory,
        session_id: Option<&str>,
    ) -> anyhow::Result<()> {
        self.scoped
            .store_for_user(&self.user_id, key, content, category, session_id)
            .await
    }

    async fn recall(
        &self,
        query: &str,
        limit: usize,
        session_id: Option<&str>,
    ) -> anyhow::Result<Vec<MemoryEntry>> {
        self.scoped
            .recall_for_user(&self.user_id, query, limit, session_id, MemoryScope::Both)
            .await
    }

    async fn get(&self, key: &str) -> anyhow::Result<Option<MemoryEntry>> {
        self.scoped.get_for_user(&self.user_id, key).await
    }

    async fn list(
        &self,
        category: Option<&MemoryCategory>,
        session_id: Option<&str>,
    ) -> anyhow::Result<Vec<MemoryEntry>> {
        self.scoped.list_for_user(&self.user_id, category, session_id).await
    }

    async fn forget(&self, key: &str) -> anyhow::Result<bool> {
        self.scoped.forget_for_user(&self.user_id, key).await
    }

    async fn count(&self) -> anyhow::Result<usize> {
        self.scoped.count_for_user(&self.user_id).await
    }

    async fn health_check(&self) -> bool {
        match self.scoped.get_user_memory(&self.user_id) {
            Ok(mem) => mem.health_check().await,
            Err(_) => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_config() -> MemoryConfig {
        MemoryConfig {
            backend: "markdown".into(),
            ..MemoryConfig::default()
        }
    }

    #[test]
    fn user_scoped_memory_creates_directories() {
        let tmp = TempDir::new().unwrap();
        let scoped = UserScopedMemory::new(tmp.path(), &test_config(), None).unwrap();

        // Global should exist
        assert!(tmp.path().join("memory").join("global").exists());

        // User shouldn't exist yet
        assert!(!scoped.user_exists("test_user"));
    }

    #[tokio::test]
    async fn user_scoped_isolates_data() {
        let tmp = TempDir::new().unwrap();
        let scoped = Arc::new(UserScopedMemory::new(tmp.path(), &test_config(), None).unwrap());

        // Store for user A
        scoped
            .store_for_user("user_a", "key1", "User A data", MemoryCategory::Core, None)
            .await
            .unwrap();

        // Store for user B
        scoped
            .store_for_user("user_b", "key1", "User B data", MemoryCategory::Core, None)
            .await
            .unwrap();

        // Recall for user A
        let results_a = scoped
            .recall_for_user("user_a", "data", 10, None, MemoryScope::User)
            .await
            .unwrap();
        assert_eq!(results_a.len(), 1);
        assert!(results_a[0].content.contains("User A"));

        // Recall for user B
        let results_b = scoped
            .recall_for_user("user_b", "data", 10, None, MemoryScope::User)
            .await
            .unwrap();
        assert_eq!(results_b.len(), 1);
        assert!(results_b[0].content.contains("User B"));
    }

    #[tokio::test]
    async fn global_memory_shared_across_users() {
        let tmp = TempDir::new().unwrap();
        let scoped = Arc::new(UserScopedMemory::new(tmp.path(), &test_config(), None).unwrap());

        // Store to global
        scoped
            .store_global("shared_key", "Shared data", MemoryCategory::Core, None)
            .await
            .unwrap();

        // Both users can see global
        let results_a = scoped
            .recall_for_user("user_a", "shared", 10, None, MemoryScope::Both)
            .await
            .unwrap();
        assert!(results_a.iter().any(|r| r.content.contains("Shared")));

        let results_b = scoped
            .recall_for_user("user_b", "shared", 10, None, MemoryScope::Both)
            .await
            .unwrap();
        assert!(results_b.iter().any(|r| r.content.contains("Shared")));
    }

    #[test]
    fn user_memory_session_adapter() {
        let tmp = TempDir::new().unwrap();
        let scoped = Arc::new(UserScopedMemory::new(tmp.path(), &test_config(), None).unwrap());
        let session = UserMemorySession::new("test_user".into(), scoped);

        assert_eq!(session.name(), "user_scoped");
    }
}
