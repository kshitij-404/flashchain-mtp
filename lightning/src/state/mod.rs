use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use ethers::types::{Address, H256, U256};
use serde::{Serialize, Deserialize};
use thiserror::Error;

pub mod channel_state;
pub mod network_state;
pub mod persistence;

use channel_state::ChannelState;
use network_state::NetworkState;

#[derive(Error, Debug)]
pub enum StateError {
    #[error("Invalid state transition: {0}")]
    InvalidTransition(String),
    #[error("State not found: {0}")]
    NotFound(String),
    #[error("State corruption: {0}")]
    Corruption(String),
    #[error("Persistence error: {0}")]
    PersistenceError(String),
    #[error("Concurrent modification error: {0}")]
    ConcurrentModification(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateUpdate {
    pub channel_id: H256,
    pub sequence: u64,
    pub timestamp: u64,
    pub previous_state: H256,
    pub new_state: H256,
    pub signatures: HashMap<Address, Vec<u8>>,
}

pub struct StateManager {
    channel_states: Arc<RwLock<HashMap<H256, ChannelState>>>,
    network_state: Arc<RwLock<NetworkState>>,
    persistence: Arc<persistence::StatePersistence>,
}

impl StateManager {
    pub async fn new(persistence: persistence::StatePersistence) -> Result<Self, StateError> {
        let manager = Self {
            channel_states: Arc::new(RwLock::new(HashMap::new())),
            network_state: Arc::new(RwLock::new(NetworkState::new())),
            persistence: Arc::new(persistence),
        };

        // Load persisted states
        manager.load_persisted_states().await?;

        Ok(manager)
    }

    pub async fn update_channel_state(
        &self,
        channel_id: H256,
        update: StateUpdate,
    ) -> Result<(), StateError> {
        // Verify state transition
        self.verify_state_update(&update).await?;

        // Acquire write lock
        let mut states = self.channel_states.write().await;

        // Get current state
        let current_state = states.get_mut(&channel_id)
            .ok_or_else(|| StateError::NotFound(format!("Channel {}", channel_id)))?;

        // Apply update
        current_state.apply_update(update.clone()).await?;

        // Persist update
        self.persistence.persist_state_update(&update).await
            .map_err(|e| StateError::PersistenceError(e.to_string()))?;

        Ok(())
    }

    pub async fn get_channel_state(&self, channel_id: H256) -> Result<ChannelState, StateError> {
        let states = self.channel_states.read().await;
        states.get(&channel_id)
            .cloned()
            .ok_or_else(|| StateError::NotFound(format!("Channel {}", channel_id)))
    }

    pub async fn create_channel_state(
        &self,
        channel_id: H256,
        participants: Vec<Address>,
        capacity: U256,
    ) -> Result<(), StateError> {
        let mut states = self.channel_states.write().await;
        
        if states.contains_key(&channel_id) {
            return Err(StateError::InvalidTransition("Channel already exists".into()));
        }

        let state = ChannelState::new(channel_id, participants, capacity);
        states.insert(channel_id, state.clone());

        // Persist initial state
        self.persistence.persist_channel_state(&state).await
            .map_err(|e| StateError::PersistenceError(e.to_string()))?;

        Ok(())
    }

    pub async fn close_channel_state(&self, channel_id: H256) -> Result<(), StateError> {
        let mut states = self.channel_states.write().await;
        
        let state = states.get_mut(&channel_id)
            .ok_or_else(|| StateError::NotFound(format!("Channel {}", channel_id)))?;

        state.close().await?;

        // Persist final state
        self.persistence.persist_channel_state(state).await
            .map_err(|e| StateError::PersistenceError(e.to_string()))?;

        // Remove from active states
        states.remove(&channel_id);

        Ok(())
    }

    pub async fn update_network_state(&self, update: NetworkState) -> Result<(), StateError> {
        let mut network = self.network_state.write().await;
        *network = update;

        // Persist network state
        self.persistence.persist_network_state(&update).await
            .map_err(|e| StateError::PersistenceError(e.to_string()))?;

        Ok(())
    }

    pub async fn get_network_state(&self) -> NetworkState {
        self.network_state.read().await.clone()
    }

    async fn load_persisted_states(&self) -> Result<(), StateError> {
        // Load channel states
        let channel_states = self.persistence.load_channel_states().await
            .map_err(|e| StateError::PersistenceError(e.to_string()))?;

        let mut states = self.channel_states.write().await;
        *states = channel_states;

        // Load network state
        let network_state = self.persistence.load_network_state().await
            .map_err(|e| StateError::PersistenceError(e.to_string()))?;

        let mut network = self.network_state.write().await;
        *network = network_state;

        Ok(())
    }

    async fn verify_state_update(&self, update: &StateUpdate) -> Result<(), StateError> {
        // Verify sequence number
        let states = self.channel_states.read().await;
        let current_state = states.get(&update.channel_id)
            .ok_or_else(|| StateError::NotFound(format!("Channel {}", update.channel_id)))?;

        if update.sequence != current_state.sequence + 1 {
            return Err(StateError::InvalidTransition("Invalid sequence number".into()));
        }

        // Verify signatures
        if !self.verify_signatures(update).await? {
            return Err(StateError::InvalidTransition("Invalid signatures".into()));
        }

        Ok(())
    }

    async fn verify_signatures(&self, update: &StateUpdate) -> Result<bool, StateError> {
        let states = self.channel_states.read().await;
        let current_state = states.get(&update.channel_id)
            .ok_or_else(|| StateError::NotFound(format!("Channel {}", update.channel_id)))?;

        // Verify all required participants have signed
        for participant in &current_state.participants {
            if !update.signatures.contains_key(participant) {
                return Ok(false);
            }
        }

        // Verify each signature
        for (address, signature) in &update.signatures {
            if !current_state.verify_signature(address, &update.new_state, signature) {
                return Ok(false);
            }
        }

        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::test;

    #[test]
    async fn test_state_creation_and_update() {
        // Create mock persistence
        let persistence = persistence::MockStatePersistence::new();
        let state_manager = StateManager::new(persistence).await.unwrap();

        // Create channel state
        let channel_id = H256::random();
        let participants = vec![Address::random(), Address::random()];
        let capacity = U256::from(1000000);

        state_manager.create_channel_state(channel_id, participants.clone(), capacity)
            .await
            .unwrap();

        // Verify state creation
        let state = state_manager.get_channel_state(channel_id).await.unwrap();
        assert_eq!(state.participants, participants);
        assert_eq!(state.capacity, capacity);

        // Create state update
        let update = StateUpdate {
            channel_id,
            sequence: 1,
            timestamp: 12345,
            previous_state: state.state_hash(),
            new_state: H256::random(),
            signatures: HashMap::new(),
        };

        // Update state
        state_manager.update_channel_state(channel_id, update.clone())
            .await
            .unwrap();

        // Verify update
        let updated_state = state_manager.get_channel_state(channel_id).await.unwrap();
        assert_eq!(updated_state.sequence, 1);
    }

    #[test]
    async fn test_channel_closing() {
        let persistence = persistence::MockStatePersistence::new();
        let state_manager = StateManager::new(persistence).await.unwrap();

        // Create and close channel
        let channel_id = H256::random();
        let participants = vec![Address::random(), Address::random()];
        let capacity = U256::from(1000000);

        state_manager.create_channel_state(channel_id, participants, capacity)
            .await
            .unwrap();

        state_manager.close_channel_state(channel_id).await.unwrap();

        // Verify channel is closed
        let result = state_manager.get_channel_state(channel_id).await;
        assert!(result.is_err());
    }
}