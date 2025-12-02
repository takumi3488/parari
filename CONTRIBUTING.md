# Contributing to parari

Thank you for your interest in contributing to parari! This document provides guidelines and instructions for contributing.

## Table of Contents

- [Development Setup](#development-setup)
- [Architecture](#architecture)
- [Development Workflow](#development-workflow)
- [Testing](#testing)
- [Code Style](#code-style)
- [Pull Request Process](#pull-request-process)
- [Commit Messages](#commit-messages)

## Development Setup

### Prerequisites

- Rust (edition 2024)
- Git with worktree support
- One or more AI CLI tools: `claude`, `gemini`, `codex`

### Getting Started

```bash
# Clone the repository
git clone https://github.com/yourname/parari.git
cd parari

# Build the project
cargo build

# Run tests
cargo test --all-features

# Run linting (format check + clippy)
cargo lint
```

## Architecture

This project uses a **layered architecture** for testability and separation of concerns.

### Directory Structure

```
src/
├── main.rs              # Entry point
├── lib.rs               # Library root (for testing)
│
├── cli/                 # CLI layer
│   ├── args.rs          # clap argument definitions
│   ├── editor.rs        # $EDITOR prompt input
│   ├── progress.rs      # Progress display and agent styles
│   ├── split_view.rs    # lazydocker-style split view UI
│   └── ui.rs            # User interface (result selection, etc.)
│
├── domain/              # Domain layer
│   ├── task.rs          # Task execution coordination
│   ├── result.rs        # Result comparison and selection
│   └── worktree.rs      # Worktree management logic
│
├── executor/            # Execution layer
│   ├── traits.rs        # Executor trait (for mocking)
│   ├── claude.rs
│   ├── gemini.rs
│   └── codex.rs
│
├── git/                 # Git operations layer
│   ├── worktree.rs      # Worktree creation/deletion
│   └── merge.rs         # Merge processing
│
├── config/              # Configuration layer
│   └── paths.rs         # Path definitions
│
└── error.rs             # Error type definitions
```

### Module Naming Convention

Use directory-adjacent `.rs` files instead of `mod.rs`:
- ✅ `cli.rs` + `cli/args.rs`
- ❌ `cli/mod.rs` + `cli/args.rs`

### Layer Responsibilities

| Layer | Responsibility |
|-------|----------------|
| `cli` | User input parsing, result display |
| `domain` | Task execution flow, result comparison logic |
| `executor` | AI CLI tool execution (mockable via trait) |
| `git` | Git worktree operations, merging |
| `config` | Path and configuration management |
| `error` | Unified error types |

## Development Workflow

We follow the **Red-Green-Refactor** approach:

1. **Red**: Write a failing test first
2. **Green**: Write the minimum code to make the test pass
3. **Refactor**: Improve the code while keeping tests green

### Adding a New Feature

1. Write tests that describe the expected behavior
2. Implement the feature to make tests pass
3. Refactor if needed
4. Run `cargo lint` to ensure code quality

### Mocking AI CLI Calls

AI CLI invocations should be mocked for testing. Use the `mock` feature flag:

```bash
# Run tests with mock executors
cargo test --features mock
```

## Testing

### Running Tests

```bash
# Run all tests
cargo test --all-features

# Run a specific test
cargo test test_name

# Run tests with output
cargo test -- --nocapture
```

### Snapshot Testing

We use [insta](https://insta.rs/) for snapshot testing:

```bash
# Review snapshot changes
cargo insta review

# Update snapshots
cargo insta accept
```

## Code Style

### Formatting and Linting

Before submitting a PR, ensure your code passes all checks:

```bash
# Run all checks (format, clippy, etc.)
cargo lint

# Or run individually:
cargo fmt -- --check
cargo clippy --all-features -- -D warnings
```

### Guidelines

- Write documentation and comments in **English**
- Follow Rust naming conventions
- Keep functions small and focused
- Use meaningful variable and function names
- Avoid over-engineering; keep solutions simple

## Pull Request Process

1. **Fork** the repository and create a feature branch
2. **Write tests** for your changes
3. **Implement** the feature or fix
4. **Run checks** locally:
   ```bash
   cargo lint
   cargo test --all-features
   ```
5. **Push** your changes and open a PR
6. **Describe** your changes clearly in the PR description
7. **Wait for review** and address any feedback

### CI Checks

All PRs must pass the following CI checks:

- `cargo check --all-features`
- `cargo test --all-features`
- `cargo clippy --all-features -- -D warnings`
- `cargo fmt -- --check`
- `taplo` (Cargo.toml formatting)

## Commit Messages

Write clear, concise commit messages:

```
<type>: <short summary>

<optional body with more details>
```

### Types

- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation changes
- `test`: Adding or updating tests
- `refactor`: Code refactoring
- `chore`: Maintenance tasks

### Examples

```
feat: add split view UI for result comparison

fix: handle edge case when no agents are available

test: add snapshot tests for progress display

docs: update README with usage examples
```

## Questions?

If you have questions or need help, feel free to open an issue on GitHub.

Thank you for contributing! 🎉
