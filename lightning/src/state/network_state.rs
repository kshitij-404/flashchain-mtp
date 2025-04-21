use std::collections::{HashMap, HashSet};
use ethers::types::{Address, H256};
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeState {
    pub address: Address,
    pub last_seen: u64,
    pub channels: HashSet<H256>,
    pub capacity: u64,
    pub reputation: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkState {
    pub nodes: HashMap<Address, NodeState>,
    pub channels: HashSet<H256>,
    pub routing_table: HashMap<Address, HashSet<Address>>,
    pub last_update: u64,
}

impl NetworkState {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            channels: HashSet::new(),
            routing_table: HashMap::new(),
            last_update: chrono::Utc::now().timestamp() as u64,
        }
    }

    pub fn add_node(&mut self, address: Address) {
        let node = NodeState {
            address,
            last_seen: chrono::Utc::now().timestamp() as u64,
            channels: HashSet::new(),
            capacity: 0,
            reputation: 1.0,
        };
        self.nodes.insert(address, node);
        self.routing_table.insert(address, HashSet::new());
    }

    pub fn remove_node(&mut self, address: &Address) {
        if let Some(node) = self.nodes.remove(address) {
            // Remove channels
            for channel_id in node.channels {
                self.channels.remove(&channel_id);
            }

            // Update routing table
            self.routing_table.remove(address);
            for connections in self.routing_table.values_mut() {
                connections.remove(address);
            }
        }
    }

    pub fn add_channel(&mut self, channel_id: H256, node1: Address, node2: Address) {
        self.channels.insert(channel_id);

        // Update node states
        if let Some(node) = self.nodes.get_mut(&node1) {
            node.channels.insert(channel_id);
        }
        if let Some(node) = self.nodes.get_mut(&node2) {
            node.channels.insert(channel_id);
        }

        // Update routing table
        if let Some(connections) = self.routing_table.get_mut(&node1) {
            connections.insert(node2);
        }
        if let Some(connections) = self.routing_table.get_mut(&node2) {
            connections.insert(node1);
        }
    }

    pub fn remove_channel(&mut self, channel_id: &H256) {
        self.channels.remove(channel_id);

        // Update node states
        for node in self.nodes.values_mut() {
            node.channels.remove(channel_id);
        }
    }

    pub fn update_node_reputation(&mut self, address: &Address, success: bool) {
        if let Some(node) = self.nodes.get_mut(address) {
            // Simple exponential moving average
            let alpha = 0.1;
            let success_value = if success { 1.0 } else { 0.0 };
            node.reputation = (1.0 - alpha) * node.reputation + alpha * success_value;
        }
    }

    pub fn get_connected_nodes(&self, address: &Address) -> HashSet<Address> {
        self.routing_table.get(address)
            .cloned()
            .unwrap_or_default()
    }

    pub fn is_route_available(&self, source: &Address, target: &Address) -> bool {
        let mut visited = HashSet::new();
        let mut queue = vec![*source];

        while let Some(current) = queue.pop() {
            if current == *target {
                return true;
            }

            if !visited.insert(current) {
                continue;
            }

            if let Some(neighbors) = self.routing_table.get(&current) {
                queue.extend(neighbors);
            }
        }

        false
    }

    pub fn cleanup_stale_nodes(&mut self, timeout: u64) {
        let current_time = chrono::Utc::now().timestamp() as u64;
        let stale_nodes: Vec<Address> = self.nodes.iter()
            .filter(|(_, node)| current_time - node.last_seen > timeout)
            .map(|(addr, _)| *addr)
            .collect();

        for address in stale_nodes {
            self.remove_node(&address);
        }
    }

    pub fn get_network_statistics(&self) -> NetworkStatistics {
        NetworkStatistics {
            total_nodes: self.nodes.len(),
            total_channels: self.channels.len(),
            average_node_degree: self.calculate_average_node_degree(),
            network_density: self.calculate_network_density(),
            average_reputation: self.calculate_average_reputation(),
        }
    }

    // Helper methods

    fn calculate_average_node_degree(&self) -> f64 {
        if self.nodes.is_empty() {
            return 0.0;
        }

        let total_degree: usize = self.routing_table.values()
            .map(|connections| connections.len())
            .sum();

        total_degree as f64 / self.nodes.len() as f64
    }

    fn calculate_network_density(&self) -> f64 {
        let node_count = self.nodes.len();
        if node_count <= 1 {
            return 0.0;
        }

        let max_edges = (node_count * (node_count - 1)) / 2;
        self.channels.len() as f64 / max_edges as f64
    }

    fn calculate_average_reputation(&self) -> f64 {
        if self.nodes.is_empty() {
            return 0.0;
        }

        let total_reputation: f64 = self.nodes.values()
            .map(|node| node.reputation)
            .sum();

        total_reputation / self.nodes.len() as f64
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkStatistics {
    pub total_nodes: usize,
    pub total_channels: usize,
    pub average_node_degree: f64,
    pub network_density: f64,
    pub average_reputation: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_state_operations() {
        let mut network = NetworkState::new();
        let node1 = Address::random();
        let node2 = Address::random();
        let channel = H256::random();

        // Add nodes
        network.add_node(node1);
        network.add_node(node2);
        assert_eq!(network.nodes.len(), 2);

        // Add channel
        network.add_channel(channel, node1, node2);
        assert!(network.channels.contains(&channel));
        assert!(network.is_route_available(&node1, &node2));

        // Update reputation
        network.update_node_reputation(&node1, true);
        assert!(network.nodes.get(&node1).unwrap().reputation > 0.0);

        // Remove channel
        network.remove_channel(&channel);
        assert!(!network.channels.contains(&channel));

        // Remove node
        network.remove_node(&node1);
        assert!(!network.nodes.contains_key(&node1));
    }

    #[test]
    fn test_network_statistics() {
        let mut network = NetworkState::new();
        let node1 = Address::random();
        let node2 = Address::random();
        let node3 = Address::random();

        network.add_node(node1);
        network.add_node(node2);
        network.add_node(node3);

        network.add_channel(H256::random(), node1, node2);
        network.add_channel(H256::random(), node2, node3);

        let stats = network.get_network_statistics();
        assert_eq!(stats.total_nodes, 3);
        assert_eq!(stats.total_channels, 2);
        assert!(stats.network_density > 0.0);
    }
}