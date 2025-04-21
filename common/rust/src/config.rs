use serde::{Serialize, Deserialize};
use std::path::PathBuf;
use std::fs;
use super::types::NetworkConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub network: NetworkConfig,
    pub logging: LogConfig,
    pub metrics: MetricsConfig,
    pub security: SecurityConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogConfig {
    pub level: String,
    pub file: Option<PathBuf>,
    pub format: LogFormat,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogFormat {
    Plain,
    Json,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    pub enabled: bool,
    pub port: u16,
    pub host: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub min_validator_stake: String,
    pub max_channels_per_node: usize,
    pub timeout_period: u64,
}

impl Config {
    pub fn load(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let contents = fs::read_to_string(path)?;
        let config = serde_json::from_str(&contents)?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), String> {
        // Validate network configuration
        if self.network.rpc_url.is_empty() {
            return Err("RPC URL cannot be empty".into());
        }

        // Validate logging configuration
        if let Some(file) = &self.logging.file {
            if let Some(parent) = file.parent() {
                if !parent.exists() {
                    return Err("Log file directory does not exist".into());
                }
            }
        }

        // Validate metrics configuration
        if self.metrics.enabled {
            if self.metrics.port == 0 {
                return Err("Invalid metrics port".into());
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_validation() {
        let config = Config {
            network: NetworkConfig {
                network_id: 1,
                chain_id: 1,
                rpc_url: "http://localhost:8545".into(),
                ws_url: None,
                contracts: Default::default(),
                gas_settings: Default::default(),
            },
            logging: LogConfig {
                level: "info".into(),
                file: None,
                format: LogFormat::Plain,
            },
            metrics: MetricsConfig {
                enabled: true,
                port: 9090,
                host: "127.0.0.1".into(),
            },
            security: SecurityConfig {
                min_validator_stake: "1000".into(),
                max_channels_per_node: 100,
                timeout_period: 86400,
            },
        };

        assert!(config.validate().is_ok());
    }
}