use std::path::PathBuf;

/// Base directory for parari data
///
/// Returns `$HOME/.parari`, or `/tmp/.parari` if the home directory cannot be determined.
#[must_use]
pub fn base_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
        .join(".parari")
}

/// Directory for storing worktrees
///
/// Returns `$HOME/.parari/worktrees`
#[must_use]
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
