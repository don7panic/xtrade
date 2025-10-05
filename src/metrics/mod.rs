//! Metrics collection and monitoring module
//!
//! Provides performance metrics, latency measurement, and connection monitoring.

use std::collections::VecDeque;
use std::time::{SystemTime, UNIX_EPOCH};

/// Connection status enumeration
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    Error(String),
}

/// Connection quality levels
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionQualityLevel {
    Excellent, // < 100ms latency, > 1000 msgs/sec
    Good,      // < 500ms latency, > 500 msgs/sec
    Fair,      // < 1000ms latency, > 100 msgs/sec
    Poor,      // > 1000ms latency, < 100 msgs/sec
    Critical,  // No messages for > 30 seconds
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
    pub connection_quality: ConnectionQualityLevel,
    pub uptime_seconds: u64,
    pub total_messages: u64,
    pub error_count: u32,
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
            connection_quality: ConnectionQualityLevel::Poor,
            uptime_seconds: 0,
            total_messages: 0,
            error_count: 0,
        }
    }
}

/// Metrics collector for performance monitoring
pub struct MetricsCollector {
    latency_samples: Vec<u64>,
    message_count: u64,
    last_reset: SystemTime,
    max_samples: usize,
    connection_start_time: SystemTime,
    error_count: u32,
    reconnect_count: u32,
    message_history: VecDeque<u64>, // Timestamps of last 1000 messages
}

impl MetricsCollector {
    /// Create a new metrics collector
    pub fn new(max_samples: usize) -> Self {
        Self {
            latency_samples: Vec::with_capacity(max_samples),
            message_count: 0,
            last_reset: SystemTime::now(),
            max_samples,
            connection_start_time: SystemTime::now(),
            error_count: 0,
            reconnect_count: 0,
            message_history: VecDeque::with_capacity(1000),
        }
    }

    /// Record connection start time
    pub fn record_connection_start(&mut self) {
        self.connection_start_time = SystemTime::now();
        self.reconnect_count += 1;
    }

    /// Record an error occurrence
    pub fn record_error(&mut self) {
        self.error_count += 1;
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

        // Record message timestamp for rate calculation
        if self.message_history.len() >= 1000 {
            self.message_history.pop_front();
        }
        self.message_history.push_back(now);

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
        self.message_history.clear();
        self.last_reset = SystemTime::now();
    }

    /// Calculate connection quality based on recent performance
    pub fn calculate_connection_quality(&self) -> ConnectionQualityLevel {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        // Check if we have recent messages
        let last_message_time = self.message_history.back().copied().unwrap_or(0);
        let time_since_last_message = now.saturating_sub(last_message_time);

        // If no messages for 30 seconds, connection is critical
        if time_since_last_message > 30_000 {
            return ConnectionQualityLevel::Critical;
        }

        // Calculate recent message rate (last minute)
        let minute_ago = now.saturating_sub(60_000);
        let recent_message_count = self
            .message_history
            .iter()
            .filter(|&&t| t >= minute_ago)
            .count();
        let recent_rate = recent_message_count as f64 / 60.0; // messages per second

        // Calculate average latency
        let (p50, _p95, _p99) = self.calculate_percentiles();
        let avg_latency = p50; // Use p50 as representative latency

        // Determine quality based on latency and message rate
        if avg_latency < 100 && recent_rate > 1000.0 {
            ConnectionQualityLevel::Excellent
        } else if avg_latency < 500 && recent_rate > 500.0 {
            ConnectionQualityLevel::Good
        } else if avg_latency < 1000 && recent_rate > 100.0 {
            ConnectionQualityLevel::Fair
        } else {
            ConnectionQualityLevel::Poor
        }
    }

    /// Get comprehensive connection metrics
    pub fn get_connection_metrics(&self, status: ConnectionStatus) -> ConnectionMetrics {
        let (p50, p95, p99) = self.calculate_percentiles();
        let uptime = self
            .connection_start_time
            .elapsed()
            .unwrap_or_default()
            .as_secs();

        ConnectionMetrics {
            status,
            latency_p50: p50,
            latency_p95: p95,
            latency_p99: p99,
            reconnect_count: self.reconnect_count,
            last_message_time: self.message_history.back().copied().unwrap_or(0),
            messages_per_second: self.messages_per_second(),
            connection_quality: self.calculate_connection_quality(),
            uptime_seconds: uptime,
            total_messages: self.message_count,
            error_count: self.error_count,
        }
    }

    /// Check if connection quality is acceptable
    pub fn is_connection_acceptable(&self) -> bool {
        let quality = self.calculate_connection_quality();
        matches!(
            quality,
            ConnectionQualityLevel::Excellent
                | ConnectionQualityLevel::Good
                | ConnectionQualityLevel::Fair
        )
    }

    /// Run metrics collection (placeholder for async operation)
    pub async fn run(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Metrics collection runs in background
        Ok(())
    }

    /// Handle market event for metrics collection
    pub async fn handle_market_event(
        &mut self,
        event: crate::market_data::MarketEvent,
    ) -> anyhow::Result<()> {
        match event {
            crate::market_data::MarketEvent::PriceUpdate { time, .. } => {
                self.record_message_latency(time);
            }
            crate::market_data::MarketEvent::Error { .. } => {
                self.record_error();
            }
            _ => {
                // Other events don't affect metrics directly
            }
        }
        Ok(())
    }

    /// Shutdown metrics collector
    pub async fn shutdown(&mut self) -> anyhow::Result<()> {
        // Cleanup metrics resources
        Ok(())
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
