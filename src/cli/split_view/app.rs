use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::widgets::ListState;

use super::types::{FocusedPanel, InputMode, SplitViewResult, ViewMode};
use crate::domain::ResultInfo;

/// Application state
pub struct App {
    pub result_infos: Vec<ResultInfo>,
    pub list_state: ListState,
    pub current_mode: ViewMode,
    pub focused_panel: FocusedPanel,
    pub input_mode: InputMode,
    pub scroll_offset: u16,
    pub content_height: u16,
    pub search_query: String,
    pub search_matches: Vec<u16>,
    pub search_match_index: usize,
    pub result: Option<SplitViewResult>,
}

impl App {
    pub fn new(result_infos: Vec<ResultInfo>) -> Self {
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

    pub fn selected_index(&self) -> usize {
        self.list_state.selected().unwrap_or(0)
    }

    pub fn selected_info(&self) -> Option<&ResultInfo> {
        self.result_infos.get(self.selected_index())
    }

    pub fn next_model(&mut self) {
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

    pub fn previous_model(&mut self) {
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

    pub fn scroll_down(&mut self, lines: u16) {
        self.scroll_offset = self.scroll_offset.saturating_add(lines);
    }

    pub fn scroll_up(&mut self, lines: u16) {
        self.scroll_offset = self.scroll_offset.saturating_sub(lines);
    }

    pub fn scroll_to_top(&mut self) {
        self.scroll_offset = 0;
    }

    pub fn scroll_to_bottom(&mut self) {
        if self.content_height > 0 {
            self.scroll_offset = self.content_height.saturating_sub(1);
        }
    }

    pub fn half_page_down(&mut self, viewport_height: u16) {
        self.scroll_down(viewport_height / 2);
    }

    pub fn half_page_up(&mut self, viewport_height: u16) {
        self.scroll_up(viewport_height / 2);
    }

    pub fn set_mode(&mut self, mode: ViewMode) {
        if self.current_mode != mode {
            self.current_mode = mode;
            self.scroll_offset = 0;
            self.clear_search();
        }
    }

    pub fn toggle_focus(&mut self) {
        self.focused_panel = match self.focused_panel {
            FocusedPanel::Models => FocusedPanel::Details,
            FocusedPanel::Details => FocusedPanel::Models,
        };
    }

    pub fn start_search(&mut self) {
        self.input_mode = InputMode::Search;
        self.search_query.clear();
        self.search_matches.clear();
        self.search_match_index = 0;
    }

    pub fn cancel_search(&mut self) {
        self.input_mode = InputMode::Normal;
    }

    pub fn clear_search(&mut self) {
        self.search_query.clear();
        self.search_matches.clear();
        self.search_match_index = 0;
    }

    pub fn execute_search(&mut self, content: &str) {
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

    pub fn next_search_match(&mut self) {
        if self.search_matches.is_empty() {
            return;
        }
        self.search_match_index = (self.search_match_index + 1) % self.search_matches.len();
        self.scroll_offset = self.search_matches[self.search_match_index];
    }

    pub fn previous_search_match(&mut self) {
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

    pub fn apply(&mut self) {
        self.result = Some(SplitViewResult::Apply(self.selected_index()));
    }

    pub fn start_confirm(&mut self) {
        self.input_mode = InputMode::Confirm;
    }

    pub fn cancel_confirm(&mut self) {
        self.input_mode = InputMode::Normal;
    }

    pub fn start_confirm_cancel(&mut self) {
        self.input_mode = InputMode::ConfirmCancel;
    }

    pub fn cancel(&mut self) {
        self.result = Some(SplitViewResult::Cancel);
    }

    pub fn handle_event(&mut self, event: Event, viewport_height: u16, content: &str) -> bool {
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
