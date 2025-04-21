use ethers::types::{Address, U256, H256};
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum StateError {
    #[error("Invalid balance allocation")]
    InvalidBalance,
    #[error("Invalid state transition: {0}")]
    InvalidTransition(String),
    #[error("Missing participant: {0}")]
    MissingParticipant(Address),
    #[error("Invalid lock: {0}")]
    InvalidLock(String),
    #[error("Lock already exists: {0}")]
    LockExists(H256),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ChannelStatus {
    Initializing,
    Active,
    Locked,
    Closing,
    Disputed,
    Closed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeLock {
    pub lock_id: H256,
    pub amount: U256,
    pub expiration_height: u64,
    pub recipient: Address,
    pub secret_hash: H256,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelState {
    pub balances: HashMap<Address, U256>,
    pub locks: HashMap<H256, TimeLock>,
    pub merkle_root: H256,
    pub sequence_number: u64,
    pub total_locked: U256,
}

impl Default for ChannelState {
    fn default() -> Self {
        Self {
            balances: HashMap::new(),
            locks: HashMap::new(),
            merkle_root: H256::zero(),
            sequence_number: 0,
            total_locked: U256::zero(),
        }
    }
}

impl ChannelState {
    pub fn new(initial_balances: HashMap<Address, U256>) -> Result<Self, StateError> {
        let total_balance = initial_balances.values().fold(U256::zero(), |acc, &val| acc + val);
        if total_balance.is_zero() {
            return Err(StateError::InvalidBalance);
        }

        Ok(Self {
            balances: initial_balances,
            locks: HashMap::new(),
            merkle_root: H256::zero(),
            sequence_number: 0,
            total_locked: U256::zero(),
        })
    }

    pub fn transfer(
        &mut self,
        from: Address,
        to: Address,
        amount: U256,
    ) -> Result<(), StateError> {
        // Verify participants exist
        let from_balance = self.balances.get(&from)
            .ok_or(StateError::MissingParticipant(from))?;
        
        if amount > *from_balance {
            return Err(StateError::InvalidBalance);
        }

        // Update balances
        *self.balances.entry(from).or_insert(U256::zero()) -= amount;
        *self.balances.entry(to).or_insert(U256::zero()) += amount;
        self.sequence_number += 1;

        // Update merkle root
        self.update_merkle_root()?;

        Ok(())
    }

    pub fn create_lock(
        &mut self,
        sender: Address,
        recipient: Address,
        amount: U256,
        expiration_height: u64,
        secret_hash: H256,
    ) -> Result<H256, StateError> {
        // Verify sender has sufficient balance
        let sender_balance = self.balances.get(&sender)
            .ok_or(StateError::MissingParticipant(sender))?;

        if amount > *sender_balance {
            return Err(StateError::InvalidBalance);
        }

        // Create lock
        let lock_id = self.generate_lock_id(sender, recipient, amount, secret_hash);
        if self.locks.contains_key(&lock_id) {
            return Err(StateError::LockExists(lock_id));
        }

        let lock = TimeLock {
            lock_id,
            amount,
            expiration_height,
            recipient,
            secret_hash,
        };

        // Update state
        *self.balances.get_mut(&sender).unwrap() -= amount;
        self.total_locked += amount;
        self.locks.insert(lock_id, lock);
        self.sequence_number += 1;

        // Update merkle root
        self.update_merkle_root()?;

        Ok(lock_id)
    }

    pub fn unlock(
        &mut self,
        lock_id: H256,
        secret: H256,
    ) -> Result<(), StateError> {
        let lock = self.locks.get(&lock_id)
            .ok_or(StateError::InvalidLock("Lock not found".to_string()))?;

        // Verify secret
        if !self.verify_secret(lock.secret_hash, secret) {
            return Err(StateError::InvalidLock("Invalid secret".to_string()));
        }

        // Transfer locked amount to recipient
        *self.balances.entry(lock.recipient).or_insert(U256::zero()) += lock.amount;
        self.total_locked -= lock.amount;
        self.locks.remove(&lock_id);
        self.sequence_number += 1;

        // Update merkle root
        self.update_merkle_root()?;

        Ok(())
    }

    pub fn expire_lock(&mut self, lock_id: H256, current_height: u64) -> Result<(), StateError> {
        let lock = self.locks.get(&lock_id)
            .ok_or(StateError::InvalidLock("Lock not found".to_string()))?;

        if current_height < lock.expiration_height {
            return Err(StateError::InvalidLock("Lock not expired".to_string()));
        }

        // Return locked amount to sender
        let sender = self.find_lock_sender(lock_id)?;
        *self.balances.entry(sender).or_insert(U256::zero()) += lock.amount;
        self.total_locked -= lock.amount;
        self.locks.remove(&lock_id);
        self.sequence_number += 1;

        // Update merkle root
        self.update_merkle_root()?;

        Ok(())
    }

    pub fn verify_state(&self, capacity: U256) -> Result<(), StateError> {
        // Verify total balances don't exceed capacity
        let total_balance: U256 = self.balances.values().fold(U256::zero(), |acc, &val| acc + val);
        if total_balance + self.total_locked > capacity {
            return Err(StateError::InvalidBalance);
        }

        Ok(())
    }

    pub fn get_participant_balance(&self, participant: &Address) -> U256 {
        self.balances.get(participant).copied().unwrap_or_default()
    }

    pub fn get_lock(&self, lock_id: &H256) -> Option<TimeLock> {
        self.locks.get(lock_id).cloned()
    }

    // Helper functions

    fn update_merkle_root(&mut self) -> Result<(), StateError> {
        // Implement merkle root calculation
        // This should include balances and locks in the merkle tree
        self.merkle_root = H256::zero(); // Placeholder
        Ok(())
    }

    fn generate_lock_id(
        &self,
        sender: Address,
        recipient: Address,
        amount: U256,
        secret_hash: H256,
    ) -> H256 {
        // Implement lock ID generation
        H256::zero() // Placeholder
    }

    fn verify_secret(&self, secret_hash: H256, secret: H256) -> bool {
        // Implement secret verification
        true // Placeholder
    }

    fn find_lock_sender(&self, lock_id: H256) -> Result<Address, StateError> {
        // Implement sender lookup logic
        Ok(Address::zero()) // Placeholder
    }

    pub fn generate_proof(&self, participant: Address) -> Vec<u8> {
        // Implement merkle proof generation for participant's balance
        Vec::new() // Placeholder
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_state_creation() {
        let mut initial_balances = HashMap::new();
        initial_balances.insert(Address::random(), U256::from(100));
        initial_balances.insert(Address::random(), U256::from(200));

        let state = ChannelState::new(initial_balances.clone()).unwrap();
        assert_eq!(state.sequence_number, 0);
        assert_eq!(state.total_locked, U256::zero());
        assert_eq!(state.balances, initial_balances);
    }

    #[test]
    fn test_transfer() {
        // Add transfer tests
    }

    #[test]
    fn test_lock_creation() {
        // Add lock creation tests
    }

    #[test]
    fn test_unlock() {
        // Add unlock tests
    }

    #[test]
    fn test_lock_expiration() {
        // Add lock expiration tests
    }
}