//! Group 6: Validation benchmarks (L1, L1+L2, L1+L2+L3).
#![allow(clippy::expect_used)]

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use omtsf_bench::{SizeTier, generate_supply_chain};
use omtsf_core::validation::{ValidationConfig, validate};

fn bench_validation(c: &mut Criterion) {
    let mut group = c.benchmark_group("validation");

    for (name, tier) in [
        ("S", SizeTier::Small),
        ("M", SizeTier::Medium),
        ("L", SizeTier::Large),
        ("XL", SizeTier::XLarge),
    ] {
        let file = generate_supply_chain(&tier.config(42));
        let elements = (file.nodes.len() + file.edges.len()) as u64;

        group.throughput(Throughput::Elements(elements));

        group.bench_with_input(BenchmarkId::new("L1", name), &file, |b, file| {
            let config = ValidationConfig {
                run_l1: true,
                run_l2: false,
                run_l3: false,
            };
            b.iter(|| {
                let _ = validate(file, &config, None);
            });
        });

        group.bench_with_input(BenchmarkId::new("L1_L2", name), &file, |b, file| {
            let config = ValidationConfig {
                run_l1: true,
                run_l2: true,
                run_l3: false,
            };
            b.iter(|| {
                let _ = validate(file, &config, None);
            });
        });

        group.bench_with_input(BenchmarkId::new("L1_L2_L3", name), &file, |b, file| {
            let config = ValidationConfig {
                run_l1: true,
                run_l2: true,
                run_l3: true,
            };
            b.iter(|| {
                let _ = validate(file, &config, None);
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_validation);
criterion_main!(benches);
