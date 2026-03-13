use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use console::style;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use tokio::sync::Mutex;

/// Agent emoji and color configuration
#[derive(Clone)]
pub struct AgentStyle {
    pub emoji: &'static str,
    pub color: &'static str,
}

impl AgentStyle {
    #[must_use]
    pub fn for_agent(name: &str) -> Self {
        match name.to_lowercase().as_str() {
            "claude" => AgentStyle {
                emoji: "🤖",
                color: "magenta",
            },
            "gemini" => AgentStyle {
                emoji: "💎",
                color: "cyan",
            },
            "codex" => AgentStyle {
                emoji: "🧠",
                color: "green",
            },
            _ => AgentStyle {
                emoji: "⚡",
                color: "yellow",
            },
        }
    }
}

/// Status of an agent execution
#[derive(Clone, Copy, Debug)]
pub enum AgentStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

impl AgentStatus {
    fn emoji(self) -> &'static str {
        match self {
            AgentStatus::Pending => "⏳",
            AgentStatus::Running => "🔄",
            AgentStatus::Completed => "✅",
            AgentStatus::Failed => "❌",
        }
    }
}

/// Progress tracker for multiple agents
pub struct ProgressTracker {
    multi_progress: MultiProgress,
    bars: HashMap<String, ProgressBar>,
}

impl ProgressTracker {
    /// Create a new progress tracker for the given agent names
    #[must_use]
    pub fn new(agent_names: &[&str]) -> Self {
        let multi_progress = MultiProgress::new();
        let mut bars = HashMap::new();

        // Create spinner style with custom characters
        // The template is a constant string, so it should always be valid.
        let spinner_style = ProgressStyle::with_template("{spinner:.bold} {prefix:.bold} {msg}")
            .unwrap_or_else(|_| ProgressStyle::default_spinner())
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏");

        for name in agent_names {
            let agent_style = AgentStyle::for_agent(name);
            let pb = multi_progress.add(ProgressBar::new_spinner());
            pb.set_style(spinner_style.clone());

            // Set initial message with agent emoji (pad name to 6 chars for alignment)
            let prefix = format!("{} {:<6}", agent_style.emoji, name);
            pb.set_prefix(prefix);
            pb.set_message(format!("{} Waiting...", AgentStatus::Pending.emoji()));
            pb.enable_steady_tick(Duration::from_millis(100));

            bars.insert(name.to_string(), pb);
        }

        Self {
            multi_progress,
            bars,
        }
    }

    /// Update the status of an agent
    pub fn update_status(&self, agent_name: &str, status: &AgentStatus) {
        if let Some(pb) = self.bars.get(agent_name) {
            match status {
                AgentStatus::Pending => {
                    pb.set_message(format!("{} Waiting...", status.emoji()));
                }
                AgentStatus::Running => {
                    pb.set_message(format!("{} Running...", status.emoji()));
                }
                AgentStatus::Completed => {
                    pb.set_message(format!("{} Completed!", status.emoji()));
                    pb.finish();
                }
                AgentStatus::Failed => {
                    pb.set_message(format!("{} Failed", status.emoji()));
                    pb.finish();
                }
            }
        }
    }

    /// Update with a custom message
    pub fn update_message(&self, agent_name: &str, message: &str) {
        if let Some(pb) = self.bars.get(agent_name) {
            pb.set_message(format!("🔄 {message}"));
        }
    }

    /// Finish all progress bars
    pub fn finish_all(&self) {
        for pb in self.bars.values() {
            pb.finish();
        }
    }

    /// Get the multi-progress instance for spawning in background
    #[must_use]
    pub fn multi_progress(&self) -> &MultiProgress {
        &self.multi_progress
    }
}

/// Shared progress tracker that can be used across async tasks
pub type SharedProgressTracker = Arc<Mutex<ProgressTracker>>;

/// Create a shared progress tracker
#[must_use]
pub fn create_shared_tracker(agent_names: &[&str]) -> SharedProgressTracker {
    Arc::new(Mutex::new(ProgressTracker::new(agent_names)))
}

/// Display header with colorful styling
pub fn display_header(agent_names: &[&str]) {
    println!();
    println!("{}", style("━".repeat(50)).cyan());
    println!(
        "  {} {}",
        style("🚀").bold(),
        style("Running AI Agents in Parallel").bold().cyan()
    );
    println!("{}", style("━".repeat(50)).cyan());
    println!();

    for name in agent_names {
        let agent_style = AgentStyle::for_agent(name);
        println!(
            "  {} {} {}",
            agent_style.emoji,
            style(name).bold(),
            style("ready").dim()
        );
    }
    println!();
}

/// Display completion summary
pub fn display_completion_summary(completed: &[&str], failed: &[&str]) {
    println!();
    println!("{}", style("━".repeat(50)).cyan());

    if !completed.is_empty() {
        println!(
            "  {} {} agent(s) completed successfully",
            style("✅").green(),
            style(completed.len()).green().bold()
        );
        for name in completed {
            let agent_style = AgentStyle::for_agent(name);
            println!("     {} {}", agent_style.emoji, style(name).green());
        }
    }

    if !failed.is_empty() {
        println!(
            "  {} {} agent(s) failed",
            style("❌").red(),
            style(failed.len()).red().bold()
        );
        for name in failed {
            let agent_style = AgentStyle::for_agent(name);
            println!("     {} {}", agent_style.emoji, style(name).red());
        }
    }

    println!("{}", style("━".repeat(50)).cyan());
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_style_known_agents() {
        let claude = AgentStyle::for_agent("claude");
        assert_eq!(claude.emoji, "🤖");

        let gemini = AgentStyle::for_agent("gemini");
        assert_eq!(gemini.emoji, "💎");

        let codex = AgentStyle::for_agent("codex");
        assert_eq!(codex.emoji, "🧠");
    }

    #[test]
    fn test_agent_style_unknown_agent() {
        let unknown = AgentStyle::for_agent("unknown_agent");
        assert_eq!(unknown.emoji, "⚡");
    }

    #[test]
    fn test_agent_status_emoji() {
        assert_eq!(AgentStatus::Pending.emoji(), "⏳");
        assert_eq!(AgentStatus::Running.emoji(), "🔄");
        assert_eq!(AgentStatus::Completed.emoji(), "✅");
        assert_eq!(AgentStatus::Failed.emoji(), "❌");
    }
}
