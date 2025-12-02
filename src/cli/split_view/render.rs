use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Position};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, List, ListItem, Paragraph, Wrap};

use super::app::App;
use super::content::{get_agent_emoji, get_styled_content, get_styled_content_with_search};
use super::types::{FocusedPanel, InputMode, ViewMode};

pub fn render(frame: &mut Frame, app: &mut App, content: &str) {
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
