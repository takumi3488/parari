use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::git;

/// Manages worktrees for parallel execution
#[derive(Debug)]
pub struct WorktreeManager {
    /// Path to the original repository
    repo_path: PathBuf,
    /// Active worktrees
    worktrees: Vec<git::WorktreeInfo>,
}

impl WorktreeManager {
    /// Create a new worktree manager for the given repository
    pub async fn new(repo_path: impl AsRef<Path>) -> Result<Self> {
        let repo_path = git::get_repo_root(repo_path.as_ref()).await?;

        Ok(Self {
            repo_path,
            worktrees: Vec::new(),
        })
    }

    /// Get the repository path
    pub fn repo_path(&self) -> &Path {
        &self.repo_path
    }

    /// Create worktrees for the given executor names
    pub async fn create_worktrees(&mut self, executor_names: &[&str]) -> Result<()> {
        // First cleanup old worktrees to stay under limit
        git::cleanup_old_worktrees(&self.repo_path).await?;

        for name in executor_names {
            let info = git::create_worktree(&self.repo_path, name).await?;
            self.worktrees.push(info);
        }

        Ok(())
    }

    /// Get the worktree for a specific executor
    pub fn get_worktree(&self, executor_name: &str) -> Option<&git::WorktreeInfo> {
        self.worktrees
            .iter()
            .find(|w| w.executor_name == executor_name)
    }

    /// Get all worktrees
    pub fn worktrees(&self) -> &[git::WorktreeInfo] {
        &self.worktrees
    }

    /// Cleanup all managed worktrees
    pub async fn cleanup(&mut self) -> Result<()> {
        for worktree in &self.worktrees {
            let _ = git::remove_worktree(&self.repo_path, &worktree.path).await;
        }
        self.worktrees.clear();
        Ok(())
    }
}

impl Drop for WorktreeManager {
    fn drop(&mut self) {
        // Best-effort cleanup on drop
        // We can't use async in drop, so we spawn a blocking task
        let repo_path = self.repo_path.clone();
        let worktrees: Vec<_> = self.worktrees.iter().map(|w| w.path.clone()).collect();

        if !worktrees.is_empty() {
            std::thread::spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build();

                if let Ok(rt) = rt {
                    rt.block_on(async {
                        for path in worktrees {
                            let _ = git::remove_worktree(&repo_path, &path).await;
                        }
                    });
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_worktree_manager_creation() {
        let cwd = std::env::current_dir().unwrap();
        let manager = WorktreeManager::new(&cwd).await;
        assert!(manager.is_ok());
    }
}
