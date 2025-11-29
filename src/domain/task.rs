use std::path::Path;
use std::sync::Arc;

use futures::future::join_all;

use crate::cli::progress::{AgentStatus, ProgressTracker};
use crate::error::{Error, Result};
use crate::executor::traits::{ExecutionResult, Executor};
use crate::git;

use super::worktree::WorktreeManager;

/// Result of a task execution including the worktree path
#[derive(Debug, Clone)]
pub struct TaskResult {
    /// The execution result from the AI CLI
    pub execution: ExecutionResult,
    /// Path to the worktree where changes were made
    pub worktree_path: std::path::PathBuf,
    /// Summary of changes made
    pub change_summary: Option<git::ChangeSummary>,
}

/// Orchestrates task execution across multiple executors
pub struct TaskRunner {
    /// The worktree manager
    worktree_manager: WorktreeManager,
}

impl TaskRunner {
    /// Create a new task runner for the given repository
    pub async fn new(repo_path: impl AsRef<Path>) -> Result<Self> {
        let worktree_manager = WorktreeManager::new(repo_path).await?;
        Ok(Self { worktree_manager })
    }

    /// Get reference to worktree manager
    pub fn worktree_manager(&self) -> &WorktreeManager {
        &self.worktree_manager
    }

    /// Get mutable reference to worktree manager
    pub fn worktree_manager_mut(&mut self) -> &mut WorktreeManager {
        &mut self.worktree_manager
    }

    /// Run the task with the given executors in parallel
    ///
    /// Returns results from all executors that completed successfully
    pub async fn run(
        &mut self,
        prompt: &str,
        executors: Vec<Arc<dyn Executor>>,
    ) -> Result<Vec<TaskResult>> {
        self.run_with_progress(prompt, executors, None).await
    }

    /// Run the task with the given executors in parallel with progress tracking
    ///
    /// Returns results from all executors that completed successfully
    pub async fn run_with_progress(
        &mut self,
        prompt: &str,
        executors: Vec<Arc<dyn Executor>>,
        progress: Option<Arc<ProgressTracker>>,
    ) -> Result<Vec<TaskResult>> {
        // Filter to available executors
        let mut available_executors = Vec::new();
        for executor in executors {
            if executor.is_available().await {
                available_executors.push(executor);
            }
        }

        if available_executors.is_empty() {
            return Err(Error::NoExecutorsAvailable);
        }

        // Create worktrees for each executor
        let executor_names: Vec<&str> = available_executors.iter().map(|e| e.name()).collect();
        self.worktree_manager
            .create_worktrees(&executor_names)
            .await?;

        // Execute in parallel
        let repo_path = self.worktree_manager.repo_path().to_path_buf();
        let futures: Vec<_> = available_executors
            .iter()
            .map(|executor| {
                let executor = Arc::clone(executor);
                let worktree = self
                    .worktree_manager
                    .get_worktree(executor.name())
                    .expect("Worktree should exist")
                    .clone();
                let prompt = prompt.to_string();
                let repo_path = repo_path.clone();
                let progress = progress.clone();

                async move {
                    let executor_name = executor.name().to_string();

                    // Update progress: Running
                    if let Some(ref p) = progress {
                        p.update_status(&executor_name, AgentStatus::Running);
                    }

                    let result = executor.execute(&prompt, &worktree.path).await;

                    match result {
                        Ok(execution) => {
                            // Get change summary
                            let change_summary =
                                git::get_change_summary(&repo_path, &worktree.path)
                                    .await
                                    .ok();

                            // Update progress based on execution success
                            if let Some(ref p) = progress {
                                if execution.success {
                                    p.update_status(&executor_name, AgentStatus::Completed);
                                } else {
                                    p.update_status(&executor_name, AgentStatus::Failed);
                                }
                            }

                            Some(TaskResult {
                                execution,
                                worktree_path: worktree.path,
                                change_summary,
                            })
                        }
                        Err(_) => {
                            // Update progress: Failed
                            if let Some(ref p) = progress {
                                p.update_status(&executor_name, AgentStatus::Failed);
                            }
                            None
                        }
                    }
                }
            })
            .collect();

        let results: Vec<_> = join_all(futures).await.into_iter().flatten().collect();

        // Finish all progress bars
        if let Some(ref p) = progress {
            p.finish_all();
        }

        Ok(results)
    }

    /// Cleanup worktrees
    pub async fn cleanup(&mut self) -> Result<()> {
        self.worktree_manager.cleanup().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::executor::mock::MockExecutor;

    #[tokio::test]
    async fn test_task_runner_creation() {
        let cwd = std::env::current_dir().unwrap();
        let runner = TaskRunner::new(&cwd).await;
        assert!(runner.is_ok());
    }

    #[tokio::test]
    async fn test_task_runner_no_executors() {
        let cwd = std::env::current_dir().unwrap();
        let mut runner = TaskRunner::new(&cwd).await.unwrap();

        let executors: Vec<Arc<dyn Executor>> =
            vec![Arc::new(MockExecutor::new("test").with_available(false))];

        let result = runner.run("test prompt", executors).await;
        assert!(matches!(result, Err(Error::NoExecutorsAvailable)));
    }
}
