use std::path::Path;

use tokio::process::Command;

use crate::error::Result;

/// Check if target directory has uncommitted changes
pub async fn has_uncommitted_changes(target: &Path) -> Result<bool> {
    let output = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(target)
        .output()
        .await?;

    let status = String::from_utf8_lossy(&output.stdout);
    Ok(!status.trim().is_empty())
}

/// Get list of uncommitted files in target directory
pub async fn get_uncommitted_files(target: &Path) -> Result<Vec<String>> {
    let output = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(target)
        .output()
        .await?;

    let status = String::from_utf8_lossy(&output.stdout);
    let files: Vec<String> = status
        .lines()
        .filter(|line| line.len() >= 3)
        .map(|line| line[3..].to_string())
        .collect();

    Ok(files)
}

/// Check for conflicts between worktree changes and target uncommitted changes
///
/// Returns a list of files that would conflict (exist in both worktree changes and target uncommitted changes)
pub async fn check_conflicts(worktree: &Path, target: &Path) -> Result<Vec<String>> {
    let worktree_changes = get_uncommitted_files(worktree).await?;
    let target_changes = get_uncommitted_files(target).await?;

    let conflicts: Vec<String> = worktree_changes
        .iter()
        .filter(|f| target_changes.contains(f))
        .cloned()
        .collect();

    Ok(conflicts)
}

/// Apply changes from a worktree to the target directory
///
/// This copies all files from the worktree to the target, excluding .git
pub async fn apply_changes(worktree: &Path, target: &Path) -> Result<()> {
    copy_dir_recursive(worktree, target).await
}

#[async_recursion::async_recursion]
async fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
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
            tokio::fs::create_dir_all(&dst_path).await?;
            copy_dir_recursive(&src_path, &dst_path).await?;
        } else if file_type.is_file() {
            // Remove target file first to avoid "Text file busy" error
            // when overwriting a running executable (ETXTBSY)
            if dst_path.exists() {
                tokio::fs::remove_file(&dst_path).await?;
            }
            tokio::fs::copy(&src_path, &dst_path).await?;
        }
        // Skip symlinks for now
    }

    Ok(())
}

/// Get a summary of changes between original and worktree
#[derive(Debug, Clone)]
pub struct ChangeSummary {
    /// Number of files added
    pub files_added: usize,
    /// Number of files modified
    pub files_modified: usize,
    /// Number of files deleted
    pub files_deleted: usize,
    /// List of changed file paths
    pub changed_files: Vec<String>,
}

/// Get a summary of changes in a worktree compared to HEAD
pub async fn get_change_summary(_original: &Path, worktree: &Path) -> Result<ChangeSummary> {
    // Use git status --porcelain to get all changes including untracked files
    let output = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(worktree)
        .output()
        .await?;

    let status = String::from_utf8_lossy(&output.stdout);

    let mut files_added = 0;
    let mut files_modified = 0;
    let mut files_deleted = 0;
    let mut changed_files = Vec::new();

    for line in status.lines() {
        if line.len() < 3 {
            continue;
        }

        let status_code = &line[0..2];
        let file_name = line[3..].to_string();
        changed_files.push(file_name);

        // Parse status codes:
        // ?? = untracked (new file)
        // A  = added (staged)
        // M  = modified
        // D  = deleted
        // First char = staged status, second char = unstaged status
        match status_code {
            "??" | "A " | " A" => files_added += 1,
            "D " | " D" => files_deleted += 1,
            _ => files_modified += 1,
        }
    }

    Ok(ChangeSummary {
        files_added,
        files_modified,
        files_deleted,
        changed_files,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_change_summary_default() {
        let summary = ChangeSummary {
            files_added: 0,
            files_modified: 0,
            files_deleted: 0,
            changed_files: vec![],
        };
        assert_eq!(summary.files_added, 0);
        assert_eq!(summary.files_modified, 0);
        assert_eq!(summary.files_deleted, 0);
    }
}
