use super::app::App;
use super::content::{get_agent_emoji, get_log_content_string, strip_ansi_codes};
use super::render::render;
use super::types::{FocusedPanel, InputMode, ViewMode};

use crate::domain::ResultInfo;
use crate::executor::OutputLine;

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
    use crate::git::ChangeSummary;
    use insta::assert_snapshot;
    use ratatui::{Terminal, backend::TestBackend};
    use std::path::PathBuf;

    /// Create test result infos for snapshot tests
    fn create_test_result_infos() -> Vec<ResultInfo> {
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
