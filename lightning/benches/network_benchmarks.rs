use criterion::{Criterion, BenchmarkId};
use super::*;

pub fn bench_network_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("Network Operations");

    // Benchmark peer connection
    let node_counts = vec![10, 100, 1000];
    for &count in &node_counts {
        group.bench_with_input(
            BenchmarkId::new("peer_connection", count),
            &count,
            |b, &count| {
                let network = setup_network_with_nodes(count);
                let peer_info = PeerInfo {
                    address: Address::random(),
                    endpoint: "127.0.0.1:8545".to_string(),
                    shard_id: 0,
                    version: "1.0.0".to_string(),
                    capabilities: vec![PeerCapability::FullNode],
                    last_seen: chrono::Utc::now().timestamp() as u64,
                };
                
                b.iter(|| {
                    network.connect_peer(peer_info.clone());
                });
            },
        );
    }

    // Benchmark message broadcasting
    for &count in &node_counts {
        group.bench_with_input(
            BenchmarkId::new("message_broadcast", count),
            &count,
            |b, &count| {
                let network = setup_network_with_nodes(count);
                let message = NetworkMessage::Heartbeat {
                    peer_address: Address::random(),
                    timestamp: chrono::Utc::now().timestamp() as u64,
                    metrics: PeerMetrics {
                        channels_count: 10,
                        active_transfers: 5,
                        pending_messages: 2,
                        bandwidth_usage: 100.0,
                        latency_ms: 50,
                    },
                };
                
                b.iter(|| {
                    network.broadcast_message(message.clone());
                });
            },
        );
    }

    // Benchmark topology updates
    group.bench_function("topology_optimization", |b| {
        let network = setup_network_with_nodes(100);
        b.iter(|| {
            network.optimize_topology();
        });
    });

    group.finish();
}