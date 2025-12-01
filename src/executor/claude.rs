use std::path::Path;

use async_trait::async_trait;
use tokio::process::Command;

use super::traits::{ExecutionResult, Executor, execute_with_ordered_output};
use crate::error::{Error, Result};

/// Executor for Claude Code CLI
#[derive(Debug, Default)]
pub struct ClaudeExecutor;

impl ClaudeExecutor {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Executor for ClaudeExecutor {
    fn name(&self) -> &str {
        "claude"
    }

    async fn is_available(&self) -> bool {
        Command::new("which")
            .arg("claude")
            .output()
            .await
            .is_ok_and(|output| output.status.success())
    }

    async fn execute(&self, prompt: &str, working_dir: &Path) -> Result<ExecutionResult> {
        if !working_dir.exists() {
            return Err(Error::WorkingDirectoryNotFound {
                path: working_dir.to_path_buf(),
            });
        }

        let mut cmd = Command::new("claude");
        cmd.arg("--print")
            .arg("--dangerously-skip-permissions")
            .arg(prompt)
            .current_dir(working_dir);

        let result = execute_with_ordered_output(cmd, self.name()).await?;
        Ok(result)
    }
}
