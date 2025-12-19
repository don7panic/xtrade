use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::metrics::ConnectionStatus as MetricsConnectionStatus;
use crate::session::session_manager::SessionStats;
use crate::ui::AppState;

pub(super) fn render_header(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &AppState,
    session_stats: &SessionStats,
) {
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
