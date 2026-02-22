#![allow(clippy::expect_used)]

use std::collections::BTreeMap;
use std::collections::HashSet;

use petgraph::stable_graph::NodeIndex;

use super::*;
use crate::enums::{EdgeType, EdgeTypeTag, NodeType, NodeTypeTag};
use crate::file::OmtsFile;
use crate::graph::build_graph;
use crate::newtypes::{CalendarDate, EdgeId, FileSalt, NodeId, SemVer};
use crate::structures::{Edge, EdgeProperties, Node};

const SALT: &str = "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef";

fn semver(s: &str) -> SemVer {
    SemVer::try_from(s).expect("valid SemVer")
}

fn date(s: &str) -> CalendarDate {
    CalendarDate::try_from(s).expect("valid CalendarDate")
}

fn file_salt(s: &str) -> FileSalt {
    FileSalt::try_from(s).expect("valid FileSalt")
}

fn node_id(s: &str) -> NodeId {
    NodeId::try_from(s).expect("valid NodeId")
}

fn edge_id(s: &str) -> EdgeId {
    NodeId::try_from(s).expect("valid EdgeId")
}

fn minimal_file(nodes: Vec<Node>, edges: Vec<Edge>) -> OmtsFile {
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

fn org_node(id: &str) -> Node {
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

fn supplies_edge(id: &str, source: &str, target: &str) -> Edge {
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

fn ownership_edge(id: &str, source: &str, target: &str) -> Edge {
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

/// Resolve a local ID to a [`NodeIndex`] in `graph`, panicking if missing (test helper).
fn idx(graph: &crate::graph::OmtsGraph, id: &str) -> NodeIndex {
    *graph.node_index(id).expect("node must exist")
}

fn linear_chain() -> crate::graph::OmtsGraph {
    let nodes = vec![org_node("a"), org_node("b"), org_node("c"), org_node("d")];
    let edges = vec![
        supplies_edge("e-ab", "a", "b"),
        supplies_edge("e-bc", "b", "c"),
        supplies_edge("e-cd", "c", "d"),
    ];
    build_graph(&minimal_file(nodes, edges)).expect("linear chain builds")
}

/// Forward reachability from the head of a linear chain returns all other nodes.
#[test]
fn test_reachable_forward_linear_chain() {
    let g = linear_chain();
    let reached = reachable_from(&g, "a", Direction::Forward, None).expect("should succeed");
    assert_eq!(reached.len(), 3);
    assert!(reached.contains(&idx(&g, "b")));
    assert!(reached.contains(&idx(&g, "c")));
    assert!(reached.contains(&idx(&g, "d")));
    assert!(!reached.contains(&idx(&g, "a")));
}

/// Backward reachability from the tail of a linear chain returns all other nodes.
#[test]
fn test_reachable_backward_linear_chain() {
    let g = linear_chain();
    let reached = reachable_from(&g, "d", Direction::Backward, None).expect("should succeed");
    assert_eq!(reached.len(), 3);
    assert!(reached.contains(&idx(&g, "a")));
    assert!(reached.contains(&idx(&g, "b")));
    assert!(reached.contains(&idx(&g, "c")));
    assert!(!reached.contains(&idx(&g, "d")));
}

/// Forward reachability from the tail of a chain returns empty set.
#[test]
fn test_reachable_forward_from_tail_is_empty() {
    let g = linear_chain();
    let reached = reachable_from(&g, "d", Direction::Forward, None).expect("should succeed");
    assert!(reached.is_empty());
}

/// Both-direction reachability from any node in a linear chain returns all others.
#[test]
fn test_reachable_both_linear_chain() {
    let g = linear_chain();
    let reached = reachable_from(&g, "b", Direction::Both, None).expect("should succeed");
    // b can reach a (backward), c, d (forward) — all three other nodes.
    assert_eq!(reached.len(), 3);
    assert!(reached.contains(&idx(&g, "a")));
    assert!(reached.contains(&idx(&g, "c")));
    assert!(reached.contains(&idx(&g, "d")));
}

/// Reachability in a graph with cycles does not loop infinitely.
#[test]
fn test_reachable_handles_cycle() {
    // Graph: a → b → c → a (cycle), plus a → d
    let nodes = vec![org_node("a"), org_node("b"), org_node("c"), org_node("d")];
    let edges = vec![
        supplies_edge("e-ab", "a", "b"),
        supplies_edge("e-bc", "b", "c"),
        supplies_edge("e-ca", "c", "a"),
        supplies_edge("e-ad", "a", "d"),
    ];
    let g = build_graph(&minimal_file(nodes, edges)).expect("builds");
    let reached =
        reachable_from(&g, "a", Direction::Forward, None).expect("should succeed without looping");
    assert_eq!(reached.len(), 3);
    assert!(reached.contains(&idx(&g, "b")));
    assert!(reached.contains(&idx(&g, "c")));
    assert!(reached.contains(&idx(&g, "d")));
    assert!(!reached.contains(&idx(&g, "a")));
}

/// Edge-type filtering restricts reachability to matching edge types.
#[test]
fn test_reachable_edge_type_filter() {
    // Graph: a -supplies-> b -ownership-> c
    // With filter={Supplies}, only b is reachable from a.
    let nodes = vec![org_node("a"), org_node("b"), org_node("c")];
    let edges = vec![
        supplies_edge("e-ab", "a", "b"),
        ownership_edge("e-bc", "b", "c"),
    ];
    let g = build_graph(&minimal_file(nodes, edges)).expect("builds");

    let filter: HashSet<EdgeTypeTag> = [EdgeTypeTag::Known(EdgeType::Supplies)]
        .into_iter()
        .collect();

    let reached =
        reachable_from(&g, "a", Direction::Forward, Some(&filter)).expect("should succeed");
    assert_eq!(reached.len(), 1);
    assert!(reached.contains(&idx(&g, "b")));
    assert!(!reached.contains(&idx(&g, "c")));
}

/// Reachability with a filter that excludes all edges returns empty set.
#[test]
fn test_reachable_filter_excludes_all_edges() {
    let g = linear_chain(); // all supplies edges
    let filter: HashSet<EdgeTypeTag> = [EdgeTypeTag::Known(EdgeType::Ownership)]
        .into_iter()
        .collect();
    let reached =
        reachable_from(&g, "a", Direction::Forward, Some(&filter)).expect("should succeed");
    assert!(reached.is_empty());
}

/// Reachability returns `NodeNotFound` when start node is absent.
#[test]
fn test_reachable_node_not_found() {
    let g = linear_chain();
    let err = reachable_from(&g, "nonexistent", Direction::Forward, None)
        .expect_err("should fail for unknown node");
    assert_eq!(err, QueryError::NodeNotFound("nonexistent".to_owned()));
}

/// Shortest path in a linear chain is the chain itself.
#[test]
fn test_shortest_path_linear_chain() {
    let g = linear_chain();
    let path = shortest_path(&g, "a", "d", Direction::Forward, None)
        .expect("should succeed")
        .expect("path should exist");
    assert_eq!(path.len(), 4);
    assert_eq!(path[0], idx(&g, "a"));
    assert_eq!(path[1], idx(&g, "b"));
    assert_eq!(path[2], idx(&g, "c"));
    assert_eq!(path[3], idx(&g, "d"));
}

/// Shortest path from a node to itself is a single-element vector.
#[test]
fn test_shortest_path_from_node_to_itself() {
    let g = linear_chain();
    let path = shortest_path(&g, "b", "b", Direction::Forward, None)
        .expect("should succeed")
        .expect("trivial path should exist");
    assert_eq!(path, vec![idx(&g, "b")]);
}

/// Shortest path between disconnected nodes returns None.
#[test]
fn test_shortest_path_no_path_returns_none() {
    let g = linear_chain();
    let result = shortest_path(&g, "d", "a", Direction::Forward, None).expect("should not error");
    assert!(result.is_none());
}

/// Shortest path in a branching graph picks the shorter branch.
#[test]
fn test_shortest_path_branching_graph_picks_shorter() {
    // Graph:
    //   a → b → d      (3 hops: a-b-d via 2 edges)
    //   a → c → c2 → d (4 hops: a-c-c2-d via 3 edges)
    let nodes = vec![
        org_node("a"),
        org_node("b"),
        org_node("c"),
        org_node("c2"),
        org_node("d"),
    ];
    let edges = vec![
        supplies_edge("e-ab", "a", "b"),
        supplies_edge("e-bd", "b", "d"),
        supplies_edge("e-ac", "a", "c"),
        supplies_edge("e-cc2", "c", "c2"),
        supplies_edge("e-c2d", "c2", "d"),
    ];
    let g = build_graph(&minimal_file(nodes, edges)).expect("builds");
    let path = shortest_path(&g, "a", "d", Direction::Forward, None)
        .expect("should succeed")
        .expect("path should exist");
    assert_eq!(path.len(), 3);
    assert_eq!(path[0], idx(&g, "a"));
    assert_eq!(path[2], idx(&g, "d"));
}

/// Backward shortest path traverses incoming edges.
#[test]
fn test_shortest_path_backward_direction() {
    let g = linear_chain();
    let path = shortest_path(&g, "d", "a", Direction::Backward, None)
        .expect("should succeed")
        .expect("backward path should exist");
    assert_eq!(path.len(), 4);
    assert_eq!(path[0], idx(&g, "d"));
    assert_eq!(path[3], idx(&g, "a"));
}

/// Edge-type filtering on shortest path excludes paths through filtered-out edges.
#[test]
fn test_shortest_path_edge_filter_no_path() {
    // a -supplies-> b -ownership-> c
    // With filter={Ownership}, there is no path from a to c.
    let nodes = vec![org_node("a"), org_node("b"), org_node("c")];
    let edges = vec![
        supplies_edge("e-ab", "a", "b"),
        ownership_edge("e-bc", "b", "c"),
    ];
    let g = build_graph(&minimal_file(nodes, edges)).expect("builds");
    let filter: HashSet<EdgeTypeTag> = [EdgeTypeTag::Known(EdgeType::Ownership)]
        .into_iter()
        .collect();
    let result =
        shortest_path(&g, "a", "c", Direction::Forward, Some(&filter)).expect("should not error");
    assert!(result.is_none());
}

/// `NodeNotFound` is returned when the source node is absent.
#[test]
fn test_shortest_path_from_node_not_found() {
    let g = linear_chain();
    let err = shortest_path(&g, "ghost", "a", Direction::Forward, None).expect_err("should fail");
    assert_eq!(err, QueryError::NodeNotFound("ghost".to_owned()));
}

/// `NodeNotFound` is returned when the destination node is absent.
#[test]
fn test_shortest_path_to_node_not_found() {
    let g = linear_chain();
    let err = shortest_path(&g, "a", "ghost", Direction::Forward, None).expect_err("should fail");
    assert_eq!(err, QueryError::NodeNotFound("ghost".to_owned()));
}

/// [`all_paths`] on a linear chain returns exactly one path.
#[test]
fn test_all_paths_linear_chain_single_path() {
    let g = linear_chain();
    let paths = all_paths(&g, "a", "d", DEFAULT_MAX_DEPTH, Direction::Forward, None)
        .expect("should succeed");
    assert_eq!(paths.len(), 1);
    let path = &paths[0];
    assert_eq!(path.len(), 4);
    assert_eq!(path[0], idx(&g, "a"));
    assert_eq!(path[3], idx(&g, "d"));
}

/// [`all_paths`] on a branching graph returns all simple paths.
#[test]
fn test_all_paths_branching_graph_multiple_paths() {
    // Graph:
    //   a → b → d
    //   a → c → d
    let nodes = vec![org_node("a"), org_node("b"), org_node("c"), org_node("d")];
    let edges = vec![
        supplies_edge("e-ab", "a", "b"),
        supplies_edge("e-bd", "b", "d"),
        supplies_edge("e-ac", "a", "c"),
        supplies_edge("e-cd", "c", "d"),
    ];
    let g = build_graph(&minimal_file(nodes, edges)).expect("builds");
    let paths = all_paths(&g, "a", "d", DEFAULT_MAX_DEPTH, Direction::Forward, None)
        .expect("should succeed");
    assert_eq!(paths.len(), 2);
    for path in &paths {
        assert_eq!(path[0], idx(&g, "a"));
        assert_eq!(*path.last().expect("non-empty"), idx(&g, "d"));
    }
}

/// [`all_paths`] returns empty when no path exists.
#[test]
fn test_all_paths_no_path_returns_empty() {
    let g = linear_chain();
    let paths = all_paths(&g, "d", "a", DEFAULT_MAX_DEPTH, Direction::Forward, None)
        .expect("should succeed");
    assert!(paths.is_empty());
}

/// Depth limit enforcement: with `max_depth=1`, only paths of 1 hop are returned.
#[test]
fn test_all_paths_depth_limit_enforced() {
    let g = linear_chain(); // a → b → c → d
    // With max_depth=1, only a→b can be found from a to b; a to c/d have no path.
    let paths_to_b = all_paths(&g, "a", "b", 1, Direction::Forward, None).expect("should succeed");
    assert_eq!(paths_to_b.len(), 1);

    let paths_to_c_depth_1 =
        all_paths(&g, "a", "c", 1, Direction::Forward, None).expect("should succeed");
    assert!(
        paths_to_c_depth_1.is_empty(),
        "depth 1 cannot reach c from a"
    );

    let paths_to_c_depth_2 =
        all_paths(&g, "a", "c", 2, Direction::Forward, None).expect("should succeed");
    assert_eq!(
        paths_to_c_depth_2.len(),
        1,
        "depth 2 reaches c from a via b"
    );
}

/// [`all_paths`] handles cycles by enforcing simple paths (no node revisited).
#[test]
fn test_all_paths_cycle_handling_simple_paths_only() {
    // Graph: a → b → c → a (cycle), plus b → d
    let nodes = vec![org_node("a"), org_node("b"), org_node("c"), org_node("d")];
    let edges = vec![
        supplies_edge("e-ab", "a", "b"),
        supplies_edge("e-bc", "b", "c"),
        supplies_edge("e-ca", "c", "a"),
        supplies_edge("e-bd", "b", "d"),
    ];
    let g = build_graph(&minimal_file(nodes, edges)).expect("builds");
    // Paths from a to d: only a→b→d (a→b→c→a cycle cannot revisit b or a)
    let paths = all_paths(&g, "a", "d", DEFAULT_MAX_DEPTH, Direction::Forward, None)
        .expect("should succeed");
    assert_eq!(paths.len(), 1);
    assert_eq!(paths[0], vec![idx(&g, "a"), idx(&g, "b"), idx(&g, "d")]);
}

/// Edge-type filtering restricts [`all_paths`] to allowed edge types.
#[test]
fn test_all_paths_edge_type_filter() {
    // Graph: a -supplies-> b -ownership-> c
    //        a -ownership-> c
    // With filter={Ownership}, only direct a→c path exists.
    let nodes = vec![org_node("a"), org_node("b"), org_node("c")];
    let edges = vec![
        supplies_edge("e-ab", "a", "b"),
        ownership_edge("e-bc", "b", "c"),
        ownership_edge("e-ac", "a", "c"),
    ];
    let g = build_graph(&minimal_file(nodes, edges)).expect("builds");

    let filter: HashSet<EdgeTypeTag> = [EdgeTypeTag::Known(EdgeType::Ownership)]
        .into_iter()
        .collect();

    let paths = all_paths(
        &g,
        "a",
        "c",
        DEFAULT_MAX_DEPTH,
        Direction::Forward,
        Some(&filter),
    )
    .expect("should succeed");
    assert_eq!(paths.len(), 1);
    assert_eq!(paths[0], vec![idx(&g, "a"), idx(&g, "c")]);
}

/// `NodeNotFound` is returned for unknown source node.
#[test]
fn test_all_paths_from_node_not_found() {
    let g = linear_chain();
    let err = all_paths(
        &g,
        "ghost",
        "a",
        DEFAULT_MAX_DEPTH,
        Direction::Forward,
        None,
    )
    .expect_err("should fail");
    assert_eq!(err, QueryError::NodeNotFound("ghost".to_owned()));
}

/// `NodeNotFound` is returned for unknown destination node.
#[test]
fn test_all_paths_to_node_not_found() {
    let g = linear_chain();
    let err = all_paths(
        &g,
        "a",
        "ghost",
        DEFAULT_MAX_DEPTH,
        Direction::Forward,
        None,
    )
    .expect_err("should fail");
    assert_eq!(err, QueryError::NodeNotFound("ghost".to_owned()));
}

/// [`QueryError`] Display output contains the relevant ID.
#[test]
fn test_query_error_display() {
    let err = QueryError::NodeNotFound("missing-node".to_owned());
    let msg = err.to_string();
    assert!(msg.contains("missing-node"));
}

/// [`Direction::Both`] allows traversal in both forward and backward directions.
#[test]
fn test_shortest_path_both_direction_disconnected_without_both() {
    // Graph: a ← b → c (b points to a and c; a and c have no forward edge to each other)
    let nodes = vec![org_node("a"), org_node("b"), org_node("c")];
    let edges = vec![
        supplies_edge("e-ba", "b", "a"),
        supplies_edge("e-bc", "b", "c"),
    ];
    let g = build_graph(&minimal_file(nodes, edges)).expect("builds");

    let fwd = shortest_path(&g, "a", "c", Direction::Forward, None).expect("should not error");
    assert!(fwd.is_none());

    let both = shortest_path(&g, "a", "c", Direction::Both, None).expect("should not error");
    assert!(both.is_some());
    let path = both.expect("path exists");
    assert_eq!(path[0], idx(&g, "a"));
    assert_eq!(*path.last().expect("non-empty"), idx(&g, "c"));
}
