use criterion::{Criterion, BenchmarkId};
use super::*;

pub fn bench_state_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("State Operations");

    // Benchmark state creation
    group.bench_function("state_creation", |b| {
        b.iter(|| {
            let channel_id = H256::random();
            let participants = vec![Address::random(), Address::random()];
            let capacity = U256::from(1_000_000);
            ChannelState::new(channel_id, participants, capacity);
        });
    });

    // Benchmark state updates with different sizes
    let update_sizes = vec![1, 10, 100];
    for &size in &update_sizes {
        group.bench_with_input(
            BenchmarkId::new("state_update", size),
            &size,
            |b, &size| {
                let mut state = setup_test_channel_state();
                let updates = generate_batch_updates(size);
                b.iter(|| {
                    for update in &updates {
                        state.apply_update(update.clone()).unwrap();
                    }
                });
            },
        );
    }

    // Benchmark state verification
    group.bench_function("state_verification", |b| {
        let state = setup_test_channel_state();
        let update = generate_random_state_update();
        b.iter(|| {
            state.verify_state_update(&update);
        });
    });

    // Benchmark state persistence
    group.bench_function("state_persistence", |b| {
        let state = setup_test_channel_state();
        b.iter(|| {
            state.persist_state();
        });
    });

    group.finish();
}

fn setup_test_channel_state() -> ChannelState {
    ChannelState::new(
        H256::random(),
        vec![Address::random(), Address::random()],
        U256::from(1_000_000),
    )
}

fn generate_batch_updates(size: usize) -> Vec<StateUpdate> {
    (0..size).map(|_| generate_random_state_update()).collect()
}