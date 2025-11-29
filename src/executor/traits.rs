use std::path::Path;

use async_trait::async_trait;

use crate::error::Result;

/// Result of executing an AI CLI tool
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    /// Name of the executor (e.g., "claude", "gemini", "codex")
    pub executor_name: String,
    /// Whether the execution was successful
    pub success: bool,
    /// Standard output from the CLI
    pub stdout: String,
    /// Standard error from the CLI
    pub stderr: String,
    /// Exit code if available
    pub exit_code: Option<i32>,
}

impl ExecutionResult {
    /// Create a successful execution result
    pub fn success(executor_name: impl Into<String>, stdout: String) -> Self {
        Self {
            executor_name: executor_name.into(),
            success: true,
            stdout,
            stderr: String::new(),
            exit_code: Some(0),
        }
    }

    /// Create a failed execution result
    pub fn failure(
        executor_name: impl Into<String>,
        stderr: String,
        exit_code: Option<i32>,
    ) -> Self {
        Self {
            executor_name: executor_name.into(),
            success: false,
            stdout: String::new(),
            stderr,
            exit_code,
        }
    }
}

/// Trait for executing AI CLI tools
///
/// This trait abstracts the execution of AI CLI tools (claude, gemini, codex)
/// to allow for mocking in tests.
#[async_trait]
pub trait Executor: Send + Sync {
    /// Returns the name of the executor (e.g., "claude", "gemini", "codex")
    fn name(&self) -> &str;

    /// Check if the executor is available in PATH
    async fn is_available(&self) -> bool;

    /// Execute the CLI tool with the given prompt in the specified working directory
    ///
    /// # Arguments
    /// * `prompt` - The task/prompt to send to the AI CLI
    /// * `working_dir` - The directory to run the CLI in (typically a worktree)
    ///
    /// # Returns
    /// * `Ok(ExecutionResult)` - The result of the execution
    /// * `Err(Error)` - If the execution could not be started
    async fn execute(&self, prompt: &str, working_dir: &Path) -> Result<ExecutionResult>;
}
