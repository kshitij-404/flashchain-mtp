use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;
use serde::{Serialize, Deserialize};
use ethers::types::{Address, H256};
use crate::network::{NetworkError, PeerMetrics, NetworkMessage};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    pub address: Address,
    pub endpoint: String,
    pub shard_id: u64,
    pub version: String,
    pub capabilities: Vec<PeerCapability>,
    pub last_seen: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PeerStatus {
    Connected,
    Disconnected,
    Handshaking,
    Banned,
    Timeout,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PeerCapability {
    CrossShardTransfer,
    StateSync,
    FastSync,
    LightClient,
    FullNode,
}

#[derive(Debug)]
pub struct Peer {
    info: PeerInfo,
    status: PeerStatus,
    metrics: PeerMetrics,
    message_tx: mpsc::Sender<NetworkMessage>,
    last_heartbeat: u64,
    retry_count: u32,
    banned_until: Option<u64>,
}

impl Peer {
    pub fn new(info: PeerInfo) -> Self {
        let (message_tx, _) = mpsc::channel(1000);
        
        Self {
            info,
            status: PeerStatus::Handshaking,
            metrics: PeerMetrics {
                channels_count: 0,
                active_transfers: 0,
                pending_messages: 0,
                bandwidth_usage: 0.0,
                latency_ms: 0,
            },
            message_tx,
            last_heartbeat: current_timestamp(),
            retry_count: 0,
            banned_until: None,
        }
    }

    pub async fn connect(&mut self) -> Result<(), NetworkError> {
        if let Some(banned_until) = self.banned_until {
            if current_timestamp() < banned_until {
                return Err(NetworkError::ConnectionFailed("Peer is banned".into()));
            }
            self.banned_until = None;
        }

        // Perform handshake
        self.status = PeerStatus::Handshaking;
        if let Err(e) = self.handshake().await {
            self.retry_count += 1;
            return Err(e);
        }

        self.status = PeerStatus::Connected;
        self.retry_count = 0;
        self.last_heartbeat = current_timestamp();
        Ok(())
    }

    pub async fn disconnect(&mut self) {
        self.status = PeerStatus::Disconnected;
        // Cleanup resources
    }

    pub async fn send_message(&self, message: NetworkMessage) -> Result<(), NetworkError> {
        if self.status != PeerStatus::Connected {
            return Err(NetworkError::ConnectionFailed("Peer not connected".into()));
        }

        self.message_tx.send(message).await
            .map_err(|e| NetworkError::MessageDeliveryFailed(e.to_string()))
    }

    pub fn update_metrics(&mut self, metrics: PeerMetrics) {
        self.metrics = metrics;
        self.last_heartbeat = current_timestamp();
    }

    pub fn is_alive(&self) -> bool {
        let timeout = Duration::from_secs(30); // Configurable timeout
        current_timestamp().saturating_sub(self.last_heartbeat) < timeout.as_secs()
    }

    pub fn ban(&mut self, duration: Duration) {
        self.banned_until = Some(current_timestamp() + duration.as_secs());
        self.status = PeerStatus::Banned;
    }

    pub fn address(&self) -> Address {
        self.info.address
    }

    pub fn status(&self) -> PeerStatus {
        self.status.clone()
    }

    pub fn info(&self) -> &PeerInfo {
        &self.info
    }

    pub fn metrics(&self) -> &PeerMetrics {
        &self.metrics
    }

    async fn handshake(&mut self) -> Result<(), NetworkError> {
        // Implement handshake protocol
        // 1. Version verification
        // 2. Capability negotiation
        // 3. Network ID verification
        // 4. Shard ID verification
        Ok(())
    }
}

#[derive(Debug)]
pub struct PeerManager {
    max_peers: usize,
    peers: HashMap<Address, Peer>,
    banned_peers: HashMap<Address, u64>,
    metrics_tx: mpsc::Sender<(Address, PeerMetrics)>,
}

impl PeerManager {
    pub fn new(max_peers: usize) -> Self {
        let (metrics_tx, _) = mpsc::channel(1000);
        
        Self {
            max_peers,
            peers: HashMap::new(),
            banned_peers: HashMap::new(),
            metrics_tx,
        }
    }

    pub async fn add_peer(&mut self, info: PeerInfo) -> Result<(), NetworkError> {
        if self.peers.len() >= self.max_peers {
            return Err(NetworkError::ConnectionFailed("Max peers reached".into()));
        }

        if let Some(banned_until) = self.banned_peers.get(&info.address) {
            if current_timestamp() < *banned_until {
                return Err(NetworkError::ConnectionFailed("Peer is banned".into()));
            }
            self.banned_peers.remove(&info.address);
        }

        let mut peer = Peer::new(info);
        peer.connect().await?;
        self.peers.insert(peer.address(), peer);
        Ok(())
    }

    pub async fn remove_peer(&mut self, address: Address) -> Result<(), NetworkError> {
        if let Some(mut peer) = self.peers.remove(&address) {
            peer.disconnect().await;
        }
        Ok(())
    }

    pub fn ban_peer(&mut self, address: Address, duration: Duration) {
        if let Some(mut peer) = self.peers.remove(&address) {
            peer.ban(duration);
            self.banned_peers.insert(address, current_timestamp() + duration.as_secs());
        }
    }

    pub async fn broadcast(&self, message: NetworkMessage) -> Result<(), NetworkError> {
        for peer in self.peers.values() {
            if peer.status() == PeerStatus::Connected {
                if let Err(e) = peer.send_message(message.clone()).await {
                    log::error!("Failed to send message to peer {}: {:?}", peer.address(), e);
                }
            }
        }
        Ok(())
    }

    pub fn get_peer(&self, address: Address) -> Option<&Peer> {
        self.peers.get(&address)
    }

    pub fn get_peer_mut(&mut self, address: Address) -> Option<&mut Peer> {
        self.peers.get_mut(&address)
    }

    pub fn connected_peers(&self) -> Vec<Address> {
        self.peers
            .values()
            .filter(|p| p.status() == PeerStatus::Connected)
            .map(|p| p.address())
            .collect()
    }

    pub async fn monitor_peers(&mut self) {
        let mut disconnected = Vec::new();
        
        for (address, peer) in &self.peers {
            if !peer.is_alive() {
                disconnected.push(*address);
            }
        }

        for address in disconnected {
            let _ = self.remove_peer(address).await;
        }
    }
}

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_peer_connection() {
        let info = PeerInfo {
            address: Address::random(),
            endpoint: "127.0.0.1:8545".to_string(),
            shard_id: 0,
            version: "1.0.0".to_string(),
            capabilities: vec![PeerCapability::FullNode],
            last_seen: current_timestamp(),
        };

        let mut peer = Peer::new(info.clone());
        assert_eq!(peer.status(), PeerStatus::Handshaking);

        peer.connect().await.unwrap();
        assert_eq!(peer.status(), PeerStatus::Connected);
    }

    #[tokio::test]
    async fn test_peer_banning() {
        let mut manager = PeerManager::new(10);
        let address = Address::random();
        let info = PeerInfo {
            address,
            endpoint: "127.0.0.1:8545".to_string(),
            shard_id: 0,
            version: "1.0.0".to_string(),
            capabilities: vec![PeerCapability::FullNode],
            last_seen: current_timestamp(),
        };

        manager.add_peer(info.clone()).await.unwrap();
        manager.ban_peer(address, Duration::from_secs(3600));

        assert!(manager.add_peer(info).await.is_err());
    }

    #[tokio::test]
    async fn test_peer_metrics() {
        let info = PeerInfo {
            address: Address::random(),
            endpoint: "127.0.0.1:8545".to_string(),
            shard_id: 0,
            version: "1.0.0".to_string(),
            capabilities: vec![PeerCapability::FullNode],
            last_seen: current_timestamp(),
        };

        let mut peer = Peer::new(info);
        
        let metrics = PeerMetrics {
            channels_count: 5,
            active_transfers: 2,
            pending_messages: 10,
            bandwidth_usage: 1024.0,
            latency_ms: 100,
        };

        peer.update_metrics(metrics.clone());
        assert_eq!(peer.metrics().channels_count, 5);
    }
}