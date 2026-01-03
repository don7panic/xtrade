use anyhow::Result;
use tokio::sync::mpsc;
use tracing::{error, info};

use crate::notify::SystemNotifier;

use crate::session::action_channel::{ActionChannel, SessionEvent};
use crate::session::alert_manager::{
    AlertDirection, AlertManager, AlertOptions, AlertRepeat, AlertTrigger,
};
use crate::session::command_router::{AlertAction, ClearTarget};

pub(super) struct AlertingState {
    manager: AlertManager,
    notifier: SystemNotifier,
}

impl AlertingState {
    pub(super) fn new(app_name: &str) -> Self {
        Self {
            manager: AlertManager::default(),
            notifier: SystemNotifier::new(app_name),
        }
    }

    pub(super) fn handle_action(
        &mut self,
        action: AlertAction,
        enable_tui: bool,
        ui_event_tx: Option<&mpsc::UnboundedSender<SessionEvent>>,
        action_channel: &ActionChannel,
    ) -> Result<()> {
        match action {
            AlertAction::List => {
                let alerts = self.manager.list_alerts();
                let mut entries = Vec::new();
                if alerts.is_empty() {
                    entries.push("No alerts configured.".to_string());
                } else {
                    for alert in &alerts {
                        let status = if alert.triggered {
                            "triggered"
                        } else {
                            "armed"
                        };
                        let mode = match alert.repeat {
                            AlertRepeat::Once => "once".to_string(),
                            AlertRepeat::Repeat => {
                                if alert.cooldown_ms > 0 {
                                    format!("repeat/{}s", alert.cooldown_ms / 1_000)
                                } else {
                                    "repeat".to_string()
                                }
                            }
                        };
                        let cooldown = if alert.cooldown_ms > 0 {
                            format!("{}s", alert.cooldown_ms / 1_000)
                        } else {
                            "0".to_string()
                        };
                        let hysteresis = if alert.hysteresis > 0.0 {
                            format!("{:.4}", alert.hysteresis)
                        } else {
                            "0".to_string()
                        };
                        entries.push(format!(
                            "#{} {} {:?} {} ({}, mode={}, cooldown={}, hysteresis={})",
                            alert.id,
                            alert.symbol,
                            alert.direction,
                            alert.threshold,
                            status,
                            mode,
                            cooldown,
                            hysteresis
                        ));
                    }
                }

                if enable_tui {
                    Self::forward_to_ui(ui_event_tx, SessionEvent::AlertList { entries });
                    Self::forward_to_ui(ui_event_tx, SessionEvent::AlertSnapshot { alerts });
                } else {
                    for entry in &entries {
                        println!("{}", entry);
                    }
                }
            }
            AlertAction::Clear { target } => match target {
                ClearTarget::All => {
                    let removed = self.manager.clear_all();
                    let message = format!("Cleared {} alerts", removed);
                    self.emit_notification(enable_tui, ui_event_tx, message);
                    self.send_snapshot(enable_tui, ui_event_tx);
                }
                ClearTarget::Id(id) => {
                    if self.manager.clear_alert(id) {
                        let message = format!("Cleared alert #{}", id);
                        self.emit_notification(enable_tui, ui_event_tx, message);
                        self.send_snapshot(enable_tui, ui_event_tx);
                    } else {
                        let message = format!("Alert #{} not found", id);
                        action_channel.send_event(SessionEvent::Error { message })?;
                    }
                }
            },
        }

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn add_from_ui(
        &mut self,
        symbol: String,
        direction: AlertDirection,
        price: f64,
        options: AlertOptions,
        enable_tui: bool,
        ui_event_tx: Option<&mpsc::UnboundedSender<SessionEvent>>,
        action_channel: &ActionChannel,
    ) -> Result<()> {
        match self
            .manager
            .add_alert_with_options(symbol, direction, price, options)
        {
            Ok(alert) => {
                let mode = match alert.repeat {
                    AlertRepeat::Once => "once".to_string(),
                    AlertRepeat::Repeat => {
                        if alert.cooldown_ms > 0 {
                            format!("repeat/{}s", alert.cooldown_ms / 1_000)
                        } else {
                            "repeat".to_string()
                        }
                    }
                };
                let cooldown = if alert.cooldown_ms > 0 {
                    format!("{}s", alert.cooldown_ms / 1_000)
                } else {
                    "0".to_string()
                };
                let hysteresis = if alert.hysteresis > 0.0 {
                    format!("{:.4}", alert.hysteresis)
                } else {
                    "0".to_string()
                };
                let message = format!(
                    "Alert #{} added: {} {:?} {} (mode={}, cooldown={}, hysteresis={})",
                    alert.id,
                    alert.symbol,
                    alert.direction,
                    alert.threshold,
                    mode,
                    cooldown,
                    hysteresis
                );
                self.emit_notification(enable_tui, ui_event_tx, message);
                self.send_snapshot(enable_tui, ui_event_tx);
            }
            Err(e) => {
                let message = format!("Failed to add alert: {}", e);
                action_channel.send_event(SessionEvent::Error { message })?;
            }
        }

        Ok(())
    }

    pub(super) fn evaluate(
        &mut self,
        symbol: &str,
        price: f64,
        enable_tui: bool,
        ui_event_tx: Option<&mpsc::UnboundedSender<SessionEvent>>,
    ) -> Result<()> {
        let normalized = symbol.to_ascii_uppercase();
        let (triggers, state_changed) = self.manager.evaluate_price(&normalized, price);

        for trigger in triggers {
            let direction_str = match trigger.direction {
                AlertDirection::Above => "above",
                AlertDirection::Below => "below",
            };
            let message = format!(
                "Alert #{} triggered: {} {} {} (price {})",
                trigger.id, trigger.symbol, direction_str, trigger.threshold, trigger.price
            );
            self.emit_notification(enable_tui, ui_event_tx, message);
            self.send_price_trigger_notification(&trigger);
        }

        if state_changed {
            self.send_snapshot(enable_tui, ui_event_tx);
        }

        Ok(())
    }

    fn emit_notification(
        &self,
        enable_tui: bool,
        ui_event_tx: Option<&mpsc::UnboundedSender<SessionEvent>>,
        message: String,
    ) {
        info!("{}", message);

        if enable_tui {
            Self::forward_to_ui(ui_event_tx, SessionEvent::AlertNotification { message });
        } else {
            println!("{}", message);
        }
    }

    fn send_price_trigger_notification(&self, trigger: &AlertTrigger) {
        let direction = match trigger.direction {
            AlertDirection::Above => "above",
            AlertDirection::Below => "below",
        };
        let title = format!("{} price alert", trigger.symbol);
        let body = format!(
            "Price {} {:.4} (last {})",
            direction, trigger.threshold, trigger.price
        );
        self.notifier.notify(title, body);
    }

    fn send_snapshot(
        &self,
        enable_tui: bool,
        ui_event_tx: Option<&mpsc::UnboundedSender<SessionEvent>>,
    ) {
        if enable_tui {
            let alerts = self.manager.list_alerts();
            Self::forward_to_ui(ui_event_tx, SessionEvent::AlertSnapshot { alerts });
        }
    }

    fn forward_to_ui(
        ui_event_tx: Option<&mpsc::UnboundedSender<SessionEvent>>,
        event: SessionEvent,
    ) {
        if let Some(tx) = ui_event_tx {
            if let Err(e) = tx.send(event) {
                error!("Failed to forward event to UI: {}", e);
            }
        }
    }
}
