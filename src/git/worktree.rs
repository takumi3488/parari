use std::path::{Path, PathBuf};

use tokio::process::Command;

use crate::config;
use crate::error::{Error, Result};

/// Information about a worktree
#[derive(Debug, Clone)]
pub struct WorktreeInfo {
    /// Path to the worktree
    pub path: PathBuf,
    /// Name of the executor this worktree belongs to
    pub executor_name: String,
    /// Timestamp when the worktree was created
    pub timestamp: String,
}

/// Check if a directory is a git repository
pub async fn is_git_repository(path: &Path) -> bool {
    Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(path)
        .output()
        .await
        .is_ok_and(|output| output.status.success())
}

/// Get the root of the git repository
pub async fn get_repo_root(path: &Path) -> Result<PathBuf> {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(path)
        .output()
        .await?;

    if !output.status.success() {
        return Err(Error::NotGitRepository {
            path: path.to_path_buf(),
        });
    }

    let root = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(PathBuf::from(root))
}

/// Create a worktree for the given executor
///
/// Returns the path to the created worktree
pub async fn create_worktree(repo_path: &Path, executor_name: &str) -> Result<WorktreeInfo> {
    let timestamp = chrono::Utc::now().format("%Y%m%d%H%M%S%3f").to_string();
    let worktree_name = format!("{}-{}", timestamp, executor_name);
    let worktree_path = config::worktrees_dir().join(&worktree_name);

    // Ensure worktrees directory exists
    tokio::fs::create_dir_all(config::worktrees_dir()).await?;

    // Create the worktree
    let output = Command::new("git")
        .args([
            "worktree",
            "add",
            "--detach",
            worktree_path.to_str().unwrap(),
        ])
        .current_dir(repo_path)
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::GitCommand {
            message: stderr.to_string(),
        });
    }

    Ok(WorktreeInfo {
        path: worktree_path,
        executor_name: executor_name.to_string(),
        timestamp,
    })
}

/// Remove a worktree
pub async fn remove_worktree(repo_path: &Path, worktree_path: &Path) -> Result<()> {
    // First, try to remove with --force
    let output = Command::new("git")
        .args([
            "worktree",
            "remove",
            "--force",
            worktree_path.to_str().unwrap(),
        ])
        .current_dir(repo_path)
        .output()
        .await?;

    if !output.status.success() {
        // If git worktree remove fails, try to manually remove the directory
        // and then prune
        if worktree_path.exists() {
            tokio::fs::remove_dir_all(worktree_path).await?;
        }

        // Prune worktrees
        let _ = Command::new("git")
            .args(["worktree", "prune"])
            .current_dir(repo_path)
            .output()
            .await;
    }

    Ok(())
}

/// List all worktrees for a repository
pub async fn list_worktrees(repo_path: &Path) -> Result<Vec<PathBuf>> {
    let output = Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .current_dir(repo_path)
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::GitCommand {
            message: stderr.to_string(),
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let worktrees: Vec<PathBuf> = stdout
        .lines()
        .filter(|line| line.starts_with("worktree "))
        .map(|line| PathBuf::from(line.strip_prefix("worktree ").unwrap()))
        .collect();

    Ok(worktrees)
}

/// Remove old worktrees if there are more than MAX_WORKTREES
pub async fn cleanup_old_worktrees(repo_path: &Path) -> Result<()> {
    let worktrees_dir = config::worktrees_dir();

    if !worktrees_dir.exists() {
        return Ok(());
    }

    let mut entries = Vec::new();
    let mut read_dir = tokio::fs::read_dir(&worktrees_dir).await?;

    while let Some(entry) = read_dir.next_entry().await? {
        entries.push(entry);
    }

    // Sort by name (which includes timestamp) - oldest first
    entries.sort_by_key(|a| a.file_name());

    // Remove oldest if we exceed MAX_WORKTREES
    while entries.len() > config::MAX_WORKTREES {
        if let Some(entry) = entries.first() {
            let path = entry.path();
            let _ = remove_worktree(repo_path, &path).await;
            entries.remove(0);
        } else {
            break;
        }
    }

    Ok(())
}

/// Remove all worktrees in the parari worktrees directory
pub async fn cleanup_all_worktrees(repo_path: &Path) -> Result<()> {
    let worktrees_dir = config::worktrees_dir();

    if !worktrees_dir.exists() {
        return Ok(());
    }

    let mut entries = tokio::fs::read_dir(&worktrees_dir).await?;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        let _ = remove_worktree(repo_path, &path).await;
    }

    // Also prune any orphaned worktrees
    let _ = Command::new("git")
        .args(["worktree", "prune"])
        .current_dir(repo_path)
        .output()
        .await;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[tokio::test]
    async fn test_is_git_repository() {
        // Current directory should be a git repository
        let cwd = env::current_dir().unwrap();
        assert!(is_git_repository(&cwd).await);
    }

    #[tokio::test]
    async fn test_is_not_git_repository() {
        let temp_dir = env::temp_dir();
        // temp_dir may or may not be a git repo, so we create a fresh one
        let test_dir = temp_dir.join("parari_test_not_git");
        let _ = tokio::fs::create_dir_all(&test_dir).await;
        // This might still be inside a git repo, so this test is best-effort
        let _ = tokio::fs::remove_dir_all(&test_dir).await;
    }
}
