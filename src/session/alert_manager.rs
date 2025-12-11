//! In-memory alert manager for price threshold alerts

use anyhow::{Result, anyhow};
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_ALERTS: usize = 50;

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
        if self.alerts.len() >= MAX_ALERTS {
            return Err(anyhow!(
                "Maximum alert limit ({}) reached. Clear alerts before adding more.",
                MAX_ALERTS
            ));
        }

        if !threshold.is_finite() || threshold <= 0.0 {
            return Err(anyhow!("Threshold must be a positive, finite number"));
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
    pub fn evaluate_price(&mut self, symbol: &str, price: f64) -> Vec<AlertTrigger> {
        if !price.is_finite() {
            return Vec::new();
        }

        let mut triggers = Vec::new();

        for alert in self.alerts.iter_mut().filter(|a| a.symbol == symbol) {
            let previous_price = alert.last_price;

            match alert.direction {
                AlertDirection::Above => {
                    if alert.triggered {
                        if price < alert.threshold {
                            alert.triggered = false; // re-arm
                        }
                    } else if price >= alert.threshold
                        && previous_price.map_or(true, |prev| prev < alert.threshold)
                    {
                        alert.triggered = true;
                        triggers.push(AlertTrigger {
                            id: alert.id,
                            symbol: alert.symbol.clone(),
                            direction: alert.direction,
                            threshold: alert.threshold,
                            price,
                        });
                    }
                }
                AlertDirection::Below => {
                    if alert.triggered {
                        if price > alert.threshold {
                            alert.triggered = false; // re-arm
                        }
                    } else if price <= alert.threshold
                        && previous_price.map_or(true, |prev| prev > alert.threshold)
                    {
                        alert.triggered = true;
                        triggers.push(AlertTrigger {
                            id: alert.id,
                            symbol: alert.symbol.clone(),
                            direction: alert.direction,
                            threshold: alert.threshold,
                            price,
                        });
                    }
                }
            }

            alert.last_price = Some(price);
        }

        triggers
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_and_lists_alerts() {
        let mut mgr = AlertManager::new();
        let alert = mgr
            .add_alert("btcusdt", AlertDirection::Above, 45_000.0)
            .expect("should add");
        assert_eq!(alert.id, 1);
        let alerts = mgr.list_alerts();
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].symbol, "BTCUSDT");
    }

    #[test]
    fn triggers_on_crossing_above_and_rearms() {
        let mut mgr = AlertManager::new();
        mgr.add_alert("ETHUSDT", AlertDirection::Above, 2000.0)
            .unwrap();

        // No trigger below threshold
        assert!(mgr.evaluate_price("ETHUSDT", 1995.0).is_empty());
        // Trigger on crossing above
        let triggers = mgr.evaluate_price("ETHUSDT", 2001.0);
        assert_eq!(triggers.len(), 1);
        assert_eq!(triggers[0].id, 1);
        // Stay above should not re-trigger
        assert!(mgr.evaluate_price("ETHUSDT", 2005.0).is_empty());
        // Drop below to re-arm
        assert!(mgr.evaluate_price("ETHUSDT", 1990.0).is_empty());
        // Cross above again should trigger
        let triggers = mgr.evaluate_price("ETHUSDT", 2005.0);
        assert_eq!(triggers.len(), 1);
    }

    #[test]
    fn triggers_on_crossing_below_and_rearms() {
        let mut mgr = AlertManager::new();
        mgr.add_alert("BTCUSDT", AlertDirection::Below, 30_000.0)
            .unwrap();

        assert!(mgr.evaluate_price("BTCUSDT", 30_100.0).is_empty());
        let triggers = mgr.evaluate_price("BTCUSDT", 29_999.0);
        assert_eq!(triggers.len(), 1);
        assert_eq!(
            triggers[0],
            AlertTrigger {
                id: 1,
                symbol: "BTCUSDT".to_string(),
                direction: AlertDirection::Below,
                threshold: 30_000.0,
                price: 29_999.0
            }
        );
        assert!(mgr.evaluate_price("BTCUSDT", 29_500.0).is_empty());
        assert!(mgr.evaluate_price("BTCUSDT", 30_500.0).is_empty()); // re-arm
        let retrigger = mgr.evaluate_price("BTCUSDT", 29_500.0);
        assert_eq!(retrigger.len(), 1);
    }

    #[test]
    fn enforce_max_alerts() {
        let mut mgr = AlertManager::new();
        for i in 0..MAX_ALERTS {
            mgr.add_alert(format!("SYM{}", i), AlertDirection::Above, 1.0)
                .unwrap();
        }
        let err = mgr
            .add_alert("OVER", AlertDirection::Above, 1.0)
            .unwrap_err();
        assert!(err.to_string().contains("Maximum alert limit"));
    }
}
