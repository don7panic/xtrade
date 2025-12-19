use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Cell, Row, Table};

use crate::ui::{AppState, MarketDataState};

pub(super) fn render_symbol_overview(frame: &mut Frame<'_>, area: Rect, app: &AppState) {
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
