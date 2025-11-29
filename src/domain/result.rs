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
    })
}

/// Apply the selected result to the target directory
pub async fn apply_result(result: &TaskResult, target: &Path) -> Result<()> {
    git::apply_changes(&result.worktree_path, target).await
}

/// Compare results and return indices sorted by number of changes (descending)
pub fn rank_results(results: &[TaskResult]) -> Vec<usize> {
    let mut indexed: Vec<(usize, usize)> = results
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let changes = r
                .change_summary
                .as_ref()
                .map(|s| s.files_added + s.files_modified + s.files_deleted)
                .unwrap_or(0);
            (i, changes)
        })
        .collect();

    // Sort by changes descending (more changes = potentially more work done)
    indexed.sort_by(|a, b| b.1.cmp(&a.1));

    indexed.into_iter().map(|(i, _)| i).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::executor::traits::ExecutionResult;

    fn make_result(executor_name: &str, files_changed: usize) -> TaskResult {
        TaskResult {
            execution: ExecutionResult::success(executor_name.to_string(), "output".to_string()),
            worktree_path: std::path::PathBuf::from("/tmp/test"),
            change_summary: Some(git::ChangeSummary {
                files_added: files_changed,
                files_modified: 0,
                files_deleted: 0,
                changed_files: vec![],
            }),
        }
    }

    #[test]
    fn test_rank_results() {
        let results = vec![
            make_result("a", 5),
            make_result("b", 10),
            make_result("c", 3),
        ];

        let ranked = rank_results(&results);
        assert_eq!(ranked, vec![1, 0, 2]); // b=10, a=5, c=3
    }

    #[test]
    fn test_display_options_default() {
        let opts = DisplayOptions::default();
        assert!(opts.show_summary);
    }
}
