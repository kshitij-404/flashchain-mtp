use serde::{Serialize, Deserialize};
use ethers::types::{Address, H256, U256};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub network_id: u64,
    pub chain_id: u64,
    pub rpc_url: String,
    pub ws_url: Option<String>,
    pub contracts: ContractAddresses,
    pub gas_settings: GasSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractAddresses {
    pub bridge_core: Address,
    pub channel_manager: Address,
    pub validator_set: Address,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GasSettings {
    pub max_gas_price: U256,
    pub gas_multiplier: f64,
    pub max_fee_per_gas: Option<U256>,
    pub max_priority_fee_per_gas: Option<U256>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metrics {
    pub total_channels: usize,
    pub active_channels: usize,
    pub total_locked_value: U256,
    pub total_transactions: u64,
    pub average_transaction_time: f64,
    pub success_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelMetrics {
    pub channel_id: H256,
    pub total_transactions: u64,
    pub total_value_transferred: U256,
    pub average_transaction_size: U256,
    pub uptime: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NetworkStatus {
    Connected,
    Disconnected,
    Syncing,
    Error(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}