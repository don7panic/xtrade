use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, Wrap};

use crate::session::alert_manager::{AlertDirection, AlertRepeat};
use crate::ui::{AlertFormField, AppState};

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
            Constraint::Length(10),
            Constraint::Length(6),
            Constraint::Length(12),
            Constraint::Length(12),
            Constraint::Length(10),
            Constraint::Length(8),
            Constraint::Length(6),
            Constraint::Length(8),
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
                let mode = match alert.repeat {
                    AlertRepeat::Once => "Once",
                    AlertRepeat::Repeat => "Repeat",
                };
                let cooldown = if alert.cooldown_ms > 0 {
                    format!("{}s", alert.cooldown_ms / 1_000)
                } else {
                    "-".to_string()
                };
                let hysteresis = if alert.hysteresis > 0.0 {
                    format!("{:.4}", alert.hysteresis)
                } else {
                    "-".to_string()
                };

                let mut row = Row::new(vec![
                    Cell::from(format!("#{}", alert.id)),
                    Cell::from(alert.symbol.clone()),
                    Cell::from(direction),
                    Cell::from(format!("{:.4}", alert.threshold)),
                    Cell::from(last_price),
                    Cell::from(status),
                    Cell::from(mode),
                    Cell::from(cooldown),
                    Cell::from(hysteresis),
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
                Row::new(vec![
                    "ID",
                    "Symbol",
                    "Dir",
                    "Threshold",
                    "Last",
                    "State",
                    "Mode",
                    "CD",
                    "Hys",
                ])
                .style(
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
    let popup_area = centered_rect(62, 32, frame.size());
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
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(2),
            Constraint::Length(1),
        ])
        .margin(1)
        .split(inner_area);

    let active_field = app.alert_form.active_field;
    let active_label_style = Style::default()
        .fg(Color::Black)
        .bg(Color::LightCyan)
        .add_modifier(Modifier::BOLD);
    let active_value_style = Style::default()
        .fg(Color::Black)
        .bg(Color::LightCyan)
        .add_modifier(Modifier::BOLD);
    let label_style = Style::default().fg(Color::Cyan);
    let value_style = Style::default()
        .add_modifier(Modifier::BOLD)
        .fg(Color::White);

    // Symbol row
    let symbol_text = format!("Symbol: {}", app.alert_form.symbol);
    frame.render_widget(
        Paragraph::new(symbol_text)
            .style(Style::default().fg(Color::Cyan))
            .wrap(Wrap { trim: true }),
        inner_layout[0],
    );

    // Direction row
    let direction_label_style = if active_field == AlertFormField::Direction {
        active_label_style
    } else {
        label_style
    };
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
        Span::styled("Direction: ", direction_label_style),
        Span::styled("Above", above_style),
        Span::raw("  "),
        Span::styled("Below", below_style),
    ]);
    frame.render_widget(
        Paragraph::new(dir_line).wrap(Wrap { trim: true }),
        inner_layout[1],
    );

    // Mode row
    let mode_label_style = if active_field == AlertFormField::Mode {
        active_label_style
    } else {
        label_style
    };
    let (once_style, repeat_style) = match app.alert_form.repeat {
        AlertRepeat::Once => (
            Style::default()
                .fg(Color::Black)
                .bg(Color::LightCyan)
                .add_modifier(Modifier::BOLD),
            Style::default().fg(Color::Gray),
        ),
        AlertRepeat::Repeat => (
            Style::default().fg(Color::Gray),
            Style::default()
                .fg(Color::Black)
                .bg(Color::LightCyan)
                .add_modifier(Modifier::BOLD),
        ),
    };
    let mode_line = Line::from(vec![
        Span::styled("Mode: ", mode_label_style),
        Span::styled("Once", once_style),
        Span::raw("  "),
        Span::styled("Repeat", repeat_style),
    ]);
    frame.render_widget(
        Paragraph::new(mode_line).wrap(Wrap { trim: true }),
        inner_layout[2],
    );

    // Price row
    let price_value = if app.alert_form.price_input.is_empty() {
        " ".to_string()
    } else {
        app.alert_form.price_input.clone()
    };
    let price_label_style = if active_field == AlertFormField::Price {
        active_label_style
    } else {
        label_style
    };
    let price_value_style = if active_field == AlertFormField::Price {
        active_value_style
    } else {
        value_style
    };
    let price_line = Line::from(vec![
        Span::styled("Price: ", price_label_style),
        Span::styled(price_value, price_value_style),
        // Cursor indicator
        Span::styled(" ", Style::default().bg(Color::LightCyan)),
    ]);
    frame.render_widget(
        Paragraph::new(price_line).wrap(Wrap { trim: true }),
        inner_layout[4],
    );

    // Cooldown row
    let cooldown_value = if app.alert_form.cooldown_input.is_empty() {
        "-".to_string()
    } else {
        app.alert_form.cooldown_input.clone()
    };
    let cooldown_label_style = if active_field == AlertFormField::Cooldown {
        active_label_style
    } else {
        label_style
    };
    let cooldown_value_style = if active_field == AlertFormField::Cooldown {
        active_value_style
    } else {
        value_style
    };
    let cooldown_line = Line::from(vec![
        Span::styled("Cooldown(s): ", cooldown_label_style),
        Span::styled(cooldown_value, cooldown_value_style),
    ]);
    frame.render_widget(
        Paragraph::new(cooldown_line).wrap(Wrap { trim: true }),
        inner_layout[5],
    );

    // Hysteresis row
    let hysteresis_value = if app.alert_form.hysteresis_input.is_empty() {
        "-".to_string()
    } else {
        app.alert_form.hysteresis_input.clone()
    };
    let hysteresis_label_style = if active_field == AlertFormField::Hysteresis {
        active_label_style
    } else {
        label_style
    };
    let hysteresis_value_style = if active_field == AlertFormField::Hysteresis {
        active_value_style
    } else {
        value_style
    };
    let hysteresis_line = Line::from(vec![
        Span::styled("Hysteresis: ", hysteresis_label_style),
        Span::styled(hysteresis_value, hysteresis_value_style),
    ]);
    frame.render_widget(
        Paragraph::new(hysteresis_line).wrap(Wrap { trim: true }),
        inner_layout[6],
    );

    // Error/info row
    let (error_line, error_style) = if let Some(err) = app.alert_form.error.clone() {
        (err, Style::default().fg(Color::LightRed))
    } else {
        (
            "Up/Down switch fields, Tab toggles options, Enter to save, Esc to cancel".to_string(),
            Style::default().fg(Color::Gray),
        )
    };
    frame.render_widget(
        Paragraph::new(error_line).style(error_style),
        inner_layout[7],
    );

    // Hint row
    frame.render_widget(
        Paragraph::new("Hint: hysteresis supports % (e.g. 0.2%), cooldown in seconds")
            .style(Style::default().fg(Color::Gray)),
        inner_layout[8],
    );
}
