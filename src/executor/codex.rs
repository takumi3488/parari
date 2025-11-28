use std::path::Path;

use tokio::process::Command;

use super::traits::{ExecutionResult, Executor};
use crate::error::{Error, Result};

/// Executor for OpenAI Codex CLI
#[derive(Debug, Default)]
pub struct CodexExecutor;

impl CodexExecutor {
    pub fn new() -> Self {
        Self
    }
}

impl Executor for CodexExecutor {
    fn name(&self) -> &str {
        "codex"
    }

    async fn is_available(&self) -> bool {
        Command::new("which")
            .arg("codex")
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

        let output = Command::new("codex")
            .arg("--full-auto")
            .arg("exec")
            .arg(prompt)
            .current_dir(working_dir)
            .output()
            .await?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let exit_code = output.status.code();

        if output.status.success() {
            Ok(ExecutionResult {
                executor_name: self.name().to_string(),
                success: true,
                stdout,
                stderr,
                exit_code,
            })
        } else {
            Ok(ExecutionResult {
                executor_name: self.name().to_string(),
                success: false,
                stdout,
                stderr,
                exit_code,
            })
        }
    }
}
