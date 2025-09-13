//! Metrics collection and monitoring module
//!
//! Provides performance metrics, latency measurement, and connection monitoring.

use std::time::{SystemTime, UNIX_EPOCH};

/// Connection status enumeration
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
}

/// Connection metrics structure
#[derive(Debug, Clone)]
pub struct ConnectionMetrics {
    pub status: ConnectionStatus,
    pub latency_p50: u64,
    pub latency_p95: u64,
    pub latency_p99: u64,
    pub reconnect_count: u32,
    pub last_message_time: u64,
    pub messages_per_second: f64,
}

impl Default for ConnectionMetrics {
    fn default() -> Self {
        Self {
            status: ConnectionStatus::Disconnected,
            latency_p50: 0,
            latency_p95: 0,
            latency_p99: 0,
            reconnect_count: 0,
            last_message_time: 0,
            messages_per_second: 0.0,
        }
    }
}

/// Metrics collector for performance monitoring
pub struct MetricsCollector {
    latency_samples: Vec<u64>,
    message_count: u64,
    last_reset: SystemTime,
    max_samples: usize,
}

impl MetricsCollector {
    /// Create a new metrics collector
    pub fn new(max_samples: usize) -> Self {
        Self {
            latency_samples: Vec::with_capacity(max_samples),
            message_count: 0,
            last_reset: SystemTime::now(),
            max_samples,
        }
    }

    /// Record message latency
    pub fn record_message_latency(&mut self, event_time: u64) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        let latency = now.saturating_sub(event_time);

        // Add to samples, maintain max size
        if self.latency_samples.len() >= self.max_samples {
            self.latency_samples.remove(0);
        }
        self.latency_samples.push(latency);
        self.message_count += 1;

        // Record metrics (placeholder for actual metrics implementation)
        // metrics::histogram!("message_latency_ms").record(latency as f64);
        // metrics::counter!("messages_received").increment(1);
    }

    /// Calculate latency percentiles
    pub fn calculate_percentiles(&self) -> (u64, u64, u64) {
        if self.latency_samples.is_empty() {
            return (0, 0, 0);
        }

        let mut sorted = self.latency_samples.clone();
        sorted.sort_unstable();

        let len = sorted.len();
        let p50_idx = (len * 50) / 100;
        let p95_idx = (len * 95) / 100;
        let p99_idx = (len * 99) / 100;

        let p50 = sorted.get(p50_idx).copied().unwrap_or(0);
        let p95 = sorted.get(p95_idx.min(len - 1)).copied().unwrap_or(0);
        let p99 = sorted.get(p99_idx.min(len - 1)).copied().unwrap_or(0);

        (p50, p95, p99)
    }

    /// Calculate messages per second
    pub fn messages_per_second(&self) -> f64 {
        let duration = self.last_reset.elapsed().unwrap_or_default();
        if duration.as_secs() == 0 {
            return 0.0;
        }
        self.message_count as f64 / duration.as_secs_f64()
    }

    /// Reset metrics
    pub fn reset(&mut self) {
        self.latency_samples.clear();
        self.message_count = 0;
        self.last_reset = SystemTime::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_collector() {
        let mut collector = MetricsCollector::new(100);

        // Record some latencies
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        collector.record_message_latency(now - 10);
        collector.record_message_latency(now - 20);
        collector.record_message_latency(now - 15);

        let (p50, p95, p99) = collector.calculate_percentiles();
        assert!(p50 > 0);
        assert!(p95 >= p50);
        assert!(p99 >= p95);
    }

    #[test]
    fn test_connection_metrics_default() {
        let metrics = ConnectionMetrics::default();
        assert_eq!(metrics.status, ConnectionStatus::Disconnected);
        assert_eq!(metrics.reconnect_count, 0);
    }
}
