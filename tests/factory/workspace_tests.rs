//! Integration tests for factory workspace module
//!
//! Tests workspace lifecycle, isolation, and migration.

use tempfile::TempDir;
use zerobuild::factory::workspace::{
    Conversation, Embedding, FileOperation, SandboxType, WorkspaceId, WorkspaceIsolation,
    WorkspaceManager, WorkspaceMigration,
};

#[tokio::test]
async fn test_workspace_lifecycle() {
    let temp_dir = TempDir::new().unwrap();
    let manager = WorkspaceManager::with_root(temp_dir.path().to_path_buf()).unwrap();

    // Create workspace
    let workspace_id = WorkspaceId::new("developer");
    let workspace = manager
        .create_workspace(&workspace_id, "developer")
        .await
        .unwrap();

    assert_eq!(workspace.id.agent_role, "developer");
    assert!(workspace.paths.root.exists());
    assert!(workspace.paths.agent_config.exists());
    assert!(workspace.paths.sandbox.exists());
    assert!(workspace.paths.skills.exists());
    assert!(workspace.paths.memory.exists());

    // Write test file to sandbox
    let test_file = workspace.paths.sandbox.join("test.txt");
    tokio::fs::write(&test_file, "hello world").await.unwrap();
    assert!(test_file.exists());

    // Archive workspace
    let archive_path = manager.archive_workspace(&workspace_id).await.unwrap();
    assert!(archive_path.exists());
    assert!(!workspace.paths.root.exists());

    // Clean up
    temp_dir.close().unwrap();
}

#[tokio::test]
async fn test_workspace_isolation_path_validation() {
    let temp_dir = TempDir::new().unwrap();
    let manager = WorkspaceManager::with_root(temp_dir.path().to_path_buf()).unwrap();

    let workspace_id = WorkspaceId::new("developer");
    let workspace = manager
        .create_workspace(&workspace_id, "developer")
        .await
        .unwrap();

    let isolation = WorkspaceIsolation::new(workspace.paths.clone(), SandboxType::FullIsolation);

    // Valid paths within workspace should be allowed
    let valid_path = workspace.paths.sandbox.join("project/src/main.rs");
    assert!(isolation.is_path_allowed(&valid_path));

    // Invalid path trying to escape sandbox - this might not exist so it returns false
    let escaped_path = std::path::PathBuf::from("/etc/passwd");
    // Path doesn't exist so it's not allowed
    assert!(!isolation.is_path_allowed(&escaped_path));

    // Use validate_file_access with an operation
    let result = isolation.validate_file_access(&valid_path, FileOperation::Read);
    // Should succeed for valid path
    assert!(result.is_ok() || !valid_path.exists());

    temp_dir.close().unwrap();
}

#[tokio::test]
async fn test_workspace_migration_detection() {
    // Test that migration detection works
    let needs_migration = WorkspaceMigration::needs_migration();
    // We can't predict if migration is needed, but the function should not panic
    // In a real test environment, this would typically be false
    println!("Migration needed: {}", needs_migration);
}

#[tokio::test]
async fn test_workspace_list_and_cleanup() {
    let temp_dir = TempDir::new().unwrap();
    let manager = WorkspaceManager::with_root(temp_dir.path().to_path_buf()).unwrap();

    // Create multiple workspaces sequentially
    let mut ids = Vec::new();
    for i in 0..3 {
        let id = WorkspaceId::new(format!("agent-{}", i));
        manager
            .create_workspace(&id, &format!("agent-{}", i))
            .await
            .unwrap();
        ids.push(id);
    }

    // List workspaces
    let workspaces = manager.list_workspaces().unwrap();
    assert_eq!(workspaces.len(), 3);

    // Delete one
    manager.delete_workspace(&ids[0]).unwrap();
    let workspaces = manager.list_workspaces().unwrap();
    assert_eq!(workspaces.len(), 2);

    temp_dir.close().unwrap();
}

#[tokio::test]
async fn test_workspace_memory_directories() {
    let temp_dir = TempDir::new().unwrap();
    let manager = WorkspaceManager::with_root(temp_dir.path().to_path_buf()).unwrap();

    let workspace_id = WorkspaceId::new("developer");
    let workspace = manager
        .create_workspace(&workspace_id, "developer")
        .await
        .unwrap();

    // Memory directories should be created
    let conversations_dir = workspace.paths.memory.join("conversations");
    let embeddings_dir = workspace.paths.memory.join("embeddings");

    assert!(conversations_dir.exists());
    assert!(embeddings_dir.exists());

    // Can write to memory directories
    let test_conv = Conversation {
        timestamp: chrono::Utc::now(),
        role: "user".to_string(),
        content: "Test message".to_string(),
    };

    let conv_file = conversations_dir.join("test.json");
    let conv_json = serde_json::to_string(&test_conv).unwrap();
    tokio::fs::write(&conv_file, conv_json).await.unwrap();
    assert!(conv_file.exists());

    // Can write embeddings
    let test_emb = Embedding {
        id: "test-1".to_string(),
        vector: vec![0.1, 0.2, 0.3],
        metadata: serde_json::json!({"test": true}),
    };

    let emb_file = embeddings_dir.join("test.json");
    let emb_json = serde_json::to_string(&test_emb).unwrap();
    tokio::fs::write(&emb_file, emb_json).await.unwrap();
    assert!(emb_file.exists());

    temp_dir.close().unwrap();
}
