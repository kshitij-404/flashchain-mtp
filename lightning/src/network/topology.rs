use std::collections::{HashMap, HashSet};
use ethers::types::{Address, H256};
use priority_queue::PriorityQueue;
use serde::{Serialize, Deserialize};
use thiserror::Error;
use tokio::sync::RwLock;

use super::{NetworkError, PeerInfo};

#[derive(Error, Debug)]
pub enum TopologyError {
    #[error("Invalid shard connection: {0}")]
    InvalidConnection(String),
    #[error("Route not found between shards")]
    RouteNotFound,
    #[error("Capacity exceeded")]
    CapacityExceeded,
    #[error("Invalid topology state: {0}")]
    InvalidState(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardConnection {
    pub source_shard: u64,
    pub target_shard: u64,
    pub connection_id: H256,
    pub active_channels: u32,
    pub bandwidth: f64,
    pub latency: u64,
    pub reliability: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopologyMetrics {
    pub total_connections: usize,
    pub average_latency: f64,
    pub network_diameter: u32,
    pub connection_density: f64,
}

pub struct NetworkTopology {
    // Shard-to-shard connections
    connections: HashMap<(u64, u64), ShardConnection>,
    // Peer-to-shard mapping
    peer_shards: HashMap<Address, u64>,
    // Shard routing table
    routing_table: HashMap<u64, HashMap<u64, Vec<u64>>>,
    // Connection metrics
    metrics: RwLock<TopologyMetrics>,
    // Configuration
    max_connections_per_shard: usize,
    min_reliability_threshold: f64,
}

impl NetworkTopology {
    pub fn new() -> Self {
        Self {
            connections: HashMap::new(),
            peer_shards: HashMap::new(),
            routing_table: HashMap::new(),
            metrics: RwLock::new(TopologyMetrics {
                total_connections: 0,
                average_latency: 0.0,
                network_diameter: 0,
                connection_density: 0.0,
            }),
            max_connections_per_shard: 10,
            min_reliability_threshold: 0.95,
        }
    }

    pub fn add_peer(&mut self, peer_info: PeerInfo) -> Result<(), NetworkError> {
        self.peer_shards.insert(peer_info.address, peer_info.shard_id);
        self.update_routing_table()?;
        Ok(())
    }

    pub fn remove_peer(&mut self, address: Address) -> Result<(), NetworkError> {
        self.peer_shards.remove(&address);
        self.update_routing_table()?;
        Ok(())
    }

    pub async fn establish_connection(
        &mut self,
        source_shard: u64,
        target_shard: u64,
        bandwidth: f64,
        latency: u64,
    ) -> Result<H256, NetworkError> {
        // Validate connection
        if source_shard == target_shard {
            return Err(NetworkError::TopologyError(
                TopologyError::InvalidConnection("Self-connection not allowed".into())
            ));
        }

        let connection_id = self.generate_connection_id(source_shard, target_shard);
        let connection = ShardConnection {
            source_shard,
            target_shard,
            connection_id,
            active_channels: 0,
            bandwidth,
            latency,
            reliability: 1.0,
        };

        self.connections.insert((source_shard, target_shard), connection);
        self.update_routing_table()?;

        Ok(connection_id)
    }

    pub async fn find_route(
        &self,
        source_shard: u64,
        target_shard: u64,
    ) -> Result<Vec<u64>, NetworkError> {
        // Implement Dijkstra's algorithm for route finding
        let mut distances: HashMap<u64, f64> = HashMap::new();
        let mut previous: HashMap<u64, u64> = HashMap::new();
        let mut queue = PriorityQueue::new();

        // Initialize distances
        for &shard in self.peer_shards.values() {
            distances.insert(shard, f64::INFINITY);
        }
        distances.insert(source_shard, 0.0);
        queue.push(source_shard, std::cmp::Reverse(0.0));

        while let Some((current_shard, _)) = queue.pop() {
            if current_shard == target_shard {
                return Ok(self.reconstruct_path(source_shard, target_shard, &previous));
            }

            let current_distance = *distances.get(&current_shard).unwrap();

            // Check all neighbors
            for (neighbor_shard, connection) in self.get_shard_connections(current_shard) {
                let distance = current_distance + connection.latency as f64;
                if distance < *distances.get(&neighbor_shard).unwrap_or(&f64::INFINITY) {
                    distances.insert(neighbor_shard, distance);
                    previous.insert(neighbor_shard, current_shard);
                    queue.push(neighbor_shard, std::cmp::Reverse(distance));
                }
            }
        }

        Err(NetworkError::TopologyError(TopologyError::RouteNotFound))
    }

    pub async fn optimize(&mut self) -> Result<(), NetworkError> {
        // Implement topology optimization logic
        self.balance_connections()?;
        self.prune_unreliable_connections()?;
        self.update_metrics().await?;
        Ok(())
    }

    pub fn get_connection_metrics(&self, connection_id: H256) -> Option<ShardConnection> {
        self.connections.values()
            .find(|conn| conn.connection_id == connection_id)
            .cloned()
    }

    pub async fn update_connection_metrics(
        &mut self,
        connection_id: H256,
        bandwidth: f64,
        latency: u64,
        reliability: f64,
    ) -> Result<(), NetworkError> {
        for connection in self.connections.values_mut() {
            if connection.connection_id == connection_id {
                connection.bandwidth = bandwidth;
                connection.latency = latency;
                connection.reliability = reliability;
                return Ok(());
            }
        }
        Err(NetworkError::TopologyError(
            TopologyError::InvalidConnection("Connection not found".into())
        ))
    }

    // Helper methods

    fn generate_connection_id(&self, source_shard: u64, target_shard: u64) -> H256 {
        let mut data = Vec::new();
        data.extend_from_slice(&source_shard.to_be_bytes());
        data.extend_from_slice(&target_shard.to_be_bytes());
        H256::from_slice(&keccak256(&data))
    }

    fn get_shard_connections(&self, shard_id: u64) -> Vec<(u64, ShardConnection)> {
        self.connections.iter()
            .filter(|((source, target), _)| *source == shard_id || *target == shard_id)
            .map(|((_, target), conn)| (*target, conn.clone()))
            .collect()
    }

    fn reconstruct_path(
        &self,
        source: u64,
        target: u64,
        previous: &HashMap<u64, u64>,
    ) -> Vec<u64> {
        let mut path = vec![target];
        let mut current = target;

        while current != source {
            current = *previous.get(&current).unwrap();
            path.push(current);
        }

        path.reverse();
        path
    }

    fn update_routing_table(&mut self) -> Result<(), NetworkError> {
        self.routing_table.clear();

        // Build routing table using Floyd-Warshall algorithm
        let shards: HashSet<u64> = self.peer_shards.values().cloned().collect();
        
        for &shard in &shards {
            let mut routes = HashMap::new();
            routes.insert(shard, Vec::new());
            self.routing_table.insert(shard, routes);
        }

        for ((source, target), connection) in &self.connections {
            if connection.reliability >= self.min_reliability_threshold {
                self.routing_table.get_mut(source)
                    .unwrap()
                    .insert(*target, vec![*target]);
            }
        }

        Ok(())
    }

    async fn update_metrics(&mut self) -> Result<(), NetworkError> {
        let total_connections = self.connections.len();
        let mut total_latency = 0.0;
        let mut max_diameter = 0;

        for connection in self.connections.values() {
            total_latency += connection.latency as f64;
            max_diameter = max_diameter.max(connection.latency);
        }

        let average_latency = if total_connections > 0 {
            total_latency / total_connections as f64
        } else {
            0.0
        };

        let mut metrics = self.metrics.write().await;
        *metrics = TopologyMetrics {
            total_connections,
            average_latency,
            network_diameter: max_diameter as u32,
            connection_density: self.calculate_connection_density(),
        };

        Ok(())
    }

    fn calculate_connection_density(&self) -> f64 {
        let total_shards = self.peer_shards.values().collect::<HashSet<_>>().len();
        if total_shards <= 1 {
            return 0.0;
        }

        let max_possible_connections = (total_shards * (total_shards - 1)) / 2;
        self.connections.len() as f64 / max_possible_connections as f64
    }

    fn balance_connections(&mut self) -> Result<(), NetworkError> {
        // Implement load balancing logic
        Ok(())
    }

    fn prune_unreliable_connections(&mut self) -> Result<(), NetworkError> {
        self.connections.retain(|_, conn| {
            conn.reliability >= self.min_reliability_threshold
        });
        Ok(())
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

    #[tokio::test]
    async fn test_connection_establishment() {
        let mut topology = NetworkTopology::new();
        let connection_id = topology.establish_connection(1, 2, 1000.0, 50)
            .await
            .unwrap();

        assert!(topology.get_connection_metrics(connection_id).is_some());
    }

    #[tokio::test]
    async fn test_route_finding() {
        let mut topology = NetworkTopology::new();
        
        // Create a simple network
        topology.establish_connection(1, 2, 1000.0, 50).await.unwrap();
        topology.establish_connection(2, 3, 1000.0, 50).await.unwrap();

        let route = topology.find_route(1, 3).await.unwrap();
        assert_eq!(route, vec![1, 2, 3]);
    }

    #[tokio::test]
    async fn test_metrics_update() {
        let mut topology = NetworkTopology::new();
        
        topology.establish_connection(1, 2, 1000.0, 50).await.unwrap();
        topology.establish_connection(2, 3, 1000.0, 50).await.unwrap();
        
        topology.optimize().await.unwrap();
        
        let metrics = topology.metrics.read().await;
        assert!(metrics.total_connections > 0);
        assert!(metrics.average_latency > 0.0);
    }
}