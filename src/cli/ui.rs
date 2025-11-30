use std::path::Path;
use std::process::{Command, Stdio};

use console::style;

use crate::cli::progress::AgentStyle;
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

/// Display results in a split view and allow user to select one
///
/// Uses a lazydocker-style split view with:
/// - Left panel: model list (Claude, Gemini, Codex)
/// - Right panel: details (Output/Diff) for the selected model
/// - Tab: switch between Output and Diff modes
/// - 'a': apply the selected result
/// - Esc/q: cancel
pub fn select_result(results: &[TaskResult], result_infos: &[ResultInfo]) -> Result<usize> {
    if results.is_empty() {
        return Err(Error::NoExecutorsAvailable);
    }

    // Use the new split view
    super::split_view::select_result_split_view(result_infos)
}

/// Display a message when applying changes
pub fn show_applying_message(executor_name: &str) {
    let agent_style = AgentStyle::for_agent(executor_name);
    println!();
    println!(
        "  {} {} Applying changes from {}...",
        style("🔧").bold(),
        agent_style.emoji,
        style(executor_name.to_uppercase()).bold().cyan()
    );
}

/// Display a success message
pub fn show_success_message() {
    println!();
    println!("{}", style("━".repeat(50)).green());
    println!(
        "  {} {}",
        style("✅").bold(),
        style("Changes applied successfully!").bold().green()
    );
    println!("{}", style("━".repeat(50)).green());
    println!();
}

/// Display an error message
pub fn show_error(error: &Error) {
    eprintln!();
    eprintln!("{}", style("━".repeat(50)).red());
    eprintln!(
        "  {} {} {}",
        style("❌").bold(),
        style("Error:").bold().red(),
        style(error).red()
    );
    eprintln!("{}", style("━".repeat(50)).red());
    eprintln!();
}

/// Display progress message
pub fn show_progress(message: &str) {
    println!("  {} {}", style("ℹ️").bold(), style(message).cyan());
}

/// Display waiting message while executors are running
pub fn show_running_message(executor_names: &[&str]) {
    println!("\nRunning AI CLI tools in parallel:");
    for name in executor_names {
        println!("  - {}", name);
    }
    println!("\nThis may take a while...\n");
}

/// Display warning about uncommitted changes and ask for confirmation
pub fn confirm_overwrite_uncommitted(uncommitted_files: &[String]) -> Result<bool> {
    use crossterm::style::Stylize;

    println!(
        "\n{}",
        "Warning: You have uncommitted changes!".yellow().bold()
    );
    println!("The following files will be overwritten:\n");

    for file in uncommitted_files.iter().take(10) {
        println!("  {}", file.as_str().yellow());
    }

    if uncommitted_files.len() > 10 {
        println!("  ... and {} more files", uncommitted_files.len() - 10);
    }

    println!();
    print!("Do you want to continue? [y/N]: ");
    std::io::Write::flush(&mut std::io::stdout())?;

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;

    let answer = input.trim().to_lowercase();
    Ok(answer == "y" || answer == "yes")
}

/// Display warning about conflicting files and ask for confirmation
pub fn confirm_apply_with_conflicts(conflicts: &[String]) -> Result<bool> {
    use crossterm::style::Stylize;

    println!(
        "\n{}",
        "Warning: The following files have conflicts!".red().bold()
    );
    println!(
        "These files have been modified both in your working directory and the selected result:\n"
    );

    for file in conflicts.iter().take(10) {
        println!("  {}", file.as_str().red());
    }

    if conflicts.len() > 10 {
        println!("  ... and {} more files", conflicts.len() - 10);
    }

    println!("\nApplying will overwrite your local changes in these files.");
    print!("Do you want to continue? [y/N]: ");
    std::io::Write::flush(&mut std::io::stdout())?;

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;

    let answer = input.trim().to_lowercase();
    Ok(answer == "y" || answer == "yes")
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
