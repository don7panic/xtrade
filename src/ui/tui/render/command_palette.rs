use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};

use crate::ui::ui_manager::RenderState;
use crate::ui::{AppState, InputMode};

use super::alerts::render_alert_popup;

pub(super) fn render_command_palette(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &AppState,
    render_state: &RenderState,
) {
    if matches!(app.input_mode, InputMode::AlertPopup) {
        render_alert_popup(frame, area, app);
        return;
    }

    let title = match app.input_mode {
        InputMode::Normal => " Command Hints ",
        InputMode::Command => " Command Entry ",
        InputMode::AlertPopup => " Command Hints ",
        InputMode::Alerts => " Alerts ",
    };

    let block = Block::default().title(title).borders(Borders::ALL);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if matches!(app.input_mode, InputMode::Command) {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(1)])
            .split(inner);

        let input_line = Paragraph::new(Text::from(vec![Line::from(vec![
            Span::styled(">", Style::default().fg(Color::Cyan)),
            Span::raw(" "),
            Span::raw(app.command_buffer.clone()),
        ])]))
        .wrap(Wrap { trim: true });
        frame.render_widget(input_line, layout[0]);

        let prompt_offset = 2u16; // ">" plus trailing space
        let max_cursor_x = layout[0]
            .x
            .saturating_add(layout[0].width.saturating_sub(1));
        let cursor_x = layout[0]
            .x
            .saturating_add(prompt_offset)
            .saturating_add(app.command_buffer.len() as u16)
            .min(max_cursor_x);
        frame.set_cursor(cursor_x, layout[0].y);

        let suggestion_items: Vec<ListItem> = if app.filtered_commands.is_empty() {
            vec![ListItem::new(Span::styled(
                "No matching commands",
                Style::default().fg(Color::DarkGray),
            ))]
        } else {
            app.filtered_commands
                .iter()
                .map(|cmd| {
                    ListItem::new(Line::from(vec![
                        Span::styled(
                            cmd.trigger,
                            Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::raw(" "),
                        Span::styled(cmd.usage, Style::default().fg(Color::Gray)),
                        Span::raw("  "),
                        Span::raw(cmd.description),
                    ]))
                })
                .collect()
        };

        let mut state = ListState::default();
        if !app.filtered_commands.is_empty() {
            state.select(Some(
                app.selected_command_index
                    .min(app.filtered_commands.len().saturating_sub(1)),
            ));
        }

        let list = List::new(suggestion_items)
            .highlight_style(
                Style::default()
                    .fg(Color::White)
                    .bg(Color::Blue)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("▶ ");

        frame.render_stateful_widget(list, layout[1], &mut state);
    } else {
        let mut hints = vec![
            Span::styled("←/→/↑/↓", Style::default().fg(Color::Cyan)),
            Span::raw(": Switch symbol   "),
            Span::styled("j/k", Style::default().fg(Color::Cyan)),
            Span::raw(": Scroll logs   "),
            Span::styled("a", Style::default().fg(Color::Cyan)),
            Span::raw(": Alert popup   "),
            Span::styled("Shift+A", Style::default().fg(Color::Cyan)),
            Span::raw(": Alerts view   "),
            Span::styled("/", Style::default().fg(Color::Cyan)),
            Span::raw(": Command palette   "),
            Span::styled("Space", Style::default().fg(Color::Cyan)),
            Span::raw(": Pause   "),
            Span::styled("q", Style::default().fg(Color::Cyan)),
            Span::raw(": Quit"),
        ];

        if let Some(msg) = render_state.pending_messages.last() {
            hints.push(Span::raw("   |   "));
            hints.push(Span::styled(
                msg.clone(),
                Style::default().fg(Color::LightBlue),
            ));
        }

        let paragraph =
            Paragraph::new(Text::from(vec![Line::from(hints)])).wrap(Wrap { trim: true });
        frame.render_widget(paragraph, inner);
    }
}
