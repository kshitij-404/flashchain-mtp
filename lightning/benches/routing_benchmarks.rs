use criterion::{Criterion, BenchmarkId};
use super::*;

pub fn bench_routing_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("Routing Operations");

    // Benchmark path finding with different network sizes
    let network_sizes = vec![10, 50, 100];
    for &size in &network_sizes {
        group.bench_with_input(
            BenchmarkId::new("path_finding", size),
            &size,
            |b, &size| {
                let network = setup_network_with_nodes(size);
                let source = Address::random();
                let target = Address::random();
                let amount = U256::from(1000);
                
                b.iter(|| {
                    network.find_route(source, target, amount, None);
                });
            },
        );
    }

    // Benchmark payment sending
    let hop_counts = vec![2, 5, 10];
    for &hops in &hop_counts {
        group.bench_with_input(
            BenchmarkId::new("payment_sending", hops),
            &hops,
            |b, &hops| {
                let route = generate_random_route(hops);
                let payment_hash = generate_random_payment_hash();
                let payment_secret = generate_random_preimage();
                
                b.iter(|| {
                    network.send_payment(
                        route.clone(),
                        payment_hash,
                        payment_secret,
                    );
                });
            },
        );
    }

    // Benchmark route validation
    group.bench_function("route_validation", |b| {
        let route = generate_random_route(5);
        b.iter(|| {
            network.validate_route(&route);
        });
    });

    group.finish();
}