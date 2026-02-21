//! Post-operation invariant tests using generated data.
#![allow(clippy::expect_used)]

use std::collections::HashSet;

use omtsf_bench::correctness;
use omtsf_bench::{SizeTier, generate_supply_chain};
use omtsf_core::enums::{DisclosureScope, EdgeType, EdgeTypeTag};
use omtsf_core::graph::queries::{self, Direction};
use omtsf_core::validation::{ValidationConfig, validate};
use omtsf_core::{build_graph, diff, merge, redact};

fn medium_file() -> omtsf_core::OmtsFile {
    generate_supply_chain(&SizeTier::Medium.config(42))
}

#[test]
fn graph_construction_invariants() {
    let file = medium_file();
    let graph = build_graph(&file).expect("builds");
    correctness::check_graph_invariants(&file, &graph).expect("graph invariants hold");
}

#[test]
fn reachable_from_excludes_start() {
    let file = medium_file();
    let graph = build_graph(&file).expect("builds");

    let start = file.nodes[0].id.to_string();
    let reachable =
        queries::reachable_from(&graph, &start, Direction::Forward, None).expect("query succeeds");

    correctness::check_reachable_excludes_start(&graph, &start, &reachable)
        .expect("reachable invariants hold");
}

#[test]
fn reachable_from_with_edge_filter() {
    let file = medium_file();
    let graph = build_graph(&file).expect("builds");

    let start = file.nodes[0].id.to_string();
    let filter: HashSet<EdgeTypeTag> = [EdgeTypeTag::Known(EdgeType::Supplies)]
        .into_iter()
        .collect();
    let reachable = queries::reachable_from(&graph, &start, Direction::Forward, Some(&filter))
        .expect("query succeeds");

    correctness::check_reachable_excludes_start(&graph, &start, &reachable)
        .expect("filtered reachable invariants hold");
}

#[test]
fn shortest_path_invariants() {
    let file = medium_file();
    let graph = build_graph(&file).expect("builds");

    let from = file.nodes[0].id.to_string();
    let reachable =
        queries::reachable_from(&graph, &from, Direction::Forward, None).expect("works");

    if let Some(&target_idx) = reachable.iter().next() {
        let target_id = graph
            .node_weight(target_idx)
            .expect("weight exists")
            .local_id
            .clone();

        let path = queries::shortest_path(&graph, &from, &target_id, Direction::Forward, None)
            .expect("query succeeds");

        if let Some(p) = path {
            correctness::check_shortest_path(&graph, &from, &target_id, &p)
                .expect("shortest path invariants hold");
        }
    }
}

#[test]
fn all_paths_invariants() {
    let file = generate_supply_chain(&SizeTier::Small.config(42));
    let graph = build_graph(&file).expect("builds");

    let from = file.nodes[0].id.to_string();
    let reachable =
        queries::reachable_from(&graph, &from, Direction::Forward, None).expect("works");

    if let Some(&target_idx) = reachable.iter().next() {
        let target_id = graph
            .node_weight(target_idx)
            .expect("weight exists")
            .local_id
            .clone();

        let paths = queries::all_paths(&graph, &from, &target_id, 5, Direction::Forward, None)
            .expect("query succeeds");

        correctness::check_all_paths(&graph, &from, &target_id, &paths)
            .expect("all paths invariants hold");
    }
}

#[test]
fn induced_subgraph_invariants() {
    let file = medium_file();
    let graph = build_graph(&file).expect("builds");

    let count = file.nodes.len() / 4;
    let node_ids: Vec<&str> = file.nodes[..count].iter().map(|n| n.id.as_ref()).collect();

    let sub = omtsf_core::graph::extraction::induced_subgraph(&graph, &file, &node_ids)
        .expect("extraction succeeds");

    correctness::check_subgraph(&file, &sub, &node_ids).expect("subgraph invariants hold");
}

#[test]
fn ego_graph_returns_valid_subgraph() {
    let file = medium_file();
    let graph = build_graph(&file).expect("builds");

    let center = file.nodes[0].id.to_string();
    let ego = omtsf_core::graph::extraction::ego_graph(&graph, &file, &center, 2, Direction::Both)
        .expect("ego graph succeeds");

    let node_set: HashSet<String> = ego.nodes.iter().map(|n| n.id.to_string()).collect();
    for edge in &ego.edges {
        assert!(
            node_set.contains(&edge.source.to_string()),
            "ego edge source not in ego nodes"
        );
        assert!(
            node_set.contains(&edge.target.to_string()),
            "ego edge target not in ego nodes"
        );
    }
}

#[test]
fn merge_self_is_idempotent() {
    let file = generate_supply_chain(&SizeTier::Small.config(42));
    let result = merge(&[file.clone(), file.clone()]).expect("merge succeeds");

    correctness::check_merge(&[&file, &file], &result.file).expect("merge invariants hold");

    let config = ValidationConfig {
        run_l1: true,
        run_l2: false,
        run_l3: false,
    };
    let validation = validate(&result.file, &config, None);
    assert!(
        !validation.has_errors(),
        "merged output should pass L1: {:?}",
        validation.errors().collect::<Vec<_>>()
    );
}

#[test]
fn merge_disjoint_files() {
    let file_a = generate_supply_chain(&SizeTier::Small.config(42));
    let file_b = generate_supply_chain(&SizeTier::Small.config(99));

    let result = merge(&[file_a.clone(), file_b.clone()]).expect("merge succeeds");
    correctness::check_merge(&[&file_a, &file_b], &result.file).expect("merge invariants hold");

    let config = ValidationConfig {
        run_l1: true,
        run_l2: false,
        run_l3: false,
    };
    let validation = validate(&result.file, &config, None);
    assert!(
        !validation.has_errors(),
        "merged output should pass L1: {:?}",
        validation.errors().collect::<Vec<_>>()
    );
}

#[test]
fn redact_to_partner_scope() {
    let file = generate_supply_chain(&SizeTier::Small.config(42));
    let retain: HashSet<omtsf_core::NodeId> = file.nodes.iter().map(|n| n.id.clone()).collect();

    let redacted = redact(&file, DisclosureScope::Partner, &retain).expect("redact succeeds");
    correctness::check_redaction(&redacted, &DisclosureScope::Partner)
        .expect("redaction invariants hold");

    let config = ValidationConfig {
        run_l1: true,
        run_l2: false,
        run_l3: false,
    };
    let validation = validate(&redacted, &config, None);
    assert!(
        !validation.has_errors(),
        "redacted output should pass L1: {:?}",
        validation.errors().collect::<Vec<_>>()
    );
}

#[test]
fn redact_to_public_scope_removes_persons() {
    let file = generate_supply_chain(&SizeTier::Small.config(42));
    let retain: HashSet<omtsf_core::NodeId> = file.nodes.iter().map(|n| n.id.clone()).collect();

    let redacted = redact(&file, DisclosureScope::Public, &retain).expect("redact succeeds");
    correctness::check_redaction(&redacted, &DisclosureScope::Public)
        .expect("redaction invariants hold");
}

#[test]
fn diff_self_is_empty() {
    let file = generate_supply_chain(&SizeTier::Small.config(42));
    let result = diff(&file, &file);
    correctness::check_self_diff(&result).expect("self-diff should be empty");
}

#[test]
fn diff_accounting_holds() {
    let file_a = generate_supply_chain(&SizeTier::Small.config(42));
    let file_b = generate_supply_chain(&SizeTier::Small.config(99));
    let result = diff(&file_a, &file_b);
    correctness::check_diff_accounting(&file_a, &file_b, &result)
        .expect("diff accounting invariants hold");
}
