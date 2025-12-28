use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Cell, Row, Table};

use crate::ui::{AppState, MarketDataState};

pub(super) fn render_symbol_overview(frame: &mut Frame<'_>, area: Rect, app: &AppState) {
    let block = Block::default().title(" Markets ").borders(Borders::ALL);

    let is_perp = matches!(app.market_type, crate::binance::types::MarketType::PerpUsdt);

    let (widths, header) = if is_perp {
        (
            [
                Constraint::Length(12),
                Constraint::Length(12),
                Constraint::Length(10),
                Constraint::Length(10),
                Constraint::Length(12), // Mark
                Constraint::Length(10), // Funding
                Constraint::Length(12), // OI
            ]
            .as_slice(),
            Row::new(["Symbol", "Price", "Δ%", "Vol", "Mark", "Fund%", "OI"]).style(
                Style::default()
                    .fg(Color::Gray)
                    .add_modifier(Modifier::BOLD),
            ),
        )
    } else {
        (
            [
                Constraint::Length(12),
                Constraint::Length(12),
                Constraint::Length(10),
                Constraint::Length(14),
            ]
            .as_slice(),
            Row::new(["Symbol", "Price", "Δ%", "Vol"]).style(
                Style::default()
                    .fg(Color::Gray)
                    .add_modifier(Modifier::BOLD),
            ),
        )
    };

    let rows: Vec<Row> = app
        .market_keys
        .iter()
        .enumerate()
        .map(|(idx, key)| {
            let default = MarketDataState {
                symbol: key.symbol.clone(),
                ..MarketDataState::default()
            };

            let data = app.market_data.get(key).unwrap_or(&default);
            let change_style = if data.change_percent > 0.0 {
                Style::default().fg(Color::Green)
            } else if data.change_percent < 0.0 {
                Style::default().fg(Color::Red)
            } else {
                Style::default()
            };

            let mut cells = vec![
                Cell::from(key.symbol.clone()),
                Cell::from(format!("{:.2}", data.price)),
                Cell::from(format!("{:+.2}", data.change_percent)).style(change_style),
                Cell::from(format!("{:.1}k", data.volume_24h / 1000.0)),
            ];

            if is_perp {
                let mark_str = data
                    .mark_price
                    .map(|p| format!("{:.2}", p))
                    .unwrap_or_else(|| "-".to_string());
                let fund_str = data
                    .funding_rate
                    .map(|r| format!("{:.4}%", r * 100.0))
                    .unwrap_or_else(|| "-".to_string());
                let oi_str = data
                    .open_interest
                    .map(|oi| format!("{:.0}", oi))
                    .unwrap_or_else(|| "-".to_string());

                cells.push(Cell::from(mark_str));
                cells.push(Cell::from(fund_str));
                cells.push(Cell::from(oi_str));
            }

            let mut row = Row::new(cells);

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
