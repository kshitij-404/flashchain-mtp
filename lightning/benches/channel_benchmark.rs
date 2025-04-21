use criterion::{Criterion, BenchmarkId};
use super::*;

pub fn bench_channel_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("Channel Operations");

    // Benchmark channel creation
    group.bench_function("channel_creation", |b| {
        b.iter(|| {
            setup_test_channel();
        });
    });

    // Benchmark state updates
    group.bench_function("state_update", |b| {
        let mut channel = setup_test_channel();
        let update = generate_random_state_update();
        b.iter(|| {
            channel.update_state(update.clone()).unwrap();
        });
    });

    // Benchmark HTLC creation with different amounts
    let amounts = vec![1000, 10000, 100000];
    for amount in amounts {
        group.bench_with_input(
            BenchmarkId::new("htlc_creation", amount),
            &amount,
            |b, &amount| {
                let mut channel = setup_test_channel();
                let payment_hash = generate_random_payment_hash();
                b.iter(|| {
                    channel.create_htlc(
                        channel.participants[0],
                        channel.participants[1],
                        U256::from(amount),
                        payment_hash,
                        100,
                    ).unwrap();
                });
            },
        );
    }

    // Benchmark HTLC fulfillment
    group.bench_function("htlc_fulfillment", |b| {
        let mut channel = setup_test_channel();
        let payment_hash = generate_random_payment_hash();
        let preimage = generate_random_preimage();
        let htlc_id = channel.create_htlc(
            channel.participants[0],
            channel.participants[1],
            U256::from(1000),
            payment_hash,
            100,
        ).unwrap();
        
        b.iter(|| {
            channel.fulfill_htlc(htlc_id, preimage).unwrap();
        });
    });

    // Benchmark signature verification
    group.bench_function("signature_verification", |b| {
        let channel = setup_test_channel();
        let state_hash = H256::random();
        let signature = vec![0u8; 65]; // Mock signature
        
        b.iter(|| {
            channel.verify_signature(
                &channel.participants[0],
                &state_hash,
                &signature,
            );
        });
    });

    group.finish();
}

fn generate_random_state_update() -> StateUpdate {
    StateUpdate {
        channel_id: H256::random(),
        sequence: rand::random::<u64>(),
        timestamp: chrono::Utc::now().timestamp() as u64,
        previous_state: H256::random(),
        new_state: H256::random(),
        signatures: HashMap::new(),
    }
}