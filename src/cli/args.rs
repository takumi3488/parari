use clap::Parser;

/// Run AI CLI tools in parallel using git worktrees
#[derive(Parser, Debug)]
#[command(name = "parari")]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// The prompt/task to send to AI CLI tools (opens editor if not provided)
    pub prompt: Option<String>,

    /// Use only Claude CLI
    #[arg(long, conflicts_with_all = &["gemini_only", "codex_only"])]
    pub claude_only: bool,

    /// Use only Gemini CLI
    #[arg(long, conflicts_with_all = &["claude_only", "codex_only"])]
    pub gemini_only: bool,

    /// Use only Codex CLI
    #[arg(long, conflicts_with_all = &["claude_only", "gemini_only"])]
    pub codex_only: bool,

    /// Working directory (defaults to current directory)
    #[arg(short = 'C', long, default_value = ".")]
    pub directory: String,

    /// Skip result selection and exit after execution
    #[arg(long)]
    pub no_select: bool,

    /// Automatically select the result with most changes
    #[arg(long)]
    pub auto_select: bool,
}

impl Args {
    /// Parse arguments from command line
    pub fn parse_args() -> Self {
        Self::parse()
    }

    /// Get which executors should be used based on flags
    pub fn get_executor_filter(&self) -> ExecutorFilter {
        if self.claude_only {
            ExecutorFilter::ClaudeOnly
        } else if self.gemini_only {
            ExecutorFilter::GeminiOnly
        } else if self.codex_only {
            ExecutorFilter::CodexOnly
        } else {
            ExecutorFilter::All
        }
    }
}

/// Filter for which executors to use
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutorFilter {
    All,
    ClaudeOnly,
    GeminiOnly,
    CodexOnly,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_executor_filter_default() {
        let args = Args {
            prompt: Some("test".to_string()),
            claude_only: false,
            gemini_only: false,
            codex_only: false,
            directory: ".".to_string(),
            no_select: false,
            auto_select: false,
        };

        assert_eq!(args.get_executor_filter(), ExecutorFilter::All);
    }

    #[test]
    fn test_executor_filter_claude_only() {
        let args = Args {
            prompt: Some("test".to_string()),
            claude_only: true,
            gemini_only: false,
            codex_only: false,
            directory: ".".to_string(),
            no_select: false,
            auto_select: false,
        };

        assert_eq!(args.get_executor_filter(), ExecutorFilter::ClaudeOnly);
    }
}
