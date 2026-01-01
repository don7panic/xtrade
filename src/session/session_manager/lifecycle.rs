use anyhow::Result;
use tracing::{info, warn};

use super::SessionManager;
use crate::session::action_channel::SessionEvent;
use crate::session::command_router::CommandRouter;

impl SessionManager {
    /// Initialize the session
    pub async fn initialize(&mut self) -> Result<()> {
        info!("Initializing interactive session");

        if self.config.enable_tui {
            self.initialize_ui().await?;
        }

        if self.config.enable_metrics {
            self.metrics.initialize()?;
        }

        if self.config.auto_subscribe {
            self.spawn_auto_subscribe_symbols();
        }

        if self.config.enable_tui {
            let help_lines = CommandRouter::help_messages()
                .iter()
                .map(|line| (*line).to_string())
                .collect();
            self.forward_to_ui(SessionEvent::HelpInfo { lines: help_lines });
        }

        self.state = super::SessionState::Running;
        info!("Session initialized successfully");

        Ok(())
    }

    /// Start the session using the appropriate execution mode
    pub async fn start(&mut self) -> Result<()> {
        if self.cli.is_dry_run_mode() {
            return self.run_dry_run_mode().await;
        }

        if !self.config.enable_tui {
            self.display_welcome_page().await?;
        } else {
            info!("TUI mode enabled, deferring welcome message to UI");
        }

        self.initialize().await?;

        self.run().await
    }

    async fn run_dry_run_mode(&mut self) -> Result<()> {
        info!("Running in dry-run mode - showing welcome page and configuration");

        self.state = super::SessionState::Running;

        self.display_welcome_page().await?;
        self.print_dry_run_summary()?;

        info!("Dry-run mode completed");
        Ok(())
    }

    fn print_dry_run_summary(&self) -> Result<()> {
        println!();
        println!("Dry-run mode configuration:");
        println!("Config file: {}", self.cli.config_file);
        println!("Log level: {}", self.cli.effective_log_level());
        self.app_config.display_summary()
    }

    /// Run the main session loop
    pub async fn run(&mut self) -> Result<()> {
        info!("Starting interactive session loop");

        if self.config.enable_metrics {
            self.metrics.spawn_collector();
        }

        let mut shutdown_rx = self.shutdown_rx.take().unwrap();
        let market_event_rx = self.market_manager.event_receiver();

        while self.state != super::SessionState::Terminated {
            let market_event_rx = market_event_rx.clone();
            tokio::select! {
                _ = shutdown_rx.recv() => {
                    info!("Received shutdown signal");
                    self.shutdown().await?;
                }

                Some(command) = self.command_router.next_command() => {
                    self.handle_command(command).await?;
                }

                Some(event) = self.action_channel.next_event() => {
                    self.handle_event(event).await?;
                }

                _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => {
                    self.check_timeout().await?;
                }

                market_event = async {
                    let mut receiver = market_event_rx.lock().await;
                    receiver.recv().await
                } => {
                    match market_event {
                        Some(market_event) => {
                            self.market_manager.process_market_event(&market_event).await;
                            self.handle_market_event(market_event).await?;
                        }
                        None => {
                            warn!("Market event channel closed");
                            self.state = super::SessionState::Terminated;
                        }
                    }
                }
            }
        }

        info!("Session loop terminated");
        Ok(())
    }

    /// Check session timeout
    async fn check_timeout(&mut self) -> Result<()> {
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let session_duration = current_time - self.stats.start_time;

        if session_duration > self.config.session_timeout_ms {
            warn!(
                "Session timeout reached ({}ms), shutting down",
                session_duration
            );
            self.shutdown().await?;
        }

        Ok(())
    }

    /// Graceful shutdown
    pub async fn shutdown(&mut self) -> Result<()> {
        info!("Initiating graceful shutdown");

        self.state = super::SessionState::ShuttingDown;

        if let Some(ui_event_tx) = self.ui_event_tx.take() {
            if let Err(e) = ui_event_tx.send(SessionEvent::ShutdownRequested) {
                tracing::error!("Failed to notify UI of shutdown: {}", e);
            }
        }
        if let Some(ui_task) = self.ui_task.take() {
            if let Err(e) = ui_task.await {
                tracing::error!("UI task terminated with error: {}", e);
            }
        }

        self.metrics.shutdown().await?;

        let subscriptions = self.market_manager.list_subscriptions().await;

        for symbol in subscriptions {
            if let Err(e) = self.market_manager.unsubscribe(&symbol).await {
                tracing::error!(
                    "Failed to unsubscribe from {} during shutdown: {}",
                    symbol,
                    e
                );
            }
        }

        self.state = super::SessionState::Terminated;
        info!("Shutdown completed");

        Ok(())
    }
}
