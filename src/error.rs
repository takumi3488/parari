use std::path::PathBuf;

/// Error types for parari
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Executor '{name}' not found in PATH")]
    ExecutorNotFound { name: String },

    #[error("Executor '{name}' failed with exit code {code:?}: {stderr}")]
    ExecutorFailed {
        name: String,
        code: Option<i32>,
        stderr: String,
    },

    #[error("Working directory does not exist: {path}")]
    WorkingDirectoryNotFound { path: PathBuf },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
