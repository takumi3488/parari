use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use crate::error::Result;
use crate::git;

/// Global registry to track all worktrees created by this process.
/// This enables cleanup on process termination (including signals).
static WORKTREE_REGISTRY: OnceLock<Mutex<WorktreeRegistry>> = OnceLock::new();

/// Registry of worktrees created by this process
#[derive(Debug, Default)]
struct WorktreeRegistry {
    /// Map of worktree path to repo path
    entries: Vec<(PathBuf, PathBuf)>,
}

impl WorktreeRegistry {
    fn register(&mut self, repo_path: PathBuf, worktree_path: PathBuf) {
        self.entries.push((worktree_path, repo_path));
    }

    fn unregister(&mut self, worktree_path: &Path) {
        self.entries.retain(|(path, _)| path != worktree_path);
    }

    fn take_all(&mut self) -> Vec<(PathBuf, PathBuf)> {
        std::mem::take(&mut self.entries)
    }
}

fn get_registry() -> &'static Mutex<WorktreeRegistry> {
    WORKTREE_REGISTRY.get_or_init(|| Mutex::new(WorktreeRegistry::default()))
}

/// Register a worktree for cleanup tracking
fn register_worktree(repo_path: &Path, worktree_path: &Path) {
    if let Ok(mut registry) = get_registry().lock() {
        registry.register(repo_path.to_path_buf(), worktree_path.to_path_buf());
    }
}

/// Unregister a worktree from cleanup tracking
fn unregister_worktree(worktree_path: &Path) {
    if let Ok(mut registry) = get_registry().lock() {
        registry.unregister(worktree_path);
    }
}

/// Cleanup all registered worktrees (called on signal/shutdown).
/// This is a synchronous function that creates its own runtime.
pub fn cleanup_all_registered_worktrees() {
    let entries = {
        match get_registry().lock() {
            Ok(mut registry) => registry.take_all(),
            Err(_) => return,
        }
    };

    if entries.is_empty() {
        return;
    }

    // Create a new runtime for cleanup since we may be called from a signal handler
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build();

    if let Ok(rt) = rt {
        rt.block_on(async {
            for (worktree_path, repo_path) in entries {
                let _ = git::remove_worktree(&repo_path, &worktree_path).await;
            }
        });
    }
}

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
            // Register worktree for cleanup on process termination
            register_worktree(&self.repo_path, &info.path);
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
            // Unregister from global registry
            unregister_worktree(&worktree.path);
        }
        self.worktrees.clear();
        Ok(())
    }
}

impl Drop for WorktreeManager {
    fn drop(&mut self) {
        // Best-effort cleanup on drop
        // We can't use async in drop, so we spawn a separate thread
        // to avoid "Cannot start a runtime from within a runtime" panic
        let repo_path = self.repo_path.clone();
        let worktrees: Vec<_> = self.worktrees.iter().map(|w| w.path.clone()).collect();

        if worktrees.is_empty() {
            return;
        }

        // Spawn a new thread to create a runtime and run cleanup
        // This avoids issues when Drop is called from within a tokio runtime
        let handle = std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build();

            if let Ok(rt) = rt {
                rt.block_on(async {
                    for path in worktrees {
                        let _ = git::remove_worktree(&repo_path, &path).await;
                        unregister_worktree(&path);
                    }
                });
            }
        });

        // Wait for cleanup to complete
        let _ = handle.join();
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
