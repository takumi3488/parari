use std::path::Path;
use std::process::Command;
use std::time::Duration;

use ratatui::Frame;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::layout::{Constraint, Layout, Position};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, List, ListItem, ListState, Paragraph, Wrap};

use crate::domain::ResultInfo;
use crate::error::{Error, Result};
use crate::executor::OutputLine;

/// Mode for the detail view
#[derive(Debug, Clone, Copy, PartialEq)]
enum ViewMode {
    Log,
    Diff,
}

/// Which panel is focused
#[derive(Debug, Clone, Copy, PartialEq)]
enum FocusedPanel {
    Models,
    Details,
}

/// Input mode for the application
#[derive(Debug, Clone, Copy, PartialEq)]
enum InputMode {
    Normal,
    Search,
    Confirm,
    ConfirmCancel,
}

/// Result from the split view selection
#[derive(Debug, Clone)]
pub enum SplitViewResult {
    Apply(usize),
    Cancel,
}

/// Application state
struct App {
    result_infos: Vec<ResultInfo>,
    list_state: ListState,
    current_mode: ViewMode,
    focused_panel: FocusedPanel,
    input_mode: InputMode,
    scroll_offset: u16,
    content_height: u16,
    search_query: String,
    search_matches: Vec<u16>,
    search_match_index: usize,
    result: Option<SplitViewResult>,
}

impl App {
    fn new(result_infos: Vec<ResultInfo>) -> Self {
        let mut list_state = ListState::default();
        if !result_infos.is_empty() {
            list_state.select(Some(0));
        }

        Self {
            result_infos,
            list_state,
            current_mode: ViewMode::Log,
            focused_panel: FocusedPanel::Models,
            input_mode: InputMode::Normal,
            scroll_offset: 0,
            content_height: 0,
            search_query: String::new(),
            search_matches: Vec::new(),
            search_match_index: 0,
            result: None,
        }
    }

    fn selected_index(&self) -> usize {
        self.list_state.selected().unwrap_or(0)
    }

    fn selected_info(&self) -> Option<&ResultInfo> {
        self.result_infos.get(self.selected_index())
    }

    fn next_model(&mut self) {
        if self.result_infos.is_empty() {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => {
                if i + 1 < self.result_infos.len() {
                    i + 1
                } else {
                    i
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
        self.scroll_offset = 0;
        self.clear_search();
    }

    fn previous_model(&mut self) {
        let i = match self.list_state.selected() {
            Some(i) => {
                if i > 0 {
                    i - 1
                } else {
                    0
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
        self.scroll_offset = 0;
        self.clear_search();
    }

    fn scroll_down(&mut self, lines: u16) {
        self.scroll_offset = self.scroll_offset.saturating_add(lines);
    }

    fn scroll_up(&mut self, lines: u16) {
        self.scroll_offset = self.scroll_offset.saturating_sub(lines);
    }

    fn scroll_to_top(&mut self) {
        self.scroll_offset = 0;
    }

    fn scroll_to_bottom(&mut self) {
        if self.content_height > 0 {
            self.scroll_offset = self.content_height.saturating_sub(1);
        }
    }

    fn half_page_down(&mut self, viewport_height: u16) {
        self.scroll_down(viewport_height / 2);
    }

    fn half_page_up(&mut self, viewport_height: u16) {
        self.scroll_up(viewport_height / 2);
    }

    fn set_mode(&mut self, mode: ViewMode) {
        if self.current_mode != mode {
            self.current_mode = mode;
            self.scroll_offset = 0;
            self.clear_search();
        }
    }

    fn toggle_focus(&mut self) {
        self.focused_panel = match self.focused_panel {
            FocusedPanel::Models => FocusedPanel::Details,
            FocusedPanel::Details => FocusedPanel::Models,
        };
    }

    fn start_search(&mut self) {
        self.input_mode = InputMode::Search;
        self.search_query.clear();
        self.search_matches.clear();
        self.search_match_index = 0;
    }

    fn cancel_search(&mut self) {
        self.input_mode = InputMode::Normal;
    }

    fn clear_search(&mut self) {
        self.search_query.clear();
        self.search_matches.clear();
        self.search_match_index = 0;
    }

    fn execute_search(&mut self, content: &str) {
        self.input_mode = InputMode::Normal;
        if self.search_query.is_empty() {
            return;
        }

        self.search_matches.clear();
        let query_lower = self.search_query.to_lowercase();

        for (line_num, line) in content.lines().enumerate() {
            if line.to_lowercase().contains(&query_lower) {
                self.search_matches.push(line_num as u16);
            }
        }

        if !self.search_matches.is_empty() {
            self.search_match_index = 0;
            self.scroll_offset = self.search_matches[0];
        }
    }

    fn next_search_match(&mut self) {
        if self.search_matches.is_empty() {
            return;
        }
        self.search_match_index = (self.search_match_index + 1) % self.search_matches.len();
        self.scroll_offset = self.search_matches[self.search_match_index];
    }

    fn previous_search_match(&mut self) {
        if self.search_matches.is_empty() {
            return;
        }
        if self.search_match_index == 0 {
            self.search_match_index = self.search_matches.len() - 1;
        } else {
            self.search_match_index -= 1;
        }
        self.scroll_offset = self.search_matches[self.search_match_index];
    }

    fn apply(&mut self) {
        self.result = Some(SplitViewResult::Apply(self.selected_index()));
    }

    fn start_confirm(&mut self) {
        self.input_mode = InputMode::Confirm;
    }

    fn cancel_confirm(&mut self) {
        self.input_mode = InputMode::Normal;
    }

    fn start_confirm_cancel(&mut self) {
        self.input_mode = InputMode::ConfirmCancel;
    }

    fn cancel(&mut self) {
        self.result = Some(SplitViewResult::Cancel);
    }

    fn handle_event(&mut self, event: Event, viewport_height: u16, content: &str) -> bool {
        if let Event::Key(key) = event {
            if key.kind != KeyEventKind::Press {
                return false;
            }

            match self.input_mode {
                InputMode::Confirm => {
                    match key.code {
                        KeyCode::Char('y') | KeyCode::Char('Y') => {
                            self.apply();
                            return true;
                        }
                        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                            self.cancel_confirm();
                        }
                        _ => {}
                    }
                    return false;
                }
                InputMode::ConfirmCancel => {
                    match key.code {
                        KeyCode::Char('y') | KeyCode::Char('Y') => {
                            self.cancel();
                            return true;
                        }
                        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                            self.cancel_confirm();
                        }
                        _ => {}
                    }
                    return false;
                }
                InputMode::Search => {
                    match key.code {
                        KeyCode::Enter => {
                            self.execute_search(content);
                        }
                        KeyCode::Esc => {
                            self.cancel_search();
                        }
                        KeyCode::Backspace => {
                            self.search_query.pop();
                        }
                        KeyCode::Char(c) => {
                            self.search_query.push(c);
                        }
                        _ => {}
                    }
                    return false;
                }
                InputMode::Normal => {
                    match self.focused_panel {
                        FocusedPanel::Models => {
                            match key.code {
                                // Model navigation
                                KeyCode::Char('j') | KeyCode::Down => self.next_model(),
                                KeyCode::Char('k') | KeyCode::Up => self.previous_model(),

                                // Focus switch
                                KeyCode::Tab | KeyCode::Char('l') | KeyCode::Right => {
                                    self.toggle_focus()
                                }

                                // Mode switching
                                KeyCode::Char('L') => self.set_mode(ViewMode::Log),
                                KeyCode::Char('D') => self.set_mode(ViewMode::Diff),

                                // Actions
                                KeyCode::Char('a') | KeyCode::Enter => {
                                    self.start_confirm();
                                }
                                KeyCode::Char('q') | KeyCode::Esc => {
                                    self.start_confirm_cancel();
                                }

                                _ => {}
                            }
                        }
                        FocusedPanel::Details => {
                            match key.code {
                                // Vim-style scrolling
                                KeyCode::Char('j') | KeyCode::Down => self.scroll_down(1),
                                KeyCode::Char('k') | KeyCode::Up => self.scroll_up(1),
                                KeyCode::Char('d')
                                    if key.modifiers.contains(event::KeyModifiers::CONTROL) =>
                                {
                                    self.half_page_down(viewport_height)
                                }
                                KeyCode::Char('u')
                                    if key.modifiers.contains(event::KeyModifiers::CONTROL) =>
                                {
                                    self.half_page_up(viewport_height)
                                }
                                KeyCode::Char('f')
                                    if key.modifiers.contains(event::KeyModifiers::CONTROL) =>
                                {
                                    self.scroll_down(viewport_height)
                                }
                                KeyCode::Char('b')
                                    if key.modifiers.contains(event::KeyModifiers::CONTROL) =>
                                {
                                    self.scroll_up(viewport_height)
                                }
                                KeyCode::Char('g') => self.scroll_to_top(),
                                KeyCode::Char('G') => self.scroll_to_bottom(),
                                KeyCode::Home => self.scroll_to_top(),
                                KeyCode::End => self.scroll_to_bottom(),
                                KeyCode::PageDown => self.scroll_down(viewport_height),
                                KeyCode::PageUp => self.scroll_up(viewport_height),

                                // Search
                                KeyCode::Char('/') => self.start_search(),
                                KeyCode::Char('n') => self.next_search_match(),
                                KeyCode::Char('N') => self.previous_search_match(),

                                // Focus switch
                                KeyCode::Tab | KeyCode::Char('h') | KeyCode::Left => {
                                    self.toggle_focus()
                                }

                                // Mode switching (lowercase and uppercase)
                                KeyCode::Char('l') | KeyCode::Char('L') => {
                                    self.set_mode(ViewMode::Log)
                                }
                                KeyCode::Char('d')
                                    if !key.modifiers.contains(event::KeyModifiers::CONTROL) =>
                                {
                                    self.set_mode(ViewMode::Diff)
                                }
                                KeyCode::Char('D') => self.set_mode(ViewMode::Diff),

                                // Actions (also available in detail view)
                                KeyCode::Char('a') => {
                                    self.start_confirm();
                                }
                                KeyCode::Char('q') | KeyCode::Esc => {
                                    self.start_confirm_cancel();
                                }

                                _ => {}
                            }
                        }
                    }
                }
            }
        }
        false
    }
}

/// Display results in a split view and allow user to select one
pub fn select_result_split_view(result_infos: &[ResultInfo]) -> Result<usize> {
    if result_infos.is_empty() {
        return Err(Error::NoExecutorsAvailable);
    }

    let mut terminal = ratatui::init();
    let mut app = App::new(result_infos.to_vec());
    let mut cached_content = String::new();
    let mut last_selected = 0usize;
    let mut last_mode = app.current_mode;

    loop {
        // Update content cache if selection or mode changed
        if app.selected_index() != last_selected || app.current_mode != last_mode {
            if let Some(info) = app.selected_info() {
                cached_content = match app.current_mode {
                    ViewMode::Log => get_log_content_string(info),
                    ViewMode::Diff => get_diff_content_string(&info.worktree_path),
                };
                app.content_height = cached_content.lines().count() as u16;
            }
            last_selected = app.selected_index();
            last_mode = app.current_mode;
        }

        let viewport_height = terminal
            .size()
            .map(|s| s.height.saturating_sub(4))
            .unwrap_or(20);

        terminal
            .draw(|frame| render(frame, &mut app, &cached_content))
            .map_err(|e| Error::Io(std::io::Error::other(e.to_string())))?;

        if event::poll(Duration::from_millis(100))
            .map_err(|e| Error::Io(std::io::Error::other(e.to_string())))?
        {
            let event =
                event::read().map_err(|e| Error::Io(std::io::Error::other(e.to_string())))?;
            if app.handle_event(event, viewport_height, &cached_content) {
                break;
            }
        }
    }

    ratatui::restore();

    match app.result {
        Some(SplitViewResult::Apply(index)) => Ok(index),
        Some(SplitViewResult::Cancel) | None => Err(Error::UserCancelled),
    }
}

fn render(frame: &mut Frame, app: &mut App, content: &str) {
    // Main layout: body + search bar (if searching) + footer
    let layout = if app.input_mode == InputMode::Search {
        Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(frame.area())
    } else {
        Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).split(frame.area())
    };

    let body = layout[0];
    let footer_idx = layout.len() - 1;

    // Body layout: left panel (models) + right panel (details)
    let [left_panel, right_panel] =
        Layout::horizontal([Constraint::Length(32), Constraint::Fill(1)]).areas(body);

    // Render model list
    render_model_list(frame, app, left_panel);

    // Render detail panel
    render_detail_panel(frame, app, right_panel, content);

    // Render search bar if in search mode
    if app.input_mode == InputMode::Search {
        render_search_bar(frame, app, layout[1]);
    }

    // Render help footer
    render_footer(frame, app, layout[footer_idx]);
}

fn render_model_list(frame: &mut Frame, app: &mut App, area: ratatui::layout::Rect) {
    let items: Vec<ListItem> = app
        .result_infos
        .iter()
        .map(|info| {
            let emoji = get_agent_emoji(&info.executor_name);
            let status = if info.success { "+" } else { "x" };
            let label = format!(
                "{} {} [{}] ({} files)",
                emoji, info.executor_name, status, info.files_changed
            );
            ListItem::new(label)
        })
        .collect();

    let is_focused = app.focused_panel == FocusedPanel::Models;
    let border_style = if is_focused {
        Style::new().fg(Color::Cyan)
    } else {
        Style::new().fg(Color::DarkGray)
    };

    let title = if is_focused {
        "▶ Models "
    } else {
        " Models "
    };

    let list = List::new(items)
        .block(Block::bordered().title(title).border_style(border_style))
        .highlight_style(
            Style::new()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    frame.render_stateful_widget(list, area, &mut app.list_state);
}

fn render_detail_panel(frame: &mut Frame, app: &App, area: ratatui::layout::Rect, content: &str) {
    let mode_name = match app.current_mode {
        ViewMode::Log => "Log",
        ViewMode::Diff => "Diff",
    };

    let is_focused = app.focused_panel == FocusedPanel::Details;
    let border_style = if is_focused {
        Style::new().fg(Color::Cyan)
    } else {
        Style::new().fg(Color::DarkGray)
    };

    let title = if is_focused {
        format!("▶ {} ", mode_name)
    } else {
        format!(" {} ", mode_name)
    };

    // Build styled content with search highlighting
    let text = if app.search_query.is_empty() {
        get_styled_content(content, app.current_mode)
    } else {
        get_styled_content_with_search(content, app.current_mode, &app.search_query)
    };

    // Show search match count if searching
    let title_with_search = if !app.search_matches.is_empty() {
        format!(
            "{} [{}/{}]",
            title,
            app.search_match_index + 1,
            app.search_matches.len()
        )
    } else if !app.search_query.is_empty() {
        format!("{} [no matches]", title)
    } else {
        title
    };

    let paragraph = Paragraph::new(text)
        .block(
            Block::bordered()
                .title(title_with_search)
                .border_style(border_style),
        )
        .wrap(Wrap { trim: false })
        .scroll((app.scroll_offset, 0));

    frame.render_widget(paragraph, area);
}

fn render_search_bar(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let search_line = Line::from(vec![
        Span::styled("/", Style::new().fg(Color::Yellow)),
        Span::raw(&app.search_query),
        Span::styled("_", Style::new().add_modifier(Modifier::SLOW_BLINK)),
    ]);

    let search_bar = Paragraph::new(search_line);
    frame.render_widget(search_bar, area);

    // Set cursor position
    frame.set_cursor_position(Position::new(
        area.x + 1 + app.search_query.len() as u16,
        area.y,
    ));
}

fn render_footer(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let help_spans = match app.input_mode {
        InputMode::Confirm => {
            let name = app
                .selected_info()
                .map(|info| info.executor_name.as_str())
                .unwrap_or("unknown");
            vec![
                Span::styled(
                    format!(" Apply changes from {}? ", name),
                    Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                ),
                Span::styled(" y ", Style::new().fg(Color::Black).bg(Color::Green)),
                Span::raw(" Yes  "),
                Span::styled(" n/Esc ", Style::new().fg(Color::Black).bg(Color::Red)),
                Span::raw(" No"),
            ]
        }
        InputMode::ConfirmCancel => {
            vec![
                Span::styled(
                    " Quit without applying changes? ",
                    Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                ),
                Span::styled(" y ", Style::new().fg(Color::Black).bg(Color::Green)),
                Span::raw(" Yes  "),
                Span::styled(" n/Esc ", Style::new().fg(Color::Black).bg(Color::Red)),
                Span::raw(" No"),
            ]
        }
        InputMode::Search => vec![
            Span::styled(" Search: ", Style::new().fg(Color::Yellow)),
            Span::raw(&app.search_query),
            Span::styled(" Enter ", Style::new().fg(Color::Black).bg(Color::Cyan)),
            Span::raw(" Execute  "),
            Span::styled(" Esc ", Style::new().fg(Color::Black).bg(Color::Cyan)),
            Span::raw(" Cancel"),
        ],
        InputMode::Normal => match app.focused_panel {
            FocusedPanel::Models => vec![
                Span::styled(" j/k ", Style::new().fg(Color::Black).bg(Color::Cyan)),
                Span::raw(" Select  "),
                Span::styled(" Tab/l ", Style::new().fg(Color::Black).bg(Color::Cyan)),
                Span::raw(" Details  "),
                Span::styled(" L ", Style::new().fg(Color::Black).bg(Color::Cyan)),
                Span::raw(" Log  "),
                Span::styled(" D ", Style::new().fg(Color::Black).bg(Color::Cyan)),
                Span::raw(" Diff  "),
                Span::styled(" a/Enter ", Style::new().fg(Color::Black).bg(Color::Cyan)),
                Span::raw(" Apply  "),
                Span::styled(" q ", Style::new().fg(Color::Black).bg(Color::Cyan)),
                Span::raw(" Quit"),
            ],
            FocusedPanel::Details => vec![
                Span::styled(" j/k ", Style::new().fg(Color::Black).bg(Color::Cyan)),
                Span::raw(" Scroll  "),
                Span::styled(" Tab/h ", Style::new().fg(Color::Black).bg(Color::Cyan)),
                Span::raw(" Models  "),
                Span::styled(" / ", Style::new().fg(Color::Black).bg(Color::Cyan)),
                Span::raw(" Search  "),
                Span::styled(" n/N ", Style::new().fg(Color::Black).bg(Color::Cyan)),
                Span::raw(" Next/Prev  "),
                Span::styled(" l ", Style::new().fg(Color::Black).bg(Color::Cyan)),
                Span::raw(" Log  "),
                Span::styled(" d ", Style::new().fg(Color::Black).bg(Color::Cyan)),
                Span::raw(" Diff  "),
                Span::styled(" a ", Style::new().fg(Color::Black).bg(Color::Cyan)),
                Span::raw(" Apply  "),
                Span::styled(" q ", Style::new().fg(Color::Black).bg(Color::Cyan)),
                Span::raw(" Quit"),
            ],
        },
    };

    let help_line = Line::from(help_spans);
    let help = Paragraph::new(help_line);

    frame.render_widget(help, area);
}

/// Strip ANSI escape codes from a string
fn strip_ansi_codes(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\x1b' {
            if chars.peek() == Some(&'[') {
                chars.next();
                while let Some(&next) = chars.peek() {
                    chars.next();
                    if next.is_ascii_alphabetic() {
                        break;
                    }
                }
            } else if chars.peek() == Some(&']') {
                chars.next();
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

/// Special marker for stderr lines (invisible character used for detection in style_log_line)
const STDERR_MARKER: &str = "\x01STDERR\x02";

fn get_log_content_string(info: &ResultInfo) -> String {
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

    // Output (stdout and stderr interleaved in order of arrival)
    content.push_str(&"-".repeat(50));
    content.push('\n');
    content.push_str("Output:\n");
    content.push_str(&"-".repeat(50));
    content.push('\n');

    if info.output_lines.is_empty() {
        content.push_str("(no output)\n");
    } else {
        for output_line in &info.output_lines {
            match output_line {
                OutputLine::Stdout(line) => {
                    let cleaned = strip_ansi_codes(line);
                    content.push_str(&cleaned);
                    content.push('\n');
                }
                OutputLine::Stderr(line) => {
                    // Add marker for stderr lines so style_log_line can detect them
                    let cleaned = strip_ansi_codes(line);
                    content.push_str(STDERR_MARKER);
                    content.push_str(&cleaned);
                    content.push('\n');
                }
            }
        }
    }

    content
}

fn get_diff_content_string(worktree_path: &Path) -> String {
    let diff_output = Command::new("git")
        .args(["diff", "HEAD"])
        .current_dir(worktree_path)
        .output();

    match diff_output {
        Ok(output) => {
            let diff_str = String::from_utf8_lossy(&output.stdout);
            if diff_str.is_empty() {
                get_untracked_files_string(worktree_path)
            } else {
                diff_str.to_string()
            }
        }
        Err(e) => format!("Error getting diff: {}", e),
    }
}

fn get_untracked_files_string(worktree_path: &Path) -> String {
    let status_output = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(worktree_path)
        .output();

    match status_output {
        Ok(status) => {
            let status_str = String::from_utf8_lossy(&status.stdout);
            if status_str.is_empty() {
                "No changes detected.".to_string()
            } else {
                let mut content = String::new();
                content.push_str("New/Untracked files:\n\n");

                for line in status_str.lines() {
                    if line.starts_with("??") || line.starts_with("A ") {
                        let file = line.split_whitespace().last().unwrap_or("");
                        let file_path = worktree_path.join(file);

                        if file_path.exists() && file_path.is_file() {
                            content.push_str(&format!("+ {}\n", file));
                            content.push_str(&"-".repeat(40));
                            content.push('\n');

                            if let Ok(file_content) = std::fs::read_to_string(&file_path) {
                                for (i, file_line) in file_content.lines().enumerate().take(100) {
                                    content.push_str(&format!("{:4} | +{}\n", i + 1, file_line));
                                }
                                if file_content.lines().count() > 100 {
                                    content.push_str("... (truncated)\n");
                                }
                            }
                            content.push('\n');
                        }
                    }
                }
                content
            }
        }
        Err(e) => format!("Error getting status: {}", e),
    }
}

fn get_styled_content(content: &str, mode: ViewMode) -> Text<'static> {
    let mut lines = Vec::new();

    for line in content.lines() {
        let styled_line = match mode {
            ViewMode::Log => style_log_line(line),
            ViewMode::Diff => style_diff_line(line),
        };
        lines.push(styled_line);
    }

    Text::from(lines)
}

fn get_styled_content_with_search(content: &str, mode: ViewMode, query: &str) -> Text<'static> {
    let mut lines = Vec::new();
    let query_lower = query.to_lowercase();

    for line in content.lines() {
        // Handle stderr marker
        let (actual_line, is_stderr) = if let Some(stripped) = line.strip_prefix(STDERR_MARKER) {
            (stripped, true)
        } else {
            (line, false)
        };

        let line_lower = actual_line.to_lowercase();
        if line_lower.contains(&query_lower) {
            // Highlight search matches
            let mut spans = Vec::new();
            let mut last_end = 0;

            // Base style for stderr lines
            let base_style = if is_stderr {
                Style::new().fg(Color::Red)
            } else {
                Style::new()
            };

            for (start, _) in line_lower.match_indices(&query_lower) {
                if start > last_end {
                    spans.push(Span::styled(
                        actual_line[last_end..start].to_string(),
                        base_style,
                    ));
                }
                spans.push(Span::styled(
                    actual_line[start..start + query.len()].to_string(),
                    Style::new()
                        .fg(Color::Black)
                        .bg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ));
                last_end = start + query.len();
            }

            if last_end < actual_line.len() {
                spans.push(Span::styled(
                    actual_line[last_end..].to_string(),
                    base_style,
                ));
            }

            lines.push(Line::from(spans));
        } else {
            let styled_line = match mode {
                ViewMode::Log => style_log_line(line),
                ViewMode::Diff => style_diff_line(line),
            };
            lines.push(styled_line);
        }
    }

    Text::from(lines)
}

fn style_log_line(line: &str) -> Line<'static> {
    // Check for stderr marker first - display in red and remove the marker
    if let Some(content) = line.strip_prefix(STDERR_MARKER) {
        return Line::styled(content.to_string(), Style::new().fg(Color::Red));
    }

    if line.starts_with("Output:") || line.starts_with("Summary:") {
        Line::styled(line.to_string(), Style::new().add_modifier(Modifier::BOLD))
    } else if line.starts_with("  +") {
        Line::styled(line.to_string(), Style::new().fg(Color::Green))
    } else if line.starts_with("  ~") {
        Line::styled(line.to_string(), Style::new().fg(Color::Yellow))
    } else if line.starts_with("  -") {
        Line::styled(line.to_string(), Style::new().fg(Color::Red))
    } else if line.starts_with('=') || line.starts_with('-') {
        Line::styled(line.to_string(), Style::new().fg(Color::DarkGray))
    } else if line.contains("Success") {
        Line::styled(
            line.to_string(),
            Style::new().fg(Color::Green).add_modifier(Modifier::BOLD),
        )
    } else if line.contains("Failed") {
        Line::styled(
            line.to_string(),
            Style::new().fg(Color::Red).add_modifier(Modifier::BOLD),
        )
    } else if line == "(no output)" {
        Line::styled(line.to_string(), Style::new().fg(Color::DarkGray))
    } else {
        Line::raw(line.to_string())
    }
}

fn style_diff_line(line: &str) -> Line<'static> {
    if line.starts_with("+++") || line.starts_with("---") {
        Line::styled(line.to_string(), Style::new().fg(Color::Yellow))
    } else if line.starts_with('+') {
        Line::styled(line.to_string(), Style::new().fg(Color::Green))
    } else if line.starts_with('-') {
        Line::styled(line.to_string(), Style::new().fg(Color::Red))
    } else if line.starts_with("@@") || line.starts_with("diff ") || line.starts_with("index ") {
        Line::styled(line.to_string(), Style::new().fg(Color::Cyan))
    } else if line.starts_with("New/Untracked") {
        Line::styled(
            line.to_string(),
            Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )
    } else {
        Line::raw(line.to_string())
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
        assert_eq!(strip_ansi_codes("\x1b[31mred\x1b[0m"), "red");
        assert_eq!(
            strip_ansi_codes("\x1b[1;32mbold green\x1b[0m"),
            "bold green"
        );
        assert_eq!(strip_ansi_codes("plain text"), "plain text");
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

    #[test]
    fn test_app_navigation() {
        use crate::git::ChangeSummary;
        use std::path::PathBuf;

        let result_infos = vec![
            ResultInfo {
                executor_name: "claude".to_string(),
                success: true,
                stdout: "output".to_string(),
                stderr: "".to_string(),
                output_lines: vec![OutputLine::Stdout("output".to_string())],
                files_changed: 1,
                worktree_path: PathBuf::from("/tmp/test1"),
                change_summary: Some(ChangeSummary {
                    files_added: 1,
                    files_modified: 0,
                    files_deleted: 0,
                    changed_files: vec!["test.rs".to_string()],
                }),
            },
            ResultInfo {
                executor_name: "gemini".to_string(),
                success: true,
                stdout: "output".to_string(),
                stderr: "".to_string(),
                output_lines: vec![OutputLine::Stdout("output".to_string())],
                files_changed: 2,
                worktree_path: PathBuf::from("/tmp/test2"),
                change_summary: None,
            },
        ];

        let mut app = App::new(result_infos);
        assert_eq!(app.selected_index(), 0);

        app.next_model();
        assert_eq!(app.selected_index(), 1);

        app.next_model();
        assert_eq!(app.selected_index(), 1); // Should stay at last

        app.previous_model();
        assert_eq!(app.selected_index(), 0);

        app.previous_model();
        assert_eq!(app.selected_index(), 0); // Should stay at first
    }

    #[test]
    fn test_app_mode_switching() {
        let mut app = App::new(vec![]);
        assert_eq!(app.current_mode, ViewMode::Log);

        app.set_mode(ViewMode::Diff);
        assert_eq!(app.current_mode, ViewMode::Diff);

        app.set_mode(ViewMode::Log);
        assert_eq!(app.current_mode, ViewMode::Log);
    }

    #[test]
    fn test_app_scrolling() {
        let mut app = App::new(vec![]);
        assert_eq!(app.scroll_offset, 0);

        app.scroll_down(1);
        assert_eq!(app.scroll_offset, 1);

        app.scroll_up(1);
        assert_eq!(app.scroll_offset, 0);

        app.scroll_up(1);
        assert_eq!(app.scroll_offset, 0); // Should not go negative

        app.half_page_down(40);
        assert_eq!(app.scroll_offset, 20);

        app.half_page_up(40);
        assert_eq!(app.scroll_offset, 0);
    }

    #[test]
    fn test_app_focus_toggle() {
        let mut app = App::new(vec![]);
        assert_eq!(app.focused_panel, FocusedPanel::Models);

        app.toggle_focus();
        assert_eq!(app.focused_panel, FocusedPanel::Details);

        app.toggle_focus();
        assert_eq!(app.focused_panel, FocusedPanel::Models);
    }

    #[test]
    fn test_app_search() {
        let mut app = App::new(vec![]);
        let content = "line one\nline two\nline three\nline one again";

        app.search_query = "one".to_string();
        app.execute_search(content);

        assert_eq!(app.search_matches.len(), 2);
        assert_eq!(app.search_matches[0], 0);
        assert_eq!(app.search_matches[1], 3);
        assert_eq!(app.scroll_offset, 0);

        app.next_search_match();
        assert_eq!(app.search_match_index, 1);
        assert_eq!(app.scroll_offset, 3);

        app.next_search_match();
        assert_eq!(app.search_match_index, 0); // Wrap around
        assert_eq!(app.scroll_offset, 0);

        app.previous_search_match();
        assert_eq!(app.search_match_index, 1); // Wrap around backwards
        assert_eq!(app.scroll_offset, 3);
    }

    mod snapshot_tests {
        use super::*;
        use insta::assert_snapshot;
        use ratatui::{Terminal, backend::TestBackend};
        use std::path::PathBuf;

        /// Create test result infos for snapshot tests
        fn create_test_result_infos() -> Vec<ResultInfo> {
            use crate::git::ChangeSummary;

            vec![
                ResultInfo {
                    executor_name: "claude".to_string(),
                    success: true,
                    stdout: "Analyzing the code...\nMade changes to src/main.rs".to_string(),
                    stderr: "".to_string(),
                    output_lines: vec![
                        OutputLine::Stdout("Analyzing the code...".to_string()),
                        OutputLine::Stdout("Made changes to src/main.rs".to_string()),
                    ],
                    files_changed: 2,
                    worktree_path: PathBuf::from("/tmp/worktree-claude"),
                    change_summary: Some(ChangeSummary {
                        files_added: 1,
                        files_modified: 1,
                        files_deleted: 0,
                        changed_files: vec!["src/main.rs".to_string(), "src/lib.rs".to_string()],
                    }),
                },
                ResultInfo {
                    executor_name: "gemini".to_string(),
                    success: true,
                    stdout: "Processing request...\nUpdated 3 files".to_string(),
                    stderr: "".to_string(),
                    output_lines: vec![
                        OutputLine::Stdout("Processing request...".to_string()),
                        OutputLine::Stdout("Updated 3 files".to_string()),
                    ],
                    files_changed: 3,
                    worktree_path: PathBuf::from("/tmp/worktree-gemini"),
                    change_summary: Some(ChangeSummary {
                        files_added: 2,
                        files_modified: 1,
                        files_deleted: 0,
                        changed_files: vec![
                            "src/main.rs".to_string(),
                            "src/utils.rs".to_string(),
                            "tests/test.rs".to_string(),
                        ],
                    }),
                },
                ResultInfo {
                    executor_name: "codex".to_string(),
                    success: false,
                    stdout: "Starting task...".to_string(),
                    stderr: "Error: Something went wrong".to_string(),
                    output_lines: vec![
                        OutputLine::Stdout("Starting task...".to_string()),
                        OutputLine::Stderr("Error: Something went wrong".to_string()),
                    ],
                    files_changed: 0,
                    worktree_path: PathBuf::from("/tmp/worktree-codex"),
                    change_summary: None,
                },
            ]
        }

        #[test]
        fn test_render_split_view_log_mode() {
            let result_infos = create_test_result_infos();
            let mut app = App::new(result_infos);
            let content = app
                .selected_info()
                .map(get_log_content_string)
                .unwrap_or_default();

            let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
            terminal
                .draw(|frame| render(frame, &mut app, &content))
                .unwrap();

            assert_snapshot!(terminal.backend());
        }

        #[test]
        fn test_render_split_view_diff_mode() {
            let result_infos = create_test_result_infos();
            let mut app = App::new(result_infos);
            app.set_mode(ViewMode::Diff);
            // Simulate diff content
            let content = "+++ b/src/main.rs\n--- a/src/main.rs\n@@ -1,3 +1,5 @@\n fn main() {\n+    println!(\"Hello\");\n     run();\n }";

            let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
            terminal
                .draw(|frame| render(frame, &mut app, content))
                .unwrap();

            assert_snapshot!(terminal.backend());
        }

        #[test]
        fn test_render_split_view_focused_on_details() {
            let result_infos = create_test_result_infos();
            let mut app = App::new(result_infos);
            app.toggle_focus(); // Focus on details panel
            let content = app
                .selected_info()
                .map(get_log_content_string)
                .unwrap_or_default();

            let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
            terminal
                .draw(|frame| render(frame, &mut app, &content))
                .unwrap();

            assert_snapshot!(terminal.backend());
        }

        #[test]
        fn test_render_split_view_search_mode() {
            let result_infos = create_test_result_infos();
            let mut app = App::new(result_infos);
            app.input_mode = InputMode::Search;
            app.search_query = "code".to_string();
            let content = app
                .selected_info()
                .map(get_log_content_string)
                .unwrap_or_default();

            let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
            terminal
                .draw(|frame| render(frame, &mut app, &content))
                .unwrap();

            assert_snapshot!(terminal.backend());
        }

        #[test]
        fn test_render_split_view_second_model_selected() {
            let result_infos = create_test_result_infos();
            let mut app = App::new(result_infos);
            app.next_model(); // Select gemini
            let content = app
                .selected_info()
                .map(get_log_content_string)
                .unwrap_or_default();

            let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
            terminal
                .draw(|frame| render(frame, &mut app, &content))
                .unwrap();

            assert_snapshot!(terminal.backend());
        }

        #[test]
        fn test_render_split_view_failed_model_selected() {
            let result_infos = create_test_result_infos();
            let mut app = App::new(result_infos);
            app.next_model(); // Select gemini
            app.next_model(); // Select codex (failed)
            let content = app
                .selected_info()
                .map(get_log_content_string)
                .unwrap_or_default();

            let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
            terminal
                .draw(|frame| render(frame, &mut app, &content))
                .unwrap();

            assert_snapshot!(terminal.backend());
        }

        #[test]
        fn test_render_split_view_empty_results() {
            let mut app = App::new(vec![]);
            let content = "";

            let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
            terminal
                .draw(|frame| render(frame, &mut app, content))
                .unwrap();

            assert_snapshot!(terminal.backend());
        }

        #[test]
        fn test_render_split_view_confirm_mode() {
            let result_infos = create_test_result_infos();
            let mut app = App::new(result_infos);
            app.input_mode = InputMode::Confirm;
            let content = app
                .selected_info()
                .map(get_log_content_string)
                .unwrap_or_default();

            let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
            terminal
                .draw(|frame| render(frame, &mut app, &content))
                .unwrap();

            assert_snapshot!(terminal.backend());
        }
    }
}
