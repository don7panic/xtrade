use anyhow::Result;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tracing::error;

use crate::market_data::MarketEvent;
use crate::metrics::{ConnectionStatus as MetricsConnectionStatus, MetricsCollector};

use crate::session::action_channel::{ActionChannel, SessionEvent};

pub(super) struct MetricsState {
    collector: Option<Arc<Mutex<MetricsCollector>>>,
    status: MetricsConnectionStatus,
    last_emit: Instant,
    emit_interval: Duration,
}

impl MetricsState {
    pub(super) fn new() -> Self {
        Self {
            collector: None,
            status: MetricsConnectionStatus::Disconnected,
            last_emit: Instant::now(),
            emit_interval: Duration::from_millis(100),
        }
    }

    pub(super) fn initialize(&mut self) -> Result<()> {
        let metrics_collector = MetricsCollector::new(1000); // 1000 samples max
        self.collector = Some(Arc::new(Mutex::new(metrics_collector)));
        Ok(())
    }

    pub(super) fn set_emit_interval(&mut self, refresh_rate_ms: u64) {
        self.emit_interval = Duration::from_millis(refresh_rate_ms.max(50));
    }

    pub(super) fn spawn_collector(&self) {
        if let Some(collector) = &self.collector {
            let collector = collector.clone();
            tokio::spawn(async move {
                if let Err(e) = collector.lock().await.run().await {
                    error!("Metrics collector error: {}", e);
                }
            });
        }
    }

    pub(super) async fn handle_market_event(
        &mut self,
        event: &MarketEvent,
        action_channel: &ActionChannel,
    ) -> Result<()> {
        if let MarketEvent::ConnectionStatus { status, .. } = event {
            self.status = match status {
                crate::binance::types::ConnectionStatus::Disconnected => {
                    MetricsConnectionStatus::Disconnected
                }
                crate::binance::types::ConnectionStatus::Connecting => {
                    MetricsConnectionStatus::Connecting
                }
                crate::binance::types::ConnectionStatus::Connected => {
                    MetricsConnectionStatus::Connected
                }
                crate::binance::types::ConnectionStatus::Reconnecting => {
                    MetricsConnectionStatus::Reconnecting
                }
                crate::binance::types::ConnectionStatus::Error(err) => {
                    MetricsConnectionStatus::Error(err.clone())
                }
            };
        }

        let Some(collector) = &self.collector else {
            return Ok(());
        };

        let mut collector = collector.lock().await;
        collector.handle_market_event(event.clone()).await?;

        let now = Instant::now();
        let should_emit = now.duration_since(self.last_emit) >= self.emit_interval;
        let metrics_snapshot = if should_emit {
            Some(collector.get_connection_metrics(self.status.clone()))
        } else {
            None
        };
        drop(collector);

        if let Some(metrics_snapshot) = metrics_snapshot {
            if let Err(e) = action_channel.send_event(SessionEvent::MetricsUpdate {
                metrics: metrics_snapshot,
            }) {
                error!("Failed to forward metrics update: {}", e);
            } else {
                self.last_emit = now;
            }
        }

        Ok(())
    }

    pub(super) async fn shutdown(&mut self) -> Result<()> {
        if let Some(collector) = &self.collector {
            collector.lock().await.shutdown().await?;
        }
        Ok(())
    }
}
