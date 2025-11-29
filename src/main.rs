use std::path::PathBuf;
use std::sync::Arc;

use parari::cli::{Args, ExecutorFilter};
use parari::domain::{self, DisplayOptions, TaskRunner, cleanup_all_registered_worktrees};
use parari::error::{Error, Result};
#[cfg(not(feature = "mock"))]
use parari::executor::claude::ClaudeExecutor;
#[cfg(not(feature = "mock"))]
use parari::executor::codex::CodexExecutor;
#[cfg(not(feature = "mock"))]
use parari::executor::gemini::GeminiExecutor;
#[cfg(feature = "mock")]
use parari::executor::mock::MockExecutor;
use parari::executor::traits::Executor;
use parari::{cli, git};

#[tokio::main]
async fn main() {
    // Run the main task with signal handling
    let result = tokio::select! {
        result = run() => result,
        _ = tokio::signal::ctrl_c() => {
            eprintln!("\nReceived interrupt signal, cleaning up worktrees...");
            cleanup_all_registered_worktrees();
            std::process::exit(130); // Standard exit code for SIGINT
        }
    };

    // Cleanup any remaining worktrees on normal exit
    cleanup_all_registered_worktrees();

    if let Err(e) = result {
        match e {
            Error::UserCancelled => {
                eprintln!("Cancelled.");
            }
            _ => {
                cli::show_error(&e);
                std::process::exit(1);
            }
        }
    }
}

async fn run() -> Result<()> {
    let args = Args::parse_args();

    // Get prompt from args or open editor
    let prompt = match args.prompt.clone() {
        Some(p) => p,
        None => cli::open_editor_for_prompt()?,
    };

    // Resolve working directory
    let working_dir = PathBuf::from(&args.directory).canonicalize()?;

    // Check if it's a git repository
    if !git::is_git_repository(&working_dir).await {
        return Err(Error::NotGitRepository {
            path: working_dir.clone(),
        });
    }

    cli::show_progress(&format!("Working directory: {}", working_dir.display()));

    // Create task runner
    let mut runner = TaskRunner::new(&working_dir).await?;

    // Collect available executors based on filter
    let executors = get_executors(&args.get_executor_filter()).await;

    if executors.is_empty() {
        return Err(Error::NoExecutorsAvailable);
    }

    let executor_names: Vec<&str> = executors.iter().map(|e| e.name()).collect();
    cli::show_running_message(&executor_names);

    // Run the task
    let results = runner.run(&prompt, executors).await?;

    if results.is_empty() {
        cli::show_progress("No results were produced.");
        runner.cleanup().await?;
        return Ok(());
    }

    // Prepare result info for display
    let display_options = DisplayOptions::default();

    let mut result_infos = Vec::new();
    for result in &results {
        let info = domain::prepare_result_info(result, &working_dir, &display_options).await?;
        result_infos.push(info);
    }

    // Handle selection
    let selected_index = if args.no_select {
        cli::show_progress("Skipping result selection (--no-select)");
        runner.cleanup().await?;
        return Ok(());
    } else if args.auto_select {
        let ranked = domain::rank_results(&results);
        ranked.first().copied().unwrap_or(0)
    } else {
        cli::select_result(&results, &result_infos)?
    };

    // Apply the selected result
    let selected_result = &results[selected_index];
    let selected_info = &result_infos[selected_index];

    cli::show_applying_message(&selected_info.executor_name);
    domain::apply_result(selected_result, &working_dir).await?;
    cli::show_success_message();

    // Cleanup worktrees
    runner.cleanup().await?;

    Ok(())
}

/// Get executors based on filter (mock version for development/testing)
#[cfg(feature = "mock")]
async fn get_executors(filter: &ExecutorFilter) -> Vec<Arc<dyn Executor>> {
    eprintln!("[MOCK MODE] Using mock executors for development");

    let mut executors: Vec<Arc<dyn Executor>> = Vec::new();

    match filter {
        ExecutorFilter::All => {
            executors.push(Arc::new(MockExecutor::new("mock-claude").with_file(
                "mock-claude-output.txt",
                "This is mock output from Claude",
            )));
            executors.push(Arc::new(MockExecutor::new("mock-gemini").with_file(
                "mock-gemini-output.txt",
                "This is mock output from Gemini",
            )));
            executors.push(Arc::new(
                MockExecutor::new("mock-codex")
                    .with_file("mock-codex-output.txt", "This is mock output from Codex"),
            ));
        }
        ExecutorFilter::ClaudeOnly => {
            executors.push(Arc::new(MockExecutor::new("mock-claude").with_file(
                "mock-claude-output.txt",
                "This is mock output from Claude",
            )));
        }
        ExecutorFilter::GeminiOnly => {
            executors.push(Arc::new(MockExecutor::new("mock-gemini").with_file(
                "mock-gemini-output.txt",
                "This is mock output from Gemini",
            )));
        }
        ExecutorFilter::CodexOnly => {
            executors.push(Arc::new(
                MockExecutor::new("mock-codex")
                    .with_file("mock-codex-output.txt", "This is mock output from Codex"),
            ));
        }
    }

    executors
}

/// Get executors based on filter (production version)
#[cfg(not(feature = "mock"))]
async fn get_executors(filter: &ExecutorFilter) -> Vec<Arc<dyn Executor>> {
    let mut executors: Vec<Arc<dyn Executor>> = Vec::new();

    match filter {
        ExecutorFilter::All => {
            let claude = Arc::new(ClaudeExecutor::new());
            if claude.is_available().await {
                executors.push(claude);
            }

            let gemini = Arc::new(GeminiExecutor::new());
            if gemini.is_available().await {
                executors.push(gemini);
            }

            let codex = Arc::new(CodexExecutor::new());
            if codex.is_available().await {
                executors.push(codex);
            }
        }
        ExecutorFilter::ClaudeOnly => {
            let claude = Arc::new(ClaudeExecutor::new());
            if claude.is_available().await {
                executors.push(claude);
            }
        }
        ExecutorFilter::GeminiOnly => {
            let gemini = Arc::new(GeminiExecutor::new());
            if gemini.is_available().await {
                executors.push(gemini);
            }
        }
        ExecutorFilter::CodexOnly => {
            let codex = Arc::new(CodexExecutor::new());
            if codex.is_available().await {
                executors.push(codex);
            }
        }
    }

    executors
}
