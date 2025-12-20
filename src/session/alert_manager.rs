//! In-memory alert manager for price threshold alerts

use anyhow::{Result, anyhow};
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_ALERTS: usize = 50;
const DEFAULT_ALERT_COOLDOWN_MS: u64 = 0;
const DEFAULT_ALERT_HYSTERESIS_PCT: f64 = 0.0;

/// Re-trigger behavior for alerts
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertRepeat {
    Once,
    Repeat,
}

/// Additional alert options for re-trigger and noise control
#[derive(Debug, Clone, Copy)]
pub struct AlertOptions {
    pub repeat: AlertRepeat,
    pub cooldown_ms: u64,
    pub hysteresis: f64,
}

impl AlertOptions {
    pub fn default_for_threshold(threshold: f64) -> Self {
        let hysteresis = if DEFAULT_ALERT_HYSTERESIS_PCT > 0.0 && threshold.is_finite() {
            threshold * (DEFAULT_ALERT_HYSTERESIS_PCT / 100.0)
        } else {
            0.0
        };
        Self {
            repeat: AlertRepeat::Repeat,
            cooldown_ms: DEFAULT_ALERT_COOLDOWN_MS,
            hysteresis,
        }
    }
}

/// Direction for price threshold comparison
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertDirection {
    Above,
    Below,
}

/// Alert definition
#[derive(Debug, Clone)]
pub struct Alert {
    pub id: u64,
    pub symbol: String,
    pub direction: AlertDirection,
    pub threshold: f64,
    pub triggered: bool,
    pub last_price: Option<f64>,
    pub created_at_ms: u64,
    pub repeat: AlertRepeat,
    pub cooldown_ms: u64,
    pub hysteresis: f64,
    pub last_notified_ms: Option<u64>,
}

/// Trigger information returned when an alert fires
#[derive(Debug, Clone, PartialEq)]
pub struct AlertTrigger {
    pub id: u64,
    pub symbol: String,
    pub direction: AlertDirection,
    pub threshold: f64,
    pub price: f64,
}

/// Manages session-scoped alerts
pub struct AlertManager {
    alerts: Vec<Alert>,
    next_id: u64,
}

impl AlertManager {
    pub fn new() -> Self {
        Self {
            alerts: Vec::new(),
            next_id: 1,
        }
    }

    /// Add a new alert after basic validation and normalization
    pub fn add_alert(
        &mut self,
        symbol: impl Into<String>,
        direction: AlertDirection,
        threshold: f64,
    ) -> Result<Alert> {
        let options = AlertOptions::default_for_threshold(threshold);
        self.add_alert_with_options(symbol, direction, threshold, options)
    }

    /// Add a new alert with explicit options
    pub fn add_alert_with_options(
        &mut self,
        symbol: impl Into<String>,
        direction: AlertDirection,
        threshold: f64,
        options: AlertOptions,
    ) -> Result<Alert> {
        if self.alerts.len() >= MAX_ALERTS {
            return Err(anyhow!(
                "Maximum alert limit ({}) reached. Clear alerts before adding more.",
                MAX_ALERTS
            ));
        }

        if !threshold.is_finite() || threshold <= 0.0 {
            return Err(anyhow!("Threshold must be a positive, finite number"));
        }
        if !options.hysteresis.is_finite() || options.hysteresis < 0.0 {
            return Err(anyhow!("Hysteresis must be a non-negative, finite number"));
        }

        let symbol = symbol.into().to_ascii_uppercase();
        if symbol.trim().is_empty() {
            return Err(anyhow!("Symbol cannot be empty"));
        }

        let alert = Alert {
            id: self.next_id,
            symbol,
            direction,
            threshold,
            triggered: false,
            last_price: None,
            created_at_ms: now_ms(),
            repeat: options.repeat,
            cooldown_ms: options.cooldown_ms,
            hysteresis: options.hysteresis,
            last_notified_ms: None,
        };
        self.next_id += 1;
        self.alerts.push(alert.clone());
        Ok(alert)
    }

    /// Return a snapshot of current alerts
    pub fn list_alerts(&self) -> Vec<Alert> {
        self.alerts.clone()
    }

    /// Clear a single alert by id
    pub fn clear_alert(&mut self, id: u64) -> bool {
        let len_before = self.alerts.len();
        self.alerts.retain(|alert| alert.id != id);
        len_before != self.alerts.len()
    }

    /// Clear all alerts, returning the number removed
    pub fn clear_all(&mut self) -> usize {
        let removed = self.alerts.len();
        self.alerts.clear();
        removed
    }

    /// Evaluate alerts for a symbol against the latest price and return any triggers
    pub fn evaluate_price(&mut self, symbol: &str, price: f64) -> (Vec<AlertTrigger>, bool) {
        if !price.is_finite() {
            return (Vec::new(), false);
        }

        let now = now_ms();
        let mut triggers = Vec::new();
        let mut state_changed = false;

        for alert in self.alerts.iter_mut().filter(|a| a.symbol == symbol) {
            let was_triggered = alert.triggered;
            let previous_price = alert.last_price;
            let should_notify = |alert: &Alert| match alert.repeat {
                AlertRepeat::Once => alert.last_notified_ms.is_none(),
                AlertRepeat::Repeat => alert
                    .last_notified_ms
                    .map(|last| now.saturating_sub(last) >= alert.cooldown_ms)
                    .unwrap_or(true),
            };

            match alert.direction {
                AlertDirection::Above => {
                    if alert.triggered {
                        if matches!(alert.repeat, AlertRepeat::Repeat)
                            && price <= alert.threshold - alert.hysteresis
                        {
                            alert.triggered = false; // re-arm
                        }
                    } else if price >= alert.threshold
                        && previous_price.map_or(true, |prev| prev < alert.threshold)
                    {
                        alert.triggered = true;
                        if should_notify(alert) {
                            triggers.push(AlertTrigger {
                                id: alert.id,
                                symbol: alert.symbol.clone(),
                                direction: alert.direction,
                                threshold: alert.threshold,
                                price,
                            });
                            alert.last_notified_ms = Some(now);
                        }
                    }
                }
                AlertDirection::Below => {
                    if alert.triggered {
                        if matches!(alert.repeat, AlertRepeat::Repeat)
                            && price >= alert.threshold + alert.hysteresis
                        {
                            alert.triggered = false; // re-arm
                        }
                    } else if price <= alert.threshold
                        && previous_price.map_or(true, |prev| prev > alert.threshold)
                    {
                        alert.triggered = true;
                        if should_notify(alert) {
                            triggers.push(AlertTrigger {
                                id: alert.id,
                                symbol: alert.symbol.clone(),
                                direction: alert.direction,
                                threshold: alert.threshold,
                                price,
                            });
                            alert.last_notified_ms = Some(now);
                        }
                    }
                }
            }

            alert.last_price = Some(price);

            if alert.triggered != was_triggered {
                state_changed = true;
            }
        }

        (triggers, state_changed)
    }
}

impl Default for AlertManager {
    fn default() -> Self {
        Self::new()
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
