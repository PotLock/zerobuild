//! Integration tests for factory pool module
//!
//! Tests agent pool lifecycle, warm/cold states, and scaling.

use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use zerobuild::factory::pool::{AgentPool, AgentState, PoolConfig};
use zerobuild::factory::roles::AgentRole;
use zerobuild::factory::workspace::{AgentConfig, WorkspaceManager};

#[tokio::test]
async fn test_pool_create_and_acquire_agent() {
    let temp_dir = TempDir::new().unwrap();
    let workspace_manager =
        Arc::new(WorkspaceManager::with_root(temp_dir.path().to_path_buf()).unwrap());
    let pool = AgentPool::new(workspace_manager);

    // Initially no agents
    let stats_before = pool.get_stats();
    assert_eq!(stats_before.total_agents, 0);

    let config = AgentConfig::for_role(AgentRole::Developer);
    let id = pool
        .acquire_agent(AgentRole::Developer, config)
        .await
        .unwrap();

    // One agent should exist and be busy
    let stats_after = pool.get_stats();
    assert_eq!(stats_after.total_agents, 1);
    assert_eq!(stats_after.busy_agents, 1);

    temp_dir.close().unwrap();
}

#[tokio::test]
async fn test_pool_release_and_reuse() {
    let temp_dir = TempDir::new().unwrap();
    let workspace_manager =
        Arc::new(WorkspaceManager::with_root(temp_dir.path().to_path_buf()).unwrap());
    let pool = AgentPool::new(workspace_manager);

    // Acquire and release
    let config = AgentConfig::for_role(AgentRole::Tester);
    let id1 = pool
        .acquire_agent(AgentRole::Tester, config.clone())
        .await
        .unwrap();
    pool.release_agent(id1).unwrap();

    // After release, agent should be warm/idle
    let stats_after_release = pool.get_stats();
    assert_eq!(stats_after_release.total_agents, 1);
    assert_eq!(stats_after_release.idle_agents, 1);

    // Acquire again - should reuse warm agent
    let id2 = pool.acquire_agent(AgentRole::Tester, config).await.unwrap();

    // Should be busy again
    let stats_final = pool.get_stats();
    assert_eq!(stats_final.total_agents, 1);
    assert_eq!(stats_final.busy_agents, 1);

    temp_dir.close().unwrap();
}

#[tokio::test]
async fn test_pool_error_tracking() {
    let temp_dir = TempDir::new().unwrap();
    let workspace_manager =
        Arc::new(WorkspaceManager::with_root(temp_dir.path().to_path_buf()).unwrap());
    let pool = AgentPool::new(workspace_manager);

    let config = AgentConfig::for_role(AgentRole::DevOps);
    let id = pool.acquire_agent(AgentRole::DevOps, config).await.unwrap();

    // Record some errors
    pool.record_agent_error(id);
    pool.record_agent_error(id);

    // Release and acquire new agent
    pool.release_agent(id).unwrap();

    // Third error - should cause agent to be terminated
    pool.record_agent_error(id);

    // Stats should show error agents
    let stats = pool.get_stats();
    assert!(stats.error_agents > 0 || stats.total_agents == 0);

    temp_dir.close().unwrap();
}

#[tokio::test]
async fn test_pool_stats() {
    let temp_dir = TempDir::new().unwrap();
    let workspace_manager =
        Arc::new(WorkspaceManager::with_root(temp_dir.path().to_path_buf()).unwrap());
    let pool = AgentPool::new(workspace_manager);

    // Initially empty
    let stats = pool.get_stats();
    assert_eq!(stats.total_agents, 0);

    // Acquire agent
    let config = AgentConfig::for_role(AgentRole::Developer);
    let _id = pool
        .acquire_agent(AgentRole::Developer, config)
        .await
        .unwrap();

    let stats = pool.get_stats();
    assert_eq!(stats.total_agents, 1);
    assert_eq!(stats.busy_agents, 1);

    temp_dir.close().unwrap();
}

#[tokio::test]
async fn test_pool_max_agents_limit() {
    let temp_dir = TempDir::new().unwrap();
    let workspace_manager =
        Arc::new(WorkspaceManager::with_root(temp_dir.path().to_path_buf()).unwrap());

    // Configure with max 2 agents
    let mut config = PoolConfig::default();
    config.max_agents_per_role = 2;
    let pool = AgentPool::with_config(workspace_manager, config);

    // Acquire 2 agents
    let config = AgentConfig::for_role(AgentRole::Developer);
    let id1 = pool
        .acquire_agent(AgentRole::Developer, config.clone())
        .await
        .unwrap();
    let id2 = pool
        .acquire_agent(AgentRole::Developer, config.clone())
        .await
        .unwrap();

    // Verify we have 2 agents
    let stats = pool.get_stats();
    assert_eq!(stats.total_agents, 2);

    // Third acquisition should fail
    let result = pool.acquire_agent(AgentRole::Developer, config).await;
    assert!(result.is_err());

    temp_dir.close().unwrap();
}

#[tokio::test]
async fn test_pool_config_update() {
    let temp_dir = TempDir::new().unwrap();
    let workspace_manager =
        Arc::new(WorkspaceManager::with_root(temp_dir.path().to_path_buf()).unwrap());
    let pool = AgentPool::new(workspace_manager);

    // Update config
    let new_config = PoolConfig {
        max_agents_per_role: 10,
        min_warm_agents: 3,
        idle_timeout: Duration::from_secs(600),
        max_agent_lifetime: Duration::from_secs(7200),
        graceful_shutdown_timeout: Duration::from_secs(60),
        auto_scaling_enabled: false,
        scale_up_threshold: 20,
        scale_down_threshold: 5,
    };

    pool.update_config(new_config);

    // Verify by acquiring more agents than default limit
    let config = AgentConfig::for_role(AgentRole::Tester);
    for i in 0..5 {
        let id = pool.acquire_agent(AgentRole::Tester, config.clone()).await;
        assert!(id.is_ok(), "Should be able to acquire agent {}", i);
    }

    temp_dir.close().unwrap();
}

#[tokio::test]
async fn test_pool_queue_depth_tracking() {
    let temp_dir = TempDir::new().unwrap();
    let workspace_manager =
        Arc::new(WorkspaceManager::with_root(temp_dir.path().to_path_buf()).unwrap());
    let pool = AgentPool::new(workspace_manager);

    // Update queue depth
    pool.update_queue_depth(AgentRole::Developer, 15);
    pool.update_queue_depth(AgentRole::Tester, 5);

    // Just verify it doesn't panic - scaling logic would check these values
    temp_dir.close().unwrap();
}

#[tokio::test]
async fn test_pool_shutdown() {
    let temp_dir = TempDir::new().unwrap();
    let workspace_manager =
        Arc::new(WorkspaceManager::with_root(temp_dir.path().to_path_buf()).unwrap());
    let pool = AgentPool::new(workspace_manager);

    // Create some agents
    let config = AgentConfig::for_role(AgentRole::Developer);
    let _id1 = pool
        .acquire_agent(AgentRole::Developer, config.clone())
        .await
        .unwrap();
    let id2 = pool
        .acquire_agent(AgentRole::Developer, config)
        .await
        .unwrap();
    pool.release_agent(id2).unwrap();

    // Shutdown
    pool.shutdown_all().await.unwrap();

    // All agents should be cleared
    let stats = pool.get_stats();
    assert_eq!(stats.total_agents, 0);

    temp_dir.close().unwrap();
}

#[tokio::test]
async fn test_pool_warm_agents() {
    let temp_dir = TempDir::new().unwrap();
    let workspace_manager =
        Arc::new(WorkspaceManager::with_root(temp_dir.path().to_path_buf()).unwrap());

    // Configure with minimum warm agents
    let mut config = PoolConfig::default();
    config.min_warm_agents = 2;
    let pool = AgentPool::with_config(workspace_manager, config);

    // Acquire and release to create warm agents
    let agent_config = AgentConfig::for_role(AgentRole::Developer);
    let id1 = pool
        .acquire_agent(AgentRole::Developer, agent_config.clone())
        .await
        .unwrap();
    pool.release_agent(id1).unwrap();

    let id2 = pool
        .acquire_agent(AgentRole::Developer, agent_config)
        .await
        .unwrap();
    pool.release_agent(id2).unwrap();

    // Both should now be warm
    let stats = pool.get_stats();
    assert_eq!(stats.warm_agents, 2);
    assert_eq!(stats.idle_agents, 2);

    temp_dir.close().unwrap();
}
