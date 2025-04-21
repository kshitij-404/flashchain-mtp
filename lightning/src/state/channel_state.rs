use ethers::types::{Address, H256, U256};
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use super::StateError;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ChannelStatus {
    Created,
    Active,
    Closing,
    Closed,
    Disputed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Balance {
    pub amount: U256,
    pub locked: U256,
    pub pending_htlcs: Vec<H256>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Htlc {
    pub id: H256,
    pub sender: Address,
    pub receiver: Address,
    pub amount: U256,
    pub hash_lock: H256,
    pub timeout: u64,
    pub status: HtlcStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum HtlcStatus {
    Pending,
    Fulfilled,
    Failed,
    Expired,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelState {
    pub channel_id: H256,
    pub participants: Vec<Address>,
    pub capacity: U256,
    pub balances: HashMap<Address, Balance>,
    pub htlcs: HashMap<H256, Htlc>,
    pub status: ChannelStatus,
    pub sequence: u64,
    pub dispute_timeout: u64,
    pub last_update: u64,
}

impl ChannelState {
    pub fn new(channel_id: H256, participants: Vec<Address>, capacity: U256) -> Self {
        let mut balances = HashMap::new();
        for participant in &participants {
            balances.insert(*participant, Balance {
                amount: U256::zero(),
                locked: U256::zero(),
                pending_htlcs: Vec::new(),
            });
        }

        Self {
            channel_id,
            participants,
            capacity,
            balances,
            htlcs: HashMap::new(),
            status: ChannelStatus::Created,
            sequence: 0,
            dispute_timeout: 144 * 7, // ~1 week in blocks
            last_update: chrono::Utc::now().timestamp() as u64,
        }
    }

    pub async fn apply_update(&mut self, update: super::StateUpdate) -> Result<(), StateError> {
        // Verify sequence
        if update.sequence != self.sequence + 1 {
            return Err(StateError::InvalidTransition("Invalid sequence number".into()));
        }

        // Verify state transition
        if !self.is_valid_transition(&update) {
            return Err(StateError::InvalidTransition("Invalid state transition".into()));
        }

        // Update state
        self.sequence = update.sequence;
        self.last_update = update.timestamp;

        Ok(())
    }

    pub fn create_htlc(
        &mut self,
        sender: Address,
        receiver: Address,
        amount: U256,
        hash_lock: H256,
        timeout: u64,
    ) -> Result<H256, StateError> {
        // Verify participants
        if !self.participants.contains(&sender) || !self.participants.contains(&receiver) {
            return Err(StateError::InvalidTransition("Invalid participants".into()));
        }

        // Verify sufficient balance
        let sender_balance = self.balances.get(&sender)
            .ok_or_else(|| StateError::NotFound("Sender balance not found".into()))?;

        if sender_balance.amount < amount {
            return Err(StateError::InvalidTransition("Insufficient balance".into()));
        }

        // Create HTLC
        let htlc_id = self.generate_htlc_id(sender, receiver, amount, hash_lock);
        let htlc = Htlc {
            id: htlc_id,
            sender,
            receiver,
            amount,
            hash_lock,
            timeout,
            status: HtlcStatus::Pending,
        };

        // Update balances
        if let Some(balance) = self.balances.get_mut(&sender) {
            balance.amount -= amount;
            balance.locked += amount;
            balance.pending_htlcs.push(htlc_id);
        }

        self.htlcs.insert(htlc_id, htlc);
        Ok(htlc_id)
    }

    pub fn fulfill_htlc(&mut self, htlc_id: H256, preimage: H256) -> Result<(), StateError> {
        let htlc = self.htlcs.get_mut(&htlc_id)
            .ok_or_else(|| StateError::NotFound("HTLC not found".into()))?;

        // Verify HTLC status
        if htlc.status != HtlcStatus::Pending {
            return Err(StateError::InvalidTransition("HTLC not pending".into()));
        }

        // Verify preimage
        if !self.verify_preimage(htlc.hash_lock, preimage) {
            return Err(StateError::InvalidTransition("Invalid preimage".into()));
        }

        // Update HTLC status
        htlc.status = HtlcStatus::Fulfilled;

        // Update balances
        if let Some(sender_balance) = self.balances.get_mut(&htlc.sender) {
            sender_balance.locked -= htlc.amount;
            sender_balance.pending_htlcs.retain(|&id| id != htlc_id);
        }

        if let Some(receiver_balance) = self.balances.get_mut(&htlc.receiver) {
            receiver_balance.amount += htlc.amount;
        }

        Ok(())
    }

    pub fn close(&mut self) -> Result<(), StateError> {
        match self.status {
            ChannelStatus::Active => {
                self.status = ChannelStatus::Closing;
                Ok(())
            }
            ChannelStatus::Closing => {
                self.status = ChannelStatus::Closed;
                Ok(())
            }
            _ => Err(StateError::InvalidTransition(
                "Invalid state for closing".into()
            )),
        }
    }

    pub fn state_hash(&self) -> H256 {
        let mut data = Vec::new();
        data.extend_from_slice(&self.sequence.to_be_bytes());
        
        // Add balances
        let mut sorted_balances: Vec<_> = self.balances.iter().collect();
        sorted_balances.sort_by_key(|&(addr, _)| *addr);
        
        for (addr, balance) in sorted_balances {
            data.extend_from_slice(addr.as_bytes());
            data.extend_from_slice(&balance.amount.to_be_bytes());
            data.extend_from_slice(&balance.locked.to_be_bytes());
        }

        // Add HTLCs
        let mut sorted_htlcs: Vec<_> = self.htlcs.values().collect();
        sorted_htlcs.sort_by_key(|htlc| htlc.id);

        for htlc in sorted_htlcs {
            data.extend_from_slice(htlc.id.as_bytes());
            data.extend_from_slice(&htlc.amount.to_be_bytes());
            data.extend_from_slice(htlc.hash_lock.as_bytes());
        }

        H256::from_slice(&keccak256(&data))
    }

    pub fn verify_signature(&self, signer: &Address, state: &H256, signature: &[u8]) -> bool {
        // Implement signature verification
        // This would use proper cryptographic verification in production
        true
    }

    // Helper methods

    fn is_valid_transition(&self, update: &super::StateUpdate) -> bool {
        // Implement state transition validation logic
        true
    }

    fn generate_htlc_id(
        &self,
        sender: Address,
        receiver: Address,
        amount: U256,
        hash_lock: H256,
    ) -> H256 {
        let mut data = Vec::new();
        data.extend_from_slice(sender.as_bytes());
        data.extend_from_slice(receiver.as_bytes());
        data.extend_from_slice(&amount.to_be_bytes());
        data.extend_from_slice(hash_lock.as_bytes());
        H256::from_slice(&keccak256(&data))
    }

    fn verify_preimage(&self, hash_lock: H256, preimage: H256) -> bool {
        keccak256(preimage.as_bytes()) == hash_lock.as_bytes()
    }
}

fn keccak256(data: &[u8]) -> [u8; 32] {
    use sha3::{Digest, Keccak256};
    let mut hasher = Keccak256::new();
    hasher.update(data);
    hasher.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_state_creation() {
        let channel_id = H256::random();
        let participants = vec![Address::random(), Address::random()];
        let capacity = U256::from(1000000);

        let state = ChannelState::new(channel_id, participants.clone(), capacity);

        assert_eq!(state.status, ChannelStatus::Created);
        assert_eq!(state.sequence, 0);
        assert_eq!(state.capacity, capacity);
        assert_eq!(state.participants, participants);
    }

    #[test]
    fn test_htlc_creation_and_fulfillment() {
        let mut state = ChannelState::new(
            H256::random(),
            vec![Address::random(), Address::random()],
            U256::from(1000000),
        );

        // Set initial balance
        if let Some(balance) = state.balances.get_mut(&state.participants[0]) {
            balance.amount = U256::from(1000);
        }

        // Create HTLC
        let preimage = H256::random();
        let hash_lock = H256::from_slice(&keccak256(preimage.as_bytes()));
        
        let htlc_id = state.create_htlc(
            state.participants[0],
            state.participants[1],
            U256::from(100),
            hash_lock,
            100,
        ).unwrap();

        // Verify HTLC creation
        assert!(state.htlcs.contains_key(&htlc_id));
        
        // Fulfill HTLC
        state.fulfill_htlc(htlc_id, preimage).unwrap();
        
        // Verify balances
        let receiver_balance = state.balances.get(&state.participants[1]).unwrap();
        assert_eq!(receiver_balance.amount, U256::from(100));
    }
}