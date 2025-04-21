use ethers::core::k256::ecdsa::{SigningKey, VerifyingKey, Signature};
use ethers::types::{H256, Address};
use sha3::{Keccak256, Digest};
use rand::Rng;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CryptoError {
    #[error("Invalid key: {0}")]
    InvalidKey(String),
    #[error("Signing error: {0}")]
    SigningError(String),
    #[error("Verification error: {0}")]
    VerificationError(String),
    #[error("Hash error: {0}")]
    HashError(String),
}

pub struct CryptoUtils {
    signing_key: Option<SigningKey>,
}

impl CryptoUtils {
    pub fn new() -> Self {
        Self {
            signing_key: None,
        }
    }

    pub fn generate_keypair(&mut self) -> Result<(SigningKey, VerifyingKey), CryptoError> {
        let mut rng = rand::thread_rng();
        let signing_key = SigningKey::random(&mut rng);
        let verifying_key = signing_key.verifying_key();
        self.signing_key = Some(signing_key.clone());
        Ok((signing_key, verifying_key))
    }

    pub fn set_signing_key(&mut self, key: SigningKey) {
        self.signing_key = Some(key);
    }

    pub fn sign_message(&self, message: &[u8]) -> Result<Signature, CryptoError> {
        let signing_key = self.signing_key.as_ref()
            .ok_or_else(|| CryptoError::InvalidKey("No signing key set".into()))?;

        signing_key.sign(message)
            .map_err(|e| CryptoError::SigningError(e.to_string()))
    }

    pub fn verify_signature(
        message: &[u8],
        signature: &Signature,
        public_key: &VerifyingKey,
    ) -> Result<bool, CryptoError> {
        public_key.verify(message, signature)
            .map_err(|e| CryptoError::VerificationError(e.to_string()))
            .map(|_| true)
    }

    pub fn hash_message(message: &[u8]) -> H256 {
        let mut hasher = Keccak256::new();
        hasher.update(message);
        H256::from_slice(&hasher.finalize())
    }

    pub fn derive_address(public_key: &VerifyingKey) -> Address {
        let public_key_bytes = public_key.to_encoded_point(false).as_bytes().to_vec();
        let hash = Self::hash_message(&public_key_bytes);
        let mut address_bytes = [0u8; 20];
        address_bytes.copy_from_slice(&hash.as_bytes()[12..]);
        Address::from_slice(&address_bytes)
    }

    pub fn generate_shared_secret(
        private_key: &SigningKey,
        public_key: &VerifyingKey,
    ) -> Result<H256, CryptoError> {
        // Implement ECDH
        let shared_point = public_key
            .as_affine()
            .mul_by_scalar(private_key.as_nonzero_scalar());

        let shared_key = H256::from_slice(
            &Keccak256::new()
                .chain_update(shared_point.to_encoded_point(false).as_bytes())
                .finalize()
        );

        Ok(shared_key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_generation() {
        let mut crypto = CryptoUtils::new();
        let (signing_key, verifying_key) = crypto.generate_keypair().unwrap();
        
        let message = b"test message";
        let signature = crypto.sign_message(message).unwrap();
        
        assert!(CryptoUtils::verify_signature(
            message,
            &signature,
            &verifying_key
        ).unwrap());
    }

    #[test]
    fn test_address_derivation() {
        let mut crypto = CryptoUtils::new();
        let (_, verifying_key) = crypto.generate_keypair().unwrap();
        
        let address = CryptoUtils::derive_address(&verifying_key);
        assert_eq!(address.as_bytes().len(), 20);
    }
}