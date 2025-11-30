use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Mutex};

use cursive::Cursive;
use cursive::event::Key;
use cursive::theme::{BaseColor, Color, Style};
use cursive::traits::*;
use cursive::utils::markup::StyledString;
use cursive::views::{Dialog, DummyView, LinearLayout, Panel, ScrollView, SelectView, TextView};

use crate::domain::ResultInfo;
use crate::error::{Error, Result};

/// Mode for the detail view
#[derive(Debug, Clone, Copy, PartialEq)]
enum ViewMode {
    Log,
    Diff,
}

/// Result from the split view selection
#[derive(Debug, Clone)]
pub enum SplitViewResult {
    Apply(usize),
    Cancel,
}

/// Which panel is focused
#[derive(Debug, Clone, Copy, PartialEq)]
enum FocusedPanel {
    Models,
    Details,
}

/// State shared between callbacks
struct ViewState {
    result_infos: Vec<ResultInfo>,
    current_mode: ViewMode,
    selected_index: usize,
    result: Option<SplitViewResult>,
    focused_panel: FocusedPanel,
}

/// Display results in a split view and allow user to select one
pub fn select_result_split_view(result_infos: &[ResultInfo]) -> Result<usize> {
    if result_infos.is_empty() {
        return Err(Error::NoExecutorsAvailable);
    }

    let state = Arc::new(Mutex::new(ViewState {
        result_infos: result_infos.to_vec(),
        current_mode: ViewMode::Log,
        selected_index: 0,
        result: None,
        focused_panel: FocusedPanel::Models,
    }));

    let mut siv = cursive::default();

    // Build the UI
    build_ui(&mut siv, Arc::clone(&state));

    // Set up global callbacks
    setup_callbacks(&mut siv, Arc::clone(&state));

    // Run the UI
    siv.run();

    // Get the result
    let state = state.lock().unwrap();
    match &state.result {
        Some(SplitViewResult::Apply(index)) => Ok(*index),
        Some(SplitViewResult::Cancel) | None => Err(Error::UserCancelled),
    }
}

fn build_ui(siv: &mut Cursive, state: Arc<Mutex<ViewState>>) {
    let state_guard = state.lock().unwrap();

    // Build left panel: model list with highlighting
    let mut select_view = SelectView::<usize>::new().h_align(cursive::align::HAlign::Left);
    for (i, info) in state_guard.result_infos.iter().enumerate() {
        let emoji = get_agent_emoji(&info.executor_name);
        let status = if info.success { "+" } else { "x" };
        let label = format!(
            "{} {} [{}] ({} files)",
            emoji, info.executor_name, status, info.files_changed
        );
        select_view.add_item(label, i);
    }

    let state_for_select = Arc::clone(&state);
    select_view.set_on_select(move |siv, &index| {
        let mut state = state_for_select.lock().unwrap();
        state.selected_index = index;
        drop(state);
        update_detail_panel(siv, Arc::clone(&state_for_select));
    });

    // Style for focused panel title (cyan color with arrow)
    let focused_style = Style::from(Color::Light(BaseColor::Cyan));
    let mut models_title = StyledString::new();
    models_title.append_styled("▶ Models", focused_style);

    let select_panel = Panel::new(
        select_view
            .with_name("model_list")
            .scrollable()
            .min_width(28),
    )
    .title(models_title)
    .with_name("models_panel");

    // Build right panel: detail view
    let detail_content = get_detail_content(&state_guard.result_infos[0], ViewMode::Log);

    let scroll_view = ScrollView::new(TextView::new(detail_content).with_name("detail_view"))
        .scroll_x(true)
        .with_name("detail_scroll");

    let detail_panel = Panel::new(scroll_view)
        .title("Log")
        .with_name("detail_panel")
        .full_screen();

    drop(state_guard);

    // Build the main layout
    let main_layout = LinearLayout::horizontal()
        .child(select_panel)
        .child(DummyView.fixed_width(1))
        .child(detail_panel);

    // Build the help bar
    let help_text = "[f] Focus  [l] Log  [d] Diff  [a] Apply  [q] Cancel";

    let root_layout = LinearLayout::vertical()
        .child(main_layout)
        .child(DummyView.fixed_height(1))
        .child(TextView::new(help_text).center());

    // Set up theme for better visibility
    siv.set_theme(create_theme());

    siv.add_layer(
        Dialog::around(root_layout)
            .title("Parari - Results")
            .full_screen(),
    );
}

fn create_theme() -> cursive::theme::Theme {
    use cursive::theme::{BaseColor, Color, PaletteColor, Theme};

    let mut theme = Theme::default();

    // Make selection more visible with cyan highlight
    theme.palette[PaletteColor::Highlight] = Color::Dark(BaseColor::Cyan);
    theme.palette[PaletteColor::HighlightInactive] = Color::Dark(BaseColor::Blue);
    theme.palette[PaletteColor::HighlightText] = Color::Dark(BaseColor::Black);

    theme
}

fn setup_callbacks(siv: &mut Cursive, state: Arc<Mutex<ViewState>>) {
    // 'f': toggle focus between panels
    // Note: Tab key is reserved by cursive for default focus navigation
    let state_for_f = Arc::clone(&state);
    siv.add_global_callback('f', move |siv| {
        {
            let mut state_guard = state_for_f.lock().unwrap();
            state_guard.focused_panel = match state_guard.focused_panel {
                FocusedPanel::Models => FocusedPanel::Details,
                FocusedPanel::Details => FocusedPanel::Models,
            };
        }
        update_panel_titles(siv, Arc::clone(&state_for_f));

        let focused = state_for_f.lock().unwrap().focused_panel;
        match focused {
            FocusedPanel::Models => {
                siv.focus_name("model_list").ok();
            }
            FocusedPanel::Details => {
                siv.focus_name("detail_scroll").ok();
            }
        }
    });

    // 'l': show log (output)
    let state_for_log = Arc::clone(&state);
    siv.add_global_callback('l', move |siv| {
        {
            let mut state = state_for_log.lock().unwrap();
            state.current_mode = ViewMode::Log;
        }
        update_detail_panel(siv, Arc::clone(&state_for_log));
        update_panel_titles(siv, Arc::clone(&state_for_log));
    });

    // 'd': show diff
    let state_for_diff = Arc::clone(&state);
    siv.add_global_callback('d', move |siv| {
        {
            let mut state = state_for_diff.lock().unwrap();
            state.current_mode = ViewMode::Diff;
        }
        update_detail_panel(siv, Arc::clone(&state_for_diff));
        update_panel_titles(siv, Arc::clone(&state_for_diff));
    });

    // 'a' or 'A': apply
    let state_for_apply = Arc::clone(&state);
    siv.add_global_callback('a', move |siv| {
        let mut state = state_for_apply.lock().unwrap();
        state.result = Some(SplitViewResult::Apply(state.selected_index));
        siv.quit();
    });

    let state_for_apply_upper = Arc::clone(&state);
    siv.add_global_callback('A', move |siv| {
        let mut state = state_for_apply_upper.lock().unwrap();
        state.result = Some(SplitViewResult::Apply(state.selected_index));
        siv.quit();
    });

    // Esc or 'q': cancel
    let state_for_cancel = Arc::clone(&state);
    siv.add_global_callback(Key::Esc, move |siv| {
        let mut state = state_for_cancel.lock().unwrap();
        state.result = Some(SplitViewResult::Cancel);
        siv.quit();
    });

    let state_for_quit = Arc::clone(&state);
    siv.add_global_callback('q', move |siv| {
        let mut state = state_for_quit.lock().unwrap();
        state.result = Some(SplitViewResult::Cancel);
        siv.quit();
    });

    // Vim-style navigation
    let state_for_j = Arc::clone(&state);
    siv.add_global_callback('j', move |siv| {
        let new_index = siv
            .call_on_name("model_list", |view: &mut SelectView<usize>| {
                let current = view.selected_id().unwrap_or(0);
                let count = view.len();
                if current + 1 < count {
                    view.set_selection(current + 1);
                    Some(current + 1)
                } else {
                    None
                }
            })
            .flatten();

        if let Some(index) = new_index {
            let mut state = state_for_j.lock().unwrap();
            state.selected_index = index;
            drop(state);
            update_detail_panel(siv, Arc::clone(&state_for_j));
        }
    });

    let state_for_k = Arc::clone(&state);
    siv.add_global_callback('k', move |siv| {
        let new_index = siv
            .call_on_name("model_list", |view: &mut SelectView<usize>| {
                let current = view.selected_id().unwrap_or(0);
                if current > 0 {
                    view.set_selection(current - 1);
                    Some(current - 1)
                } else {
                    None
                }
            })
            .flatten();

        if let Some(index) = new_index {
            let mut state = state_for_k.lock().unwrap();
            state.selected_index = index;
            drop(state);
            update_detail_panel(siv, Arc::clone(&state_for_k));
        }
    });
}

fn update_detail_panel(siv: &mut Cursive, state: Arc<Mutex<ViewState>>) {
    let state_guard = state.lock().unwrap();
    let info = &state_guard.result_infos[state_guard.selected_index];
    let content = get_detail_content(info, state_guard.current_mode);
    drop(state_guard);

    siv.call_on_name("detail_view", |view: &mut TextView| {
        view.set_content(content);
    });
}

fn update_panel_titles(siv: &mut Cursive, state: Arc<Mutex<ViewState>>) {
    let state_guard = state.lock().unwrap();
    let mode = state_guard.current_mode;
    let focused = state_guard.focused_panel;
    drop(state_guard);

    // Style for focused panel title (cyan color with arrow)
    let focused_style = Style::from(Color::Light(BaseColor::Cyan));

    // Update models panel title
    let models_title = match focused {
        FocusedPanel::Models => {
            let mut styled = StyledString::new();
            styled.append_styled("▶ Models", focused_style);
            styled
        }
        FocusedPanel::Details => StyledString::plain("Models"),
    };

    // Update detail panel title with mode and focus indicator
    let detail_title = match (focused, mode) {
        (FocusedPanel::Details, ViewMode::Log) => {
            let mut styled = StyledString::new();
            styled.append_styled("▶ Log", focused_style);
            styled
        }
        (FocusedPanel::Details, ViewMode::Diff) => {
            let mut styled = StyledString::new();
            styled.append_styled("▶ Diff", focused_style);
            styled
        }
        (FocusedPanel::Models, ViewMode::Log) => StyledString::plain("Log"),
        (FocusedPanel::Models, ViewMode::Diff) => StyledString::plain("Diff"),
    };

    // Type aliases for readability
    type ModelsPanel = Panel<
        cursive::views::ResizedView<
            cursive::views::ScrollView<cursive::views::NamedView<SelectView<usize>>>,
        >,
    >;
    type DetailPanel =
        Panel<cursive::views::NamedView<ScrollView<cursive::views::NamedView<TextView>>>>;

    siv.call_on_name("models_panel", |view: &mut ModelsPanel| {
        view.set_title(models_title);
    });

    siv.call_on_name("detail_panel", |view: &mut DetailPanel| {
        view.set_title(detail_title);
    });
}

fn get_detail_content(info: &ResultInfo, mode: ViewMode) -> StyledString {
    match mode {
        ViewMode::Log => StyledString::plain(get_log_content(info)),
        ViewMode::Diff => get_diff_content_styled(&info.worktree_path),
    }
}

/// Strip ANSI escape codes from a string
fn strip_ansi_codes(s: &str) -> String {
    // Regex pattern for ANSI escape codes: ESC [ ... m (and other sequences)
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // Check for escape sequence
            if chars.peek() == Some(&'[') {
                chars.next(); // consume '['
                // Skip until we hit a letter (the terminator)
                while let Some(&next) = chars.peek() {
                    chars.next();
                    if next.is_ascii_alphabetic() {
                        break;
                    }
                }
            } else if chars.peek() == Some(&']') {
                // OSC sequence (Operating System Command)
                chars.next(); // consume ']'
                // Skip until we hit BEL (\x07) or ST (ESC \)
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

fn get_log_content(info: &ResultInfo) -> String {
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

    // STDOUT (with ANSI codes stripped)
    content.push_str(&"-".repeat(50));
    content.push('\n');
    content.push_str("STDOUT:\n");
    content.push_str(&"-".repeat(50));
    content.push('\n');
    if info.stdout.is_empty() {
        content.push_str("(no output)\n");
    } else {
        let cleaned_stdout = strip_ansi_codes(&info.stdout);
        content.push_str(&cleaned_stdout);
        if !cleaned_stdout.ends_with('\n') {
            content.push('\n');
        }
    }
    content.push('\n');

    // STDERR (with ANSI codes stripped)
    if !info.stderr.is_empty() {
        content.push_str(&"-".repeat(50));
        content.push('\n');
        content.push_str("STDERR:\n");
        content.push_str(&"-".repeat(50));
        content.push('\n');
        let cleaned_stderr = strip_ansi_codes(&info.stderr);
        content.push_str(&cleaned_stderr);
        if !cleaned_stderr.ends_with('\n') {
            content.push('\n');
        }
    }

    content
}

/// Parse diff output and return a styled string with colors
fn get_diff_content_styled(worktree_path: &Path) -> StyledString {
    // Get diff from the worktree
    let diff_output = Command::new("git")
        .args(["diff", "HEAD"])
        .current_dir(worktree_path)
        .output();

    match diff_output {
        Ok(output) => {
            let diff_str = String::from_utf8_lossy(&output.stdout);
            if diff_str.is_empty() {
                // Try to get status for untracked files
                get_untracked_files_styled(worktree_path)
            } else {
                // Parse and colorize the diff
                colorize_diff(&diff_str)
            }
        }
        Err(e) => StyledString::plain(format!("Error getting diff: {}", e)),
    }
}

/// Colorize diff output with appropriate colors
fn colorize_diff(diff: &str) -> StyledString {
    let mut styled = StyledString::new();

    let green = Style::from(Color::Dark(BaseColor::Green));
    let red = Style::from(Color::Dark(BaseColor::Red));
    let cyan = Style::from(Color::Dark(BaseColor::Cyan));
    let yellow = Style::from(Color::Dark(BaseColor::Yellow));

    for line in diff.lines() {
        if line.starts_with("+++") || line.starts_with("---") {
            // File headers - yellow/bold
            styled.append_styled(line, yellow);
        } else if line.starts_with('+') {
            // Added lines - green
            styled.append_styled(line, green);
        } else if line.starts_with('-') {
            // Removed lines - red
            styled.append_styled(line, red);
        } else if line.starts_with("@@") {
            // Hunk headers - cyan
            styled.append_styled(line, cyan);
        } else if line.starts_with("diff ") {
            // Diff command line - cyan
            styled.append_styled(line, cyan);
        } else if line.starts_with("index ") {
            // Index line - cyan
            styled.append_styled(line, cyan);
        } else {
            // Context lines - default color
            styled.append_plain(line);
        }
        styled.append_plain("\n");
    }

    styled
}

/// Get styled content for untracked files
fn get_untracked_files_styled(worktree_path: &Path) -> StyledString {
    let status_output = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(worktree_path)
        .output();

    match status_output {
        Ok(status) => {
            let status_str = String::from_utf8_lossy(&status.stdout);
            if status_str.is_empty() {
                StyledString::plain("No changes detected.")
            } else {
                let mut styled = StyledString::new();
                let green = Style::from(Color::Dark(BaseColor::Green));
                let cyan = Style::from(Color::Dark(BaseColor::Cyan));

                styled.append_styled("New/Untracked files:\n\n", cyan);

                for line in status_str.lines() {
                    if line.starts_with("??") || line.starts_with("A ") {
                        let file = line.split_whitespace().last().unwrap_or("");
                        let file_path = worktree_path.join(file);

                        if file_path.exists() && file_path.is_file() {
                            styled.append_styled(format!("+ {}\n", file), green);
                            styled.append_plain("-".repeat(40));
                            styled.append_plain("\n");

                            if let Ok(file_content) = std::fs::read_to_string(&file_path) {
                                for (i, file_line) in file_content.lines().enumerate().take(100) {
                                    styled.append_plain(format!("{:4} | ", i + 1));
                                    styled.append_styled(format!("+{}\n", file_line), green);
                                }
                                if file_content.lines().count() > 100 {
                                    styled.append_plain("... (truncated)\n");
                                }
                            }
                            styled.append_plain("\n");
                        }
                    }
                }
                styled
            }
        }
        Err(e) => StyledString::plain(format!("Error getting status: {}", e)),
    }
}

fn get_agent_emoji(name: &str) -> &'static str {
    match name.to_lowercase().as_str() {
        "claude" => "\u{1F916}", // Robot
        "gemini" => "\u{2728}",  // Sparkles
        "codex" => "\u{1F4E6}",  // Package
        _ => "\u{1F4BB}",        // Computer
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_ansi_codes() {
        // Test basic ANSI codes
        assert_eq!(strip_ansi_codes("\x1b[31mred\x1b[0m"), "red");
        assert_eq!(
            strip_ansi_codes("\x1b[1;32mbold green\x1b[0m"),
            "bold green"
        );
        // Test no ANSI codes
        assert_eq!(strip_ansi_codes("plain text"), "plain text");
        // Test empty string
        assert_eq!(strip_ansi_codes(""), "");
    }

    #[test]
    fn test_get_agent_emoji() {
        assert_eq!(get_agent_emoji("claude"), "\u{1F916}");
        assert_eq!(get_agent_emoji("Claude"), "\u{1F916}");
        assert_eq!(get_agent_emoji("gemini"), "\u{2728}");
        assert_eq!(get_agent_emoji("codex"), "\u{1F4E6}");
        assert_eq!(get_agent_emoji("unknown"), "\u{1F4BB}");
    }
}
