use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;

use async_trait::async_trait;

use super::traits::{ExecutionResult, Executor};
use crate::error::Result;

/// Action to perform on a file during mock execution
#[derive(Debug, Clone)]
pub enum FileAction {
    /// Create or overwrite a file with the given content
    Write {
        /// Relative path from working directory
        path: String,
        /// Content to write
        content: String,
    },
    /// Delete a file
    Delete {
        /// Relative path from working directory
        path: String,
    },
    /// Create a directory
    CreateDir {
        /// Relative path from working directory
        path: String,
    },
}

/// A mock executor for testing
///
/// This executor allows you to configure the behavior of the executor
/// for testing purposes. It can also create files in the working directory
/// to simulate real AI CLI tool output.
#[derive(Debug)]
pub struct MockExecutor {
    name: String,
    available: bool,
    /// Recorded calls for verification
    calls: Arc<Mutex<Vec<MockCall>>>,
    /// Pre-configured responses
    responses: Arc<Mutex<Vec<ExecutionResult>>>,
    /// File actions to perform during execution
    file_actions: Arc<Mutex<Vec<FileAction>>>,
}

/// A recorded call to the mock executor
#[derive(Debug, Clone)]
pub struct MockCall {
    pub prompt: String,
    pub working_dir: std::path::PathBuf,
}

impl MockExecutor {
    /// Create a new mock executor with the given name
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            available: true,
            calls: Arc::new(Mutex::new(Vec::new())),
            responses: Arc::new(Mutex::new(Vec::new())),
            file_actions: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Set whether the executor is available
    pub fn with_available(mut self, available: bool) -> Self {
        self.available = available;
        self
    }

    /// Add a response to return on the next execute call
    pub fn with_response(self, result: ExecutionResult) -> Self {
        self.responses.lock().unwrap().push(result);
        self
    }

    /// Add a success response
    pub fn with_success(self, stdout: impl Into<String>) -> Self {
        let result = ExecutionResult::success(self.name.clone(), stdout.into());
        self.with_response(result)
    }

    /// Add a failure response
    pub fn with_failure(self, stderr: impl Into<String>, exit_code: Option<i32>) -> Self {
        let result = ExecutionResult::failure(self.name.clone(), stderr.into(), exit_code);
        self.with_response(result)
    }

    /// Add a file action to perform during execution
    pub fn with_file_action(self, action: FileAction) -> Self {
        self.file_actions.lock().unwrap().push(action);
        self
    }

    /// Add a file write action
    pub fn with_file(self, path: impl Into<String>, content: impl Into<String>) -> Self {
        self.with_file_action(FileAction::Write {
            path: path.into(),
            content: content.into(),
        })
    }

    /// Add a file delete action
    pub fn with_delete(self, path: impl Into<String>) -> Self {
        self.with_file_action(FileAction::Delete { path: path.into() })
    }

    /// Add a directory creation action
    pub fn with_dir(self, path: impl Into<String>) -> Self {
        self.with_file_action(FileAction::CreateDir { path: path.into() })
    }

    /// Get all recorded calls
    pub fn calls(&self) -> Vec<MockCall> {
        self.calls.lock().unwrap().clone()
    }

    /// Get the number of times execute was called
    pub fn call_count(&self) -> usize {
        self.calls.lock().unwrap().len()
    }

    /// Check if execute was called with the given prompt
    pub fn was_called_with(&self, prompt: &str) -> bool {
        self.calls
            .lock()
            .unwrap()
            .iter()
            .any(|call| call.prompt == prompt)
    }

    /// Clear all recorded calls
    pub fn clear_calls(&self) {
        self.calls.lock().unwrap().clear();
    }

    /// Perform the configured file actions in the given directory
    async fn perform_file_actions(&self, working_dir: &Path) -> Result<()> {
        let actions = self.file_actions.lock().unwrap().clone();

        for action in actions {
            match action {
                FileAction::Write { path, content } => {
                    let full_path = working_dir.join(&path);
                    if let Some(parent) = full_path.parent() {
                        tokio::fs::create_dir_all(parent).await?;
                    }
                    tokio::fs::write(&full_path, content).await?;
                }
                FileAction::Delete { path } => {
                    let full_path = working_dir.join(&path);
                    if full_path.exists() {
                        if full_path.is_dir() {
                            tokio::fs::remove_dir_all(&full_path).await?;
                        } else {
                            tokio::fs::remove_file(&full_path).await?;
                        }
                    }
                }
                FileAction::CreateDir { path } => {
                    let full_path = working_dir.join(&path);
                    tokio::fs::create_dir_all(&full_path).await?;
                }
            }
        }

        Ok(())
    }
}

impl Default for MockExecutor {
    fn default() -> Self {
        Self::new("mock")
    }
}

#[async_trait]
impl Executor for MockExecutor {
    fn name(&self) -> &str {
        &self.name
    }

    async fn is_available(&self) -> bool {
        self.available
    }

    async fn execute(&self, prompt: &str, working_dir: &Path) -> Result<ExecutionResult> {
        // Record the call
        self.calls.lock().unwrap().push(MockCall {
            prompt: prompt.to_string(),
            working_dir: working_dir.to_path_buf(),
        });

        // Perform file actions
        self.perform_file_actions(working_dir).await?;

        // Return the next configured response, or a default success
        let response = self.responses.lock().unwrap().pop();
        Ok(response.unwrap_or_else(|| ExecutionResult::success(self.name.clone(), String::new())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_mock_executor_default() {
        let mock = MockExecutor::new("test");
        assert_eq!(mock.name(), "test");
        assert!(mock.is_available().await);
    }

    #[tokio::test]
    async fn test_mock_executor_not_available() {
        let mock = MockExecutor::new("test").with_available(false);
        assert!(!mock.is_available().await);
    }

    #[tokio::test]
    async fn test_mock_executor_records_calls() {
        let mock = MockExecutor::new("test");
        let working_dir = PathBuf::from("/tmp");

        mock.execute("test prompt", &working_dir).await.unwrap();

        assert_eq!(mock.call_count(), 1);
        assert!(mock.was_called_with("test prompt"));
    }

    #[tokio::test]
    async fn test_mock_executor_returns_configured_response() {
        let mock = MockExecutor::new("test").with_success("test output");

        let working_dir = PathBuf::from("/tmp");
        let result = mock.execute("test prompt", &working_dir).await.unwrap();

        assert!(result.success);
        assert_eq!(result.stdout, "test output");
    }

    #[tokio::test]
    async fn test_mock_executor_returns_failure() {
        let mock = MockExecutor::new("test").with_failure("error message", Some(1));

        let working_dir = PathBuf::from("/tmp");
        let result = mock.execute("test prompt", &working_dir).await.unwrap();

        assert!(!result.success);
        assert_eq!(result.stderr, "error message");
        assert_eq!(result.exit_code, Some(1));
    }

    #[tokio::test]
    async fn test_mock_executor_creates_file() {
        let temp_dir = std::env::temp_dir().join("parari_test_mock_file");
        tokio::fs::create_dir_all(&temp_dir).await.unwrap();

        let mock = MockExecutor::new("test")
            .with_file("test.txt", "Hello, World!")
            .with_success("Created file");

        let result = mock.execute("create a file", &temp_dir).await.unwrap();

        assert!(result.success);
        let content = tokio::fs::read_to_string(temp_dir.join("test.txt"))
            .await
            .unwrap();
        assert_eq!(content, "Hello, World!");

        // Cleanup
        tokio::fs::remove_dir_all(&temp_dir).await.unwrap();
    }

    #[tokio::test]
    async fn test_mock_executor_creates_nested_file() {
        let temp_dir = std::env::temp_dir().join("parari_test_mock_nested");
        tokio::fs::create_dir_all(&temp_dir).await.unwrap();

        let mock = MockExecutor::new("test")
            .with_file("src/lib.rs", "pub fn hello() {}")
            .with_success("Created nested file");

        let result = mock.execute("create a file", &temp_dir).await.unwrap();

        assert!(result.success);
        let content = tokio::fs::read_to_string(temp_dir.join("src/lib.rs"))
            .await
            .unwrap();
        assert_eq!(content, "pub fn hello() {}");

        // Cleanup
        tokio::fs::remove_dir_all(&temp_dir).await.unwrap();
    }
}
