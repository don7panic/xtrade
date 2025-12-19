use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, Wrap};

use crate::session::alert_manager::AlertDirection;
use crate::ui::AppState;

use super::layout::centered_rect;

pub(super) fn render_alerts_overlay(frame: &mut Frame<'_>, app: &AppState) {
    let overlay_area = centered_rect(80, 60, frame.size());
    frame.render_widget(Clear, overlay_area);

    let block = Block::default()
        .title(" Alerts ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .style(Style::default().bg(Color::Magenta));
    frame.render_widget(block.clone(), overlay_area);

    let inner = block.inner(overlay_area);
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(1),
        ])
        .margin(1)
        .split(inner);

    let total = app.alerts.len();
    let triggered = app.alerts.iter().filter(|a| a.triggered).count();
    let armed = total.saturating_sub(triggered);
    let summary = Paragraph::new(format!(
        "Total: {}  | Armed: {}  | Triggered: {}",
        total, armed, triggered
    ))
    .style(Style::default().fg(Color::Gray));
    frame.render_widget(summary, layout[0]);

    if app.alerts.is_empty() {
        let placeholder = Paragraph::new("No alerts configured")
            .style(Style::default().fg(Color::Gray))
            .alignment(Alignment::Center);
        frame.render_widget(placeholder, layout[1]);
    } else {
        let widths = [
            Constraint::Length(6),
            Constraint::Length(12),
            Constraint::Length(8),
            Constraint::Length(14),
            Constraint::Length(14),
            Constraint::Length(12),
        ];

        let rows: Vec<Row> = app
            .alerts
            .iter()
            .enumerate()
            .map(|(idx, alert)| {
                let direction = match alert.direction {
                    AlertDirection::Above => "Above",
                    AlertDirection::Below => "Below",
                };
                let status = if alert.triggered {
                    "Triggered"
                } else {
                    "Armed"
                };
                let last_price = alert
                    .last_price
                    .map(|p| format!("{:.4}", p))
                    .unwrap_or_else(|| "-".to_string());

                let mut row = Row::new(vec![
                    Cell::from(format!("#{}", alert.id)),
                    Cell::from(alert.symbol.clone()),
                    Cell::from(direction),
                    Cell::from(format!("{:.4}", alert.threshold)),
                    Cell::from(last_price),
                    Cell::from(status),
                ]);

                if idx == app.selected_alert_index {
                    row = row.style(
                        Style::default()
                            .fg(Color::White)
                            .bg(Color::Blue)
                            .add_modifier(Modifier::BOLD),
                    );
                } else if alert.triggered {
                    row = row.style(Style::default().fg(Color::Red));
                }

                row
            })
            .collect();

        let table = Table::new(rows, widths)
            .header(
                Row::new(vec!["ID", "Symbol", "Dir", "Threshold", "Last", "State"]).style(
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
            )
            .column_spacing(1);

        frame.render_widget(table, layout[1]);
    }

    let hints = Paragraph::new("↑/↓ move   d delete   Shift+C clear all   r refresh   Esc close")
        .style(Style::default().fg(Color::Gray));
    frame.render_widget(hints, layout[2]);
}

pub(super) fn render_alert_popup(frame: &mut Frame<'_>, _area: Rect, app: &AppState) {
    // Centered box occupying a portion of the screen
    let popup_area = centered_rect(50, 20, frame.size());
    let block = Block::default()
        .title(" Add Alert ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .style(Style::default().bg(Color::Magenta));
    frame.render_widget(Clear, popup_area);
    frame.render_widget(block.clone(), popup_area);

    let inner_area = block.inner(popup_area);
    let inner_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(2),
            Constraint::Length(1),
        ])
        .margin(1)
        .split(inner_area);

    // Symbol row
    let symbol_text = format!("Symbol: {}", app.alert_form.symbol);
    frame.render_widget(
        Paragraph::new(symbol_text)
            .style(Style::default().fg(Color::Cyan))
            .wrap(Wrap { trim: true }),
        inner_layout[0],
    );

    // Direction row
    let (above_style, below_style) = if app.alert_form.direction_above {
        (
            Style::default()
                .fg(Color::Black)
                .bg(Color::LightCyan)
                .add_modifier(Modifier::BOLD),
            Style::default().fg(Color::Gray),
        )
    } else {
        (
            Style::default().fg(Color::Gray),
            Style::default()
                .fg(Color::Black)
                .bg(Color::LightCyan)
                .add_modifier(Modifier::BOLD),
        )
    };
    let dir_line = Line::from(vec![
        Span::raw("Direction: "),
        Span::styled("Above", above_style),
        Span::raw("  "),
        Span::styled("Below", below_style),
    ]);
    frame.render_widget(
        Paragraph::new(dir_line).wrap(Wrap { trim: true }),
        inner_layout[1],
    );

    // Price row
    let price_value = if app.alert_form.price_input.is_empty() {
        " ".to_string()
    } else {
        app.alert_form.price_input.clone()
    };
    let price_line = Line::from(vec![
        Span::styled("Price: ", Style::default().fg(Color::Cyan)),
        Span::styled(
            price_value,
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(Color::White),
        ),
        // Cursor indicator
        Span::styled(" ", Style::default().bg(Color::LightCyan)),
    ]);
    frame.render_widget(
        Paragraph::new(price_line).wrap(Wrap { trim: true }),
        inner_layout[2],
    );

    // Error/info row
    let (error_line, error_style) = if let Some(err) = app.alert_form.error.clone() {
        (err, Style::default().fg(Color::LightRed))
    } else {
        (
            "Enter price, Tab to toggle Above/Below, Enter to save, Esc to cancel".to_string(),
            Style::default().fg(Color::Gray),
        )
    };
    frame.render_widget(
        Paragraph::new(error_line).style(error_style),
        inner_layout[3],
    );

    // Hint row
    frame.render_widget(
        Paragraph::new("Shortcuts: a = open, Tab/←/→ toggle direction, Enter = save, Esc = cancel")
            .style(Style::default().fg(Color::Gray)),
        inner_layout[4],
    );
}
