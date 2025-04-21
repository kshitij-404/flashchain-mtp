use thiserror::Error;
use ethers::types::H256;

#[derive(Error, Debug)]
pub enum CommonError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Network error: {0}")]
    Network(String),

    #[error("Contract error: {0}")]
    Contract(String),

    #[error("State error: {0}")]
    State(String),

    #[error("Channel error: {0}")]
    Channel(String),

    #[error("Transaction error: {0}")]
    Transaction(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Resource not found: {0}")]
    NotFound(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Timeout error: {0}")]
    Timeout(String),
}

impl CommonError {
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            CommonError::Network(_) | 
            CommonError::Timeout(_)
        )
    }
}

pub type Result<T> = std::result::Result<T, CommonError>;

#[derive(Debug)]
pub struct ErrorContext {
    pub timestamp: u64,
    pub transaction_hash: Option<H256>,
    pub error: CommonError,
    pub retry_count: u32,
}

impl ErrorContext {
    pub fn new(error: CommonError) -> Self {
        Self {
            timestamp: crate::utils::current_timestamp(),
            transaction_hash: None,
            error,
            retry_count: 0,
        }
    }

    pub fn with_transaction(mut self, hash: H256) -> Self {
        self.transaction_hash = Some(hash);
        self
    }

    pub fn increment_retry(&mut self) {
        self.retry_count += 1;
    }
}