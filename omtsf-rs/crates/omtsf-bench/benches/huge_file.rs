//! Huge-tier benchmarks (~737K nodes, ~1.5M edges, ~500 MB JSON, 20-tier supply chain).
//!
//! This benchmark binary is intentionally separate from the smaller-tier benchmarks
//! so that `cargo bench` remains fast for development. Run via `just bench-huge`.
//!
//! The fixture is generated once to disk by `just gen-huge` and loaded here.
//! Setup is cached in a `OnceLock` so the deserialization cost is paid once.
#![allow(clippy::expect_used)]

use std::collections::HashSet;
use std::sync::OnceLock;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use omtsf_bench::huge_fixture_path;
use omtsf_core::enums::{EdgeType, EdgeTypeTag, NodeType, NodeTypeTag};
use omtsf_core::graph::queries::{self, Direction};
use omtsf_core::graph::{OmtsGraph, Selector, SelectorSet, extraction};
use omtsf_core::validation::{ValidationConfig, validate};
use omtsf_core::{OmtsFile, build_graph};

struct HugeSetup {
    json: String,
    file: OmtsFile,
    graph: OmtsGraph,
    node_count: usize,
    edge_count: usize,
    byte_size: u64,
    root_id: String,
    leaf_id: String,
    mid_id: String,
}

static SETUP: OnceLock<HugeSetup> = OnceLock::new();

fn get_setup() -> &'static HugeSetup {
    SETUP.get_or_init(|| {
        let path = huge_fixture_path();
        eprintln!("Loading huge fixture from {}...", path.display());
        let json = std::fs::read_to_string(&path)
            .expect("Failed to read huge fixture. Run `just gen-huge` first to generate it.");
        let byte_size = json.len() as u64;
        let file: OmtsFile = serde_json::from_str(&json).expect("deserialize huge fixture");
        let node_count = file.nodes.len();
        let edge_count = file.edges.len();
        eprintln!(
            "Huge tier ready: {} nodes, {} edges, {:.1} MB JSON",
            node_count,
            edge_count,
            byte_size as f64 / (1024.0 * 1024.0)
        );
        let graph = build_graph(&file).expect("builds");

        let root_id = file.nodes[0].id.to_string();
        let leaf_id = file.nodes[file.nodes.len() - 1].id.to_string();
        let mid_id = file.nodes[file.nodes.len() / 2].id.to_string();

        HugeSetup {
            json,
            file,
            graph,
            node_count,
            edge_count,
            byte_size,
            root_id,
            leaf_id,
            mid_id,
        }
    })
}

fn bench_huge_deserialize(c: &mut Criterion) {
    let s = get_setup();
    let mut group = c.benchmark_group("huge/deserialize");
    group.sample_size(10);
    group.measurement_time(std::time::Duration::from_secs(30));
    group.throughput(Throughput::Bytes(s.byte_size));

    group.bench_function(BenchmarkId::from_parameter("Huge"), |b| {
        b.iter(|| {
            let _: OmtsFile = serde_json::from_str(&s.json).expect("deserialize");
        });
    });
    group.finish();
}

fn bench_huge_serialize(c: &mut Criterion) {
    let s = get_setup();
    let mut group = c.benchmark_group("huge/serialize_compact");
    group.sample_size(10);
    group.measurement_time(std::time::Duration::from_secs(20));
    group.throughput(Throughput::Bytes(s.byte_size));

    group.bench_function(BenchmarkId::from_parameter("Huge"), |b| {
        b.iter(|| {
            let _ = serde_json::to_string(&s.file).expect("serialize");
        });
    });
    group.finish();
}

fn bench_huge_build_graph(c: &mut Criterion) {
    let s = get_setup();
    let elements = (s.node_count + s.edge_count) as u64;

    let mut group = c.benchmark_group("huge/build_graph");
    group.sample_size(10);
    group.measurement_time(std::time::Duration::from_secs(15));
    group.throughput(Throughput::Elements(elements));

    group.bench_function(BenchmarkId::from_parameter("Huge"), |b| {
        b.iter(|| {
            let _ = build_graph(&s.file).expect("builds");
        });
    });
    group.finish();
}

fn bench_huge_reachability(c: &mut Criterion) {
    let s = get_setup();
    let mut group = c.benchmark_group("huge/reachable_from");
    group.sample_size(20);
    group.measurement_time(std::time::Duration::from_secs(30));

    group.bench_function(BenchmarkId::new("forward_root", "Huge"), |b| {
        b.iter(|| {
            let _ = queries::reachable_from(&s.graph, &s.root_id, Direction::Forward, None)
                .expect("works");
        });
    });

    let filter: HashSet<EdgeTypeTag> = [EdgeTypeTag::Known(EdgeType::Supplies)]
        .into_iter()
        .collect();
    group.bench_function(BenchmarkId::new("filtered_supplies", "Huge"), |b| {
        b.iter(|| {
            let _ =
                queries::reachable_from(&s.graph, &s.root_id, Direction::Forward, Some(&filter))
                    .expect("works");
        });
    });

    group.bench_function(BenchmarkId::new("both_mid", "Huge"), |b| {
        b.iter(|| {
            let _ =
                queries::reachable_from(&s.graph, &s.mid_id, Direction::Both, None).expect("works");
        });
    });

    group.finish();
}

fn bench_huge_shortest_path(c: &mut Criterion) {
    let s = get_setup();
    let mut group = c.benchmark_group("huge/shortest_path");
    group.sample_size(20);
    group.measurement_time(std::time::Duration::from_secs(30));

    group.bench_function(BenchmarkId::new("root_to_leaf", "Huge"), |b| {
        b.iter(|| {
            let _ =
                queries::shortest_path(&s.graph, &s.root_id, &s.leaf_id, Direction::Forward, None)
                    .expect("works");
        });
    });

    group.bench_function(BenchmarkId::new("root_to_mid", "Huge"), |b| {
        b.iter(|| {
            let _ =
                queries::shortest_path(&s.graph, &s.root_id, &s.mid_id, Direction::Forward, None)
                    .expect("works");
        });
    });

    group.bench_function(BenchmarkId::new("no_path", "Huge"), |b| {
        b.iter(|| {
            let _ =
                queries::shortest_path(&s.graph, &s.leaf_id, &s.root_id, Direction::Forward, None)
                    .expect("works");
        });
    });

    group.finish();
}

fn bench_huge_selector_match(c: &mut Criterion) {
    let s = get_setup();
    let element_count = (s.node_count + s.edge_count) as u64;

    let mut group = c.benchmark_group("huge/selector_match");
    group.sample_size(20);
    group.throughput(Throughput::Elements(element_count));

    let label_ss = SelectorSet::from_selectors(vec![Selector::LabelKey("certified".to_owned())]);
    group.bench_function(BenchmarkId::new("label", "Huge"), |b| {
        b.iter(|| {
            let _ = extraction::selector_match(&s.file, &label_ss);
        });
    });

    let type_ss = SelectorSet::from_selectors(vec![Selector::NodeType(NodeTypeTag::Known(
        NodeType::Organization,
    ))]);
    group.bench_function(BenchmarkId::new("node_type", "Huge"), |b| {
        b.iter(|| {
            let _ = extraction::selector_match(&s.file, &type_ss);
        });
    });

    let multi_ss = SelectorSet::from_selectors(vec![
        Selector::NodeType(NodeTypeTag::Known(NodeType::Organization)),
        Selector::LabelKey("certified".to_owned()),
    ]);
    group.bench_function(BenchmarkId::new("multi", "Huge"), |b| {
        b.iter(|| {
            let _ = extraction::selector_match(&s.file, &multi_ss);
        });
    });

    group.finish();
}

fn bench_huge_selector_subgraph(c: &mut Criterion) {
    let s = get_setup();

    let mut group = c.benchmark_group("huge/selector_subgraph");
    group.sample_size(10);
    group.measurement_time(std::time::Duration::from_secs(60));

    let narrow_ss = SelectorSet::from_selectors(vec![Selector::NodeType(NodeTypeTag::Known(
        NodeType::Attestation,
    ))]);

    let output = extraction::selector_subgraph(&s.graph, &s.file, &narrow_ss, 0)
        .expect("attestations exist");
    let out_nodes = output.nodes.len() as u64;
    group.throughput(Throughput::Elements(out_nodes.max(1)));

    group.bench_function(BenchmarkId::new("narrow_exp0", "Huge"), |b| {
        b.iter(|| {
            let _ = extraction::selector_subgraph(&s.graph, &s.file, &narrow_ss, 0).expect("works");
        });
    });

    group.bench_function(BenchmarkId::new("narrow_exp1", "Huge"), |b| {
        b.iter(|| {
            let _ = extraction::selector_subgraph(&s.graph, &s.file, &narrow_ss, 1).expect("works");
        });
    });

    group.bench_function(BenchmarkId::new("narrow_exp3", "Huge"), |b| {
        b.iter(|| {
            let _ = extraction::selector_subgraph(&s.graph, &s.file, &narrow_ss, 3).expect("works");
        });
    });

    let broad_ss = SelectorSet::from_selectors(vec![Selector::NodeType(NodeTypeTag::Known(
        NodeType::Organization,
    ))]);

    group.bench_function(BenchmarkId::new("broad_exp0", "Huge"), |b| {
        b.iter(|| {
            let _ = extraction::selector_subgraph(&s.graph, &s.file, &broad_ss, 0).expect("works");
        });
    });

    group.bench_function(BenchmarkId::new("broad_exp1", "Huge"), |b| {
        b.iter(|| {
            let _ = extraction::selector_subgraph(&s.graph, &s.file, &broad_ss, 1).expect("works");
        });
    });

    group.finish();
}

fn bench_huge_validation(c: &mut Criterion) {
    let s = get_setup();
    let elements = (s.node_count + s.edge_count) as u64;

    let mut group = c.benchmark_group("huge/validation");
    group.sample_size(10);
    group.measurement_time(std::time::Duration::from_secs(60));
    group.throughput(Throughput::Elements(elements));

    group.bench_function(BenchmarkId::new("L1", "Huge"), |b| {
        let config = ValidationConfig {
            run_l1: true,
            run_l2: false,
            run_l3: false,
        };
        b.iter(|| {
            let _ = validate(&s.file, &config, None);
        });
    });

    group.bench_function(BenchmarkId::new("L1_L2_L3", "Huge"), |b| {
        let config = ValidationConfig {
            run_l1: true,
            run_l2: true,
            run_l3: true,
        };
        b.iter(|| {
            let _ = validate(&s.file, &config, None);
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_huge_deserialize,
    bench_huge_serialize,
    bench_huge_build_graph,
    bench_huge_reachability,
    bench_huge_shortest_path,
    bench_huge_selector_match,
    bench_huge_selector_subgraph,
    bench_huge_validation,
);
criterion_main!(benches);
