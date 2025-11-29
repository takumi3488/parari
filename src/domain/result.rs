use std::path::Path;

use crate::error::Result;
use crate::git;

use super::task::TaskResult;

/// Options for displaying results
#[derive(Debug, Clone)]
pub struct DisplayOptions {
    /// Show change summary
    pub show_summary: bool,
}

impl Default for DisplayOptions {
    fn default() -> Self {
        Self { show_summary: true }
    }
}

/// Information about a result for display
#[derive(Debug, Clone)]
pub struct ResultInfo {
    /// Name of the executor
    pub executor_name: String,
    /// Whether the execution was successful
    pub success: bool,
    /// Number of files changed
    pub files_changed: usize,
    /// The change summary
    pub change_summary: Option<git::ChangeSummary>,
    /// Path to the worktree
    pub worktree_path: std::path::PathBuf,
    /// Standard output from the executor
    pub stdout: String,
    /// Standard error from the executor
    pub stderr: String,
}

/// Prepare result information for display
pub async fn prepare_result_info(
    result: &TaskResult,
    _original_path: &Path,
    _options: &DisplayOptions,
) -> Result<ResultInfo> {
    let files_changed = result
        .change_summary
        .as_ref()
        .map(|s| s.files_added + s.files_modified + s.files_deleted)
        .unwrap_or(0);

    Ok(ResultInfo {
        executor_name: result.execution.executor_name.clone(),
        success: result.execution.success,
        files_changed,
        change_summary: result.change_summary.clone(),
        worktree_path: result.worktree_path.clone(),
        stdout: result.execution.stdout.clone(),
        stderr: result.execution.stderr.clone(),
    })
}

/// Apply the selected result to the target directory
pub async fn apply_result(result: &TaskResult, target: &Path) -> Result<()> {
    git::apply_changes(&result.worktree_path, target).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_options_default() {
        let opts = DisplayOptions::default();
        assert!(opts.show_summary);
    }
}
