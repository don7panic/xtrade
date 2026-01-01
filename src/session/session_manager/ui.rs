use anyhow::Result;

use crate::ui::ui_manager::UIManager;

use super::SessionManager;

impl SessionManager {
    /// Display welcome page for interactive mode
    pub async fn display_welcome_page(&mut self) -> Result<()> {
        crate::ui::display_welcome_page().map_err(|e| anyhow::anyhow!(e))
    }

    /// Initialize UI manager
    pub(super) async fn initialize_ui(&mut self) -> Result<()> {
        tracing::info!("Initializing UI manager");

        let mut ui_manager = UIManager::new(
            self.market_manager.clone(),
            self.action_channel.event_tx(),
            self.app_config.clone(),
            self.get_market_type(),
        );

        let ui_event_tx = ui_manager.ui_event_sender();
        self.ui_event_tx = Some(ui_event_tx);

        self.ui_task = Some(tokio::spawn(async move {
            if let Err(e) = ui_manager.run().await {
                tracing::error!("UI manager error: {}", e);
            }
        }));

        Ok(())
    }
}
