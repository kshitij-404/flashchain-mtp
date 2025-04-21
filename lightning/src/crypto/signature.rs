use super::CryptoError;
use ethers::types::{Address, H256, U256};
use k256::ecdsa::{Signature, SigningKey, VerifyingKey, signature::{Signer, Verifier}};
use serde::{Serialize, Deserialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignatureSet {
    pub signatures: HashMap<Address, Vec<u8>>,
    pub message_hash: H256,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedState {
    pub state_hash: H256,
    pub nonce: u64,
    pub signatures: SignatureSet,
}

#[derive(Debug, Clone)]
pub struct SignatureVerifier {
    required_signatures: usize,
    verifying_keys: HashMap<Address, VerifyingKey>,
}

impl SignatureVerifier {
    pub fn new(required_signatures: usize) -> Self {
        Self {
            required_signatures,
            verifying_keys: HashMap::new(),
        }
    }

    pub fn add_verifying_key(&mut self, address: Address, key: VerifyingKey) {
        self.verifying_keys.insert(address, key);
    }

    pub fn verify_signature_set(&self, set: &SignatureSet) -> Result<bool, CryptoError> {
        // Check if we have enough signatures
        if set.signatures.len() < self.required_signatures {
            return Ok(false);
        }

        // Verify each signature
        for (address, signature) in &set.signatures {
            if let Some(verifying_key) = self.verifying_keys.get(address) {
                let signature = Signature::try_from(signature.as_slice())
                    .map_err(|_| CryptoError::InvalidSignature)?;

                if verifying_key.verify(set.message_hash.as_ref(), &signature).is_err() {
                    return Ok(false);
                }
            } else {
                return Ok(false);
            }
        }

        Ok(true)
    }

    pub fn verify_signed_state(&self, signed_state: &SignedState) -> Result<bool, CryptoError> {
        self.verify_signature_set(&signed_state.signatures)
    }
}

pub struct SignatureAggregator {
    signatures: HashMap<Address, Vec<u8>>,
    message_hash: H256,
    required_signatures: usize,
}

impl SignatureAggregator {
    pub fn new(message_hash: H256, required_signatures: usize) -> Self {
        Self {
            signatures: HashMap::new(),
            message_hash,
            required_signatures,
        }
    }

    pub fn add_signature(&mut self, address: Address, signature: Vec<u8>) -> Result<bool, CryptoError> {
        // Validate signature format
        if signature.len() != 65 {
            return Err(CryptoError::InvalidSignature);
        }

        self.signatures.insert(address, signature);

        Ok(self.is_complete())
    }

    pub fn is_complete(&self) -> bool {
        self.signatures.len() >= self.required_signatures
    }

    pub fn build_signature_set(&self) -> Result<SignatureSet, CryptoError> {
        if !self.is_complete() {
            return Err(CryptoError::InvalidSignature);
        }

        Ok(SignatureSet {
            signatures: self.signatures.clone(),
            message_hash: self.message_hash,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        })
    }
}

pub struct SignatureBuilder {
    data: Vec<u8>,
    typed_data: bool,
}

impl SignatureBuilder {
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            typed_data: false,
        }
    }

    pub fn add_address(&mut self, address: Address) -> &mut Self {
        self.data.extend_from_slice(address.as_bytes());
        self
    }

    pub fn add_amount(&mut self, amount: U256) -> &mut Self {
        self.data.extend_from_slice(&amount.to_be_bytes());
        self
    }

    pub fn add_hash(&mut self, hash: H256) -> &mut Self {
        self.data.extend_from_slice(hash.as_bytes());
        self
    }

    pub fn add_nonce(&mut self, nonce: u64) -> &mut Self {
        self.data.extend_from_slice(&nonce.to_be_bytes());
        self
    }

    pub fn enable_typed_data(&mut self) -> &mut Self {
        self.typed_data = true;
        self
    }

    pub fn build(&self) -> H256 {
        if self.typed_data {
            // EIP-712 typed data hashing
            let domain_separator = H256::from_slice(b"FlashChain Channel v1.0\x00\x00\x00");
            let mut typed_data = Vec::new();
            typed_data.extend_from_slice(domain_separator.as_bytes());
            typed_data.extend_from_slice(&self.data);
            H256::from_slice(&keccak256(&typed_data))
        } else {
            H256::from_slice(&keccak256(&self.data))
        }
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
    use k256::SecretKey;

    fn generate_test_keypair() -> (Address, SigningKey, VerifyingKey) {
        let secret_key = SigningKey::random(&mut rand::thread_rng());
        let public_key = secret_key.verifying_key();
        let address = Address::random(); // In real implementation, derive from public key
        (address, secret_key, public_key)
    }

    #[test]
    fn test_signature_verification() {
        let (address, secret_key, public_key) = generate_test_keypair();
        
        let mut verifier = SignatureVerifier::new(1);
        verifier.add_verifying_key(address, public_key);

        let message = H256::random();
        let signature = secret_key.sign(message.as_ref());

        let mut signature_set = SignatureSet {
            signatures: HashMap::new(),
            message_hash: message,
            timestamp: 0,
        };
        signature_set.signatures.insert(address, signature.to_vec());

        assert!(verifier.verify_signature_set(&signature_set).unwrap());
    }

    #[test]
    fn test_signature_aggregation() {
        let message_hash = H256::random();
        let mut aggregator = SignatureAggregator::new(message_hash, 2);

        let (address1, secret_key1, _) = generate_test_keypair();
        let (address2, secret_key2, _) = generate_test_keypair();

        let signature1 = secret_key1.sign(message_hash.as_ref()).to_vec();
        let signature2 = secret_key2.sign(message_hash.as_ref()).to_vec();

        aggregator.add_signature(address1, signature1).unwrap();
        assert!(!aggregator.is_complete());

        aggregator.add_signature(address2, signature2).unwrap();
        assert!(aggregator.is_complete());

        let signature_set = aggregator.build_signature_set().unwrap();
        assert_eq!(signature_set.signatures.len(), 2);
    }

    #[test]
    fn test_signature_builder() {
        let mut builder = SignatureBuilder::new();
        let address = Address::random();
        let amount = U256::from(1000);
        let nonce = 1u64;

        builder
            .add_address(address)
            .add_amount(amount)
            .add_nonce(nonce)
            .enable_typed_data();

        let hash = builder.build();
        assert_ne!(hash, H256::zero());
    }
}