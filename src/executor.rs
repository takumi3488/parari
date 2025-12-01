//! Executor layer for AI CLI tools
//!
//! This module provides abstractions for executing AI CLI tools (Claude, Gemini, Codex).
//! The `Executor` trait allows for mocking in tests.

pub mod claude;
pub mod codex;
pub mod gemini;
pub mod mock;
pub mod traits;

pub use claude::ClaudeExecutor;
pub use codex::CodexExecutor;
pub use gemini::GeminiExecutor;
pub use mock::MockExecutor;
pub use traits::{ExecutionResult, Executor, OutputLine};
