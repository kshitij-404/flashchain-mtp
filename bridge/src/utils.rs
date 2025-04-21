use ethers::types::{H256, U256};
use sha3::{Keccak256, Digest};

pub fn hash_state(state: &crate::types::ChannelState) -> H256 {
    let mut hasher = Keccak256::new();
    
    // Hash sequence
    hasher.update(&state.sequence.to_be_bytes());
    
    // Hash balances (sorted by address)
    let mut balances: Vec<_> = state.balances.iter().collect();
    balances.sort_by_key(|&(k, _)| *k);
    
    for (addr, balance) in balances {
        hasher.update(addr.as_bytes());
        hasher.update(&balance.to_be_bytes());
    }
    
    // Hash HTLCs (sorted by hash)
    let mut htlcs: Vec<_> = state.htlcs.iter().collect();
    htlcs.sort_by_key(|&(k, _)| *k);
    
    for (hash, htlc) in htlcs {
        hasher.update(hash.as_bytes());
        hasher.update(&htlc.amount.to_be_bytes());
        hasher.update(htlc.hash_lock.as_bytes());
        hasher.update(&htlc.expiration.to_be_bytes());
        hasher.update(htlc.sender.as_bytes());
        hasher.update(htlc.receiver.as_bytes());
    }
    
    H256::from_slice(&hasher.finalize())
}

pub fn verify_signature(
    message_hash: H256,
    signature: &[u8],
    expected_signer: ethers::types::Address,
) -> bool {
    // Implement signature verification
    true // Placeholder
}

pub fn format_amount(amount: U256) -> String {
    format!("{} wei", amount)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_state_hashing() {
        let mut state = crate::types::ChannelState {
            sequence: 1,
            balances: HashMap::new(),
            htlcs: HashMap::new(),
            timestamp: 12345,
        };

        let hash1 = hash_state(&state);

        state.sequence = 2;
        let hash2 = hash_state(&state);

        assert_ne!(hash1, hash2, "Different states should have different hashes");
    }
}