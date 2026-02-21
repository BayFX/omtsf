//! Group 5: Selector-based query benchmarks (`selector_match`, `selector_subgraph`).
//!
//! **Group A — `selector_match`**: measures the raw scan throughput of the
//! selector engine over nodes and edges without assembling a subgraph.
//!
//! **Group B — `selector_subgraph`**: measures the full pipeline including
//! seed scan, BFS expansion, and induced-subgraph assembly.
#![allow(clippy::expect_used)]

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use omtsf_bench::{SizeTier, generate_supply_chain};
use omtsf_core::enums::{NodeType, NodeTypeTag};
use omtsf_core::graph::{Selector, SelectorSet, extraction};
use omtsf_core::newtypes::CountryCode;
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

// ---------------------------------------------------------------------------
// Group A: selector_match (scan only)
// ---------------------------------------------------------------------------

fn bench_selector_match_label(c: &mut Criterion) {
    let mut group = c.benchmark_group("selector_match/label");

    for (name, tier) in [
        ("S", SizeTier::Small),
        ("M", SizeTier::Medium),
        ("L", SizeTier::Large),
        ("XL", SizeTier::XLarge),
    ] {
        let s = setup(tier);
        let element_count = (s.file.nodes.len() + s.file.edges.len()) as u64;
        group.throughput(Throughput::Elements(element_count));

        // Label-only selector: --label certified
        let ss = SelectorSet::from_selectors(vec![Selector::LabelKey("certified".to_owned())]);

        group.bench_function(BenchmarkId::from_parameter(name), |b| {
            b.iter(|| {
                let _ = extraction::selector_match(&s.file, &ss);
            });
        });
    }
    group.finish();
}

fn bench_selector_match_node_type(c: &mut Criterion) {
    let mut group = c.benchmark_group("selector_match/node_type");

    for (name, tier) in [
        ("S", SizeTier::Small),
        ("M", SizeTier::Medium),
        ("L", SizeTier::Large),
        ("XL", SizeTier::XLarge),
    ] {
        let s = setup(tier);
        let element_count = (s.file.nodes.len() + s.file.edges.len()) as u64;
        group.throughput(Throughput::Elements(element_count));

        // Node-type selector: --node-type organization
        let ss = SelectorSet::from_selectors(vec![Selector::NodeType(NodeTypeTag::Known(
            NodeType::Organization,
        ))]);

        group.bench_function(BenchmarkId::from_parameter(name), |b| {
            b.iter(|| {
                let _ = extraction::selector_match(&s.file, &ss);
            });
        });
    }
    group.finish();
}

fn bench_selector_match_multi_selector(c: &mut Criterion) {
    let mut group = c.benchmark_group("selector_match/multi_selector");

    let de = CountryCode::try_from("DE").expect("valid");
    let fr = CountryCode::try_from("FR").expect("valid");

    for (name, tier) in [
        ("S", SizeTier::Small),
        ("M", SizeTier::Medium),
        ("L", SizeTier::Large),
        ("XL", SizeTier::XLarge),
    ] {
        let s = setup(tier);
        let element_count = (s.file.nodes.len() + s.file.edges.len()) as u64;
        group.throughput(Throughput::Elements(element_count));

        // Combined selectors: type=organization AND label=certified AND (jurisdiction=DE OR FR)
        let ss = SelectorSet::from_selectors(vec![
            Selector::NodeType(NodeTypeTag::Known(NodeType::Organization)),
            Selector::LabelKey("certified".to_owned()),
            Selector::Jurisdiction(de.clone()),
            Selector::Jurisdiction(fr.clone()),
        ]);

        group.bench_function(BenchmarkId::from_parameter(name), |b| {
            b.iter(|| {
                let _ = extraction::selector_match(&s.file, &ss);
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// Group B: selector_subgraph (full pipeline)
// ---------------------------------------------------------------------------

fn bench_selector_subgraph_narrow(c: &mut Criterion) {
    let mut group = c.benchmark_group("selector_subgraph/narrow");

    for (name, tier) in [
        ("S", SizeTier::Small),
        ("M", SizeTier::Medium),
        ("L", SizeTier::Large),
    ] {
        let s = setup(tier);

        // Narrow: ~5-10% match rate — attestation nodes only
        let ss = SelectorSet::from_selectors(vec![Selector::NodeType(NodeTypeTag::Known(
            NodeType::Attestation,
        ))]);

        // Pre-run to measure output size for throughput.
        let output =
            extraction::selector_subgraph(&s.graph, &s.file, &ss, 0).expect("attestations exist");
        let output_nodes = output.nodes.len() as u64;
        group.throughput(Throughput::Elements(output_nodes.max(1)));

        group.bench_function(BenchmarkId::from_parameter(name), |b| {
            b.iter(|| {
                let _ = extraction::selector_subgraph(&s.graph, &s.file, &ss, 0).expect("works");
            });
        });
    }
    group.finish();
}

fn bench_selector_subgraph_broad(c: &mut Criterion) {
    let mut group = c.benchmark_group("selector_subgraph/broad");

    for (name, tier) in [
        ("S", SizeTier::Small),
        ("M", SizeTier::Medium),
        ("L", SizeTier::Large),
    ] {
        let s = setup(tier);

        // Broad: ~45% match rate — organization nodes only
        let ss = SelectorSet::from_selectors(vec![Selector::NodeType(NodeTypeTag::Known(
            NodeType::Organization,
        ))]);

        let output =
            extraction::selector_subgraph(&s.graph, &s.file, &ss, 0).expect("organizations exist");
        let output_nodes = output.nodes.len() as u64;
        group.throughput(Throughput::Elements(output_nodes.max(1)));

        group.bench_function(BenchmarkId::from_parameter(name), |b| {
            b.iter(|| {
                let _ = extraction::selector_subgraph(&s.graph, &s.file, &ss, 0).expect("works");
            });
        });
    }
    group.finish();
}

fn bench_selector_subgraph_expand_1(c: &mut Criterion) {
    let mut group = c.benchmark_group("selector_subgraph/expand_1");

    for (name, tier) in [
        ("S", SizeTier::Small),
        ("M", SizeTier::Medium),
        ("L", SizeTier::Large),
    ] {
        let s = setup(tier);

        // Seed: attestation nodes; expand 1 hop
        let ss = SelectorSet::from_selectors(vec![Selector::NodeType(NodeTypeTag::Known(
            NodeType::Attestation,
        ))]);

        let output =
            extraction::selector_subgraph(&s.graph, &s.file, &ss, 1).expect("attestations exist");
        let output_nodes = output.nodes.len() as u64;
        group.throughput(Throughput::Elements(output_nodes.max(1)));

        group.bench_function(BenchmarkId::from_parameter(name), |b| {
            b.iter(|| {
                let _ = extraction::selector_subgraph(&s.graph, &s.file, &ss, 1).expect("works");
            });
        });
    }
    group.finish();
}

fn bench_selector_subgraph_expand_3(c: &mut Criterion) {
    let mut group = c.benchmark_group("selector_subgraph/expand_3");

    for (name, tier) in [
        ("S", SizeTier::Small),
        ("M", SizeTier::Medium),
        ("L", SizeTier::Large),
    ] {
        let s = setup(tier);

        // Seed: attestation nodes; expand 3 hops
        let ss = SelectorSet::from_selectors(vec![Selector::NodeType(NodeTypeTag::Known(
            NodeType::Attestation,
        ))]);

        let output =
            extraction::selector_subgraph(&s.graph, &s.file, &ss, 3).expect("attestations exist");
        let output_nodes = output.nodes.len() as u64;
        group.throughput(Throughput::Elements(output_nodes.max(1)));

        group.bench_function(BenchmarkId::from_parameter(name), |b| {
            b.iter(|| {
                let _ = extraction::selector_subgraph(&s.graph, &s.file, &ss, 3).expect("works");
            });
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_selector_match_label,
    bench_selector_match_node_type,
    bench_selector_match_multi_selector,
    bench_selector_subgraph_narrow,
    bench_selector_subgraph_broad,
    bench_selector_subgraph_expand_1,
    bench_selector_subgraph_expand_3,
);
criterion_main!(benches);
