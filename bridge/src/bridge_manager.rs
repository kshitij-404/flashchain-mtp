use std::sync::Arc;
use tokio::sync::RwLock;
use ethers::prelude::*;
use ethers::types::{Address, H256, U256};
use anyhow::Result;

use crate::contract_bindings::{BridgeCore, ChannelManager};
use crate::state_sync::StateSync;
use crate::types::*;

pub struct BridgeManager {
    bridge_contract: BridgeCore<Provider<Http>>,
    channel_manager: ChannelManager<Provider<Http>>,
    state_sync: Arc<RwLock<StateSync>>,
    wallet: LocalWallet,
    pending_transactions: Arc<RwLock<HashMap<H256, PendingTransaction>>>,
}

impl BridgeManager {
    pub async fn new(
        provider: Provider<Http>,
        bridge_address: Address,
        channel_manager_address: Address,
        wallet: LocalWallet,
    ) -> Result<Self> {
        let bridge_contract = BridgeCore::new(bridge_address, Arc::new(provider.clone()));
        let channel_manager = ChannelManager::new(channel_manager_address, Arc::new(provider.clone()));
        let state_sync = Arc::new(RwLock::new(StateSync::new()));

        Ok(Self {
            bridge_contract,
            channel_manager,
            state_sync,
            wallet,
            pending_transactions: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    pub async fn register_channel(
        &self,
        participants: Vec<Address>,
        capacity: U256,
    ) -> Result<H256> {
        let tx = self.bridge_contract
            .register_channel(participants.clone(), capacity)
            .from(self.wallet.address())
            .gas(500_000);

        let pending_tx = self.submit_transaction(tx).await?;
        
        let mut pending = self.pending_transactions.write().await;
        pending.insert(pending_tx.tx_hash, PendingTransaction {
            tx_type: TransactionType::ChannelRegistration,
            status: TransactionStatus::Pending,
            timestamp: chrono::Utc::now().timestamp(),
            data: Some(serde_json::to_value(&ChannelRegistrationData {
                participants,
                capacity,
            })?),
        });

        Ok(pending_tx.tx_hash)
    }

    pub async fn update_channel_state(
        &self,
        channel_id: H256,
        state: ChannelState,
        signatures: Vec<Signature>,
    ) -> Result<H256> {
        let state_hash = state.hash();
        
        let tx = self.bridge_contract
            .update_channel_state(channel_id, state_hash, signatures)
            .from(self.wallet.address())
            .gas(300_000);

        let pending_tx = self.submit_transaction(tx).await?;
        
        let mut pending = self.pending_transactions.write().await;
        pending.insert(pending_tx.tx_hash, PendingTransaction {
            tx_type: TransactionType::StateUpdate,
            status: TransactionStatus::Pending,
            timestamp: chrono::Utc::now().timestamp(),
            data: Some(serde_json::to_value(&StateUpdateData {
                channel_id,
                state_hash,
                state,
            })?),
        });

        Ok(pending_tx.tx_hash)
    }

    pub async fn initiate_dispute(
        &self,
        channel_id: H256,
        state: ChannelState,
        proof: Vec<u8>,
    ) -> Result<H256> {
        let tx = self.bridge_contract
            .initiate_dispute(channel_id, proof)
            .from(self.wallet.address())
            .gas(500_000);

        let pending_tx = self.submit_transaction(tx).await?;
        
        let mut pending = self.pending_transactions.write().await;
        pending.insert(pending_tx.tx_hash, PendingTransaction {
            tx_type: TransactionType::DisputeInitiation,
            status: TransactionStatus::Pending,
            timestamp: chrono::Utc::now().timestamp(),
            data: Some(serde_json::to_value(&DisputeData {
                channel_id,
                state,
                proof: hex::encode(proof),
            })?),
        });

        Ok(pending_tx.tx_hash)
    }

    pub async fn resolve_dispute(
        &self,
        channel_id: H256,
        final_state: ChannelState,
        validator_signatures: Vec<Signature>,
    ) -> Result<H256> {
        let state_hash = final_state.hash();
        
        let tx = self.bridge_contract
            .resolve_dispute(channel_id, state_hash, validator_signatures)
            .from(self.wallet.address())
            .gas(500_000);

        let pending_tx = self.submit_transaction(tx).await?;
        
        let mut pending = self.pending_transactions.write().await;
        pending.insert(pending_tx.tx_hash, PendingTransaction {
            tx_type: TransactionType::DisputeResolution,
            status: TransactionStatus::Pending,
            timestamp: chrono::Utc::now().timestamp(),
            data: Some(serde_json::to_value(&DisputeResolutionData {
                channel_id,
                final_state,
                state_hash,
            })?),
        });

        Ok(pending_tx.tx_hash)
    }

    pub async fn get_channel(&self, channel_id: H256) -> Result<Channel> {
        let channel = self.bridge_contract.get_channel(channel_id).call().await?;
        Ok(Channel {
            participants: channel.0,
            capacity: channel.1,
            locked_funds: channel.2,
            latest_state_hash: channel.3,
            is_active: channel.4,
            dispute_status: channel.5.into(),
        })
    }

    pub async fn get_pending_transaction(&self, tx_hash: H256) -> Option<PendingTransaction> {
        self.pending_transactions.read().await.get(&tx_hash).cloned()
    }

    async fn submit_transaction<T: Send + Sync + ethers::abi::Tokenize>(
        &self,
        tx: ContractCall<Provider<Http>, T>,
    ) -> Result<PendingTransactionReceipt> {
        let tx = tx.send().await?;
        Ok(PendingTransactionReceipt {
            tx_hash: tx.tx_hash(),
            block_number: None,
        })
    }

    pub async fn start_monitoring(&self) {
        let bridge_manager = self.clone();
        tokio::spawn(async move {
            loop {
                if let Err(e) = bridge_manager.monitor_pending_transactions().await {
                    log::error!("Error monitoring transactions: {:?}", e);
                }
                tokio::time::sleep(tokio::time::Duration::from_secs(15)).await;
            }
        });
    }

    async fn monitor_pending_transactions(&self) -> Result<()> {
        let mut pending = self.pending_transactions.write().await;
        let mut completed = Vec::new();

        for (tx_hash, tx) in pending.iter_mut() {
            if let Some(receipt) = self.bridge_contract
                .client()
                .get_transaction_receipt(*tx_hash)
                .await?
            {
                tx.status = if receipt.status.unwrap_or_default().as_u64() == 1 {
                    TransactionStatus::Confirmed
                } else {
                    TransactionStatus::Failed
                };
                completed.push(*tx_hash);
            }
        }

        // Remove confirmed/failed transactions after 24 hours
        let current_time = chrono::Utc::now().timestamp();
        pending.retain(|_, tx| {
            tx.status == TransactionStatus::Pending ||
            current_time - tx.timestamp < 24 * 60 * 60
        });

        Ok(())
    }
}

impl Clone for BridgeManager {
    fn clone(&self) -> Self {
        Self {
            bridge_contract: self.bridge_contract.clone(),
            channel_manager: self.channel_manager.clone(),
            state_sync: Arc::clone(&self.state_sync),
            wallet: self.wallet.clone(),
            pending_transactions: Arc::clone(&self.pending_transactions),
        }
    }
}