use std::path::PathBuf;
use std::sync::Arc;

use parari::cli::Args;
use parari::cli::progress::{ProgressTracker, display_completion_summary, display_header};
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

    // Collect available executors
    let executors = get_executors(args.agents.as_deref()).await;

    if executors.is_empty() {
        return Err(Error::NoExecutorsAvailable);
    }

    // Collect executor names before moving executors
    let executor_names: Vec<String> = executors.iter().map(|e| e.name().to_string()).collect();
    let executor_name_refs: Vec<&str> = executor_names.iter().map(|s| s.as_str()).collect();

    // Display header with agent info
    display_header(&executor_name_refs);

    // Create progress tracker
    let progress = Arc::new(ProgressTracker::new(&executor_name_refs));

    // Run the task with progress tracking
    let results = runner
        .run_with_progress(&prompt, executors, Some(progress))
        .await?;

    // Collect completed and failed agents for summary
    let completed: Vec<&str> = results
        .iter()
        .map(|r| r.execution.executor_name.as_str())
        .collect();
    let failed: Vec<&str> = executor_names
        .iter()
        .map(|s| s.as_str())
        .filter(|name| !completed.iter().any(|c| c == name))
        .collect();

    // Display completion summary
    display_completion_summary(&completed, &failed);

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
    let selected_index = cli::select_result(&results, &result_infos)?;

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

/// Filter executors based on agent filter
fn filter_executors(
    executors: Vec<Arc<dyn Executor>>,
    agent_filter: Option<&[String]>,
) -> Vec<Arc<dyn Executor>> {
    match agent_filter {
        Some(agents) => executors
            .into_iter()
            .filter(|e| {
                let name = e.name().to_lowercase();
                agents.iter().any(|a| name.contains(&a.to_lowercase()))
            })
            .collect(),
        None => executors,
    }
}

/// Get all available executors (mock version for development/testing)
#[cfg(feature = "mock")]
async fn get_executors(agent_filter: Option<&[String]>) -> Vec<Arc<dyn Executor>> {
    eprintln!("[MOCK MODE] Using mock executors for development");

    let all_executors: Vec<Arc<dyn Executor>> = vec![
        Arc::new(
            MockExecutor::new("mock-claude")
                .with_file("mock-claude-output.txt", "This is mock output from Claude"),
        ),
        Arc::new(
            MockExecutor::new("mock-gemini")
                .with_file("mock-gemini-output.txt", "This is mock output from Gemini"),
        ),
        Arc::new(
            MockExecutor::new("mock-codex")
                .with_file("mock-codex-output.txt", "This is mock output from Codex"),
        ),
    ];

    filter_executors(all_executors, agent_filter)
}

/// Get all available executors (production version)
#[cfg(not(feature = "mock"))]
async fn get_executors(agent_filter: Option<&[String]>) -> Vec<Arc<dyn Executor>> {
    let mut executors: Vec<Arc<dyn Executor>> = Vec::new();

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

    filter_executors(executors, agent_filter)
}
