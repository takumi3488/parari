use std::path::PathBuf;

/// Base directory for parari data
///
/// Returns `$HOME/.parari`
pub fn base_dir() -> PathBuf {
    dirs::home_dir()
        .expect("Could not find home directory")
        .join(".parari")
}

/// Directory for storing worktrees
///
/// Returns `$HOME/.parari/worktrees`
pub fn worktrees_dir() -> PathBuf {
    base_dir().join("worktrees")
}

/// Maximum number of worktrees to keep
pub const MAX_WORKTREES: usize = 20;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base_dir_ends_with_parari() {
        let base = base_dir();
        assert!(base.ends_with(".parari"));
    }

    #[test]
    fn test_worktrees_dir_is_under_base() {
        let base = base_dir();
        let worktrees = worktrees_dir();
        assert!(worktrees.starts_with(&base));
        assert!(worktrees.ends_with("worktrees"));
    }
}
