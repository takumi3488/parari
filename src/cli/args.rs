use clap::Parser;

/// Run AI CLI tools in parallel using git worktrees
#[derive(Parser, Debug)]
#[command(name = "parari")]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// The prompt/task to send to AI CLI tools (opens editor if not provided)
    pub prompt: Option<String>,

    /// Working directory (defaults to current directory)
    #[arg(short = 'C', long, default_value = ".")]
    pub directory: String,

    /// Comma-separated list of agents to use (e.g., "claude,gemini")
    /// Available agents: claude, gemini, codex
    #[arg(short, long, value_delimiter = ',')]
    pub agents: Option<Vec<String>>,
}

impl Args {
    /// Parse arguments from command line
    pub fn parse_args() -> Self {
        Self::parse()
    }
}
