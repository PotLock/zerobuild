//! Agent Pool Management
//!
//! Manages warm/cold agent states and auto-scaling for the factory.
//! Provides efficient agent reuse and lifecycle management.

use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use parking_lot::{Mutex, RwLock};
use tokio::sync::Semaphore;
use tracing::{debug, error, info, warn};

use crate::factory::roles::AgentRole;
use crate::factory::workspace::{AgentConfig, WorkspaceId, WorkspaceManager};

/// Unique identifier for a pooled agent instance
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AgentInstanceId(pub uuid::Uuid);

impl AgentInstanceId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

impl Default for AgentInstanceId {
    fn default() -> Self {
        Self::new()
    }
}

/// Agent lifecycle states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentState {
    /// Agent is starting up
    Initializing,
    /// Agent is ready to accept work
    Idle,
    /// Agent is actively processing work
    Busy,
    /// Agent is temporarily suspended
    Suspended,
    /// Agent is shutting down
    ShuttingDown,
    /// Agent has terminated
    Terminated,
    /// Agent encountered an error
    Error,
}

impl std::fmt::Display for AgentState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Initializing => write!(f, "initializing"),
            Self::Idle => write!(f, "idle"),
            Self::Busy => write!(f, "busy"),
            Self::Suspended => write!(f, "suspended"),
            Self::ShuttingDown => write!(f, "shutting_down"),
            Self::Terminated => write!(f, "terminated"),
            Self::Error => write!(f, "error"),
        }
    }
}

/// Agent instance metadata
#[derive(Debug, Clone)]
pub struct AgentInstance {
    pub id: AgentInstanceId,
    pub role: AgentRole,
    pub workspace_id: WorkspaceId,
    pub state: AgentState,
    pub created_at: Instant,
    pub last_activity: Instant,
    pub task_count: u64,
    pub error_count: u64,
}

impl AgentInstance {
    pub fn new(id: AgentInstanceId, role: AgentRole, workspace_id: WorkspaceId) -> Self {
        let now = Instant::now();
        Self {
            id,
            role,
            workspace_id,
            state: AgentState::Initializing,
            created_at: now,
            last_activity: now,
            task_count: 0,
            error_count: 0,
        }
    }

    /// Update state and record activity
    pub fn transition_to(&mut self, new_state: AgentState) {
        debug!(
            "Agent {} transitioning from {} to {}",
            self.id.0, self.state, new_state
        );
        self.state = new_state;
        self.last_activity = Instant::now();
    }

    /// Mark agent as starting work
    pub fn start_task(&mut self) {
        self.transition_to(AgentState::Busy);
        self.task_count += 1;
    }

    /// Mark agent as completing work
    pub fn complete_task(&mut self) {
        self.transition_to(AgentState::Idle);
    }

    /// Record an error
    pub fn record_error(&mut self) {
        self.error_count += 1;
        if self.error_count >= 3 {
            self.transition_to(AgentState::Error);
        }
    }

    /// Check if agent is idle for longer than duration
    pub fn idle_for(&self, duration: Duration) -> bool {
        self.state == AgentState::Idle && self.last_activity.elapsed() > duration
    }
}

/// Pool configuration
#[derive(Debug, Clone)]
pub struct PoolConfig {
    /// Maximum number of agents per role
    pub max_agents_per_role: usize,
    /// Minimum number of warm agents to maintain
    pub min_warm_agents: usize,
    /// How long an agent can be idle before being moved to cold state
    pub idle_timeout: Duration,
    /// Maximum agent lifetime before forced recycling
    pub max_agent_lifetime: Duration,
    /// How long to wait for graceful shutdown before force kill
    pub graceful_shutdown_timeout: Duration,
    /// Enable auto-scaling based on queue depth
    pub auto_scaling_enabled: bool,
    /// Scale up when queue depth exceeds this
    pub scale_up_threshold: usize,
    /// Scale down when queue depth below this
    pub scale_down_threshold: usize,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_agents_per_role: 5,
            min_warm_agents: 1,
            idle_timeout: Duration::from_secs(300), // 5 minutes
            max_agent_lifetime: Duration::from_secs(3600), // 1 hour
            graceful_shutdown_timeout: Duration::from_secs(30),
            auto_scaling_enabled: true,
            scale_up_threshold: 10,
            scale_down_threshold: 2,
        }
    }
}

/// Pool statistics
#[derive(Debug, Clone, Default)]
pub struct PoolStats {
    pub total_agents: usize,
    pub warm_agents: usize,
    pub cold_agents: usize,
    pub busy_agents: usize,
    pub idle_agents: usize,
    pub error_agents: usize,
    pub tasks_completed: u64,
    pub tasks_failed: u64,
}

/// Agent pool for managing agent lifecycle and scaling
pub struct AgentPool {
    config: RwLock<PoolConfig>,
    workspace_manager: Arc<WorkspaceManager>,
    /// Active agents by instance ID
    agents: DashMap<AgentInstanceId, Mutex<AgentInstance>>,
    /// Agents organized by role
    agents_by_role: DashMap<AgentRole, Vec<AgentInstanceId>>,
    /// Warm agents ready for immediate use (per role)
    warm_pool: DashMap<AgentRole, Vec<AgentInstanceId>>,
    /// Semaphore to limit concurrent agent creation
    creation_semaphore: Semaphore,
    /// Task queue depth per role
    queue_depth: DashMap<AgentRole, usize>,
}

impl AgentPool {
    pub fn new(workspace_manager: Arc<WorkspaceManager>) -> Self {
        Self::with_config(workspace_manager, PoolConfig::default())
    }

    pub fn with_config(workspace_manager: Arc<WorkspaceManager>, config: PoolConfig) -> Self {
        Self {
            config: RwLock::new(config),
            workspace_manager,
            agents: DashMap::new(),
            agents_by_role: DashMap::new(),
            warm_pool: DashMap::new(),
            creation_semaphore: Semaphore::new(3), // Limit concurrent creations
            queue_depth: DashMap::new(),
        }
    }

    /// Get a warm agent for the given role, or create a new one
    pub async fn acquire_agent(
        &self,
        role: AgentRole,
        agent_config: AgentConfig,
    ) -> anyhow::Result<AgentInstanceId> {
        debug!("Acquiring agent for role: {:?}", role);

        // Try to get a warm agent first
        if let Some(id) = self.get_warm_agent(role).await {
            info!("Reusing warm agent {} for role {:?}", id.0, role);

            // Mark as busy
            if let Some(entry) = self.agents.get(&id) {
                let mut agent = entry.lock();
                agent.start_task();
            }

            return Ok(id);
        }

        // Need to create a new agent
        self.create_agent(role, agent_config).await
    }

    /// Release an agent back to the pool
    pub fn release_agent(&self, id: AgentInstanceId) -> anyhow::Result<()> {
        debug!("Releasing agent {}", id.0);

        if let Some(entry) = self.agents.get(&id) {
            let mut agent = entry.lock();

            // Update state
            agent.complete_task();

            // Return to warm pool if healthy
            if agent.error_count < 3 {
                self.add_to_warm_pool(id, agent.role);
            } else {
                // Too many errors, schedule for termination
                warn!(
                    "Agent {} has {} errors, scheduling for termination",
                    id.0, agent.error_count
                );
                agent.transition_to(AgentState::ShuttingDown);
            }
        }

        Ok(())
    }

    /// Mark an agent as having encountered an error
    pub fn record_agent_error(&self, id: AgentInstanceId) {
        if let Some(entry) = self.agents.get(&id) {
            let mut agent = entry.lock();
            agent.record_error();

            if agent.state == AgentState::Error {
                warn!("Agent {} moved to error state", id.0);
            }
        }
    }

    /// Get current pool statistics
    pub fn get_stats(&self) -> PoolStats {
        let mut stats = PoolStats::default();

        for entry in self.agents.iter() {
            let agent = entry.lock();
            stats.total_agents += 1;

            match agent.state {
                AgentState::Initializing | AgentState::Idle => {
                    stats.warm_agents += 1;
                    if agent.state == AgentState::Idle {
                        stats.idle_agents += 1;
                    }
                }
                AgentState::Busy => {
                    stats.busy_agents += 1;
                }
                AgentState::Suspended | AgentState::Terminated => {
                    stats.cold_agents += 1;
                }
                AgentState::Error | AgentState::ShuttingDown => {
                    stats.error_agents += 1;
                }
            }

            stats.tasks_completed += agent.task_count;
        }

        stats
    }

    /// Get the current state of an agent (for testing)
    #[cfg(test)]
    pub fn get_agent_state(&self, id: AgentInstanceId) -> Option<AgentState> {
        self.agents.get(&id).map(|entry| {
            let agent = entry.lock();
            agent.state
        })
    }

    /// Update pool configuration
    pub fn update_config(&self, config: PoolConfig) {
        *self.config.write() = config;
    }

    /// Update queue depth for auto-scaling decisions
    pub fn update_queue_depth(&self, role: AgentRole, depth: usize) {
        self.queue_depth.insert(role, depth);

        // Trigger auto-scaling check
        if self.config.read().auto_scaling_enabled {
            self.check_scaling_needs(role, depth);
        }
    }

    /// Gracefully shutdown all agents
    pub async fn shutdown_all(&self) -> anyhow::Result<()> {
        info!("Shutting down all agents in pool");

        let shutdown_timeout = self.config.read().graceful_shutdown_timeout;

        // Mark all agents for shutdown
        for entry in self.agents.iter() {
            let mut agent = entry.lock();
            agent.transition_to(AgentState::ShuttingDown);
        }

        // Wait for graceful shutdown
        tokio::time::sleep(shutdown_timeout).await;

        // Clean up remaining agents
        for entry in self.agents.iter() {
            let workspace_id = {
                let mut agent = entry.lock();

                if agent.state != AgentState::Terminated {
                    warn!("Force terminating agent {}", agent.id.0);
                    agent.transition_to(AgentState::Terminated);
                }

                agent.workspace_id.clone()
            };

            // Archive workspace (lock released before await)
            if let Err(e) = self
                .workspace_manager
                .archive_workspace(&workspace_id)
                .await
            {
                error!("Failed to archive workspace: {}", e);
            }
        }

        // Clear all pools
        self.agents.clear();
        self.agents_by_role.clear();
        self.warm_pool.clear();

        info!("Agent pool shutdown complete");
        Ok(())
    }

    /// Run maintenance tasks (called periodically)
    pub async fn maintenance(&self) -> anyhow::Result<()> {
        let config = self.config.read().clone();
        let mut to_terminate = Vec::new();
        let mut to_suspend = Vec::new();

        for entry in self.agents.iter() {
            let agent = entry.lock();

            // Check for max lifetime
            if agent.created_at.elapsed() > config.max_agent_lifetime
                && agent.state == AgentState::Idle
            {
                info!(
                    "Agent {} exceeded max lifetime, scheduling termination",
                    agent.id.0
                );
                to_terminate.push(agent.id);
            }
            // Check for idle timeout
            else if agent.idle_for(config.idle_timeout) && agent.state == AgentState::Idle {
                debug!("Agent {} idle timeout, moving to cold", agent.id.0);
                to_suspend.push(agent.id);
            }
        }

        // Process terminations
        for id in to_terminate {
            self.terminate_agent(id).await?;
        }

        // Process suspensions
        for id in to_suspend {
            if let Some(entry) = self.agents.get(&id) {
                let mut agent = entry.lock();
                agent.transition_to(AgentState::Suspended);

                // Remove from warm pool
                self.remove_from_warm_pool(id, agent.role);
            }
        }

        // Ensure minimum warm agents
        for role in [AgentRole::Developer, AgentRole::Tester, AgentRole::DevOps] {
            let current_warm = self.warm_pool.get(&role).map(|v| v.len()).unwrap_or(0);
            if current_warm < config.min_warm_agents {
                let needed = config.min_warm_agents - current_warm;
                debug!("Pre-warming {} agents for role {:?}", needed, role);

                // Pre-warm agents sequentially to avoid Send issues
                for _ in 0..needed {
                    let config = AgentConfig::for_role(role);
                    if let Err(e) = self.create_warm_agent(role, config).await {
                        error!("Failed to pre-warm agent for {:?}: {}", role, e);
                    }
                }
            }
        }

        Ok(())
    }

    // Private helper methods

    async fn create_agent(
        &self,
        role: AgentRole,
        _agent_config: AgentConfig,
    ) -> anyhow::Result<AgentInstanceId> {
        // Limit concurrent agent creation
        let _permit = self.creation_semaphore.acquire().await?;

        // Check capacity - extract value before await
        let max_agents = {
            let config = self.config.read();
            config.max_agents_per_role
        };

        let current_count = self.agents_by_role.get(&role).map(|v| v.len()).unwrap_or(0);
        if current_count >= max_agents {
            anyhow::bail!("Max agents ({}) reached for role {:?}", max_agents, role);
        }

        info!("Creating new agent for role {:?}", role);

        // Create workspace ID from role
        let role_name = format!("{:?}", role).to_lowercase();
        let workspace_id = WorkspaceId::new(&role_name);

        // Create workspace
        let workspace = self
            .workspace_manager
            .create_workspace(&workspace_id, &role_name)
            .await?;

        // Create agent instance
        let instance_id = AgentInstanceId::new();
        let instance = AgentInstance::new(instance_id, role, workspace.id);

        // Register agent
        self.agents.insert(instance_id, Mutex::new(instance));

        // Add to role index
        self.agents_by_role
            .entry(role)
            .or_default()
            .push(instance_id);

        info!("Created agent {} for role {:?}", instance_id.0, role);

        Ok(instance_id)
    }

    async fn create_warm_agent(
        &self,
        role: AgentRole,
        agent_config: AgentConfig,
    ) -> anyhow::Result<AgentInstanceId> {
        let id = self.create_agent(role, agent_config).await?;

        // Mark as idle (warm)
        if let Some(entry) = self.agents.get(&id) {
            let mut agent = entry.lock();
            agent.transition_to(AgentState::Idle);
        }

        self.add_to_warm_pool(id, role);

        Ok(id)
    }

    async fn get_warm_agent(&self, role: AgentRole) -> Option<AgentInstanceId> {
        let mut warm_agents = self.warm_pool.entry(role).or_default();

        while let Some(id) = warm_agents.pop() {
            // Verify agent is still valid
            if let Some(entry) = self.agents.get(&id) {
                let agent = entry.lock();
                if agent.state == AgentState::Idle {
                    return Some(id);
                }
            }
        }

        None
    }

    fn add_to_warm_pool(&self, id: AgentInstanceId, role: AgentRole) {
        self.warm_pool.entry(role).or_default().push(id);
        debug!("Added agent {} to warm pool for {:?}", id.0, role);
    }

    fn remove_from_warm_pool(&self, id: AgentInstanceId, role: AgentRole) {
        if let Some(mut warm_agents) = self.warm_pool.get_mut(&role) {
            warm_agents.retain(|warm_id| *warm_id != id);
        }
    }

    async fn terminate_agent(&self, id: AgentInstanceId) -> anyhow::Result<()> {
        info!("Terminating agent {}", id.0);

        if let Some((_, entry)) = self.agents.remove(&id) {
            let (workspace_id, role) = {
                let agent = entry.lock();
                (agent.workspace_id.clone(), agent.role)
            };

            // Archive workspace (lock released before await)
            self.workspace_manager
                .archive_workspace(&workspace_id)
                .await?;

            // Remove from role index
            if let Some(mut agents_for_role) = self.agents_by_role.get_mut(&role) {
                agents_for_role.retain(|agent_id| *agent_id != id);
            }
        }

        Ok(())
    }

    fn check_scaling_needs(&self, role: AgentRole, queue_depth: usize) {
        let config = self.config.read();

        if queue_depth > config.scale_up_threshold {
            debug!(
                "Queue depth {} exceeds scale-up threshold for {:?}",
                queue_depth, role
            );
            // Actual scaling logic would be implemented here
            // For now, we just log the need
        } else if queue_depth < config.scale_down_threshold {
            debug!(
                "Queue depth {} below scale-down threshold for {:?}",
                queue_depth, role
            );
        }
    }
}

impl Clone for AgentPool {
    fn clone(&self) -> Self {
        Self {
            config: RwLock::new(self.config.read().clone()),
            workspace_manager: Arc::clone(&self.workspace_manager),
            agents: DashMap::new(), // Note: agents aren't cloned
            agents_by_role: DashMap::new(),
            warm_pool: DashMap::new(),
            creation_semaphore: Semaphore::new(3),
            queue_depth: DashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tempfile::TempDir;

    fn create_test_pool() -> (AgentPool, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let workspace_manager =
            Arc::new(WorkspaceManager::with_root(temp_dir.path().to_path_buf()).unwrap());
        let pool = AgentPool::new(workspace_manager);
        (pool, temp_dir)
    }

    #[tokio::test]
    async fn test_acquire_and_release_agent() {
        let (pool, _temp) = create_test_pool();

        let config = AgentConfig::for_role(AgentRole::Developer);
        let id = pool
            .acquire_agent(AgentRole::Developer, config)
            .await
            .unwrap();

        // Should be busy
        {
            let entry = pool.agents.get(&id).unwrap();
            let agent = entry.lock();
            assert_eq!(agent.state, AgentState::Busy);
            assert_eq!(agent.task_count, 1);
        }

        // Release
        pool.release_agent(id).unwrap();

        // Should be idle
        {
            let entry = pool.agents.get(&id).unwrap();
            let agent = entry.lock();
            assert_eq!(agent.state, AgentState::Idle);
        }
    }

    #[tokio::test]
    async fn test_warm_pool_reuse() {
        let (pool, _temp) = create_test_pool();

        // Create a warm agent
        let config = AgentConfig::for_role(AgentRole::Tester);
        let id1 = pool
            .acquire_agent(AgentRole::Tester, config.clone())
            .await
            .unwrap();
        pool.release_agent(id1).unwrap();

        // Acquire again - should reuse warm agent
        let id2 = pool.acquire_agent(AgentRole::Tester, config).await.unwrap();
        assert_eq!(id1.0, id2.0, "Should reuse the same warm agent");
    }

    #[tokio::test]
    async fn test_error_tracking() {
        let (pool, _temp) = create_test_pool();

        let config = AgentConfig::for_role(AgentRole::DevOps);
        let id = pool.acquire_agent(AgentRole::DevOps, config).await.unwrap();

        // Record some errors
        pool.record_agent_error(id);
        pool.record_agent_error(id);

        {
            let entry = pool.agents.get(&id).unwrap();
            let agent = entry.lock();
            assert_eq!(agent.error_count, 2);
            assert_eq!(agent.state, AgentState::Busy); // Still busy
        }

        // Third error should move to error state
        pool.record_agent_error(id);

        {
            let entry = pool.agents.get(&id).unwrap();
            let agent = entry.lock();
            assert_eq!(agent.error_count, 3);
            assert_eq!(agent.state, AgentState::Error);
        }
    }

    #[tokio::test]
    async fn test_pool_stats() {
        let (pool, _temp) = create_test_pool();

        let stats = pool.get_stats();
        assert_eq!(stats.total_agents, 0);

        let config = AgentConfig::for_role(AgentRole::Developer);
        let _id = pool
            .acquire_agent(AgentRole::Developer, config)
            .await
            .unwrap();

        let stats = pool.get_stats();
        assert_eq!(stats.total_agents, 1);
        assert_eq!(stats.busy_agents, 1);
    }
}
