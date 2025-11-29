# parari

A CLI tool that runs multiple AI coding agents in parallel using git worktrees.

## Overview

parari enables you to run [Claude Code](https://github.com/anthropics/claude-code), [Gemini CLI](https://github.com/google-gemini/gemini-cli), and [Codex CLI](https://github.com/openai/codex) simultaneously on the same task. Each assistant works in its own git worktree, allowing you to compare results and choose the best solution.

This is similar to [Cursor's worktree feature](https://cursor.com/en-US/docs/configuration/worktrees), but for the command line.

## Supported Agents

| Agent | Status |
|-------|--------|
| Claude | ✅ |
| Gemini | ✅ |
| Codex | 🚧 |

## Features

- Run multiple AI agents in parallel on the same task
- Each assistant works in an isolated git worktree
- Compare results from different agents
- Choose and merge the best solution

## Requirements

### Supported Platforms

- macOS
- Linux

### Dependencies

- Git with worktree support
- One or more of the following CLI tools in your PATH:
  - `claude` (Claude Code)
  - `gemini` (Gemini CLI)
  - `codex` (Codex CLI)

### Recommended

- [delta](https://dandavison.github.io/delta/) - A syntax-highlighting pager for git diff output. Makes comparing worktree changes much easier to read.

## Installation

```bash
cargo install parari
```

## Usage

```bash
# Run all available agents on a task
parari "Fix the bug in the login function"

# Run specific agents
parari --agents claude,gemini "Add unit tests for the parser module"

# Open default editor ($EDITOR or vi) to write a prompt
parari
```

If no prompt is provided, parari opens your default editor (set by `$EDITOR` environment variable, defaults to `vi`) where you can write a multi-line prompt.
