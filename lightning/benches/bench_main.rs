use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
use ethers::types::{Address, H256, U256};
use flashchain_lightning::{
    channel::{Channel, ChannelManager},
    network::{NetworkManager, NetworkMessage},
    state::{StateManager, ChannelState},
    routing::{RoutingManager, Route},
};
use rand::Rng;

mod channel_benchmarks;
mod network_benchmarks;
mod routing_benchmarks;
mod state_benchmarks;

criterion_group!(
    benches,
    channel_benchmarks::bench_channel_operations,
    network_benchmarks::bench_network_operations,
    routing_benchmarks::bench_routing_operations,
    state_benchmarks::bench_state_operations,
);
criterion_main!(benches);

// Utility functions for benchmarks
pub fn setup_test_channel() -> Channel {
    let channel_id = H256::random();
    let participants = vec![Address::random(), Address::random()];
    let capacity = U256::from(1_000_000);
    
    Channel::new(channel_id, participants, capacity).unwrap()
}

pub fn generate_random_route(num_hops: usize) -> Route {
    let mut path = Vec::with_capacity(num_hops);
    let mut channels = Vec::with_capacity(num_hops);
    
    for _ in 0..num_hops {
        path.push(H256::random());
        channels.push(generate_random_channel_hop());
    }

    Route {
        path,
        channels,
        total_amount: U256::from(1_000_000),
        total_fees: U256::from(1_000),
        total_timelock: 144,
    }
}

fn generate_random_channel_hop() -> ChannelHop {
    ChannelHop {
        channel_id: H256::random(),
        source: Address::random(),
        target: Address::random(),
        amount: U256::from(100_000),
        fee: U256::from(100),
        timelock: 40,
    }
}

pub fn setup_network_with_nodes(num_nodes: usize) -> NetworkManager {
    let mut network = NetworkManager::new(NetworkConfig {
        max_peers: num_nodes * 2,
        heartbeat_interval: 60,
        connection_timeout: 30,
        max_retry_attempts: 3,
        bandwidth_limit: 1000.0,
    });

    for _ in 0..num_nodes {
        let address = Address::random();
        network.connect_peer(PeerInfo {
            address,
            endpoint: format!("127.0.0.1:{}", rand::random::<u16>()),
            shard_id: 0,
            version: "1.0.0".to_string(),
            capabilities: vec![PeerCapability::FullNode],
            last_seen: chrono::Utc::now().timestamp() as u64,
        }).await.unwrap();
    }

    network
}

pub fn generate_random_payment_hash() -> H256 {
    H256::random()
}

pub fn generate_random_preimage() -> H256 {
    H256::random()
}