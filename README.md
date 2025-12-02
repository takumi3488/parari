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
| Codex | ✅ |

## Features

- Run multiple AI agents in parallel on the same task
- Each assistant works in an isolated git worktree
- Compare results from different agents with a split-view UI
- Choose and merge the best solution

### Split View UI

After all agents complete their tasks, parari displays a lazydocker-style split view:

```
┌─────────────────────────────────────────────────────────────────┐
│ Parari - Results                                                │
├────────────────────────────┬────────────────────────────────────┤
│ ▶ Models                   │ Log                                │
├────────────────────────────┼────────────────────────────────────┤
│ > 🤖 claude [+] (3 files)  │                                    │
│   ✨ gemini [+] (2 files)  │ (selected model's log/diff)        │
│   📦 codex  [x] (0 files)  │                                    │
├────────────────────────────┴────────────────────────────────────┤
│ [f] Focus  [l] Log  [d] Diff  [a] Apply  [q] Cancel             │
└─────────────────────────────────────────────────────────────────┘
```

- **Left panel**: List of AI agents with their status and file counts
- **Right panel**: Log output or diff from the selected agent
- **f**: Switch focus between panels (focused panel shows `▶` in title)
- **l**: Show log (stdout/stderr output)
- **d**: Show diff (code changes)
- **j/k**: Navigate between agents (when left panel is focused)
- **Arrow keys**: Scroll content (when right panel is focused)
- **a**: Apply the selected result
- **q**: Cancel

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
