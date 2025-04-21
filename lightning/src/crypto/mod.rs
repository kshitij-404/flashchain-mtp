use ethers::types::{Address, H256, U256};
use k256::{
    ecdsa::{SigningKey, VerifyingKey, Signature, signature::Signer, signature::Verifier},
    SecretKey,
};
use sha3::{Keccak256, Digest};
use thiserror::Error;
use rand::rngs::OsRng;
use std::collections::HashMap;

pub mod signatures;
pub mod merkle;

#[derive(Error, Debug)]
pub enum CryptoError {
    #[error("Invalid signature")]
    InvalidSignature,
    #[error("Invalid key: {0}")]
    InvalidKey(String),
    #[error("Signing error: {0}")]
    SigningError(String),
    #[error("Verification error: {0}")]
    VerificationError(String),
    #[error("Hash error: {0}")]
    HashError(String),
    #[error("Key generation error: {0}")]
    KeyGenerationError(String),
    #[error("Invalid state encoding: {0}")]
    InvalidStateEncoding(String),
}

pub struct CryptoManager {
    // Secure key storage
    keys: HashMap<Address, SecretKey>,
    // Cached verifying keys
    verifying_keys: HashMap<Address, VerifyingKey>,
}

impl CryptoManager {
    pub fn new() -> Self {
        Self {
            keys: HashMap::new(),
            verifying_keys: HashMap::new(),
        }
    }

    /// Generates a new key pair and returns the associated address
    pub fn generate_keypair(&mut self) -> Result<Address, CryptoError> {
        let secret_key = SigningKey::random(&mut OsRng);
        let public_key = secret_key.verifying_key();
        let address = self.public_key_to_address(&public_key)?;

        self.keys.insert(address, secret_key.into());
        self.verifying_keys.insert(address, public_key);

        Ok(address)
    }

    /// Signs a message with the key associated with the given address
    pub fn sign_message(&self, address: &Address, message: &[u8]) -> Result<Vec<u8>, CryptoError> {
        let secret_key = self.keys.get(address)
            .ok_or_else(|| CryptoError::InvalidKey("Key not found".into()))?;
        
        let signing_key = SigningKey::from(secret_key);
        let signature: Signature = signing_key.sign(message);
        
        Ok(signature.to_vec())
    }

    /// Verifies a signature against a message and address
    pub fn verify_signature(
        &self,
        address: &Address,
        message: &[u8],
        signature: &[u8]
    ) -> Result<bool, CryptoError> {
        let verifying_key = self.verifying_keys.get(address)
            .ok_or_else(|| CryptoError::InvalidKey("Verifying key not found".into()))?;

        let signature = Signature::try_from(signature)
            .map_err(|e| CryptoError::InvalidSignature)?;

        Ok(verifying_key.verify(message, &signature).is_ok())
    }

    /// Hashes data using Keccak256
    pub fn hash_data(&self, data: &[u8]) -> H256 {
        let mut hasher = Keccak256::new();
        hasher.update(data);
        let result = hasher.finalize();
        H256::from_slice(&result)
    }

    /// Creates a hash of the channel state
    pub fn hash_channel_state(&self, 
        balances: &HashMap<Address, U256>,
        nonce: u64,
        additional_data: &[u8]
    ) -> Result<H256, CryptoError> {
        let mut state_data = Vec::new();

        // Add nonce
        state_data.extend_from_slice(&nonce.to_be_bytes());

        // Add sorted balances
        let mut balance_entries: Vec<_> = balances.iter().collect();
        balance_entries.sort_by_key(|&(addr, _)| *addr);

        for (addr, balance) in balance_entries {
            state_data.extend_from_slice(addr.as_bytes());
            state_data.extend_from_slice(&balance.to_be_bytes());
        }

        // Add additional data
        state_data.extend_from_slice(additional_data);

        Ok(self.hash_data(&state_data))
    }

    /// Derives address from public key
    fn public_key_to_address(&self, public_key: &VerifyingKey) -> Result<Address, CryptoError> {
        let public_key_bytes = public_key.to_encoded_point(false).as_bytes().to_vec();
        let hash = self.hash_data(&public_key_bytes);
        let mut address_bytes = [0u8; 20];
        address_bytes.copy_from_slice(&hash.as_bytes()[12..]);
        Ok(Address::from_slice(&address_bytes))
    }

    /// Implements ECDH (Elliptic Curve Diffie-Hellman) for secure channel establishment
    pub fn generate_shared_secret(
        &self,
        our_address: &Address,
        their_public_key: &[u8]
    ) -> Result<H256, CryptoError> {
        let our_secret_key = self.keys.get(our_address)
            .ok_or_else(|| CryptoError::InvalidKey("Key not found".into()))?;

        let their_verifying_key = VerifyingKey::from_sec1_bytes(their_public_key)
            .map_err(|e| CryptoError::InvalidKey(e.to_string()))?;

        // Perform ECDH
        let shared_point = their_verifying_key
            .as_affine()
            .mul_by_scalar(our_secret_key.as_scalar_primitive());

        // Hash the shared point to derive the secret
        let shared_point_bytes = shared_point.to_encoded_point(false).as_bytes().to_vec();
        Ok(self.hash_data(&shared_point_bytes))
    }

    /// Implements key rotation for enhanced security
    pub fn rotate_key(&mut self, address: &Address) -> Result<(), CryptoError> {
        let new_secret_key = SigningKey::random(&mut OsRng);
        let new_public_key = new_secret_key.verifying_key();
        
        // Verify the new key generation was successful
        let new_address = self.public_key_to_address(&new_public_key)?;
        if new_address != *address {
            return Err(CryptoError::KeyGenerationError(
                "New key generated incorrect address".into()
            ));
        }

        // Update the keys
        self.keys.insert(*address, new_secret_key.into());
        self.verifying_keys.insert(*address, new_public_key);

        Ok(())
    }

    /// Implements secure state encoding for channel updates
    pub fn encode_channel_state(
        &self,
        balances: &HashMap<Address, U256>,
        locks: &HashMap<H256, Vec<u8>>,
        nonce: u64
    ) -> Result<Vec<u8>, CryptoError> {
        let mut encoded = Vec::new();

        // Encode nonce
        encoded.extend_from_slice(&nonce.to_be_bytes());

        // Encode balances
        let mut balance_entries: Vec<_> = balances.iter().collect();
        balance_entries.sort_by_key(|&(addr, _)| *addr);

        for (addr, balance) in balance_entries {
            encoded.extend_from_slice(addr.as_bytes());
            encoded.extend_from_slice(&balance.to_be_bytes());
        }

        // Encode locks
        let mut lock_entries: Vec<_> = locks.iter().collect();
        lock_entries.sort_by_key(|&(hash, _)| *hash);

        for (hash, lock_data) in lock_entries {
            encoded.extend_from_slice(hash.as_bytes());
            encoded.extend_from_slice(&(lock_data.len() as u64).to_be_bytes());
            encoded.extend_from_slice(lock_data);
        }

        Ok(encoded)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keypair_generation() {
        let mut crypto_manager = CryptoManager::new();
        let address = crypto_manager.generate_keypair().unwrap();
        assert!(crypto_manager.keys.contains_key(&address));
        assert!(crypto_manager.verifying_keys.contains_key(&address));
    }

    #[test]
    fn test_signature_verification() {
        let mut crypto_manager = CryptoManager::new();
        let address = crypto_manager.generate_keypair().unwrap();
        
        let message = b"Test message";
        let signature = crypto_manager.sign_message(&address, message).unwrap();
        
        assert!(crypto_manager.verify_signature(&address, message, &signature).unwrap());
    }

    #[test]
    fn test_shared_secret_generation() {
        let mut crypto_manager = CryptoManager::new();
        let address1 = crypto_manager.generate_keypair().unwrap();
        let address2 = crypto_manager.generate_keypair().unwrap();

        let public_key = crypto_manager.verifying_keys[&address2]
            .to_encoded_point(false)
            .as_bytes()
            .to_vec();

        let secret = crypto_manager.generate_shared_secret(&address1, &public_key).unwrap();
        assert_ne!(secret, H256::zero());
    }

    #[test]
    fn test_key_rotation() {
        let mut crypto_manager = CryptoManager::new();
        let address = crypto_manager.generate_keypair().unwrap();
        
        let old_key = crypto_manager.keys[&address].clone();
        crypto_manager.rotate_key(&address).unwrap();
        
        assert_ne!(crypto_manager.keys[&address], old_key);
    }
}