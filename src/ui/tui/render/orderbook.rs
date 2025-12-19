use ordered_float::OrderedFloat;
use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table};

use crate::ui::AppState;

pub(super) fn render_orderbook(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &AppState,
    orderbook_depth: usize,
) {
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
