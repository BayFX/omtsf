//! Group 3: Graph query benchmarks (`reachable_from`, `shortest_path`, `all_paths`).
#![allow(clippy::expect_used)]

use std::collections::HashSet;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use omtsf_bench::{SizeTier, generate_supply_chain};
use omtsf_core::enums::{EdgeType, EdgeTypeTag};
use omtsf_core::graph::queries::{self, Direction};
use omtsf_core::{OmtsFile, build_graph};

struct Setup {
    #[allow(dead_code)]
    file: OmtsFile,
    graph: omtsf_core::graph::OmtsGraph,
    root_id: String,
    leaf_id: String,
    mid_id: String,
}

fn setup(tier: SizeTier) -> Setup {
    let file = generate_supply_chain(&tier.config(42));
    let graph = build_graph(&file).expect("builds");

    let root_id = file.nodes[0].id.to_string();

    let leaf_id = file.nodes[file.nodes.len() - 1].id.to_string();

    let mid_idx = file.nodes.len() / 2;
    let mid_id = file.nodes[mid_idx].id.to_string();

    Setup {
        file,
        graph,
        root_id,
        leaf_id,
        mid_id,
    }
}

fn bench_reachable_from(c: &mut Criterion) {
    let mut group = c.benchmark_group("reachable_from");

    for (name, tier) in [
        ("S", SizeTier::Small),
        ("M", SizeTier::Medium),
        ("L", SizeTier::Large),
        ("XL", SizeTier::XLarge),
    ] {
        let s = setup(tier);

        group.bench_function(BenchmarkId::new("forward_root", name), |b| {
            b.iter(|| {
                let _ = queries::reachable_from(&s.graph, &s.root_id, Direction::Forward, None)
                    .expect("works");
            });
        });

        group.bench_function(BenchmarkId::new("forward_leaf", name), |b| {
            b.iter(|| {
                let _ = queries::reachable_from(&s.graph, &s.leaf_id, Direction::Forward, None)
                    .expect("works");
            });
        });

        let filter: HashSet<EdgeTypeTag> = [EdgeTypeTag::Known(EdgeType::Supplies)]
            .into_iter()
            .collect();
        group.bench_function(BenchmarkId::new("filtered_supplies", name), |b| {
            b.iter(|| {
                let _ = queries::reachable_from(
                    &s.graph,
                    &s.root_id,
                    Direction::Forward,
                    Some(&filter),
                )
                .expect("works");
            });
        });

        group.bench_function(BenchmarkId::new("backward_root", name), |b| {
            b.iter(|| {
                let _ = queries::reachable_from(&s.graph, &s.root_id, Direction::Backward, None)
                    .expect("works");
            });
        });

        group.bench_function(BenchmarkId::new("both_mid", name), |b| {
            b.iter(|| {
                let _ = queries::reachable_from(&s.graph, &s.mid_id, Direction::Both, None)
                    .expect("works");
            });
        });
    }
    group.finish();
}

fn bench_shortest_path(c: &mut Criterion) {
    let mut group = c.benchmark_group("shortest_path");

    for (name, tier) in [
        ("S", SizeTier::Small),
        ("M", SizeTier::Medium),
        ("L", SizeTier::Large),
        ("XL", SizeTier::XLarge),
    ] {
        let s = setup(tier);

        group.bench_function(BenchmarkId::new("root_to_leaf", name), |b| {
            b.iter(|| {
                let _ = queries::shortest_path(
                    &s.graph,
                    &s.root_id,
                    &s.leaf_id,
                    Direction::Forward,
                    None,
                )
                .expect("works");
            });
        });

        group.bench_function(BenchmarkId::new("root_to_mid", name), |b| {
            b.iter(|| {
                let _ = queries::shortest_path(
                    &s.graph,
                    &s.root_id,
                    &s.mid_id,
                    Direction::Forward,
                    None,
                )
                .expect("works");
            });
        });

        group.bench_function(BenchmarkId::new("no_path", name), |b| {
            b.iter(|| {
                let _ = queries::shortest_path(
                    &s.graph,
                    &s.leaf_id,
                    &s.root_id,
                    Direction::Forward,
                    None,
                )
                .expect("works");
            });
        });
    }
    group.finish();
}

fn bench_all_paths(c: &mut Criterion) {
    let mut group = c.benchmark_group("all_paths");
    group.sample_size(20);

    for (name, tier) in [("S", SizeTier::Small), ("M", SizeTier::Medium)] {
        let s = setup(tier);

        let reachable =
            queries::reachable_from(&s.graph, &s.root_id, Direction::Forward, None).expect("works");
        if let Some(&target_idx) = reachable.iter().next() {
            let target_id = s
                .graph
                .node_weight(target_idx)
                .expect("exists")
                .local_id
                .clone();

            for depth in [5, 10] {
                group.bench_function(BenchmarkId::new(format!("depth_{depth}"), name), |b| {
                    b.iter(|| {
                        let _ = queries::all_paths(
                            &s.graph,
                            &s.root_id,
                            &target_id,
                            depth,
                            Direction::Forward,
                            None,
                        )
                        .expect("works");
                    });
                });
            }
        }
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_reachable_from,
    bench_shortest_path,
    bench_all_paths
);
criterion_main!(benches);
