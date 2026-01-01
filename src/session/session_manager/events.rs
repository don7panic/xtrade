use anyhow::Result;
use tracing::{debug, error};

use crate::market_data::MarketEvent;

use super::SessionManager;
use crate::session::action_channel::SessionEvent;

impl SessionManager {
    /// Handle session event
    pub(super) async fn handle_event(&mut self, event: SessionEvent) -> Result<()> {
        debug!("Handling session event: {:?}", event);

        self.stats.events_processed += 1;

        match event {
            SessionEvent::ShutdownRequested => {
                self.forward_to_ui(SessionEvent::ShutdownRequested);
                self.shutdown().await?;
            }
            SessionEvent::Error { message } => {
                error!("Session error: {}", message);
                self.stats.errors_encountered += 1;
                self.forward_to_ui(SessionEvent::Error { message });
            }
            SessionEvent::AlertAdd {
                symbol,
                direction,
                price,
                options,
            } => {
                let enable_tui = self.config.enable_tui;
                let ui_event_tx = self.ui_event_tx.as_ref();
                self.alerts.add_from_ui(
                    symbol,
                    direction,
                    price,
                    options,
                    enable_tui,
                    ui_event_tx,
                    &self.action_channel,
                )?;
            }
            SessionEvent::UserCommand { command } => {
                self.handle_command(command).await?;
            }
            SessionEvent::MarketEvent(market_event) => {
                self.handle_market_event(market_event).await?;
            }
            other => {
                self.forward_to_ui(other);
            }
        }

        Ok(())
    }

    /// Handle market event
    pub(super) async fn handle_market_event(&mut self, event: MarketEvent) -> Result<()> {
        debug!("Handling market event: {:?}", event);

        if let Some((symbol, price)) = self.paper_trading.handle_market_event(&event) {
            let enable_tui = self.config.enable_tui;
            let ui_event_tx = self.ui_event_tx.as_ref();
            self.alerts
                .evaluate(&symbol, price, enable_tui, ui_event_tx)?;
        }

        if let Some(ui_event_tx) = &self.ui_event_tx {
            if let Err(e) = ui_event_tx.send(SessionEvent::MarketEvent(event.clone())) {
                error!("Failed to send market event to UI: {}", e);
            }
        }

        self.metrics
            .handle_market_event(&event, &self.action_channel)
            .await?;

        Ok(())
    }
}
