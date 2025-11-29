//! parari - Run multiple AI CLI tools in parallel using git worktrees
//!
//! This library provides the core functionality for running Claude, Gemini, and Codex
//! CLI tools in parallel, each in their own git worktree, allowing users to compare
//! results and choose the best one.

pub mod cli;
pub mod config;
pub mod domain;
pub mod error;
pub mod executor;
pub mod git;
