//! Terminal User Interface implementation
//!
//! Provides the main TUI interface using ratatui.

mod input;
mod render;

use std::io::{Stdout, stdout};

use crossterm::{
    cursor,
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

use super::AppState;
use crate::AppResult;
use crate::session::alert_manager::{AlertDirection, AlertOptions};
use crate::session::session_manager::SessionStats;
use crate::ui::ui_manager::RenderState;

pub use input::handle_key_event;
use render::render_root;

/// Actions generated from key handling
pub enum UiAction {
    None,
    SubmitCommand(String),
    SubmitAlert {
        symbol: String,
        direction: AlertDirection,
        price: f64,
        options: AlertOptions,
    },
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
        render_state: &RenderState,
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

/// Render the main TUI layout
pub fn render_app(
    app: &mut AppState,
    render_state: &RenderState,
    session_stats: &SessionStats,
    orderbook_depth: usize,
) -> AppResult<()> {
    let mut tui = Tui::new()?;
    tui.draw(app, render_state, session_stats, orderbook_depth)?;
    tui.restore()?;
    Ok(())
}
