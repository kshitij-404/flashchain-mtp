use std::collections::{HashMap, HashSet, BinaryHeap};
use std::cmp::Ordering;
use ethers::types::{Address, H256, U256};
use tokio::sync::RwLock;
use serde::{Serialize, Deserialize};

use super::RoutingError;
use super::RoutingPolicy;
use crate::channel::Channel;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteHint {
    pub channel_id: H256,
    pub source: Address,
    pub target: Address,
    pub fee_rate: u32,
    pub timelock_delta: u64,
}

#[derive(Debug, Clone)]
struct Node {
    address: Address,
    channels: HashSet<H256>,
}

#[derive(Debug, Clone)]
struct ChannelInfo {
    source: Address,
    target: Address,
    capacity: U256,
    fee_rate: u32,
    timelock_delta: u64,
    reliability: f64,
}

#[derive(Debug)]
struct PathState {
    node: Address,
    cost: u64,
    capacity: U256,
    path: Vec<H256>,
}

impl Ord for PathState {
    fn cmp(&self, other: &Self) -> Ordering {
        other.cost.cmp(&self.cost)
    }
}

impl PartialOrd for PathState {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for PathState {
    fn eq(&self, other: &Self) -> bool {
        self.cost == other.cost
    }
}

impl Eq for PathState {}

pub struct PathFinder {
    nodes: RwLock<HashMap<Address, Node>>,
    channels: RwLock<HashMap<H256, ChannelInfo>>,
    reliability_history: RwLock<HashMap<H256, Vec<bool>>>,
}

impl PathFinder {
    pub fn new() -> Self {
        Self {
            nodes: RwLock::new(HashMap::new()),
            channels: RwLock::new(HashMap::new()),
            reliability_history: RwLock::new(HashMap::new()),
        }
    }

    pub async fn find_paths(
        &self,
        channels: &HashMap<H256, Channel>,
        source: Address,
        target: Address,
        amount: U256,
        hints: Option<Vec<RouteHint>>,
        policy: &RoutingPolicy,
    ) -> Result<Vec<Vec<H256>>, RoutingError> {
        // Apply route hints if available
        if let Some(hints) = hints {
            self.apply_route_hints(hints).await?;
        }

        // Initialize data structures for pathfinding
        let mut visited = HashSet::new();
        let mut paths = Vec::new();
        let mut queue = BinaryHeap::new();

        // Initialize starting point
        queue.push(PathState {
            node: source,
            cost: 0,
            capacity: amount,
            path: Vec::new(),
        });

        while let Some(current) = queue.pop() {
            if current.node == target {
                paths.push(current.path);
                if paths.len() >= 3 {  // Limit number of paths
                    break;
                }
                continue;
            }

            if visited.contains(&current.node) {
                continue;
            }
            visited.insert(current.node);

            // Get outgoing channels
            let nodes = self.nodes.read().await;
            let channels_info = self.channels.read().await;
            
            if let Some(node) = nodes.get(&current.node) {
                for &channel_id in &node.channels {
                    if let Some(channel_info) = channels_info.get(&channel_id) {
                        // Skip if channel doesn't have enough capacity
                        if channel_info.capacity < current.capacity {
                            continue;
                        }

                        // Skip if path would exceed max hops
                        if current.path.len() >= policy.max_hops {
                            continue;
                        }

                        let next_node = if channel_info.source == current.node {
                            channel_info.target
                        } else {
                            channel_info.source
                        };

                        if !visited.contains(&next_node) {
                            let mut new_path = current.path.clone();
                            new_path.push(channel_id);

                            let new_cost = self.calculate_path_cost(
                                &new_path,
                                current.capacity,
                                &channels_info,
                            )?;

                            queue.push(PathState {
                                node: next_node,
                                cost: new_cost,
                                capacity: current.capacity,
                                path: new_path,
                            });
                        }
                    }
                }
            }
        }

        if paths.is_empty() {
            return Err(RoutingError::NoRoute("No valid paths found".into()));
        }

        Ok(paths)
    }

    pub async fn update_channel(
        &self,
        channel_id: H256,
        capacity: U256,
        fee_rate: u32,
    ) -> Result<(), RoutingError> {
        let mut channels = self.channels.write().await;
        
        if let Some(channel_info) = channels.get_mut(&channel_id) {
            channel_info.capacity = capacity;
            channel_info.fee_rate = fee_rate;
        }

        Ok(())
    }

    pub async fn record_payment_result(
        &self,
        path: &[H256],
        success: bool,
    ) -> Result<(), RoutingError> {
        let mut reliability = self.reliability_history.write().await;
        
        for &channel_id in path {
            let history = reliability.entry(channel_id)
                .or_insert_with(Vec::new);
            
            history.push(success);
            if history.len() > 100 {  // Keep last 100 results
                history.remove(0);
            }
        }

        Ok(())
    }

    async fn apply_route_hints(&self, hints: Vec<RouteHint>) -> Result<(), RoutingError> {
        let mut channels = self.channels.write().await;
        let mut nodes = self.nodes.write().await;

        for hint in hints {
            channels.insert(hint.channel_id, ChannelInfo {
                source: hint.source,
                target: hint.target,
                capacity: U256::max_value(), // Assume sufficient capacity
                fee_rate: hint.fee_rate,
                timelock_delta: hint.timelock_delta,
                reliability: 1.0,
            });

            // Update node information
            for &address in &[hint.source, hint.target] {
                let node = nodes.entry(address)
                    .or_insert_with(|| Node {
                        address,
                        channels: HashSet::new(),
                    });
                node.channels.insert(hint.channel_id);
            }
        }

        Ok(())
    }

    fn calculate_path_cost(
        &self,
        path: &[H256],
        amount: U256,
        channels: &HashMap<H256, ChannelInfo>,
    ) -> Result<u64, RoutingError> {
        let mut total_cost = 0u64;

        for &channel_id in path {
            if let Some(channel) = channels.get(&channel_id) {
                // Calculate fee
                let fee = (amount.as_u64() * channel.fee_rate as u64) / 1_000_000;
                
                // Add timelock penalty
                let timelock_cost = channel.timelock_delta * 10;  // Weight factor for timelock

                // Add reliability factor
                let reliability_cost = ((1.0 - channel.reliability) * 1000.0) as u64;

                total_cost += fee + timelock_cost + reliability_cost;
            }
        }

        Ok(total_cost)
    }

    pub async fn get_channel_reliability(&self, channel_id: H256) -> f64 {
        let reliability = self.reliability_history.read().await;
        
        if let Some(history) = reliability.get(&channel_id) {
            if history.is_empty() {
                return 1.0;
            }
            
            let successful = history.iter().filter(|&&result| result).count();
            return successful as f64 / history.len() as f64;
        }

        1.0  // Default to perfect reliability if no history
    }

    pub async fn prune_unreliable_channels(&self, threshold: f64) -> Result<(), RoutingError> {
        let mut channels = self.channels.write().await;
        let mut nodes = self.nodes.write().await;

        channels.retain(|channel_id, channel_info| {
            let reliability = self.get_channel_reliability(*channel_id).await;
            let retain = reliability >= threshold;

            if !retain {
                // Remove channel from nodes
                if let Some(node) = nodes.get_mut(&channel_info.source) {
                    node.channels.remove(channel_id);
                }
                if let Some(node) = nodes.get_mut(&channel_info.target) {
                    node.channels.remove(channel_id);
                }
            }

            retain
        });

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_path_finding() {
        let path_finder = PathFinder::new();
        
        // Setup test network
        let source = Address::random();
        let target = Address::random();
        let intermediate = Address::random();

        let channel1 = H256::random();
        let channel2 = H256::random();

        {
            let mut channels = path_finder.channels.write().await;
            let mut nodes = path_finder.nodes.write().await;

            // Add channels
            channels.insert(channel1, ChannelInfo {
                source,
                target: intermediate,
                capacity: U256::from(1000000),
                fee_rate: 100,
                timelock_delta: 40,
                reliability: 1.0,
            });

            channels.insert(channel2, ChannelInfo {
                source: intermediate,
                target,
                capacity: U256::from(1000000),
                fee_rate: 100,
                timelock_delta: 40,
                reliability: 1.0,
            });

            // Add nodes
            for &address in &[source, target, intermediate] {
                let node = nodes.entry(address)
                    .or_insert_with(|| Node {
                        address,
                        channels: HashSet::new(),
                    });
                
                if address == source || address == intermediate {
                    node.channels.insert(channel1);
                }
                if address == intermediate || address == target {
                    node.channels.insert(channel2);
                }
            }
        }

        let policy = RoutingPolicy {
            max_hops: 3,
            max_timelock: 144,
            max_fee_rate: 1000,
            min_channel_capacity: U256::from(1000),
        };

        let paths = path_finder.find_paths(
            &HashMap::new(),
            source,
            target,
            U256::from(1000),
            None,
            &policy,
        ).await.unwrap();

        assert!(!paths.is_empty());
        assert_eq!(paths[0], vec![channel1, channel2]);
    }

    #[tokio::test]
    async fn test_reliability_tracking() {
        let path_finder = PathFinder::new();
        let channel_id = H256::random();

        // Record some payment results
        path_finder.record_payment_result(&[channel_id], true).await.unwrap();
        path_finder.record_payment_result(&[channel_id], true).await.unwrap();
        path_finder.record_payment_result(&[channel_id], false).await.unwrap();

        let reliability = path_finder.get_channel_reliability(channel_id).await;
        assert_eq!(reliability, 2.0 / 3.0);
    }
}