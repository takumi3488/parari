mod app;
mod content;
mod render;
mod types;

#[cfg(test)]
mod tests;

use std::time::Duration;

use ratatui::crossterm::event;

use app::App;
use content::{get_diff_content_string, get_log_content_string};
use render::render;
use types::{SplitViewResult, ViewMode};

pub use types::SplitViewResult as SelectionResult;

use crate::domain::ResultInfo;
use crate::error::{Error, Result};

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
