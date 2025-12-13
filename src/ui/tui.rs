//! Terminal User Interface implementation
//!
//! Provides the main TUI interface using ratatui.

use std::io::{Stdout, stdout};

use chrono::{DateTime, Utc};
use crossterm::{
    cursor,
    event::{
        DisableMouseCapture, EnableMouseCapture, KeyCode, KeyEvent, KeyEventKind, KeyModifiers,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ordered_float::OrderedFloat;
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{
        Block, Borders, Cell, Clear, Gauge, List, ListItem, ListState, Paragraph, Row, Table, Wrap,
    },
};

use super::{AppState, InputMode, MarketDataState};
use crate::AppResult;
use crate::metrics::ConnectionStatus as MetricsConnectionStatus;
use crate::session::session_manager::SessionStats;

/// Actions generated from key handling
pub enum UiAction {
    None,
    SubmitCommand(String),
    QuitRequested,
}

/// RAII helper controlling the terminal lifecycle
pub struct Tui {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl Tui {
    /// Create a new TUI terminal instance
    pub fn new() -> AppResult<Self> {
        enable_raw_mode()?;
        let mut stdout = stdout();
        execute!(
            stdout,
            EnterAlternateScreen,
            cursor::Hide,
            EnableMouseCapture
        )?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;

        Ok(Self { terminal })
    }

    /// Render the application
    pub fn draw(
        &mut self,
        app: &mut AppState,
        render_state: &crate::ui::ui_manager::RenderState,
        session_stats: &SessionStats,
        orderbook_depth: usize,
    ) -> AppResult<()> {
        self.terminal.draw(|frame| {
            render_root(frame, app, render_state, session_stats, orderbook_depth);
        })?;
        Ok(())
    }

    /// Restore terminal to canonical mode
    pub fn restore(&mut self) -> AppResult<()> {
        disable_raw_mode()?;
        let mut stdout = stdout();
        execute!(stdout, cursor::Show, LeaveAlternateScreen)?;
        Ok(())
    }
}

impl Drop for Tui {
    fn drop(&mut self) {
        // Attempt to restore the terminal; ignore errors because we are in Drop
        let _ = disable_raw_mode();
        let mut stdout = stdout();
        let _ = execute!(
            stdout,
            cursor::Show,
            LeaveAlternateScreen,
            DisableMouseCapture
        );
    }
}

/// Handle keyboard events for TUI, returning actions for the session manager
pub fn handle_key_event(app: &mut AppState, key_event: KeyEvent) -> UiAction {
    if key_event.kind == KeyEventKind::Release {
        return UiAction::None;
    }

    // Global shortcut to bring up alert popup when in normal mode
    if matches!(app.input_mode, InputMode::Normal) {
        if let KeyCode::Char('a') = key_event.code {
            if !key_event.modifiers.contains(KeyModifiers::CONTROL) {
                let preset = app
                    .current_symbol()
                    .and_then(|sym| app.market_data.get(sym))
                    .map(|md| md.price);
                if let Err(e) = app.activate_alert_popup(preset) {
                    app.push_notification(e);
                }
                return UiAction::None;
            }
        }
    }

    // Global shortcuts first
    if key_event.modifiers.contains(KeyModifiers::CONTROL) {
        match key_event.code {
            KeyCode::Char('c') | KeyCode::Char('d') => {
                app.should_quit = true;
                return UiAction::QuitRequested;
            }
            KeyCode::Char('p') => {
                app.toggle_pause();
                return UiAction::None;
            }
            _ => {}
        }
    }

    match app.input_mode {
        InputMode::Normal => handle_normal_mode_keys(app, key_event),
        InputMode::Command => handle_command_mode_keys(app, key_event),
        InputMode::AlertPopup => handle_alert_popup_keys(app, key_event),
    }
}

fn handle_normal_mode_keys(app: &mut AppState, key_event: KeyEvent) -> UiAction {
    match key_event.code {
        KeyCode::Char('q') => {
            app.should_quit = true;
            UiAction::QuitRequested
        }
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
            app.clear_command();
            UiAction::None
        }
        KeyCode::Char('/') | KeyCode::Char(':') => {
            let preset = if matches!(key_event.code, KeyCode::Char('/')) {
                Some("/")
            } else {
                None
            };
            app.activate_command_mode(preset);
            UiAction::None
        }
        KeyCode::Char('p') | KeyCode::Char(' ') => {
            app.toggle_pause();
            UiAction::None
        }
        KeyCode::Left | KeyCode::Up => {
            app.previous_tab();
            UiAction::None
        }
        KeyCode::Right | KeyCode::Down => {
            app.next_tab();
            UiAction::None
        }
        KeyCode::Char('k') => {
            app.scroll_logs_up();
            UiAction::None
        }
        KeyCode::Char('j') => {
            app.scroll_logs_down();
            UiAction::None
        }
        KeyCode::Char('s') => {
            app.activate_command_mode(Some("/status"));
            UiAction::None
        }
        KeyCode::Char('L') if key_event.modifiers.contains(KeyModifiers::SHIFT) => {
            app.activate_command_mode(Some("/logs"));
            UiAction::None
        }
        KeyCode::Enter => UiAction::None,
        _ => UiAction::None,
    }
}

fn handle_command_mode_keys(app: &mut AppState, key_event: KeyEvent) -> UiAction {
    match key_event.code {
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
            app.clear_command();
            app.reset_command_suggestions();
            UiAction::None
        }
        KeyCode::Enter => {
            let command = app.command_buffer.trim().to_string();
            app.input_mode = InputMode::Normal;
            app.clear_command();
            app.reset_command_suggestions();
            if command.is_empty() {
                UiAction::None
            } else {
                UiAction::SubmitCommand(command)
            }
        }
        KeyCode::Backspace => {
            app.command_buffer.pop();
            app.update_command_suggestions();
            UiAction::None
        }
        KeyCode::Up => {
            app.select_previous_suggestion();
            UiAction::None
        }
        KeyCode::Down => {
            app.select_next_suggestion();
            UiAction::None
        }
        KeyCode::Tab => {
            app.apply_selected_suggestion();
            UiAction::None
        }
        KeyCode::Char(c) => {
            if !key_event.modifiers.contains(KeyModifiers::CONTROL) {
                app.command_buffer.push(c);
                app.update_command_suggestions();
            }
            UiAction::None
        }
        KeyCode::Left | KeyCode::Right => UiAction::None,
        _ => UiAction::None,
    }
}

fn handle_alert_popup_keys(app: &mut AppState, key_event: KeyEvent) -> UiAction {
    match key_event.code {
        KeyCode::Esc => {
            app.deactivate_alert_popup();
            UiAction::None
        }
        KeyCode::Tab | KeyCode::Left | KeyCode::Right => {
            app.toggle_alert_direction();
            UiAction::None
        }
        KeyCode::Enter => match app.alert_price() {
            Ok(price) => {
                let command = format!(
                    "/alert {} {} {}",
                    app.alert_form.symbol,
                    if app.alert_form.direction_above {
                        "above"
                    } else {
                        "below"
                    },
                    price
                );
                app.deactivate_alert_popup();
                UiAction::SubmitCommand(command)
            }
            Err(e) => {
                app.alert_form.error = Some(e);
                UiAction::None
            }
        },
        KeyCode::Backspace => {
            if !app.alert_form.price_dirty {
                app.alert_form.price_input.clear();
                app.alert_form.price_dirty = true;
            }
            app.alert_form.price_input.pop();
            app.alert_form.error = None;
            UiAction::None
        }
        KeyCode::Char(c) => {
            if c.is_ascii_digit() || c == '.' || c == '-' || c == '+' {
                if !app.alert_form.price_dirty {
                    app.alert_form.price_input.clear();
                    app.alert_form.price_dirty = true;
                }
                app.alert_form.price_input.push(c);
                app.alert_form.error = None;
            }
            UiAction::None
        }
        _ => UiAction::None,
    }
}

/// Render the main TUI layout
pub fn render_app(
    app: &mut AppState,
    render_state: &crate::ui::ui_manager::RenderState,
    session_stats: &SessionStats,
    orderbook_depth: usize,
) -> AppResult<()> {
    let mut tui = Tui::new()?;
    tui.draw(app, render_state, session_stats, orderbook_depth)?;
    tui.restore()?;
    Ok(())
}

fn render_root(
    frame: &mut Frame<'_>,
    app: &mut AppState,
    render_state: &crate::ui::ui_manager::RenderState,
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
}

fn render_header(frame: &mut Frame<'_>, area: Rect, app: &AppState, session_stats: &SessionStats) {
    let title = Span::styled(
        " XTrade Market Data Monitor ",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    );

    let status = match app.connection_metrics.status {
        MetricsConnectionStatus::Connected => {
            Span::styled("● Connected ", Style::default().fg(Color::Green))
        }
        MetricsConnectionStatus::Connecting => {
            Span::styled("● Connecting ", Style::default().fg(Color::Yellow))
        }
        MetricsConnectionStatus::Reconnecting => {
            Span::styled("● Reconnecting ", Style::default().fg(Color::Yellow))
        }
        MetricsConnectionStatus::Disconnected => {
            Span::styled("● Disconnected ", Style::default().fg(Color::Red))
        }
        MetricsConnectionStatus::Error(_) => {
            Span::styled("● Error ", Style::default().fg(Color::Red))
        }
    };

    let commands = Span::styled(
        format!(
            "Cmds: {} | Events: {} ",
            session_stats.commands_processed, session_stats.events_processed
        ),
        Style::default().fg(Color::Gray),
    );

    let body = vec![Line::from(vec![
        title,
        Span::raw(" "),
        status,
        Span::raw(" "),
        commands,
        Span::raw(" "),
        Span::styled(
            if app.paused { "PAUSED" } else { "LIVE" },
            if app.paused {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Green)
            },
        ),
    ])];

    let block = Block::default().borders(Borders::ALL).title(" Session ");

    let paragraph = Paragraph::new(body).block(block).wrap(Wrap { trim: true });
    frame.render_widget(paragraph, area);
}

fn render_symbol_overview(frame: &mut Frame<'_>, area: Rect, app: &AppState) {
    let block = Block::default().title(" Markets ").borders(Borders::ALL);

    let widths = [
        Constraint::Length(12),
        Constraint::Length(12),
        Constraint::Length(10),
        Constraint::Length(10),
    ];

    let header = Row::new(["Symbol", "Price", "Δ%", "Volume"]).style(
        Style::default()
            .fg(Color::Gray)
            .add_modifier(Modifier::BOLD),
    );

    let rows: Vec<Row> = app
        .symbols
        .iter()
        .enumerate()
        .map(|(idx, symbol)| {
            let default = MarketDataState {
                symbol: symbol.clone(),
                ..MarketDataState::default()
            };

            let data = app.market_data.get(symbol).unwrap_or(&default);
            let change_style = if data.change_percent > 0.0 {
                Style::default().fg(Color::Green)
            } else if data.change_percent < 0.0 {
                Style::default().fg(Color::Red)
            } else {
                Style::default()
            };

            let mut row = Row::new(vec![
                Cell::from(symbol.clone()),
                Cell::from(format!("{:.2}", data.price)),
                Cell::from(format!("{:+.2}", data.change_percent)).style(change_style),
                Cell::from(format!("{:.2}k", data.volume_24h / 1000.0)),
            ]);

            if app.selected_tab == idx {
                row = row.style(
                    Style::default()
                        .fg(Color::White)
                        .bg(Color::Blue)
                        .add_modifier(Modifier::BOLD),
                );
            }

            row
        })
        .collect();

    let table = Table::new(rows, widths)
        .header(header)
        .block(block)
        .column_spacing(1);

    frame.render_widget(table, area);
}

fn render_orderbook(frame: &mut Frame<'_>, area: Rect, app: &AppState, orderbook_depth: usize) {
    let block = Block::default().title(" Order Book ").borders(Borders::ALL);

    let symbol = app.current_symbol().cloned().unwrap_or_default();
    let market_data = if symbol.is_empty() {
        None
    } else {
        app.market_data.get(&symbol).cloned()
    };

    let inner_area = block.inner(area);
    frame.render_widget(block, area);

    if let Some(data) = market_data {
        let orderbook = data
            .orderbook
            .unwrap_or_else(|| crate::binance::types::OrderBook::new(symbol.clone()));

        let mut bid_rows: Vec<(OrderedFloat<f64>, f64)> = orderbook
            .bids
            .iter()
            .rev()
            .take(orderbook_depth)
            .map(|(price, qty)| (*price, *qty))
            .collect::<Vec<_>>();
        let mut ask_rows: Vec<(OrderedFloat<f64>, f64)> = orderbook
            .asks
            .iter()
            .take(orderbook_depth)
            .map(|(price, qty)| (*price, *qty))
            .collect::<Vec<_>>();

        // Ensure equal length for display
        let depth = bid_rows.len().max(ask_rows.len());
        bid_rows.resize(depth, (OrderedFloat(0.0), 0.0));
        ask_rows.resize(depth, (OrderedFloat(0.0), 0.0));

        let rows = bid_rows
            .into_iter()
            .zip(ask_rows)
            .map(|(bid, ask)| {
                let bid_price = bid.0.into_inner();
                let ask_price = ask.0.into_inner();
                Row::new(vec![
                    Cell::from(format!("{:>10.4}", bid.1)),
                    Cell::from(format!("{:>10.2}", bid_price)),
                    Cell::from(format!("{:>10.2}", ask_price)),
                    Cell::from(format!("{:>10.4}", ask.1)),
                ])
                .style(
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::DIM),
                )
            })
            .collect::<Vec<_>>();

        let widths = [
            Constraint::Length(12),
            Constraint::Length(12),
            Constraint::Length(12),
            Constraint::Length(12),
        ];

        let table = Table::new(rows, widths)
            .header(
                Row::new(vec!["Bid Size", "Bid Price", "Ask Price", "Ask Size"])
                    .style(Style::default().fg(Color::Gray)),
            )
            .column_spacing(1);

        frame.render_widget(table, inner_area);
    } else {
        let placeholder = Paragraph::new("No market data available yet")
            .style(Style::default().fg(Color::Gray))
            .alignment(ratatui::layout::Alignment::Center);
        frame.render_widget(placeholder, inner_area);
    }
}

fn render_metrics(frame: &mut Frame<'_>, area: Rect, app: &mut AppState) {
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
    let block = Block::default()
        .title(" Price Trend ")
        .borders(Borders::ALL);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width < 12 || inner.height < 4 {
        render_price_trend_placeholder(frame, inner);
        return;
    }

    let Some(symbol) = app.current_symbol().cloned() else {
        render_price_trend_placeholder(frame, inner);
        return;
    };

    let Some(data) = app.market_data.get_mut(&symbol) else {
        render_price_trend_placeholder(frame, inner);
        return;
    };

    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(8), Constraint::Min(2)])
        .split(inner);
    let price_axis_area = horizontal[0];
    let right_area = horizontal[1];

    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(2), Constraint::Length(1)])
        .split(right_area);
    let chart_area = vertical[0];
    let time_axis_area = vertical[1];

    if chart_area.width < 4 || chart_area.height < 3 {
        render_price_trend_placeholder(frame, inner);
        return;
    }

    let Some(cache) = data.ensure_kline_cache(chart_area.width) else {
        render_price_trend_placeholder(frame, inner);
        return;
    };

    if cache.samples.is_empty() {
        render_price_trend_placeholder(frame, inner);
        return;
    }

    let mut min_price = cache.min_price;
    let mut max_price = cache.max_price;
    if (max_price - min_price).abs() < f64::EPSILON {
        max_price = min_price + 1.0;
    } else {
        let padding = (max_price - min_price) * 0.05;
        min_price -= padding;
        max_price += padding;
    }

    draw_candlesticks(frame, chart_area, &cache.samples, min_price, max_price);

    // Render price axis labels (max, mid, min)
    {
        let buffer = frame.buffer_mut();
        let label_style = Style::default().fg(Color::Gray);
        let top_label = format_price_label(max_price, price_axis_area.width);
        let mid_label = format_price_label((min_price + max_price) / 2.0, price_axis_area.width);
        let bottom_label = format_price_label(min_price, price_axis_area.width);

        buffer.set_string(price_axis_area.x, price_axis_area.y, top_label, label_style);

        if price_axis_area.height > 2 {
            let mid_y = price_axis_area.y + price_axis_area.height / 2;
            buffer.set_string(price_axis_area.x, mid_y, mid_label, label_style);
        }

        let bottom_y = price_axis_area.y + price_axis_area.height.saturating_sub(1);
        buffer.set_string(price_axis_area.x, bottom_y, bottom_label, label_style);
    }

    // Render time axis summary
    let time_text = match (cache.samples.first(), cache.samples.last()) {
        (Some(first), Some(last)) => {
            let start = format_candle_date(first.open_time_ms);
            let end = format_candle_date(last.close_time_ms);
            format!("{} → {} ({} candles)", start, end, cache.samples.len())
        }
        _ => "Daily candle data unavailable".to_string(),
    };

    frame.render_widget(
        Paragraph::new(time_text).alignment(Alignment::Center),
        time_axis_area,
    );
}

fn draw_candlesticks(
    frame: &mut Frame<'_>,
    area: Rect,
    samples: &[crate::ui::CandleSample],
    min_price: f64,
    max_price: f64,
) {
    if area.width < 2 || area.height < 2 {
        return;
    }

    let price_span = (max_price - min_price).max(f64::EPSILON);
    let denom = (samples.len().saturating_sub(1)).max(1) as f64;
    let width_f = (area.width - 1) as f64;

    let buffer = frame.buffer_mut();

    for (idx, sample) in samples.iter().enumerate() {
        let rel_x = if samples.len() == 1 {
            0.0
        } else {
            idx as f64 / denom
        };
        let mut x = area.x + (rel_x * width_f).round() as u16;
        if x >= area.x + area.width {
            x = area.x + area.width - 1;
        }

        let style = if sample.close >= sample.open {
            Style::default().fg(Color::Green)
        } else {
            Style::default().fg(Color::Red)
        };

        let mut y_high = price_to_y(sample.high, min_price, price_span, area);
        let mut y_low = price_to_y(sample.low, min_price, price_span, area);
        if y_high > y_low {
            std::mem::swap(&mut y_high, &mut y_low);
        }

        for y in y_high..=y_low {
            if within(area, x, y) {
                buffer.get_mut(x, y).set_style(style).set_symbol("│");
            }
        }

        let mut y_open = price_to_y(sample.open, min_price, price_span, area);
        let mut y_close = price_to_y(sample.close, min_price, price_span, area);
        if y_open > y_close {
            std::mem::swap(&mut y_open, &mut y_close);
        }

        let x_right = x.saturating_add(1);

        if y_open == y_close {
            if within(area, x, y_open) {
                buffer.get_mut(x, y_open).set_style(style).set_symbol("─");
            }
            if within(area, x_right, y_open) {
                buffer
                    .get_mut(x_right, y_open)
                    .set_style(style)
                    .set_symbol("─");
            }
        } else {
            for y in y_open..=y_close {
                if within(area, x, y) {
                    buffer.get_mut(x, y).set_style(style).set_symbol("█");
                }
                if within(area, x_right, y) {
                    buffer.get_mut(x_right, y).set_style(style).set_symbol("█");
                }
            }
        }
    }
}

fn price_to_y(price: f64, min_price: f64, price_span: f64, area: Rect) -> u16 {
    if area.height <= 1 {
        return area.y;
    }
    let normalized = ((price - min_price) / price_span).clamp(0.0, 1.0);
    let offset = ((1.0 - normalized) * (area.height - 1) as f64).round() as u16;
    area.y + offset.min(area.height - 1)
}

fn within(area: Rect, x: u16, y: u16) -> bool {
    x >= area.x && x < area.x + area.width && y >= area.y && y < area.y + area.height
}

fn format_price_label(value: f64, width: u16) -> String {
    let mut label = format!("{:.2}", value);
    let max_len = width as usize;
    if max_len > 0 && label.len() > max_len {
        label.truncate(max_len);
    }
    label
}

fn format_candle_date(timestamp_ms: u64) -> String {
    let ts = timestamp_ms as i64;
    if let Some(datetime) = DateTime::<Utc>::from_timestamp_millis(ts) {
        datetime.format("%Y-%m-%d").to_string()
    } else {
        "-".to_string()
    }
}

fn render_price_trend_placeholder(frame: &mut Frame<'_>, area: Rect) {
    frame.render_widget(
        Paragraph::new("Collecting price data...")
            .style(Style::default().fg(Color::Gray))
            .alignment(ratatui::layout::Alignment::Center),
        area,
    );
}

fn render_logs(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &AppState,
    render_state: &crate::ui::ui_manager::RenderState,
) {
    let total_logs = app.log_messages.len();
    let max_offset = total_logs.saturating_sub(1);
    let clamped_offset = app.log_scroll_offset.min(max_offset);

    let block = if clamped_offset > 0 {
        Block::default()
            .title(format!(" Logs (older +{clamped_offset}) "))
            .borders(Borders::ALL)
    } else {
        Block::default().title(" Logs ").borders(Borders::ALL)
    };
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height == 0 {
        return;
    }

    let viewport_height = inner.height as usize;
    let mut items: Vec<ListItem> = Vec::new();
    let mut rows_remaining = viewport_height;

    if clamped_offset == 0 {
        if let Some(error) = &render_state.error_message {
            items.push(ListItem::new(Span::styled(
                error.clone(),
                Style::default().fg(Color::Red),
            )));
            rows_remaining = rows_remaining.saturating_sub(1);
        } else if let Some(info) = &render_state.info_message {
            items.push(ListItem::new(Span::styled(
                info.clone(),
                Style::default().fg(Color::LightBlue),
            )));
            rows_remaining = rows_remaining.saturating_sub(1);
        }
    }

    if rows_remaining > 0 && total_logs > 0 {
        let end_index = total_logs.saturating_sub(clamped_offset);
        let start_index = end_index.saturating_sub(rows_remaining);

        for msg in app
            .log_messages
            .iter()
            .skip(start_index)
            .take(rows_remaining)
        {
            items.push(ListItem::new(Span::raw(msg.clone())));
        }
    }

    if items.is_empty() {
        items.push(ListItem::new(Span::styled(
            "No log messages yet",
            Style::default().fg(Color::Gray),
        )));
    }

    let list = List::new(items).style(Style::default());
    frame.render_widget(list, inner);
}

fn render_alert_popup(frame: &mut Frame<'_>, _area: Rect, app: &AppState) {
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

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    let vertical_chunk = popup_layout[1];

    let horizontal_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical_chunk);

    horizontal_layout[1]
}

fn render_command_palette(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &AppState,
    render_state: &crate::ui::ui_manager::RenderState,
) {
    if matches!(app.input_mode, InputMode::AlertPopup) {
        render_alert_popup(frame, area, app);
        return;
    }

    let title = match app.input_mode {
        InputMode::Normal => " Command Hints ",
        InputMode::Command => " Command Entry ",
        InputMode::AlertPopup => " Command Hints ",
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
