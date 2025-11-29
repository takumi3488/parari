use std::path::Path;
use std::process::{Command, Stdio};

use crossterm::event::{self, Event, KeyCode};
use crossterm::terminal;
use inquire::Select;

use crate::domain::{ResultInfo, TaskResult};
use crate::error::{Error, Result};

/// Check if delta command is available
pub fn is_delta_available() -> bool {
    Command::new("delta")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Show diff using delta for a worktree
pub fn show_diff_with_delta(worktree_path: &Path) -> Result<()> {
    let use_delta = is_delta_available();

    // Get diff from the worktree
    let diff_output = Command::new("git")
        .args(["diff", "HEAD"])
        .current_dir(worktree_path)
        .output()
        .map_err(|e| Error::GitCommand {
            message: format!("Failed to get diff: {}", e),
        })?;

    let diff_str = String::from_utf8_lossy(&diff_output.stdout);

    if diff_str.is_empty() {
        // Try to get diff including untracked files
        let status_output = Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(worktree_path)
            .output()
            .map_err(|e| Error::GitCommand {
                message: format!("Failed to get status: {}", e),
            })?;

        let status_str = String::from_utf8_lossy(&status_output.stdout);
        if status_str.is_empty() {
            println!("\nNo changes detected.");
            return Ok(());
        }

        // Show new files with delta if available
        println!("\nNew/Untracked files:");
        for line in status_str.lines() {
            if line.starts_with("??") || line.starts_with("A ") {
                let file = line.split_whitespace().last().unwrap_or("");
                let file_path = worktree_path.join(file);

                if file_path.exists() && file_path.is_file() {
                    if use_delta {
                        // Use git diff --no-index with delta for new files
                        let _ = Command::new("git")
                            .args([
                                "-c",
                                "core.pager=delta --paging=never",
                                "-c",
                                "color.diff=always",
                                "diff",
                                "--no-index",
                                "/dev/null",
                            ])
                            .arg(&file_path)
                            .current_dir(worktree_path)
                            .stdout(Stdio::inherit())
                            .stderr(Stdio::inherit())
                            .status();
                    } else {
                        // Fallback: show plain diff
                        println!("  + {}", file);
                        if let Ok(content) = std::fs::read_to_string(&file_path) {
                            println!("\n--- /dev/null");
                            println!("+++ {}", file);
                            for line in content.lines().take(50) {
                                println!("+{}", line);
                            }
                            if content.lines().count() > 50 {
                                println!("... (truncated)");
                            }
                        }
                    }
                }
            }
        }
        return Ok(());
    }

    if use_delta {
        // Use git with delta as pager, forcing color output
        Command::new("git")
            .args([
                "-c",
                "core.pager=delta --paging=never",
                "-c",
                "color.diff=always",
                "diff",
                "HEAD",
            ])
            .current_dir(worktree_path)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .map_err(|e| Error::GitCommand {
                message: format!("Failed to run git diff with delta: {}", e),
            })?;
    } else {
        // Fallback: show plain diff
        println!("\n(Tip: Install 'delta' for better diff output)");
        println!("{}", diff_str);
    }

    Ok(())
}

/// Display results and allow user to select one
pub fn select_result(results: &[TaskResult], result_infos: &[ResultInfo]) -> Result<usize> {
    if results.is_empty() {
        return Err(Error::NoExecutorsAvailable);
    }

    loop {
        display_results_summary(result_infos);

        // Build options: apply options + view diff options
        let mut options: Vec<String> = Vec::new();

        // Apply options
        for (i, info) in result_infos.iter().enumerate() {
            let status = if info.success { "✓" } else { "✗" };
            let changes = format!("{} files changed", info.files_changed);
            options.push(format!(
                "Apply: {} {} - {} ({})",
                i + 1,
                status,
                info.executor_name,
                changes
            ));
        }

        // View diff options
        for (i, info) in result_infos.iter().enumerate() {
            options.push(format!("View diff: [{}] {}", i + 1, info.executor_name));
        }

        let selection = Select::new("Select an action:", options)
            .with_help_message(
                "Use arrow keys to navigate, Enter to select, Esc to cancel. View diff uses delta if available.",
            )
            .prompt();

        match selection {
            Ok(selected) => {
                if selected.starts_with("Apply:") {
                    // Extract the index from the selected string
                    let index = selected
                        .strip_prefix("Apply: ")
                        .and_then(|s| s.split_whitespace().next())
                        .and_then(|s| s.parse::<usize>().ok())
                        .map(|n| n - 1)
                        .unwrap_or(0);
                    return Ok(index);
                } else if selected.starts_with("View diff:") {
                    // Extract the index from the selected string
                    let index = selected
                        .strip_prefix("View diff: [")
                        .and_then(|s| s.split(']').next())
                        .and_then(|s| s.parse::<usize>().ok())
                        .map(|n| n - 1)
                        .unwrap_or(0);

                    if let Some(info) = result_infos.get(index) {
                        println!(
                            "\n{} Showing diff for {} {}",
                            "=".repeat(20),
                            info.executor_name.to_uppercase(),
                            "=".repeat(20)
                        );
                        if let Err(e) = show_diff_with_delta(&info.worktree_path) {
                            eprintln!("Error showing diff: {}", e);
                        }
                        println!("\nPress Enter or Esc to go back...");
                        wait_for_key_press();
                    }
                    // Loop continues to show the menu again
                }
            }
            Err(_) => return Err(Error::UserCancelled),
        }
    }
}

/// Display a summary of all results
fn display_results_summary(result_infos: &[ResultInfo]) {
    println!("\n{}", "=".repeat(50));
    println!("Results:");
    println!("{}", "=".repeat(50));

    for (i, info) in result_infos.iter().enumerate() {
        let status = if info.success { "✓" } else { "✗" };
        println!(
            "\n[{}] {} {}",
            i + 1,
            status,
            info.executor_name.to_uppercase()
        );
        println!("    Files changed: {}", info.files_changed);

        if let Some(ref summary) = info.change_summary {
            if summary.files_added > 0 {
                println!("    + {} added", summary.files_added);
            }
            if summary.files_modified > 0 {
                println!("    ~ {} modified", summary.files_modified);
            }
            if summary.files_deleted > 0 {
                println!("    - {} deleted", summary.files_deleted);
            }
        }
    }

    println!("\n{}", "=".repeat(50));
}

/// Display a message when applying changes
pub fn show_applying_message(executor_name: &str) {
    println!("\nApplying changes from {}...", executor_name);
}

/// Display a success message
pub fn show_success_message() {
    println!("Changes applied successfully!");
}

/// Display an error message
pub fn show_error(error: &Error) {
    eprintln!("Error: {}", error);
}

/// Display progress message
pub fn show_progress(message: &str) {
    println!("{}", message);
}

/// Display waiting message while executors are running
pub fn show_running_message(executor_names: &[&str]) {
    println!("\nRunning AI CLI tools in parallel:");
    for name in executor_names {
        println!("  - {}", name);
    }
    println!("\nThis may take a while...\n");
}

/// Wait for Enter or Esc key press
fn wait_for_key_press() {
    if terminal::enable_raw_mode().is_err() {
        // Fallback to stdin if raw mode fails
        let _ = std::io::stdin().read_line(&mut String::new());
        return;
    }

    loop {
        if let Ok(Event::Key(key_event)) = event::read() {
            match key_event.code {
                KeyCode::Enter | KeyCode::Esc => break,
                _ => continue,
            }
        }
    }

    let _ = terminal::disable_raw_mode();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_show_progress() {
        // Just ensure it doesn't panic
        show_progress("Test message");
    }
}
