use std::path::Path;
use std::process::Command;

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};

use super::types::ViewMode;
use crate::domain::ResultInfo;
use crate::executor::OutputLine;

/// Special marker for stderr lines (invisible character used for detection in style_log_line)
pub const STDERR_MARKER: &str = "\x01STDERR\x02";

/// Strip ANSI escape codes from a string
pub fn strip_ansi_codes(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\x1b' {
            if chars.peek() == Some(&'[') {
                chars.next();
                while let Some(&next) = chars.peek() {
                    chars.next();
                    if next.is_ascii_alphabetic() {
                        break;
                    }
                }
            } else if chars.peek() == Some(&']') {
                chars.next();
                while let Some(&next) = chars.peek() {
                    if next == '\x07' {
                        chars.next();
                        break;
                    }
                    if next == '\x1b' {
                        chars.next();
                        if chars.peek() == Some(&'\\') {
                            chars.next();
                        }
                        break;
                    }
                    chars.next();
                }
            }
        } else {
            result.push(c);
        }
    }

    result
}

pub fn get_log_content_string(info: &ResultInfo) -> String {
    let mut content = String::new();

    // Header
    let emoji = get_agent_emoji(&info.executor_name);
    let status = if info.success { "Success" } else { "Failed" };

    content.push_str(&format!(
        "{} {} - {}\n",
        emoji,
        info.executor_name.to_uppercase(),
        status
    ));
    content.push_str(&"=".repeat(50));
    content.push('\n');
    content.push('\n');

    // Summary
    content.push_str("Summary:\n");
    content.push_str(&format!("  Files changed: {}\n", info.files_changed));

    if let Some(ref summary) = info.change_summary {
        if summary.files_added > 0 {
            content.push_str(&format!("  + {} added\n", summary.files_added));
        }
        if summary.files_modified > 0 {
            content.push_str(&format!("  ~ {} modified\n", summary.files_modified));
        }
        if summary.files_deleted > 0 {
            content.push_str(&format!("  - {} deleted\n", summary.files_deleted));
        }
    }
    content.push('\n');

    // Output (stdout and stderr interleaved in order of arrival)
    content.push_str(&"-".repeat(50));
    content.push('\n');
    content.push_str("Output:\n");
    content.push_str(&"-".repeat(50));
    content.push('\n');

    if info.output_lines.is_empty() {
        content.push_str("(no output)\n");
    } else {
        for output_line in &info.output_lines {
            match output_line {
                OutputLine::Stdout(line) => {
                    let cleaned = strip_ansi_codes(line);
                    content.push_str(&cleaned);
                    content.push('\n');
                }
                OutputLine::Stderr(line) => {
                    // Add marker for stderr lines so style_log_line can detect them
                    let cleaned = strip_ansi_codes(line);
                    content.push_str(STDERR_MARKER);
                    content.push_str(&cleaned);
                    content.push('\n');
                }
            }
        }
    }

    content
}

pub fn get_diff_content_string(worktree_path: &Path) -> String {
    let diff_output = Command::new("git")
        .args(["diff", "HEAD"])
        .current_dir(worktree_path)
        .output();

    match diff_output {
        Ok(output) => {
            let diff_str = String::from_utf8_lossy(&output.stdout);
            if diff_str.is_empty() {
                get_untracked_files_string(worktree_path)
            } else {
                diff_str.to_string()
            }
        }
        Err(e) => format!("Error getting diff: {}", e),
    }
}

pub fn get_untracked_files_string(worktree_path: &Path) -> String {
    let status_output = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(worktree_path)
        .output();

    match status_output {
        Ok(status) => {
            let status_str = String::from_utf8_lossy(&status.stdout);
            if status_str.is_empty() {
                "No changes detected.".to_string()
            } else {
                let mut content = String::new();
                content.push_str("New/Untracked files:\n\n");

                for line in status_str.lines() {
                    if line.starts_with("??") || line.starts_with("A ") {
                        let file = line.split_whitespace().last().unwrap_or("");
                        let file_path = worktree_path.join(file);

                        if file_path.exists() && file_path.is_file() {
                            content.push_str(&format!("+ {}\n", file));
                            content.push_str(&"-".repeat(40));
                            content.push('\n');

                            if let Ok(file_content) = std::fs::read_to_string(&file_path) {
                                for (i, file_line) in file_content.lines().enumerate().take(100) {
                                    content.push_str(&format!("{:4} | +{}\n", i + 1, file_line));
                                }
                                if file_content.lines().count() > 100 {
                                    content.push_str("... (truncated)\n");
                                }
                            }
                            content.push('\n');
                        }
                    }
                }
                content
            }
        }
        Err(e) => format!("Error getting status: {}", e),
    }
}

pub fn get_styled_content(content: &str, mode: ViewMode) -> Text<'static> {
    let mut lines = Vec::new();

    for line in content.lines() {
        let styled_line = match mode {
            ViewMode::Log => style_log_line(line),
            ViewMode::Diff => style_diff_line(line),
        };
        lines.push(styled_line);
    }

    Text::from(lines)
}

pub fn get_styled_content_with_search(content: &str, mode: ViewMode, query: &str) -> Text<'static> {
    let mut lines = Vec::new();
    let query_lower = query.to_lowercase();

    for line in content.lines() {
        // Handle stderr marker
        let (actual_line, is_stderr) = if let Some(stripped) = line.strip_prefix(STDERR_MARKER) {
            (stripped, true)
        } else {
            (line, false)
        };

        let line_lower = actual_line.to_lowercase();
        if line_lower.contains(&query_lower) {
            // Highlight search matches
            let mut spans = Vec::new();
            let mut last_end = 0;

            // Base style for stderr lines
            let base_style = if is_stderr {
                Style::new().fg(Color::Red)
            } else {
                Style::new()
            };

            for (start, _) in line_lower.match_indices(&query_lower) {
                if start > last_end {
                    spans.push(Span::styled(
                        actual_line[last_end..start].to_string(),
                        base_style,
                    ));
                }
                spans.push(Span::styled(
                    actual_line[start..start + query.len()].to_string(),
                    Style::new()
                        .fg(Color::Black)
                        .bg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ));
                last_end = start + query.len();
            }

            if last_end < actual_line.len() {
                spans.push(Span::styled(
                    actual_line[last_end..].to_string(),
                    base_style,
                ));
            }

            lines.push(Line::from(spans));
        } else {
            let styled_line = match mode {
                ViewMode::Log => style_log_line(line),
                ViewMode::Diff => style_diff_line(line),
            };
            lines.push(styled_line);
        }
    }

    Text::from(lines)
}

pub fn style_log_line(line: &str) -> Line<'static> {
    // Check for stderr marker first - display in red and remove the marker
    if let Some(content) = line.strip_prefix(STDERR_MARKER) {
        return Line::styled(content.to_string(), Style::new().fg(Color::Red));
    }

    if line.starts_with("Output:") || line.starts_with("Summary:") {
        Line::styled(line.to_string(), Style::new().add_modifier(Modifier::BOLD))
    } else if line.starts_with("  +") {
        Line::styled(line.to_string(), Style::new().fg(Color::Green))
    } else if line.starts_with("  ~") {
        Line::styled(line.to_string(), Style::new().fg(Color::Yellow))
    } else if line.starts_with("  -") {
        Line::styled(line.to_string(), Style::new().fg(Color::Red))
    } else if line.starts_with('=') || line.starts_with('-') {
        Line::styled(line.to_string(), Style::new().fg(Color::DarkGray))
    } else if line.contains("Success") {
        Line::styled(
            line.to_string(),
            Style::new().fg(Color::Green).add_modifier(Modifier::BOLD),
        )
    } else if line.contains("Failed") {
        Line::styled(
            line.to_string(),
            Style::new().fg(Color::Red).add_modifier(Modifier::BOLD),
        )
    } else if line == "(no output)" {
        Line::styled(line.to_string(), Style::new().fg(Color::DarkGray))
    } else {
        Line::raw(line.to_string())
    }
}

pub fn style_diff_line(line: &str) -> Line<'static> {
    if line.starts_with("+++") || line.starts_with("---") {
        Line::styled(line.to_string(), Style::new().fg(Color::Yellow))
    } else if line.starts_with('+') {
        Line::styled(line.to_string(), Style::new().fg(Color::Green))
    } else if line.starts_with('-') {
        Line::styled(line.to_string(), Style::new().fg(Color::Red))
    } else if line.starts_with("@@") || line.starts_with("diff ") || line.starts_with("index ") {
        Line::styled(line.to_string(), Style::new().fg(Color::Cyan))
    } else if line.starts_with("New/Untracked") {
        Line::styled(
            line.to_string(),
            Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )
    } else {
        Line::raw(line.to_string())
    }
}

pub fn get_agent_emoji(name: &str) -> &'static str {
    match name.to_lowercase().as_str() {
        "claude" => "\u{1F916}", // Robot
        "gemini" => "\u{2728}",  // Sparkles
        "codex" => "\u{1F4E6}",  // Package
        _ => "\u{1F4BB}",        // Computer
    }
}
