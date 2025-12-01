use std::path::Path;
use std::process::Stdio;

use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use crate::error::Result;

/// A single line of output from an AI CLI tool, tagged with its source
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutputLine {
    /// A line from stdout
    Stdout(String),
    /// A line from stderr
    Stderr(String),
}

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
    /// Output lines in order of arrival (stdout and stderr interleaved)
    pub output_lines: Vec<OutputLine>,
    /// Exit code if available
    pub exit_code: Option<i32>,
}

impl ExecutionResult {
    /// Create a successful execution result
    pub fn success(executor_name: impl Into<String>, stdout: String) -> Self {
        let output_lines = stdout
            .lines()
            .map(|line| OutputLine::Stdout(line.to_string()))
            .collect();
        Self {
            executor_name: executor_name.into(),
            success: true,
            stdout,
            stderr: String::new(),
            output_lines,
            exit_code: Some(0),
        }
    }

    /// Create a failed execution result
    pub fn failure(
        executor_name: impl Into<String>,
        stderr: String,
        exit_code: Option<i32>,
    ) -> Self {
        let output_lines = stderr
            .lines()
            .map(|line| OutputLine::Stderr(line.to_string()))
            .collect();
        Self {
            executor_name: executor_name.into(),
            success: false,
            stdout: String::new(),
            stderr,
            output_lines,
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

/// Helper function to execute a command and capture stdout/stderr in order of arrival
///
/// This spawns the process with piped stdout/stderr and reads lines as they arrive,
/// preserving the interleaved order.
pub async fn execute_with_ordered_output(
    mut cmd: Command,
    executor_name: &str,
) -> std::io::Result<ExecutionResult> {
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = cmd.spawn()?;

    let stdout = child.stdout.take().expect("stdout should be piped");
    let stderr = child.stderr.take().expect("stderr should be piped");

    let mut stdout_reader = BufReader::new(stdout).lines();
    let mut stderr_reader = BufReader::new(stderr).lines();

    let mut output_lines = Vec::new();
    let mut stdout_content = String::new();
    let mut stderr_content = String::new();

    loop {
        tokio::select! {
            result = stdout_reader.next_line() => {
                match result {
                    Ok(Some(line)) => {
                        if !stdout_content.is_empty() {
                            stdout_content.push('\n');
                        }
                        stdout_content.push_str(&line);
                        output_lines.push(OutputLine::Stdout(line));
                    }
                    Ok(None) => {
                        // stdout closed, drain stderr
                        while let Ok(Some(line)) = stderr_reader.next_line().await {
                            if !stderr_content.is_empty() {
                                stderr_content.push('\n');
                            }
                            stderr_content.push_str(&line);
                            output_lines.push(OutputLine::Stderr(line));
                        }
                        break;
                    }
                    Err(e) => return Err(e),
                }
            }
            result = stderr_reader.next_line() => {
                match result {
                    Ok(Some(line)) => {
                        if !stderr_content.is_empty() {
                            stderr_content.push('\n');
                        }
                        stderr_content.push_str(&line);
                        output_lines.push(OutputLine::Stderr(line));
                    }
                    Ok(None) => {
                        // stderr closed, drain stdout
                        while let Ok(Some(line)) = stdout_reader.next_line().await {
                            if !stdout_content.is_empty() {
                                stdout_content.push('\n');
                            }
                            stdout_content.push_str(&line);
                            output_lines.push(OutputLine::Stdout(line));
                        }
                        break;
                    }
                    Err(e) => return Err(e),
                }
            }
        }
    }

    let status = child.wait().await?;
    let exit_code = status.code();
    let success = status.success();

    Ok(ExecutionResult {
        executor_name: executor_name.to_string(),
        success,
        stdout: stdout_content,
        stderr: stderr_content,
        output_lines,
        exit_code,
    })
}
