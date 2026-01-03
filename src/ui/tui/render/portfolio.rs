use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table, Wrap};

use crate::paper_trading::Decimal;
use crate::ui::AppState;

pub(super) fn render_portfolio(frame: &mut Frame<'_>, area: Rect, app: &AppState) {
    let block = Block::default().title(" Portfolio ").borders(Borders::ALL);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width < 20 || inner.height < 4 {
        frame.render_widget(
            Paragraph::new("Portfolio")
                .style(Style::default().fg(Color::Gray))
                .alignment(Alignment::Center),
            inner,
        );
        return;
    }

    let mut positions: Vec<_> = app
        .paper_portfolio
        .positions
        .values()
        .filter(|position| !position.is_empty())
        .collect();
    positions.sort_by(|a, b| a.symbol.cmp(&b.symbol));

    if positions.is_empty() {
        frame.render_widget(
            Paragraph::new("No positions yet. Use /buy <symbol> <qty>")
                .style(Style::default().fg(Color::Gray))
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: true }),
            inner,
        );
        return;
    }

    let summary_height = if inner.height >= 7 { 3 } else { 0 };
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(summary_height)])
        .split(inner);

    let header = Row::new(["Symbol", "Qty", "Avg", "Last", "PnL", "PnL%"]).style(
        Style::default()
            .fg(Color::Gray)
            .add_modifier(Modifier::BOLD),
    );
    let widths = [
        Constraint::Length(9),
        Constraint::Length(10),
        Constraint::Length(10),
        Constraint::Length(10),
        Constraint::Length(10),
        Constraint::Length(8),
    ];

    let rows = positions.into_iter().map(|position| {
        let pnl_style = pnl_style(position.unrealized_pnl);
        Row::new(vec![
            Cell::from(position.symbol.clone()),
            Cell::from(format_decimal(position.quantity, 6)),
            Cell::from(format_decimal(position.avg_cost, 2)),
            Cell::from(format_decimal(position.current_price, 2)),
            Cell::from(format_signed_decimal(position.unrealized_pnl, 2)).style(pnl_style),
            Cell::from(format!(
                "{}%",
                format_signed_decimal(position.unrealized_pnl_pct, 2)
            ))
            .style(pnl_style),
        ])
    });

    let table = Table::new(rows, widths).header(header).column_spacing(1);
    frame.render_widget(table, layout[0]);

    if summary_height > 0 {
        let open_positions = app.paper_portfolio.open_position_count();
        let cost_basis = app.paper_portfolio.total_cost_basis();
        let market_value = app.paper_portfolio.total_market_value();
        let unrealized = app.paper_portfolio.total_unrealized_pnl();
        let realized = app.paper_portfolio.realized_pnl;
        let unrealized_style = pnl_style(unrealized);
        let realized_style = pnl_style(realized);

        let summary_lines = vec![
            Line::from(vec![
                Span::raw(format!("Positions: {}", open_positions)),
                Span::raw("  "),
                Span::raw(format!("Cost: {}", format_decimal(cost_basis, 2))),
                Span::raw("  "),
                Span::raw(format!("Value: {}", format_decimal(market_value, 2))),
            ]),
            Line::from(vec![
                Span::raw("Unrealized: "),
                Span::styled(format_signed_decimal(unrealized, 2), unrealized_style),
                Span::raw("  "),
                Span::raw("Realized: "),
                Span::styled(format_signed_decimal(realized, 2), realized_style),
            ]),
        ];

        frame.render_widget(
            Paragraph::new(summary_lines).wrap(Wrap { trim: true }),
            layout[1],
        );
    }
}

fn format_decimal(value: Decimal, scale: u32) -> String {
    value.round_dp(scale).to_string()
}

fn format_signed_decimal(value: Decimal, scale: u32) -> String {
    let rounded = value.round_dp(scale);
    if rounded.is_sign_negative() || rounded.is_zero() {
        rounded.to_string()
    } else {
        format!("+{}", rounded)
    }
}

fn pnl_style(value: Decimal) -> Style {
    if value.is_sign_negative() {
        Style::default().fg(Color::Red)
    } else if value.is_zero() {
        Style::default()
    } else {
        Style::default().fg(Color::Green)
    }
}
