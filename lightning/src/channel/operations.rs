use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
use serde::{Serialize, Deserialize};
use ethers::types::{Address, U256, H256};
use async_trait::async_trait;
use thiserror::Error;

use super::state::{ChannelState, ChannelStatus, StateError};
use super::Channel;

#[derive(Error, Debug)]
pub enum OperationError {
    #[error("Operation timeout")]
    Timeout,
    #[error("Channel error: {0}")]
    ChannelError(String),
    #[error("State error: {0}")]
    StateError(#[from] StateError),
    #[error("Invalid operation: {0}")]
    InvalidOperation(String),
    #[error("Operation rejected: {0}")]
    Rejected(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChannelOperation {
    Transfer {
        channel_id: H256,
        from: Address,
        to: Address,
        amount: U256,
        response: oneshot::Sender<OperationResult<TransferResult>>,
    },
    CreateLock {
        channel_id: H256,
        sender: Address,
        recipient: Address,
        amount: U256,
        expiration_height: u64,
        secret_hash: H256,
        response: oneshot::Sender<OperationResult<LockResult>>,
    },
    Unlock {
        channel_id: H256,
        lock_id: H256,
        secret: H256,
        response: oneshot::Sender<OperationResult<UnlockResult>>,
    },
    Close {
        channel_id: H256,
        final_state: ChannelState,
        signatures: Vec<Vec<u8>>,
        response: oneshot::Sender<OperationResult<CloseResult>>,
    },
    Dispute {
        channel_id: H256,
        disputed_state: ChannelState,
        proof: Vec<u8>,
        response: oneshot::Sender<OperationResult<DisputeResult>>,
    },
    UpdateState {
        channel_id: H256,
        new_state: ChannelState,
        signatures: Vec<Vec<u8>>,
        response: oneshot::Sender<OperationResult<UpdateStateResult>>,
    },
}

pub type OperationResult<T> = Result<T, OperationError>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferResult {
    pub channel_id: H256,
    pub new_state: ChannelState,
    pub transaction_hash: H256,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockResult {
    pub channel_id: H256,
    pub lock_id: H256,
    pub new_state: ChannelState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnlockResult {
    pub channel_id: H256,
    pub lock_id: H256,
    pub new_state: ChannelState,
    pub secret: H256,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloseResult {
    pub channel_id: H256,
    pub final_state: ChannelState,
    pub closing_transaction: H256,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisputeResult {
    pub channel_id: H256,
    pub disputed_state: ChannelState,
    pub dispute_transaction: H256,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateStateResult {
    pub channel_id: H256,
    pub new_state: ChannelState,
    pub state_update_hash: H256,
}

#[async_trait]
pub trait OperationHandler {
    async fn handle_operation(&self, operation: ChannelOperation) -> Result<(), OperationError>;
}

pub struct ChannelOperationHandler {
    operation_tx: mpsc::Sender<ChannelOperation>,
    channels: Arc<tokio::sync::RwLock<std::collections::HashMap<H256, Channel>>>,
}

impl ChannelOperationHandler {
    pub fn new(
        operation_tx: mpsc::Sender<ChannelOperation>,
        channels: Arc<tokio::sync::RwLock<std::collections::HashMap<H256, Channel>>>,
    ) -> Self {
        Self {
            operation_tx,
            channels,
        }
    }

    pub async fn start_operation_processor(&self) {
        let mut operation_rx = self.operation_tx.subscribe();

        tokio::spawn(async move {
            while let Some(operation) = operation_rx.recv().await {
                match self.process_operation(operation).await {
                    Ok(_) => log::debug!("Operation processed successfully"),
                    Err(e) => log::error!("Operation processing failed: {:?}", e),
                }
            }
        });
    }

    async fn process_operation(&self, operation: ChannelOperation) -> Result<(), OperationError> {
        match operation {
            ChannelOperation::Transfer { 
                channel_id, 
                from, 
                to, 
                amount, 
                response 
            } => {
                let result = self.handle_transfer(channel_id, from, to, amount).await;
                let _ = response.send(result);
            },
            ChannelOperation::CreateLock { 
                channel_id,
                sender,
                recipient,
                amount,
                expiration_height,
                secret_hash,
                response,
            } => {
                let result = self.handle_create_lock(
                    channel_id,
                    sender,
                    recipient,
                    amount,
                    expiration_height,
                    secret_hash,
                ).await;
                let _ = response.send(result);
            },
            ChannelOperation::Unlock { 
                channel_id,
                lock_id,
                secret,
                response,
            } => {
                let result = self.handle_unlock(channel_id, lock_id, secret).await;
                let _ = response.send(result);
            },
            ChannelOperation::Close {
                channel_id,
                final_state,
                signatures,
                response,
            } => {
                let result = self.handle_close(channel_id, final_state, signatures).await;
                let _ = response.send(result);
            },
            ChannelOperation::Dispute {
                channel_id,
                disputed_state,
                proof,
                response,
            } => {
                let result = self.handle_dispute(channel_id, disputed_state, proof).await;
                let _ = response.send(result);
            },
            ChannelOperation::UpdateState {
                channel_id,
                new_state,
                signatures,
                response,
            } => {
                let result = self.handle_update_state(channel_id, new_state, signatures).await;
                let _ = response.send(result);
            },
        }

        Ok(())
    }

    async fn handle_transfer(
        &self,
        channel_id: H256,
        from: Address,
        to: Address,
        amount: U256,
    ) -> OperationResult<TransferResult> {
        let mut channels = self.channels.write().await;
        let channel = channels.get_mut(&channel_id)
            .ok_or_else(|| OperationError::ChannelError("Channel not found".to_string()))?;

        if channel.status != ChannelStatus::Active {
            return Err(OperationError::InvalidOperation("Channel not active".to_string()));
        }

        let mut new_state = channel.state.clone();
        new_state.transfer(from, to, amount)?;

        // Generate and verify merkle proof
        let proof = new_state.generate_proof(from);

        // Update channel state
        channel.state = new_state.clone();
        channel.nonce += 1;

        Ok(TransferResult {
            channel_id,
            new_state,
            transaction_hash: H256::zero(), // Generate actual transaction hash
        })
    }

    // Implement other operation handlers...

    async fn handle_create_lock(
        &self,
        channel_id: H256,
        sender: Address,
        recipient: Address,
        amount: U256,
        expiration_height: u64,
        secret_hash: H256,
    ) -> OperationResult<LockResult> {
        // Implementation
        todo!()
    }

    async fn handle_unlock(
        &self,
        channel_id: H256,
        lock_id: H256,
        secret: H256,
    ) -> OperationResult<UnlockResult> {
        // Implementation
        todo!()
    }

    async fn handle_close(
        &self,
        channel_id: H256,
        final_state: ChannelState,
        signatures: Vec<Vec<u8>>,
    ) -> OperationResult<CloseResult> {
        // Implementation
        todo!()
    }

    async fn handle_dispute(
        &self,
        channel_id: H256,
        disputed_state: ChannelState,
        proof: Vec<u8>,
    ) -> OperationResult<DisputeResult> {
        // Implementation
        todo!()
    }

    async fn handle_update_state(
        &self,
        channel_id: H256,
        new_state: ChannelState,
        signatures: Vec<Vec<u8>>,
    ) -> OperationResult<UpdateStateResult> {
        // Implementation
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Add tests for operation handling
    #[tokio::test]
    async fn test_transfer_operation() {
        // Test implementation
    }

    #[tokio::test]
    async fn test_lock_creation() {
        // Test implementation
    }

    #[tokio::test]
    async fn test_unlock_operation() {
        // Test implementation
    }

    #[tokio::test]
    async fn test_channel_closing() {
        // Test implementation
    }
}