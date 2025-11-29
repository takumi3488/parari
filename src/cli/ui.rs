use std::io::{self, Write};
use std::path::Path;
use std::process::{Command, Stdio};

use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use crossterm::terminal;
use inquire::Select;

use crate::domain::{ResultInfo, TaskResult};
use crate::error::{Error, Result};

/// Mode for detail view when a model is selected
#[derive(Debug, Clone, Copy, PartialEq)]
enum DetailMode {
    Output,
    Diff,
}

impl DetailMode {
    /// Cycle to the next mode (Tab key)
    fn next(self) -> Self {
        match self {
            DetailMode::Output => DetailMode::Diff,
            DetailMode::Diff => DetailMode::Output,
        }
    }

    /// Get display name for the mode
    fn name(self) -> &'static str {
        match self {
            DetailMode::Output => "Output",
            DetailMode::Diff => "Diff",
        }
    }
}

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

/// Display results and allow user to select one with a two-stage UI
///
/// Stage 1: Select a model (claude, gemini, codex)
/// Stage 2: View details with Tab to switch modes (Output -> Diff -> Apply)
///          - Esc returns to Stage 1
///          - Enter in Apply mode applies the changes
pub fn select_result(results: &[TaskResult], result_infos: &[ResultInfo]) -> Result<usize> {
    if results.is_empty() {
        return Err(Error::NoExecutorsAvailable);
    }

    loop {
        display_results_summary(result_infos);

        // Stage 1: Build model selection options
        let options: Vec<String> = result_infos
            .iter()
            .enumerate()
            .map(|(i, info)| {
                let status = if info.success { "✓" } else { "✗" };
                let changes = format!("{} files", info.files_changed);
                format!(
                    "[{}] {} {} ({})",
                    i + 1,
                    status,
                    info.executor_name,
                    changes
                )
            })
            .collect();

        let selection = Select::new("Select a model:", options)
            .with_help_message("Use arrow keys to navigate, Enter to select, Esc to cancel")
            .prompt();

        match selection {
            Ok(selected) => {
                // Extract the index from the selected string
                let index = selected
                    .strip_prefix('[')
                    .and_then(|s| s.split(']').next())
                    .and_then(|s| s.parse::<usize>().ok())
                    .map(|n| n - 1)
                    .unwrap_or(0);

                if let Some(info) = result_infos.get(index) {
                    // Stage 2: Show detail view with mode switching
                    match show_detail_view(info) {
                        DetailViewResult::Apply => return Ok(index),
                        DetailViewResult::Back => continue, // Return to model selector
                        DetailViewResult::Cancel => return Err(Error::UserCancelled),
                    }
                }
            }
            Err(_) => return Err(Error::UserCancelled),
        }
    }
}

/// Result from the detail view
enum DetailViewResult {
    Apply,  // User chose to apply this result
    Back,   // User pressed Esc, go back to model selector
    Cancel, // User wants to cancel entirely
}

/// Show detail view for a selected model with Tab mode switching
fn show_detail_view(info: &ResultInfo) -> DetailViewResult {
    let mut mode = DetailMode::Output;

    loop {
        // Clear screen and show current mode
        clear_screen();
        print_mode_tabs(mode);
        println!(
            "\n{} {} {}",
            "=".repeat(20),
            info.executor_name.to_uppercase(),
            "=".repeat(20)
        );

        // Show content based on current mode
        match mode {
            DetailMode::Output => {
                show_executor_output(info);
            }
            DetailMode::Diff => {
                if let Err(e) = show_diff_with_delta(&info.worktree_path) {
                    eprintln!("Error showing diff: {}", e);
                }
            }
        }

        // Show help message
        println!();
        println!(
            "Press [Tab] for {}, [Shift+Enter] to apply, [Esc] to go back",
            mode.next().name()
        );

        // Handle key input
        if terminal::enable_raw_mode().is_err() {
            return DetailViewResult::Cancel;
        }

        let result = loop {
            if let Ok(Event::Key(key_event)) = event::read() {
                match key_event.code {
                    KeyCode::Tab => {
                        mode = mode.next();
                        break None; // Redraw with new mode
                    }
                    KeyCode::Enter if key_event.modifiers.contains(KeyModifiers::SHIFT) => {
                        break Some(DetailViewResult::Apply);
                    }
                    KeyCode::Esc => {
                        break Some(DetailViewResult::Back);
                    }
                    _ => continue,
                }
            }
        };

        let _ = terminal::disable_raw_mode();

        if let Some(r) = result {
            return r;
        }
    }
}

/// Clear the screen
fn clear_screen() {
    // Use ANSI escape codes to clear screen
    print!("\x1B[2J\x1B[H");
    let _ = io::stdout().flush();
}

/// Print mode tabs showing current selection
fn print_mode_tabs(current: DetailMode) {
    let modes = [DetailMode::Output, DetailMode::Diff];
    let tabs: Vec<String> = modes
        .iter()
        .map(|&m| {
            if m == current {
                format!("[{}]", m.name())
            } else {
                format!(" {} ", m.name())
            }
        })
        .collect();

    println!("{}", tabs.join("  "));
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

/// Display the stdout and stderr output from an executor
fn show_executor_output(info: &ResultInfo) {
    println!(
        "\n{} Output from {} {}",
        "=".repeat(20),
        info.executor_name.to_uppercase(),
        "=".repeat(20)
    );

    if !info.stdout.is_empty() {
        println!("\n--- STDOUT ---");
        println!("{}", info.stdout);
    } else {
        println!("\n--- STDOUT ---");
        println!("(no output)");
    }

    if !info.stderr.is_empty() {
        println!("\n--- STDERR ---");
        println!("{}", info.stderr);
    }
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
