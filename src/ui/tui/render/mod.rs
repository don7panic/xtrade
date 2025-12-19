mod alerts;
mod command_palette;
mod header;
mod layout;
mod logs;
mod metrics;
mod orderbook;
mod overview;
mod price_trend;

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout};

use crate::session::session_manager::SessionStats;
use crate::ui::ui_manager::RenderState;
use crate::ui::{AppState, InputMode};

use self::alerts::render_alerts_overlay;
use self::command_palette::render_command_palette;
use self::header::render_header;
use self::logs::render_logs;
use self::metrics::render_metrics;
use self::orderbook::render_orderbook;
use self::overview::render_symbol_overview;

pub(super) fn render_root(
    frame: &mut Frame<'_>,
    app: &mut AppState,
    render_state: &RenderState,
    session_stats: &SessionStats,
    orderbook_depth: usize,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(10),
            Constraint::Length(6),
        ])
        .split(frame.size());

    render_header(frame, chunks[0], app, session_stats);

    let body_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(38),
            Constraint::Percentage(34),
            Constraint::Percentage(28),
        ])
        .split(chunks[1]);

    render_symbol_overview(frame, body_chunks[0], app);
    render_orderbook(frame, body_chunks[1], app, orderbook_depth);
    render_metrics(frame, body_chunks[2], app);

    render_logs(frame, chunks[2], app, render_state);
    render_command_palette(frame, chunks[3], app, render_state);

    if matches!(app.input_mode, InputMode::Alerts) {
        render_alerts_overlay(frame, app);
    }
}
