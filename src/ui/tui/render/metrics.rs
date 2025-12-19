use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Gauge, Paragraph, Wrap};

use crate::ui::AppState;

pub(super) fn render_metrics(frame: &mut Frame<'_>, area: Rect, app: &mut AppState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(7), Constraint::Min(4)])
        .split(area);

    render_latency_gauges(frame, chunks[0], app);
    render_price_trend(frame, chunks[1], app);
}

fn render_latency_gauges(frame: &mut Frame<'_>, area: Rect, app: &AppState) {
    let metrics = &app.connection_metrics;
    let block = Block::default().title(" Metrics ").borders(Borders::ALL);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines = Vec::new();
    lines.push(Line::from(vec![
        Span::styled(
            format!("Latency P95: {} ms", metrics.latency_p95),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("    "),
        Span::raw(format!("Msg/s: {:.1}", metrics.messages_per_second)),
    ]));
    lines.push(Line::from(vec![
        Span::raw(format!("Reconnects: {}", metrics.reconnect_count)),
        Span::raw("    "),
        Span::raw(format!("Errors: {}", metrics.error_count)),
    ]));
    lines.push(Line::from(vec![
        Span::raw(format!("Quality: {:?}", metrics.connection_quality)),
        Span::raw("    "),
        Span::raw(format!("Uptime: {}s", metrics.uptime_seconds)),
    ]));

    let gauge_value = (metrics.messages_per_second / 1200.0).min(1.0);
    let gauge = Gauge::default()
        .block(Block::default().borders(Borders::TOP))
        .gauge_style(
            Style::default()
                .fg(Color::Magenta)
                .bg(Color::Black)
                .add_modifier(Modifier::BOLD),
        )
        .ratio(gauge_value)
        .label(format!(
            "Throughput {:.1} msg/s",
            metrics.messages_per_second
        ));

    let sub = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(lines.len() as u16 + 1),
            Constraint::Min(1),
        ])
        .split(inner);

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), sub[0]);
    frame.render_widget(gauge, sub[1]);
}

fn render_price_trend(frame: &mut Frame<'_>, area: Rect, app: &mut AppState) {
    super::price_trend::render_price_trend(frame, area, app);
}
