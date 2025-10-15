//! Terminal User Interface implementation
//!
//! Provides the main TUI interface using ratatui.

use std::io::{Stdout, stdout};

use crossterm::{
    cursor,
    event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ordered_float::OrderedFloat;
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{
        Block, Borders, Cell, Gauge, List, ListItem, Paragraph, Row, Sparkline, Table, Wrap,
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
        execute!(stdout, EnterAlternateScreen, cursor::Hide)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;

        Ok(Self { terminal })
    }

    /// Render the application
    pub fn draw(
        &mut self,
        app: &AppState,
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
        let _ = execute!(stdout, cursor::Show, LeaveAlternateScreen);
    }
}

/// Handle keyboard events for TUI, returning actions for the session manager
pub fn handle_key_event(app: &mut AppState, key_event: KeyEvent) -> UiAction {
    if key_event.kind == KeyEventKind::Release {
        return UiAction::None;
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
    }
}

fn handle_normal_mode_keys(app: &mut AppState, key_event: KeyEvent) -> UiAction {
    match key_event.code {
        KeyCode::Char('q') | KeyCode::Esc => {
            app.should_quit = true;
            UiAction::QuitRequested
        }
        KeyCode::Char('/') | KeyCode::Char(':') => {
            app.input_mode = InputMode::Command;
            app.clear_command();
            if matches!(key_event.code, KeyCode::Char('/')) {
                app.command_buffer.push('/');
            }
            UiAction::None
        }
        KeyCode::Char('p') | KeyCode::Char(' ') => {
            app.toggle_pause();
            UiAction::None
        }
        KeyCode::Left | KeyCode::Char('h') => {
            app.previous_tab();
            UiAction::None
        }
        KeyCode::Right | KeyCode::Char('l') | KeyCode::Tab => {
            app.next_tab();
            UiAction::None
        }
        KeyCode::Up | KeyCode::Char('k') => {
            // Allow cycling notifications/logs later; no-op for now
            UiAction::None
        }
        KeyCode::Down | KeyCode::Char('j') => UiAction::None,
        KeyCode::Char('s') => {
            app.input_mode = InputMode::Command;
            app.command_buffer = "/status".to_string();
            UiAction::None
        }
        KeyCode::Char('L') if key_event.modifiers.contains(KeyModifiers::SHIFT) => {
            app.input_mode = InputMode::Command;
            app.command_buffer = "/logs".to_string();
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
            UiAction::None
        }
        KeyCode::Enter => {
            let command = app.command_buffer.trim().to_string();
            app.input_mode = InputMode::Normal;
            app.clear_command();
            if command.is_empty() {
                UiAction::None
            } else {
                UiAction::SubmitCommand(command)
            }
        }
        KeyCode::Backspace => {
            app.command_buffer.pop();
            UiAction::None
        }
        KeyCode::Char(c) => {
            if !key_event.modifiers.contains(KeyModifiers::CONTROL) {
                app.command_buffer.push(c);
            }
            UiAction::None
        }
        KeyCode::Left | KeyCode::Right | KeyCode::Tab => UiAction::None,
        KeyCode::Up | KeyCode::Down => UiAction::None,
        _ => UiAction::None,
    }
}

/// Render the main TUI layout
pub fn render_app(
    app: &AppState,
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
    app: &AppState,
    render_state: &crate::ui::ui_manager::RenderState,
    session_stats: &SessionStats,
    orderbook_depth: usize,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(7),
            Constraint::Length(3),
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
            .zip(ask_rows.into_iter())
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

fn render_metrics(frame: &mut Frame<'_>, area: Rect, app: &AppState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(7), Constraint::Min(4)])
        .split(area);

    render_latency_gauges(frame, chunks[0], app);
    render_price_sparkline(frame, chunks[1], app);
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

fn render_price_sparkline(frame: &mut Frame<'_>, area: Rect, app: &AppState) {
    let block = Block::default()
        .title(" Price Trend ")
        .borders(Borders::ALL);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if let Some(symbol) = app.current_symbol() {
        if let Some(data) = app.market_data.get(symbol) {
            let series: Vec<u64> = data
                .price_history
                .iter()
                .map(|price| (*price * 100.0) as u64)
                .collect();

            if !series.is_empty() {
                let sparkline = Sparkline::default()
                    .data(&series)
                    .style(Style::default().fg(Color::LightCyan));
                frame.render_widget(sparkline, inner);
                return;
            }
        }
    }

    frame.render_widget(
        Paragraph::new("Collecting price data...")
            .style(Style::default().fg(Color::Gray))
            .alignment(ratatui::layout::Alignment::Center),
        inner,
    );
}

fn render_logs(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &AppState,
    render_state: &crate::ui::ui_manager::RenderState,
) {
    let block = Block::default().title(" Logs ").borders(Borders::ALL);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut items: Vec<ListItem> = app
        .log_messages
        .iter()
        .rev()
        .take(5)
        .map(|msg| ListItem::new(Span::raw(msg.clone())))
        .collect();

    if let Some(error) = &render_state.error_message {
        items.insert(
            0,
            ListItem::new(Span::styled(error.clone(), Style::default().fg(Color::Red))),
        );
    } else if let Some(info) = &render_state.info_message {
        items.insert(
            0,
            ListItem::new(Span::styled(
                info.clone(),
                Style::default().fg(Color::LightBlue),
            )),
        );
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

fn render_command_palette(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &AppState,
    render_state: &crate::ui::ui_manager::RenderState,
) {
    let title = match app.input_mode {
        InputMode::Normal => " Command Hints ",
        InputMode::Command => " Command Entry ",
    };

    let block = Block::default().title(title).borders(Borders::ALL);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let text = if matches!(app.input_mode, InputMode::Command) {
        vec![Line::from(vec![
            Span::styled(">", Style::default().fg(Color::Cyan)),
            Span::raw(" "),
            Span::raw(app.command_buffer.clone()),
        ])]
    } else {
        let mut hints = vec![
            Span::styled("Tab", Style::default().fg(Color::Cyan)),
            Span::raw(": Next symbol   "),
            Span::styled("Shift+Tab", Style::default().fg(Color::Cyan)),
            Span::raw(": Prev symbol   "),
            Span::styled("/", Style::default().fg(Color::Cyan)),
            Span::raw(": Command mode   "),
            Span::styled("Shift+L", Style::default().fg(Color::Cyan)),
            Span::raw(": Logs command   "),
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

        vec![Line::from(hints)]
    };

    let paragraph = Paragraph::new(Text::from(text)).wrap(Wrap { trim: true });
    frame.render_widget(paragraph, inner);
}
