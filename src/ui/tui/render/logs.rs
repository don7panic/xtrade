use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, Borders, List, ListItem};

use crate::ui::AppState;
use crate::ui::ui_manager::RenderState;

pub(super) fn render_logs(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &AppState,
    render_state: &RenderState,
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
