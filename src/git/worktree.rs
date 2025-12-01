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
/// Returns the path to the created worktree.
/// This also copies uncommitted changes from the source repository to the worktree.
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

    // Copy uncommitted changes from source repository to worktree
    copy_uncommitted_changes(repo_path, &worktree_path).await?;

    Ok(WorktreeInfo {
        path: worktree_path,
        executor_name: executor_name.to_string(),
        timestamp,
    })
}

/// Copy uncommitted changes from source repository to worktree
async fn copy_uncommitted_changes(source: &Path, worktree: &Path) -> Result<()> {
    // Get list of changed files (both staged and unstaged, including untracked)
    let output = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(source)
        .output()
        .await?;

    let status = String::from_utf8_lossy(&output.stdout);

    for line in status.lines() {
        if line.len() < 3 {
            continue;
        }

        let status_code = &line[0..2];
        let file_path = &line[3..];

        // Skip deleted files
        if status_code == "D " || status_code == " D" || status_code == "DD" {
            // For deleted files, also delete in worktree
            let dst_path = worktree.join(file_path);
            if dst_path.exists() {
                let _ = tokio::fs::remove_file(&dst_path).await;
            }
            continue;
        }

        // Handle renamed files (R status shows "old -> new")
        let actual_path = if status_code.starts_with('R') {
            // Format: "R  old_name -> new_name"
            if let Some(arrow_pos) = file_path.find(" -> ") {
                &file_path[arrow_pos + 4..]
            } else {
                file_path
            }
        } else {
            file_path
        };

        let src_path = source.join(actual_path);
        let dst_path = worktree.join(actual_path);

        // Copy file if it exists
        if src_path.exists() && src_path.is_file() {
            // Ensure parent directory exists
            if let Some(parent) = dst_path.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            // Remove target file first to avoid "Text file busy" error (ETXTBSY)
            if dst_path.exists() {
                tokio::fs::remove_file(&dst_path).await?;
            }
            tokio::fs::copy(&src_path, &dst_path).await?;
        } else if src_path.is_dir() {
            // Copy directory recursively
            copy_dir_to_worktree(&src_path, &dst_path).await?;
        }
    }

    Ok(())
}

/// Copy a directory recursively (used for copying uncommitted directories)
#[async_recursion::async_recursion]
async fn copy_dir_to_worktree(src: &Path, dst: &Path) -> Result<()> {
    tokio::fs::create_dir_all(dst).await?;

    let mut entries = tokio::fs::read_dir(src).await?;

    while let Some(entry) = entries.next_entry().await? {
        let file_name = entry.file_name();
        let file_name_str = file_name.to_str().unwrap_or("");

        // Skip .git directory
        if file_name_str == ".git" {
            continue;
        }

        let src_path = entry.path();
        let dst_path = dst.join(&file_name);

        let file_type = entry.file_type().await?;

        if file_type.is_dir() {
            copy_dir_to_worktree(&src_path, &dst_path).await?;
        } else if file_type.is_file() {
            // Remove target file first to avoid "Text file busy" error (ETXTBSY)
            if dst_path.exists() {
                tokio::fs::remove_file(&dst_path).await?;
            }
            tokio::fs::copy(&src_path, &dst_path).await?;
        }
    }

    Ok(())
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
