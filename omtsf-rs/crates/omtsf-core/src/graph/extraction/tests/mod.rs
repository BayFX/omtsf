#![allow(clippy::expect_used)]

use super::*;
use crate::enums::{EdgeType, EdgeTypeTag, NodeType, NodeTypeTag};
use crate::file::OmtsFile;
use crate::graph::build_graph;
use crate::newtypes::{CalendarDate, EdgeId, FileSalt, NodeId, SemVer};
use crate::structures::{Edge, EdgeProperties, Node};
use std::collections::BTreeMap;

mod selector_tests;

const SALT: &str = "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef";

pub(super) fn semver(s: &str) -> SemVer {
    SemVer::try_from(s).expect("valid SemVer")
}

pub(super) fn date(s: &str) -> CalendarDate {
    CalendarDate::try_from(s).expect("valid CalendarDate")
}

pub(super) fn file_salt(s: &str) -> FileSalt {
    FileSalt::try_from(s).expect("valid FileSalt")
}

pub(super) fn node_id(s: &str) -> NodeId {
    NodeId::try_from(s).expect("valid NodeId")
}

pub(super) fn edge_id(s: &str) -> EdgeId {
    NodeId::try_from(s).expect("valid EdgeId")
}

pub(super) fn minimal_file(nodes: Vec<Node>, edges: Vec<Edge>) -> OmtsFile {
    OmtsFile {
        omtsf_version: semver("1.0.0"),
        snapshot_date: date("2026-02-19"),
        file_salt: file_salt(SALT),
        disclosure_scope: None,
        previous_snapshot_ref: None,
        snapshot_sequence: None,
        reporting_entity: None,
        nodes,
        edges,
        extra: BTreeMap::new(),
    }
}

pub(super) fn org_node(id: &str) -> Node {
    Node {
        id: node_id(id),
        node_type: NodeTypeTag::Known(NodeType::Organization),
        identifiers: None,
        data_quality: None,
        labels: None,
        name: None,
        jurisdiction: None,
        status: None,
        governance_structure: None,
        operator: None,
        address: None,
        geo: None,
        commodity_code: None,
        unit: None,
        role: None,
        attestation_type: None,
        standard: None,
        issuer: None,
        valid_from: None,
        valid_to: None,
        outcome: None,
        attestation_status: None,
        reference: None,
        risk_severity: None,
        risk_likelihood: None,
        lot_id: None,
        quantity: None,
        production_date: None,
        origin_country: None,
        direct_emissions_co2e: None,
        indirect_emissions_co2e: None,
        emission_factor_source: None,
        installation_id: None,
        extra: BTreeMap::new(),
    }
}

pub(super) fn supplies_edge(id: &str, source: &str, target: &str) -> Edge {
    Edge {
        id: edge_id(id),
        edge_type: EdgeTypeTag::Known(EdgeType::Supplies),
        source: node_id(source),
        target: node_id(target),
        identifiers: None,
        properties: EdgeProperties::default(),
        extra: BTreeMap::new(),
    }
}

pub(super) fn ownership_edge(id: &str, source: &str, target: &str) -> Edge {
    Edge {
        id: edge_id(id),
        edge_type: EdgeTypeTag::Known(EdgeType::Ownership),
        source: node_id(source),
        target: node_id(target),
        identifiers: None,
        properties: EdgeProperties::default(),
        extra: BTreeMap::new(),
    }
}

/// Extract a known subset from a linear chain; verify nodes and edges.
#[test]
fn test_induced_subgraph_subset_of_linear_chain() {
    // Graph: a → b → c → d
    let nodes = vec![org_node("a"), org_node("b"), org_node("c"), org_node("d")];
    let edges = vec![
        supplies_edge("e-ab", "a", "b"),
        supplies_edge("e-bc", "b", "c"),
        supplies_edge("e-cd", "c", "d"),
    ];
    let file = minimal_file(nodes, edges);
    let graph = build_graph(&file).expect("builds");

    // Extract {a, b, c} — should include e-ab and e-bc, but not e-cd.
    let sub = induced_subgraph(&graph, &file, &["a", "b", "c"]).expect("should succeed");

    assert_eq!(sub.nodes.len(), 3, "three nodes expected");
    assert_eq!(sub.edges.len(), 2, "two edges expected (e-ab, e-bc)");

    let node_ids: Vec<String> = sub.nodes.iter().map(|n| n.id.to_string()).collect();
    assert!(node_ids.contains(&"a".to_owned()));
    assert!(node_ids.contains(&"b".to_owned()));
    assert!(node_ids.contains(&"c".to_owned()));
    assert!(!node_ids.contains(&"d".to_owned()));

    let edge_ids: Vec<String> = sub.edges.iter().map(|e| e.id.to_string()).collect();
    assert!(edge_ids.contains(&"e-ab".to_owned()));
    assert!(edge_ids.contains(&"e-bc".to_owned()));
    assert!(!edge_ids.contains(&"e-cd".to_owned()));
}

/// Edges whose source is in the subgraph but target is not are excluded.
#[test]
fn test_induced_subgraph_excludes_cross_boundary_edges() {
    // Graph: a → b → c; extract {a, b} — e-bc must not appear.
    let nodes = vec![org_node("a"), org_node("b"), org_node("c")];
    let edges = vec![
        supplies_edge("e-ab", "a", "b"),
        supplies_edge("e-bc", "b", "c"),
    ];
    let file = minimal_file(nodes, edges);
    let graph = build_graph(&file).expect("builds");

    let sub = induced_subgraph(&graph, &file, &["a", "b"]).expect("should succeed");
    assert_eq!(sub.nodes.len(), 2);
    assert_eq!(sub.edges.len(), 1);
    let edge_ids: Vec<String> = sub.edges.iter().map(|e| e.id.to_string()).collect();
    assert!(edge_ids.contains(&"e-ab".to_owned()));
    assert!(!edge_ids.contains(&"e-bc".to_owned()));
}

/// Extracting a single node with no internal edges gives an edge-free file.
#[test]
fn test_induced_subgraph_single_node_no_edges() {
    let nodes = vec![org_node("a"), org_node("b")];
    let edges = vec![supplies_edge("e-ab", "a", "b")];
    let file = minimal_file(nodes, edges);
    let graph = build_graph(&file).expect("builds");

    let sub = induced_subgraph(&graph, &file, &["a"]).expect("should succeed");
    assert_eq!(sub.nodes.len(), 1);
    assert_eq!(sub.edges.len(), 0, "no edges: b is excluded");
}

/// Extracting all nodes returns the same graph (nodes and edges).
#[test]
fn test_induced_subgraph_all_nodes_preserves_full_graph() {
    let nodes = vec![org_node("a"), org_node("b"), org_node("c")];
    let edges = vec![
        supplies_edge("e-ab", "a", "b"),
        supplies_edge("e-bc", "b", "c"),
        supplies_edge("e-ac", "a", "c"),
    ];
    let file = minimal_file(nodes, edges);
    let graph = build_graph(&file).expect("builds");

    let sub = induced_subgraph(&graph, &file, &["a", "b", "c"]).expect("should succeed");
    assert_eq!(sub.nodes.len(), 3);
    assert_eq!(sub.edges.len(), 3);
}

/// Unknown node ID returns `NodeNotFound`.
#[test]
fn test_induced_subgraph_unknown_node_returns_error() {
    let nodes = vec![org_node("a"), org_node("b")];
    let file = minimal_file(nodes, vec![]);
    let graph = build_graph(&file).expect("builds");

    let err =
        induced_subgraph(&graph, &file, &["a", "ghost"]).expect_err("should fail for unknown node");
    assert_eq!(err, QueryError::NodeNotFound("ghost".to_owned()));
}

/// Header fields (version, date, salt) are preserved in the output.
#[test]
fn test_induced_subgraph_preserves_header_fields() {
    let nodes = vec![org_node("a"), org_node("b")];
    let edges = vec![supplies_edge("e-ab", "a", "b")];
    let file = OmtsFile {
        omtsf_version: semver("1.2.0"),
        snapshot_date: date("2025-06-01"),
        file_salt: file_salt(SALT),
        disclosure_scope: None,
        previous_snapshot_ref: Some("sha256:abc".to_owned()),
        snapshot_sequence: Some(7),
        reporting_entity: None,
        nodes,
        edges,
        extra: BTreeMap::new(),
    };
    let graph = build_graph(&file).expect("builds");
    let sub = induced_subgraph(&graph, &file, &["a", "b"]).expect("should succeed");

    assert_eq!(sub.omtsf_version, semver("1.2.0"));
    assert_eq!(sub.snapshot_date, date("2025-06-01"));
    assert_eq!(sub.previous_snapshot_ref.as_deref(), Some("sha256:abc"));
    assert_eq!(sub.snapshot_sequence, Some(7));
}

/// `reporting_entity` is preserved when the referenced node is in the subgraph.
#[test]
fn test_induced_subgraph_reporting_entity_preserved_when_present() {
    let nodes = vec![org_node("reporter"), org_node("other")];
    let edges = vec![supplies_edge("e-1", "reporter", "other")];
    let file = OmtsFile {
        reporting_entity: Some(node_id("reporter")),
        ..minimal_file(nodes, edges)
    };
    let graph = build_graph(&file).expect("builds");

    let sub = induced_subgraph(&graph, &file, &["reporter", "other"]).expect("should succeed");
    assert_eq!(sub.reporting_entity, Some(node_id("reporter")));
}

/// `reporting_entity` is omitted when the referenced node is not in the subgraph.
#[test]
fn test_induced_subgraph_reporting_entity_omitted_when_absent() {
    let nodes = vec![org_node("reporter"), org_node("other")];
    let edges = vec![supplies_edge("e-1", "reporter", "other")];
    let file = OmtsFile {
        reporting_entity: Some(node_id("reporter")),
        ..minimal_file(nodes, edges)
    };
    let graph = build_graph(&file).expect("builds");

    // Extract only "other" — reporter is excluded, so reporting_entity must be None.
    let sub = induced_subgraph(&graph, &file, &["other"]).expect("should succeed");
    assert!(
        sub.reporting_entity.is_none(),
        "reporting_entity must be omitted when referenced node is absent"
    );
}

/// Multi-edge (parallel edges between same pair) are all included when both
/// endpoints are in the subgraph.
#[test]
fn test_induced_subgraph_parallel_edges_both_included() {
    let nodes = vec![org_node("a"), org_node("b")];
    let edges = vec![
        supplies_edge("e-1", "a", "b"),
        ownership_edge("e-2", "a", "b"),
    ];
    let file = minimal_file(nodes, edges);
    let graph = build_graph(&file).expect("builds");

    let sub = induced_subgraph(&graph, &file, &["a", "b"]).expect("should succeed");
    assert_eq!(sub.edges.len(), 2, "both parallel edges must be included");
}

/// Subgraph of an empty node-ID list returns an empty file.
#[test]
fn test_induced_subgraph_empty_node_ids_returns_empty_file() {
    let nodes = vec![org_node("a"), org_node("b")];
    let edges = vec![supplies_edge("e-ab", "a", "b")];
    let file = minimal_file(nodes, edges);
    let graph = build_graph(&file).expect("builds");

    let sub = induced_subgraph(&graph, &file, &[]).expect("empty list is valid");
    assert_eq!(sub.nodes.len(), 0);
    assert_eq!(sub.edges.len(), 0);
}

/// Ego-graph with radius 0 returns only the center node (no edges to others).
#[test]
fn test_ego_graph_radius_0_returns_center_only() {
    // Graph: a → b → c
    let nodes = vec![org_node("a"), org_node("b"), org_node("c")];
    let edges = vec![
        supplies_edge("e-ab", "a", "b"),
        supplies_edge("e-bc", "b", "c"),
    ];
    let file = minimal_file(nodes, edges);
    let graph = build_graph(&file).expect("builds");

    let ego = ego_graph(&graph, &file, "b", 0, Direction::Forward).expect("should succeed");
    assert_eq!(ego.nodes.len(), 1, "only b");
    assert_eq!(ego.edges.len(), 0, "no edges within singleton {{'b'}}");
    assert_eq!(ego.nodes[0].id, node_id("b"));
}

/// Ego-graph with radius 1 in forward direction includes center and direct successors.
#[test]
fn test_ego_graph_radius_1_forward_includes_direct_neighbours() {
    // Graph: a → b → c → d
    let nodes = vec![org_node("a"), org_node("b"), org_node("c"), org_node("d")];
    let edges = vec![
        supplies_edge("e-ab", "a", "b"),
        supplies_edge("e-bc", "b", "c"),
        supplies_edge("e-cd", "c", "d"),
    ];
    let file = minimal_file(nodes, edges);
    let graph = build_graph(&file).expect("builds");

    let ego = ego_graph(&graph, &file, "b", 1, Direction::Forward).expect("should succeed");
    // b + c (1 hop forward); a and d are excluded.
    let node_ids: Vec<String> = ego.nodes.iter().map(|n| n.id.to_string()).collect();
    assert!(
        node_ids.contains(&"b".to_owned()),
        "center b must be present"
    );
    assert!(node_ids.contains(&"c".to_owned()), "c is 1 hop forward");
    assert!(
        !node_ids.contains(&"a".to_owned()),
        "a is upstream, not forward"
    );
    assert!(!node_ids.contains(&"d".to_owned()), "d is 2 hops away");

    // Edge e-bc must be included (both endpoints in subgraph).
    let edge_ids: Vec<String> = ego.edges.iter().map(|e| e.id.to_string()).collect();
    assert!(edge_ids.contains(&"e-bc".to_owned()));
    assert!(!edge_ids.contains(&"e-ab".to_owned()));
    assert!(!edge_ids.contains(&"e-cd".to_owned()));
}

/// Ego-graph with radius 2 in forward direction includes nodes up to 2 hops.
#[test]
fn test_ego_graph_radius_2_forward_limits_depth() {
    // Graph: a → b → c → d → e
    let nodes = vec![
        org_node("a"),
        org_node("b"),
        org_node("c"),
        org_node("d"),
        org_node("e"),
    ];
    let edges = vec![
        supplies_edge("e-ab", "a", "b"),
        supplies_edge("e-bc", "b", "c"),
        supplies_edge("e-cd", "c", "d"),
        supplies_edge("e-de", "d", "e"),
    ];
    let file = minimal_file(nodes, edges);
    let graph = build_graph(&file).expect("builds");

    let ego = ego_graph(&graph, &file, "a", 2, Direction::Forward).expect("should succeed");
    let node_ids: Vec<String> = ego.nodes.iter().map(|n| n.id.to_string()).collect();
    assert!(node_ids.contains(&"a".to_owned()));
    assert!(node_ids.contains(&"b".to_owned()));
    assert!(node_ids.contains(&"c".to_owned()));
    assert!(!node_ids.contains(&"d".to_owned()), "d is 3 hops away");
    assert!(!node_ids.contains(&"e".to_owned()), "e is 4 hops away");

    assert_eq!(ego.edges.len(), 2, "e-ab and e-bc only");
}

/// Ego-graph with backward direction traverses incoming edges.
#[test]
fn test_ego_graph_backward_direction_traverses_incoming_edges() {
    // Graph: a → b → c
    let nodes = vec![org_node("a"), org_node("b"), org_node("c")];
    let edges = vec![
        supplies_edge("e-ab", "a", "b"),
        supplies_edge("e-bc", "b", "c"),
    ];
    let file = minimal_file(nodes, edges);
    let graph = build_graph(&file).expect("builds");

    // Ego of c with radius 1 backward: c + b.
    let ego = ego_graph(&graph, &file, "c", 1, Direction::Backward).expect("should succeed");
    let node_ids: Vec<String> = ego.nodes.iter().map(|n| n.id.to_string()).collect();
    assert!(node_ids.contains(&"c".to_owned()));
    assert!(node_ids.contains(&"b".to_owned()));
    assert!(!node_ids.contains(&"a".to_owned()), "a is 2 hops upstream");
}

/// Ego-graph with Both direction traverses edges in either direction.
#[test]
fn test_ego_graph_both_direction_traverses_all_edges() {
    // Graph: a → b ← c
    let nodes = vec![org_node("a"), org_node("b"), org_node("c")];
    let edges = vec![
        supplies_edge("e-ab", "a", "b"),
        supplies_edge("e-cb", "c", "b"),
    ];
    let file = minimal_file(nodes, edges);
    let graph = build_graph(&file).expect("builds");

    // Ego of a with radius 2 and Both direction: a → b ← c, so all three nodes reachable.
    let ego = ego_graph(&graph, &file, "a", 2, Direction::Both).expect("should succeed");
    let node_ids: Vec<String> = ego.nodes.iter().map(|n| n.id.to_string()).collect();
    assert!(node_ids.contains(&"a".to_owned()));
    assert!(node_ids.contains(&"b".to_owned()));
    assert!(node_ids.contains(&"c".to_owned()));
}

/// Ego-graph handles a cyclic graph without looping infinitely.
#[test]
fn test_ego_graph_handles_cycle_without_infinite_loop() {
    // Graph: a → b → c → a (cycle)
    let nodes = vec![org_node("a"), org_node("b"), org_node("c")];
    let edges = vec![
        supplies_edge("e-ab", "a", "b"),
        supplies_edge("e-bc", "b", "c"),
        supplies_edge("e-ca", "c", "a"),
    ];
    let file = minimal_file(nodes, edges);
    let graph = build_graph(&file).expect("builds");

    // With radius=10 and a cycle, BFS must terminate.
    let ego = ego_graph(&graph, &file, "a", 10, Direction::Forward).expect("should succeed");
    // All three nodes are reachable.
    assert_eq!(ego.nodes.len(), 3);
    assert_eq!(ego.edges.len(), 3);
}

/// Ego-graph returns `NodeNotFound` for an unknown center node.
#[test]
fn test_ego_graph_unknown_center_returns_error() {
    let nodes = vec![org_node("a")];
    let file = minimal_file(nodes, vec![]);
    let graph = build_graph(&file).expect("builds");

    let err = ego_graph(&graph, &file, "ghost", 1, Direction::Forward)
        .expect_err("should fail for unknown center");
    assert_eq!(err, QueryError::NodeNotFound("ghost".to_owned()));
}

/// Ego-graph with a disconnected node returns just that node (radius 0).
#[test]
fn test_ego_graph_isolated_node_radius_1() {
    // Graph: a, b (no edges between them)
    let nodes = vec![org_node("a"), org_node("b")];
    let file = minimal_file(nodes, vec![]);
    let graph = build_graph(&file).expect("builds");

    let ego = ego_graph(&graph, &file, "a", 1, Direction::Forward).expect("should succeed");
    assert_eq!(ego.nodes.len(), 1, "no neighbours; only a");
    assert_eq!(ego.edges.len(), 0);
}

/// Header fields are preserved in ego-graph output.
#[test]
fn test_ego_graph_preserves_header_fields() {
    let nodes = vec![org_node("a"), org_node("b")];
    let edges = vec![supplies_edge("e-ab", "a", "b")];
    let file = OmtsFile {
        omtsf_version: semver("1.1.0"),
        snapshot_date: date("2025-12-01"),
        file_salt: file_salt(SALT),
        disclosure_scope: None,
        previous_snapshot_ref: None,
        snapshot_sequence: Some(3),
        reporting_entity: None,
        nodes,
        edges,
        extra: BTreeMap::new(),
    };
    let graph = build_graph(&file).expect("builds");
    let ego = ego_graph(&graph, &file, "a", 1, Direction::Forward).expect("should succeed");

    assert_eq!(ego.omtsf_version, semver("1.1.0"));
    assert_eq!(ego.snapshot_date, date("2025-12-01"));
    assert_eq!(ego.snapshot_sequence, Some(3));
}

/// `reporting_entity` is omitted in ego-graph output when not in the neighbourhood.
#[test]
fn test_ego_graph_reporting_entity_omitted_when_outside_radius() {
    // reporter → a → b; ego of b with radius 1 backward: {b, a}; reporter is 2 hops.
    let nodes = vec![org_node("reporter"), org_node("a"), org_node("b")];
    let edges = vec![
        supplies_edge("e-ra", "reporter", "a"),
        supplies_edge("e-ab", "a", "b"),
    ];
    let file = OmtsFile {
        reporting_entity: Some(node_id("reporter")),
        ..minimal_file(nodes, edges)
    };
    let graph = build_graph(&file).expect("builds");

    let ego = ego_graph(&graph, &file, "b", 1, Direction::Backward).expect("should succeed");
    // Neighbourhood: {b, a}; reporter is not included.
    assert!(
        ego.reporting_entity.is_none(),
        "reporter is outside the 1-hop neighbourhood; must be omitted"
    );
}

/// The output of `induced_subgraph` is a valid `OmtsFile` that can be
/// round-tripped through serde.
#[test]
fn test_induced_subgraph_output_round_trips_through_serde() {
    let nodes = vec![org_node("a"), org_node("b"), org_node("c")];
    let edges = vec![
        supplies_edge("e-ab", "a", "b"),
        supplies_edge("e-bc", "b", "c"),
    ];
    let file = minimal_file(nodes, edges);
    let graph = build_graph(&file).expect("builds");

    let sub = induced_subgraph(&graph, &file, &["a", "b"]).expect("should succeed");

    // Round-trip through JSON.
    let json = serde_json::to_string(&sub).expect("serialize");
    let back: OmtsFile = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(sub, back);
}
