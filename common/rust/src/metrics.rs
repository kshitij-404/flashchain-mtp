use prometheus::{
    Registry, Counter, Gauge, Histogram,
    register_counter, register_gauge, register_histogram,
};
use lazy_static::lazy_static;
use std::sync::Arc;

lazy_static! {
    pub static ref REGISTRY: Registry = Registry::new();

    // Channel metrics
    pub static ref TOTAL_CHANNELS: Counter = register_counter!(
        "flashchain_total_channels",
        "Total number of channels created"
    ).unwrap();

    pub static ref ACTIVE_CHANNELS: Gauge = register_gauge!(
        "flashchain_active_channels",
        "Number of currently active channels"
    ).unwrap();

    pub static ref CHANNEL_BALANCE: Gauge = register_gauge!(
        "flashchain_channel_balance",
        "Total balance locked in channels"
    ).unwrap();

    // Transaction metrics
    pub static ref TRANSACTION_LATENCY: Histogram = register_histogram!(
        "flashchain_transaction_latency_seconds",
        "Transaction processing latency in seconds",
        vec![0.1, 0.5, 1.0, 2.0, 5.0]
    ).unwrap();

    pub static ref SUCCESSFUL_TRANSACTIONS: Counter = register_counter!(
        "flashchain_successful_transactions",
        "Number of successful transactions"
    ).unwrap();

    pub static ref FAILED_TRANSACTIONS: Counter = register_counter!(
        "flashchain_failed_transactions",
        "Number of failed transactions"
    ).unwrap();

    // Bridge metrics
    pub static ref BRIDGE_OPERATIONS: Counter = register_counter!(
        "flashchain_bridge_operations",
        "Total number of bridge operations"
    ).unwrap();

    pub static ref BRIDGE_ERRORS: Counter = register_counter!(
        "flashchain_bridge_errors",
        "Number of bridge operation errors"
    ).unwrap();
}

pub struct MetricsCollector {
    registry: Arc<Registry>,
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            registry: Arc::new(Registry::new()),
        }
    }

    pub fn record_transaction_latency(&self, duration_secs: f64) {
        TRANSACTION_LATENCY.observe(duration_secs);
    }

    pub fn increment_successful_transaction(&self) {
        SUCCESSFUL_TRANSACTIONS.inc();
    }

    pub fn increment_failed_transaction(&self) {
        FAILED_TRANSACTIONS.inc();
    }

    pub fn update_channel_count(&self, count: i64) {
        ACTIVE_CHANNELS.set(count as f64);
    }

    pub fn increment_total_channels(&self) {
        TOTAL_CHANNELS.inc();
    }

    pub fn update_channel_balance(&self, balance: f64) {
        CHANNEL_BALANCE.set(balance);
    }

    pub fn increment_bridge_operation(&self) {
        BRIDGE_OPERATIONS.inc();
    }

    pub fn increment_bridge_error(&self) {
        BRIDGE_ERRORS.inc();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_recording() {
        let collector = MetricsCollector::new();

        // Record some test metrics
        collector.record_transaction_latency(1.5);
        collector.increment_successful_transaction();
        collector.update_channel_count(10);

        // Verify metrics were recorded
        assert_eq!(SUCCESSFUL_TRANSACTIONS.get() as u64, 1);
        assert_eq!(ACTIVE_CHANNELS.get() as i64, 10);
    }
}