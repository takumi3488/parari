//! Integration tests for parari
//!
//! These tests verify the full workflow using MockExecutor

use std::sync::Arc;

use parari::domain::{TaskRunner, apply_result};
use parari::executor::mock::MockExecutor;
use parari::executor::traits::Executor;

fn unique_temp_dir(name: &str) -> std::path::PathBuf {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("parari_test_{}_{}", name, timestamp))
}

async fn setup_git_repo(path: &std::path::Path) {
    tokio::fs::create_dir_all(path).await.unwrap();

    tokio::process::Command::new("git")
        .args(["init"])
        .current_dir(path)
        .output()
        .await
        .unwrap();

    // Configure git for this test repo
    tokio::process::Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(path)
        .output()
        .await
        .unwrap();

    tokio::process::Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(path)
        .output()
        .await
        .unwrap();

    tokio::fs::write(path.join("README.md"), "# Test Project\n")
        .await
        .unwrap();

    tokio::process::Command::new("git")
        .args(["add", "."])
        .current_dir(path)
        .output()
        .await
        .unwrap();

    tokio::process::Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(path)
        .output()
        .await
        .unwrap();
}

/// Test the full workflow:
/// 1. Create worktrees for multiple executors
/// 2. Each executor creates different files
/// 3. Select a result and apply it
/// 4. Verify the changes are applied
#[tokio::test]
async fn test_full_workflow_with_mock_executors() {
    let temp_dir = unique_temp_dir("full_workflow");
    if temp_dir.exists() {
        tokio::fs::remove_dir_all(&temp_dir).await.unwrap();
    }

    setup_git_repo(&temp_dir).await;

    // Create mock executors with different file outputs
    let claude_mock = Arc::new(
        MockExecutor::new("claude")
            .with_file(
                "src/main.rs",
                "fn main() { println!(\"Hello from Claude!\"); }",
            )
            .with_file("src/lib.rs", "pub fn claude_helper() {}")
            .with_success("Created files from Claude"),
    ) as Arc<dyn Executor>;

    let gemini_mock = Arc::new(
        MockExecutor::new("gemini")
            .with_file(
                "src/main.rs",
                "fn main() { println!(\"Hello from Gemini!\"); }",
            )
            .with_file("src/utils.rs", "pub fn gemini_util() {}")
            .with_success("Created files from Gemini"),
    ) as Arc<dyn Executor>;

    let executors: Vec<Arc<dyn Executor>> = vec![claude_mock, gemini_mock];

    // Create task runner and run
    let mut runner = TaskRunner::new(&temp_dir).await.unwrap();
    let results = runner
        .run("Create a Rust project", executors)
        .await
        .unwrap();

    // Verify we got results from both executors
    assert_eq!(results.len(), 2);

    // Find Claude's result
    let claude_result = results
        .iter()
        .find(|r| r.execution.executor_name == "claude")
        .unwrap();

    // Verify Claude's worktree has the expected files
    let claude_main = tokio::fs::read_to_string(claude_result.worktree_path.join("src/main.rs"))
        .await
        .unwrap();
    assert!(claude_main.contains("Hello from Claude!"));

    let claude_lib = tokio::fs::read_to_string(claude_result.worktree_path.join("src/lib.rs"))
        .await
        .unwrap();
    assert!(claude_lib.contains("claude_helper"));

    // Apply Claude's result to the original directory
    apply_result(claude_result, &temp_dir).await.unwrap();

    // Verify the changes were applied
    let applied_main = tokio::fs::read_to_string(temp_dir.join("src/main.rs"))
        .await
        .unwrap();
    assert!(applied_main.contains("Hello from Claude!"));

    let applied_lib = tokio::fs::read_to_string(temp_dir.join("src/lib.rs"))
        .await
        .unwrap();
    assert!(applied_lib.contains("claude_helper"));

    // Cleanup
    runner.cleanup().await.unwrap();
    let _ = tokio::fs::remove_dir_all(&temp_dir).await;
}

/// Test that worktrees are properly cleaned up
#[tokio::test]
async fn test_worktree_cleanup() {
    let temp_dir = unique_temp_dir("cleanup");
    if temp_dir.exists() {
        tokio::fs::remove_dir_all(&temp_dir).await.unwrap();
    }

    setup_git_repo(&temp_dir).await;

    // Create mock executor
    let mock = Arc::new(
        MockExecutor::new("test_cleanup")
            .with_file("test.txt", "test content")
            .with_success("Done"),
    ) as Arc<dyn Executor>;

    let executors: Vec<Arc<dyn Executor>> = vec![mock];

    // Run and cleanup
    let mut runner = TaskRunner::new(&temp_dir).await.unwrap();
    let results = runner.run("Test", executors).await.unwrap();

    // Get worktree path before cleanup
    let worktree_path = results[0].worktree_path.clone();
    assert!(worktree_path.exists());

    // Cleanup
    runner.cleanup().await.unwrap();

    // Verify worktree is removed
    assert!(!worktree_path.exists());

    // Final cleanup
    let _ = tokio::fs::remove_dir_all(&temp_dir).await;
}

/// Test change summary detection
#[tokio::test]
async fn test_change_summary() {
    let temp_dir = unique_temp_dir("change_summary");
    if temp_dir.exists() {
        tokio::fs::remove_dir_all(&temp_dir).await.unwrap();
    }

    setup_git_repo(&temp_dir).await;

    // Add an existing file
    tokio::fs::write(temp_dir.join("existing.txt"), "existing content\n")
        .await
        .unwrap();

    tokio::process::Command::new("git")
        .args(["add", "."])
        .current_dir(&temp_dir)
        .output()
        .await
        .unwrap();

    tokio::process::Command::new("git")
        .args(["commit", "-m", "Add existing file"])
        .current_dir(&temp_dir)
        .output()
        .await
        .unwrap();

    // Create mock executor that adds new files
    let mock = Arc::new(
        MockExecutor::new("test_summary")
            .with_file("new_file.txt", "new content")
            .with_file("another_new.txt", "more content")
            .with_success("Done"),
    ) as Arc<dyn Executor>;

    let executors: Vec<Arc<dyn Executor>> = vec![mock];

    // Run
    let mut runner = TaskRunner::new(&temp_dir).await.unwrap();
    let results = runner.run("Add files", executors).await.unwrap();

    assert_eq!(results.len(), 1);

    // Cleanup
    runner.cleanup().await.unwrap();
    let _ = tokio::fs::remove_dir_all(&temp_dir).await;
}
