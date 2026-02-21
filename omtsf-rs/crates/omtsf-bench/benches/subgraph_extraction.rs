//! Group 4: Subgraph extraction benchmarks (`induced_subgraph`, `ego_graph`).
#![allow(clippy::expect_used)]

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use omtsf_bench::{SizeTier, generate_supply_chain};
use omtsf_core::graph::extraction;
use omtsf_core::graph::queries::Direction;
use omtsf_core::{OmtsFile, build_graph};

struct Setup {
    file: OmtsFile,
    graph: omtsf_core::graph::OmtsGraph,
}

fn setup(tier: SizeTier) -> Setup {
    let file = generate_supply_chain(&tier.config(42));
    let graph = build_graph(&file).expect("builds");
    Setup { file, graph }
}

fn bench_induced_subgraph(c: &mut Criterion) {
    let mut group = c.benchmark_group("induced_subgraph");

    for (name, tier) in [
        ("S", SizeTier::Small),
        ("M", SizeTier::Medium),
        ("L", SizeTier::Large),
    ] {
        let s = setup(tier);
        let all_ids: Vec<&str> = s.file.nodes.iter().map(|n| n.id.as_ref()).collect();
        let n = all_ids.len();

        let pct10: Vec<&str> = all_ids[..n / 10].to_vec();
        group.bench_function(BenchmarkId::new("10pct", name), |b| {
            b.iter(|| {
                let _ = extraction::induced_subgraph(&s.graph, &s.file, &pct10).expect("works");
            });
        });

        let pct25: Vec<&str> = all_ids[..n / 4].to_vec();
        group.bench_function(BenchmarkId::new("25pct", name), |b| {
            b.iter(|| {
                let _ = extraction::induced_subgraph(&s.graph, &s.file, &pct25).expect("works");
            });
        });

        let pct50: Vec<&str> = all_ids[..n / 2].to_vec();
        group.bench_function(BenchmarkId::new("50pct", name), |b| {
            b.iter(|| {
                let _ = extraction::induced_subgraph(&s.graph, &s.file, &pct50).expect("works");
            });
        });

        group.bench_function(BenchmarkId::new("100pct", name), |b| {
            b.iter(|| {
                let _ = extraction::induced_subgraph(&s.graph, &s.file, &all_ids).expect("works");
            });
        });
    }
    group.finish();
}

fn bench_ego_graph(c: &mut Criterion) {
    let mut group = c.benchmark_group("ego_graph");

    for (name, tier) in [
        ("S", SizeTier::Small),
        ("M", SizeTier::Medium),
        ("L", SizeTier::Large),
    ] {
        let s = setup(tier);
        let root_id = s.file.nodes[0].id.to_string();
        let mid_id = s.file.nodes[s.file.nodes.len() / 2].id.to_string();

        for radius in [1, 2, 3] {
            group.bench_function(BenchmarkId::new(format!("root_r{radius}"), name), |b| {
                b.iter(|| {
                    let _ =
                        extraction::ego_graph(&s.graph, &s.file, &root_id, radius, Direction::Both)
                            .expect("works");
                });
            });
        }

        group.bench_function(BenchmarkId::new("mid_r2", name), |b| {
            b.iter(|| {
                let _ = extraction::ego_graph(&s.graph, &s.file, &mid_id, 2, Direction::Both)
                    .expect("works");
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_induced_subgraph, bench_ego_graph);
criterion_main!(benches);
