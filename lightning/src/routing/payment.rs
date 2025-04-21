use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::{RwLock, mpsc};
use ethers::types::{Address, H256, U256};
use serde::{Serialize, Deserialize};

use super::{Route, RoutingError};
use crate::channel::Channel;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentInfo {
    pub route: Route,
    pub payment_hash: H256,
    pub payment_secret: H256,
    pub amount: U256,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum PaymentStatus {
    Pending,
    InFlight,
    Success,
    Failed,
    TimedOut,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentResult {
    pub status: PaymentStatus,
    pub preimage: Option<H256>,
    pub failure_reason: Option<String>,
    pub completed_at: Option<u64>,
    pub fees_paid: U256,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HtlcInfo {
    channel_id: H256,
    amount: U256,
    expiry: u64,
    hash: H256,
}

pub struct PaymentProcessor {
    active_payments: Arc<RwLock<HashMap<H256, PaymentInfo>>>,
    payment_statuses: Arc<RwLock<HashMap<H256, PaymentStatus>>>,
    htlcs: Arc<RwLock<HashMap<H256, Vec<HtlcInfo>>>>,
    results: Arc<RwLock<HashMap<H256, PaymentResult>>>,
    status_tx: mpsc::Sender<(H256, PaymentStatus)>,
}

impl PaymentProcessor {
    pub fn new() -> Self {
        let (status_tx, _) = mpsc::channel(1000);
        
        Self {
            active_payments: Arc::new(RwLock::new(HashMap::new())),
            payment_statuses: Arc::new(RwLock::new(HashMap::new())),
            htlcs: Arc::new(RwLock::new(HashMap::new())),
            results: Arc::new(RwLock::new(HashMap::new())),
            status_tx,
        }
    }

    pub async fn init_payment(&self, payment_info: PaymentInfo) -> Result<(), RoutingError> {
        let mut active_payments = self.active_payments.write().await;
        let mut payment_statuses = self.payment_statuses.write().await;
        let mut htlcs = self.htlcs.write().await;

        // Check if payment already exists
        if active_payments.contains_key(&payment_info.payment_hash) {
            return Err(RoutingError::PaymentFailed("Payment already in progress".into()));
        }

        // Initialize payment tracking
        active_payments.insert(payment_info.payment_hash, payment_info.clone());
        payment_statuses.insert(payment_info.payment_hash, PaymentStatus::Pending);
        htlcs.insert(payment_info.payment_hash, Vec::new());

        Ok(())
    }

    pub async fn process_hop(
        &self,
        payment_hash: H256,
        hop_index: usize,
        channel: &Channel,
    ) -> Result<(), RoutingError> {
        let mut payment_statuses = self.payment_statuses.write().await;
        let mut htlcs = self.htlcs.write().await;

        // Get payment info
        let payment_info = self.get_payment_info(payment_hash).await?;

        // Verify hop index
        if hop_index >= payment_info.route.channels.len() {
            return Err(RoutingError::PaymentFailed("Invalid hop index".into()));
        }

        // Create HTLC
        let hop = &payment_info.route.channels[hop_index];
        let htlc = HtlcInfo {
            channel_id: hop.channel_id,
            amount: hop.amount,
            expiry: current_timestamp() + hop.timelock,
            hash: payment_hash,
        };

        // Add HTLC to tracking
        htlcs.get_mut(&payment_hash)
            .ok_or_else(|| RoutingError::PaymentFailed("Payment not found".into()))?
            .push(htlc);

        // Update payment status
        payment_statuses.insert(payment_hash, PaymentStatus::InFlight);

        // Notify status change
        self.status_tx.send((payment_hash, PaymentStatus::InFlight)).await
            .map_err(|e| RoutingError::PaymentFailed(e.to_string()))?;

        Ok(())
    }

    pub async fn complete_payment(
        &self,
        payment_hash: H256,
    ) -> Result<(), RoutingError> {
        let mut active_payments = self.active_payments.write().await;
        let mut payment_statuses = self.payment_statuses.write().await;
        let mut results = self.results.write().await;

        // Update payment status
        payment_statuses.insert(payment_hash, PaymentStatus::Success);

        // Record result
        let payment_info = active_payments.remove(&payment_hash)
            .ok_or_else(|| RoutingError::PaymentFailed("Payment not found".into()))?;

        results.insert(payment_hash, PaymentResult {
            status: PaymentStatus::Success,
            preimage: None, // Would be set in actual implementation
            failure_reason: None,
            completed_at: Some(current_timestamp()),
            fees_paid: payment_info.route.total_fees,
        });

        // Notify status change
        self.status_tx.send((payment_hash, PaymentStatus::Success)).await
            .map_err(|e| RoutingError::PaymentFailed(e.to_string()))?;

        Ok(())
    }

    pub async fn fail_payment(
        &self,
        payment_hash: H256,
        reason: String,
    ) -> Result<(), RoutingError> {
        let mut active_payments = self.active_payments.write().await;
        let mut payment_statuses = self.payment_statuses.write().await;
        let mut results = self.results.write().await;

        // Update payment status
        payment_statuses.insert(payment_hash, PaymentStatus::Failed);

        // Record result
        let payment_info = active_payments.remove(&payment_hash)
            .ok_or_else(|| RoutingError::PaymentFailed("Payment not found".into()))?;

        results.insert(payment_hash, PaymentResult {
            status: PaymentStatus::Failed,
            preimage: None,
            failure_reason: Some(reason.clone()),
            completed_at: Some(current_timestamp()),
            fees_paid: U256::zero(),
        });

        // Notify status change
        self.status_tx.send((payment_hash, PaymentStatus::Failed)).await
            .map_err(|e| RoutingError::PaymentFailed(e.to_string()))?;

        Ok(())
    }

    pub async fn cleanup_timed_out_payments(&self) -> Result<(), RoutingError> {
        let mut active_payments = self.active_payments.write().await;
        let mut payment_statuses = self.payment_statuses.write().await;
        let current_time = current_timestamp();

        let timed_out: Vec<H256> = active_payments.iter()
            .filter(|(_, payment)| {
                current_time > payment.timestamp + 300 // 5 minute timeout
            })
            .map(|(hash, _)| *hash)
            .collect();

        for payment_hash in timed_out {
            // Update status
            payment_statuses.insert(payment_hash, PaymentStatus::TimedOut);
            
            // Remove from active payments
            active_payments.remove(&payment_hash);

            // Notify status change
            self.status_tx.send((payment_hash, PaymentStatus::TimedOut)).await
                .map_err(|e| RoutingError::PaymentFailed(e.to_string()))?;
        }

        Ok(())
    }

    pub async fn get_payment_info(&self, payment_hash: H256) -> Result<PaymentInfo, RoutingError> {
        let active_payments = self.active_payments.read().await;
        active_payments.get(&payment_hash)
            .cloned()
            .ok_or_else(|| RoutingError::PaymentFailed("Payment not found".into()))
    }

    pub async fn get_payment_status(&self, payment_hash: H256) -> Result<PaymentStatus, RoutingError> {
        let payment_statuses = self.payment_statuses.read().await;
        payment_statuses.get(&payment_hash)
            .cloned()
            .ok_or_else(|| RoutingError::PaymentFailed("Payment not found".into()))
    }

    pub async fn get_payment_result(&self, payment_hash: H256) -> Result<PaymentResult, RoutingError> {
        let results = self.results.read().await;
        results.get(&payment_hash)
            .cloned()
            .ok_or_else(|| RoutingError::PaymentFailed("Payment result not found".into()))
    }

    pub async fn start_monitoring(&self) {
        let self_clone = self.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            loop {
                interval.tick().await;
                if let Err(e) = self_clone.cleanup_timed_out_payments().await {
                    log::error!("Failed to cleanup timed out payments: {:?}", e);
                }
            }
        });
    }
}

impl Clone for PaymentProcessor {
    fn clone(&self) -> Self {
        Self {
            active_payments: Arc::clone(&self.active_payments),
            payment_statuses: Arc::clone(&self.payment_statuses),
            htlcs: Arc::clone(&self.htlcs),
            results: Arc::clone(&self.results),
            status_tx: self.status_tx.clone(),
        }
    }
}

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup_test_payment() -> (PaymentProcessor, PaymentInfo) {
        let processor = PaymentProcessor::new();
        let payment_info = PaymentInfo {
            route: Route {
                path: vec![H256::random()],
                channels: vec![],
                total_amount: U256::from(1000),
                total_fees: U256::from(10),
                total_timelock: 144,
            },
            payment_hash: H256::random(),
            payment_secret: H256::random(),
            amount: U256::from(1000),
            timestamp: current_timestamp(),
        };

        processor.init_payment(payment_info.clone()).await.unwrap();
        (processor, payment_info)
    }

    #[tokio::test]
    async fn test_payment_lifecycle() {
        let (processor, payment_info) = setup_test_payment().await;

        // Check initial status
        let status = processor.get_payment_status(payment_info.payment_hash).await.unwrap();
        assert_eq!(status, PaymentStatus::Pending);

        // Complete payment
        processor.complete_payment(payment_info.payment_hash).await.unwrap();

        // Check final status
        let status = processor.get_payment_status(payment_info.payment_hash).await.unwrap();
        assert_eq!(status, PaymentStatus::Success);

        // Check result
        let result = processor.get_payment_result(payment_info.payment_hash).await.unwrap();
        assert_eq!(result.status, PaymentStatus::Success);
        assert_eq!(result.fees_paid, U256::from(10));
    }

    #[tokio::test]
    async fn test_payment_failure() {
        let (processor, payment_info) = setup_test_payment().await;

        // Fail payment
        processor.fail_payment(
            payment_info.payment_hash,
            "Test failure".into()
        ).await.unwrap();

        // Check status
        let status = processor.get_payment_status(payment_info.payment_hash).await.unwrap();
        assert_eq!(status, PaymentStatus::Failed);

        // Check result
        let result = processor.get_payment_result(payment_info.payment_hash).await.unwrap();
        assert_eq!(result.status, PaymentStatus::Failed);
        assert_eq!(result.fees_paid, U256::zero());
        assert_eq!(result.failure_reason, Some("Test failure".into()));
    }

    #[tokio::test]
    async fn test_timeout_cleanup() {
        let (processor, payment_info) = setup_test_payment().await;

        // Manipulate timestamp to simulate timeout
        let mut payments = processor.active_payments.write().await;
        let mut payment = payments.get_mut(&payment_info.payment_hash).unwrap();
        payment.timestamp = current_timestamp() - 600; // 10 minutes ago

        // Run cleanup
        drop(payments);
        processor.cleanup_timed_out_payments().await.unwrap();

        // Check status
        let status = processor.get_payment_status(payment_info.payment_hash).await.unwrap();
        assert_eq!(status, PaymentStatus::TimedOut);
    }
}