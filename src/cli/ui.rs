use inquire::Select;

use crate::domain::{ResultInfo, TaskResult};
use crate::error::{Error, Result};

/// Display results and allow user to select one
pub fn select_result(results: &[TaskResult], result_infos: &[ResultInfo]) -> Result<usize> {
    if results.is_empty() {
        return Err(Error::NoExecutorsAvailable);
    }

    if results.len() == 1 {
        println!("\nOnly one result available, auto-selecting...");
        return Ok(0);
    }

    let options: Vec<String> = result_infos
        .iter()
        .enumerate()
        .map(|(i, info)| {
            let status = if info.success { "✓" } else { "✗" };
            let changes = format!("{} files changed", info.files_changed);
            format!(
                "{} {} - {} ({})",
                i + 1,
                status,
                info.executor_name,
                changes
            )
        })
        .collect();

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

        if let Some(ref diff) = info.diff
            && !diff.is_empty()
        {
            println!("\n    Diff preview:");
            for line in diff.lines().take(20) {
                println!("    {}", line);
            }
            if diff.lines().count() > 20 {
                println!("    ... (truncated)");
            }
        }
    }

    println!("\n{}", "=".repeat(50));

    let selection = Select::new("Select a result to apply:", options)
        .with_help_message("Use arrow keys to navigate, Enter to select, Esc to cancel")
        .prompt();

    match selection {
        Ok(selected) => {
            // Extract the index from the selected string
            let index = selected
                .split_whitespace()
                .next()
                .and_then(|s| s.parse::<usize>().ok())
                .map(|n| n - 1)
                .unwrap_or(0);
            Ok(index)
        }
        Err(_) => Err(Error::UserCancelled),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_show_progress() {
        // Just ensure it doesn't panic
        show_progress("Test message");
    }
}
