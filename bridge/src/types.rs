use serde::{Serialize, Deserialize};
use ethers::types::{Address, H256, U256};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Channel {
    pub participants: Vec<Address>,
    pub capacity: U256,
    pub locked_funds: U256,
    pub latest_state_hash: H256,
    pub is_active: bool,
    pub dispute_status: DisputeStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelState {
    pub sequence: u64,
    pub balances: HashMap<Address, U256>,
    pub htlcs: HashMap<H256, HTLC>,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HTLC {
    pub amount: U256,
    pub hash_lock: H256,
    pub expiration: u64,
    pub sender: Address,
    pub receiver: Address,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateUpdate {
    pub channel_id: H256,
    pub sequence: u64,
    pub previous_state: H256,
    pub new_state: ChannelState,
    pub signatures: HashMap<Address, Vec<u8>>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum DisputeStatus {
    None,
    Initiated,
    Resolved,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingTransaction {
    pub tx_type: TransactionType,
    pub status: TransactionStatus,
    pub timestamp: i64,
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransactionType {
    ChannelRegistration,
    StateUpdate,
    DisputeInitiation,
    DisputeResolution,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TransactionStatus {
    Pending,
    Confirmed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingTransactionReceipt {
    pub tx_hash: H256,
    pub block_number: Option<u64>,
}

// Data structures for transaction-specific data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelRegistrationData {
    pub participants: Vec<Address>,
    pub capacity: U256,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateUpdateData {
    pub channel_id: H256,
    pub state_hash: H256,
    pub state: ChannelState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisputeData {
    pub channel_id: H256,
    pub state: ChannelState,
    pub proof: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisputeResolutionData {
    pub channel_id: H256,
    pub final_state: ChannelState,
    pub state_hash: H256,
}