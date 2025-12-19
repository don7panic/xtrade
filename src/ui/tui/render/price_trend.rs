use chrono::{DateTime, Utc};
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::ui::{AppState, CandleSample};

pub(super) fn render_price_trend(frame: &mut Frame<'_>, area: Rect, app: &mut AppState) {
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
    samples: &[CandleSample],
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
