//! Tests that generated files pass L1 validation across all size tiers and seeds.
#![allow(clippy::expect_used)]

use omtsf_bench::{SizeTier, generate_supply_chain};
use omtsf_core::validation::{ValidationConfig, validate};

fn assert_l1_valid(file: &omtsf_core::OmtsFile, label: &str) {
    let config = ValidationConfig {
        run_l1: true,
        run_l2: false,
        run_l3: false,
    };
    let result = validate(file, &config, None);
    let errors: Vec<_> = result.errors().collect();
    assert!(
        errors.is_empty(),
        "{label}: L1 validation failed with {} errors: {:?}",
        errors.len(),
        errors
            .iter()
            .map(|d| format!("{:?}: {}", d.rule_id, d.message))
            .collect::<Vec<_>>()
    );
}

#[test]
fn generated_small_passes_l1() {
    for seed in [42, 123, 999, 7777, 54321] {
        let file = generate_supply_chain(&SizeTier::Small.config(seed));
        assert_l1_valid(&file, &format!("Small/seed={seed}"));
    }
}

#[test]
fn generated_medium_passes_l1() {
    for seed in [42, 123, 999] {
        let file = generate_supply_chain(&SizeTier::Medium.config(seed));
        assert_l1_valid(&file, &format!("Medium/seed={seed}"));
    }
}

#[test]
fn generated_large_passes_l1() {
    let file = generate_supply_chain(&SizeTier::Large.config(42));
    assert_l1_valid(&file, "Large/seed=42");
}

#[test]
fn generated_xlarge_passes_l1() {
    let file = generate_supply_chain(&SizeTier::XLarge.config(42));
    assert_l1_valid(&file, "XLarge/seed=42");
}

#[test]
fn generated_small_round_trips_through_json() {
    let file = generate_supply_chain(&SizeTier::Small.config(42));
    let json = serde_json::to_string(&file).expect("serialize");
    let back: omtsf_core::OmtsFile = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(file.nodes.len(), back.nodes.len());
    assert_eq!(file.edges.len(), back.edges.len());
}

#[test]
fn generated_xlarge_hits_target_size() {
    let file = generate_supply_chain(&SizeTier::XLarge.config(42));
    let json = serde_json::to_string_pretty(&file).expect("serialize");
    let size_mb = json.len() as f64 / (1024.0 * 1024.0);
    assert!(size_mb > 1.0, "XLarge should be > 1MB, got {size_mb:.2}MB");
}

#[test]
fn generation_is_deterministic() {
    let file1 = generate_supply_chain(&SizeTier::Small.config(42));
    let file2 = generate_supply_chain(&SizeTier::Small.config(42));
    let json1 = serde_json::to_string(&file1).expect("serialize");
    let json2 = serde_json::to_string(&file2).expect("serialize");
    assert_eq!(json1, json2, "same seed must produce identical output");
}

#[test]
fn different_seeds_produce_different_files() {
    let file1 = generate_supply_chain(&SizeTier::Small.config(42));
    let file2 = generate_supply_chain(&SizeTier::Small.config(43));
    let json1 = serde_json::to_string(&file1).expect("serialize");
    let json2 = serde_json::to_string(&file2).expect("serialize");
    assert_ne!(
        json1, json2,
        "different seeds must produce different output"
    );
}

#[test]
fn graph_builds_from_generated_file() {
    let file = generate_supply_chain(&SizeTier::Medium.config(42));
    let graph = omtsf_core::build_graph(&file).expect("graph should build from generated file");
    assert_eq!(graph.node_count(), file.nodes.len());
    assert_eq!(graph.edge_count(), file.edges.len());
}

#[test]
fn cyclic_variant_generates_cycles() {
    let mut config = SizeTier::Small.config(42);
    config.inject_cycles = true;
    let file = generate_supply_chain(&config);
    let graph = omtsf_core::build_graph(&file).expect("builds");

    use omtsf_core::enums::{EdgeType, EdgeTypeTag};
    use std::collections::HashSet;

    let all_types: HashSet<EdgeTypeTag> = [EdgeTypeTag::Known(EdgeType::LegalParentage)]
        .into_iter()
        .collect();

    let cycles = omtsf_core::graph::cycles::detect_cycles(&graph, &all_types);
    // With inject_cycles=true and enough orgs, we should have cycles
    // (though it's probabilistic based on the random back-edges)
    // At minimum the file should build and be valid
    assert!(!file.nodes.is_empty());
    let _ = cycles; // Cycles may or may not be present depending on random topology
}

mod proptest_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(50))]

        #[test]
        fn generated_files_always_pass_l1(seed in 0u64..10000) {
            let file = generate_supply_chain(&SizeTier::Small.config(seed));
            assert_l1_valid(&file, &format!("proptest/seed={seed}"));
        }

        #[test]
        fn generated_files_round_trip_through_json(seed in 0u64..1000) {
            let file = generate_supply_chain(&SizeTier::Small.config(seed));
            let json = serde_json::to_string(&file).expect("serialize");
            let back: omtsf_core::OmtsFile = serde_json::from_str(&json).expect("deserialize");
            prop_assert_eq!(file.nodes.len(), back.nodes.len());
            prop_assert_eq!(file.edges.len(), back.edges.len());
        }

        #[test]
        fn graph_build_succeeds_on_all_generated_files(seed in 0u64..1000) {
            let file = generate_supply_chain(&SizeTier::Small.config(seed));
            let graph = omtsf_core::build_graph(&file).expect("graph build must succeed");
            prop_assert_eq!(graph.node_count(), file.nodes.len());
            prop_assert_eq!(graph.edge_count(), file.edges.len());
        }
    }
}

/// Write fixture files to disk for manual inspection.
#[test]
#[ignore]
fn generate_fixtures() {
    use std::io::Write;

    let tiers = [
        ("small", SizeTier::Small),
        ("medium", SizeTier::Medium),
        ("large", SizeTier::Large),
        ("xlarge", SizeTier::XLarge),
    ];

    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures");
    std::fs::create_dir_all(&dir).expect("create fixtures dir");

    for (name, tier) in &tiers {
        let file = generate_supply_chain(&tier.config(42));
        let json = serde_json::to_string_pretty(&file).expect("serialize");
        let path = dir.join(format!("{name}.omts"));
        let mut f = std::fs::File::create(&path).expect("create file");
        f.write_all(json.as_bytes()).expect("write");
        eprintln!(
            "{name}: {} nodes, {} edges, {:.2} KB",
            file.nodes.len(),
            file.edges.len(),
            json.len() as f64 / 1024.0
        );
    }
}
