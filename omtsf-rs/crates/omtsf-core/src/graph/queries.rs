/// Graph query algorithms: reachability, shortest path, and all-paths enumeration.
///
/// Implements Sections 3 and 4 of the graph-engine technical specification.
/// All functions operate on an [`OmtsGraph`] and return results as sequences
/// of [`NodeIndex`] values.
///
/// # Direction
///
/// Every query accepts a [`Direction`] parameter controlling which edges are
/// followed:
/// - [`Direction::Forward`] — outgoing edges only (downstream traversal).
/// - [`Direction::Backward`] — incoming edges only (upstream traversal).
/// - [`Direction::Both`] — edges in either direction (undirected view).
///
/// # Edge-Type Filtering
///
/// All three query functions accept an optional `edge_filter: Option<&HashSet<EdgeTypeTag>>`.
/// When `Some`, only edges whose `edge_type` is in the set are traversed.
/// When `None`, all edge types are traversed.
use std::collections::{HashMap, HashSet, VecDeque};

use petgraph::stable_graph::NodeIndex;
use petgraph::visit::EdgeRef;

use crate::enums::EdgeTypeTag;
use crate::graph::OmtsGraph;

// ---------------------------------------------------------------------------
// Direction
// ---------------------------------------------------------------------------

/// Controls which edges are followed during graph traversal.
///
/// Used by [`reachable_from`], [`shortest_path`], and [`all_paths`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    /// Follow outgoing edges only — traverse downstream from the start node.
    Forward,
    /// Follow incoming edges only — traverse upstream from the start node.
    Backward,
    /// Follow edges in either direction, treating the graph as undirected.
    Both,
}

// ---------------------------------------------------------------------------
// QueryError
// ---------------------------------------------------------------------------

/// Errors that can occur during graph queries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryError {
    /// A node ID supplied to a query function does not exist in the graph.
    ///
    /// The contained string is the unknown ID.
    NodeNotFound(String),
}

impl std::fmt::Display for QueryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QueryError::NodeNotFound(id) => write!(f, "node not found: {id:?}"),
        }
    }
}

impl std::error::Error for QueryError {}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Returns `true` if the edge should be traversed given the optional filter.
///
/// When `filter` is `None`, all edges pass. When `Some`, only edges whose
/// `edge_type` is in the set pass.
fn edge_passes(edge_type: &EdgeTypeTag, filter: Option<&HashSet<EdgeTypeTag>>) -> bool {
    match filter {
        None => true,
        Some(allowed) => allowed.contains(edge_type),
    }
}

/// Collects the neighbour [`NodeIndex`] values reachable from `node` in one
/// step, respecting `direction` and `edge_filter`.
///
/// Returns an iterator-style `Vec` rather than an iterator to avoid
/// lifetime entanglement with the mutable BFS state that callers maintain.
fn neighbours(
    graph: &OmtsGraph,
    node: NodeIndex,
    direction: Direction,
    edge_filter: Option<&HashSet<EdgeTypeTag>>,
) -> Vec<NodeIndex> {
    let g = graph.graph();
    let mut result = Vec::new();

    match direction {
        Direction::Forward => {
            for edge_ref in g.edges(node) {
                if edge_passes(&edge_ref.weight().edge_type, edge_filter) {
                    result.push(edge_ref.target());
                }
            }
        }
        Direction::Backward => {
            for edge_ref in g.edges_directed(node, petgraph::Direction::Incoming) {
                if edge_passes(&edge_ref.weight().edge_type, edge_filter) {
                    result.push(edge_ref.source());
                }
            }
        }
        Direction::Both => {
            for edge_ref in g.edges(node) {
                if edge_passes(&edge_ref.weight().edge_type, edge_filter) {
                    result.push(edge_ref.target());
                }
            }
            for edge_ref in g.edges_directed(node, petgraph::Direction::Incoming) {
                if edge_passes(&edge_ref.weight().edge_type, edge_filter) {
                    result.push(edge_ref.source());
                }
            }
        }
    }

    result
}

// ---------------------------------------------------------------------------
// reachable_from
// ---------------------------------------------------------------------------

/// Returns the set of all nodes reachable from `start` via BFS.
///
/// The start node itself is excluded from the result.
///
/// # Parameters
///
/// - `graph` — the graph to query.
/// - `start` — graph-local node ID of the starting node.
/// - `direction` — which edges to follow (see [`Direction`]).
/// - `edge_filter` — optional set of allowed edge types; `None` traverses all.
///
/// # Errors
///
/// Returns [`QueryError::NodeNotFound`] if `start` does not exist in the graph.
pub fn reachable_from(
    graph: &OmtsGraph,
    start: &str,
    direction: Direction,
    edge_filter: Option<&HashSet<EdgeTypeTag>>,
) -> Result<HashSet<NodeIndex>, QueryError> {
    let start_idx = *graph
        .node_index(start)
        .ok_or_else(|| QueryError::NodeNotFound(start.to_owned()))?;

    let mut visited: HashSet<NodeIndex> = HashSet::new();
    let mut queue: VecDeque<NodeIndex> = VecDeque::new();

    // Seed queue with the start node's neighbours without marking the start
    // itself as visited in the result set.
    visited.insert(start_idx);
    queue.push_back(start_idx);

    while let Some(current) = queue.pop_front() {
        for neighbour in neighbours(graph, current, direction, edge_filter) {
            if !visited.contains(&neighbour) {
                visited.insert(neighbour);
                queue.push_back(neighbour);
            }
        }
    }

    // Remove the start node so the result excludes it per spec.
    visited.remove(&start_idx);

    Ok(visited)
}

// ---------------------------------------------------------------------------
// shortest_path
// ---------------------------------------------------------------------------

/// Returns the shortest path from `from` to `to` as a sequence of node indices.
///
/// Uses BFS, terminating as soon as `to` is first reached. The returned
/// vector is ordered from `from` to `to` inclusive.
///
/// Returns `None` if no path exists between the two nodes.
///
/// # Parameters
///
/// - `graph` — the graph to query.
/// - `from` — graph-local ID of the source node.
/// - `to` — graph-local ID of the destination node.
/// - `direction` — which edges to follow (see [`Direction`]).
/// - `edge_filter` — optional set of allowed edge types; `None` traverses all.
///
/// # Errors
///
/// Returns [`QueryError::NodeNotFound`] if either `from` or `to` does not
/// exist in the graph.
pub fn shortest_path(
    graph: &OmtsGraph,
    from: &str,
    to: &str,
    direction: Direction,
    edge_filter: Option<&HashSet<EdgeTypeTag>>,
) -> Result<Option<Vec<NodeIndex>>, QueryError> {
    let from_idx = *graph
        .node_index(from)
        .ok_or_else(|| QueryError::NodeNotFound(from.to_owned()))?;
    let to_idx = *graph
        .node_index(to)
        .ok_or_else(|| QueryError::NodeNotFound(to.to_owned()))?;

    if from_idx == to_idx {
        return Ok(Some(vec![from_idx]));
    }

    // BFS with predecessor tracking.
    let mut visited: HashSet<NodeIndex> = HashSet::new();
    let mut predecessor: HashMap<NodeIndex, NodeIndex> = HashMap::new();
    let mut queue: VecDeque<NodeIndex> = VecDeque::new();

    visited.insert(from_idx);
    queue.push_back(from_idx);

    'bfs: while let Some(current) = queue.pop_front() {
        for neighbour in neighbours(graph, current, direction, edge_filter) {
            if !visited.contains(&neighbour) {
                visited.insert(neighbour);
                predecessor.insert(neighbour, current);

                if neighbour == to_idx {
                    break 'bfs;
                }

                queue.push_back(neighbour);
            }
        }
    }

    if !visited.contains(&to_idx) {
        return Ok(None);
    }

    // Reconstruct path by walking predecessors backwards.
    let mut path = Vec::new();
    let mut current = to_idx;
    loop {
        path.push(current);
        if current == from_idx {
            break;
        }
        match predecessor.get(&current) {
            Some(&prev) => {
                current = prev;
            }
            None => {
                // Should never happen: `to_idx` was reached via BFS so there
                // must be an unbroken predecessor chain back to `from_idx`.
                break;
            }
        }
    }
    path.reverse();

    Ok(Some(path))
}

// ---------------------------------------------------------------------------
// all_paths
// ---------------------------------------------------------------------------

/// Default maximum depth for [`all_paths`] when not otherwise specified.
pub const DEFAULT_MAX_DEPTH: usize = 20;

/// Returns all simple paths from `from` to `to` up to `max_depth` hops.
///
/// Uses iterative-deepening DFS (IDDFS). A "simple path" visits each node at
/// most once. The depth limit bounds the search to prevent combinatorial
/// explosion on dense subgraphs.
///
/// The default depth limit is [`DEFAULT_MAX_DEPTH`] (20 hops).
///
/// # Parameters
///
/// - `graph` — the graph to query.
/// - `from` — graph-local ID of the source node.
/// - `to` — graph-local ID of the destination node.
/// - `max_depth` — maximum number of hops (edges) in any returned path.
/// - `direction` — which edges to follow (see [`Direction`]).
/// - `edge_filter` — optional set of allowed edge types; `None` traverses all.
///
/// # Errors
///
/// Returns [`QueryError::NodeNotFound`] if either `from` or `to` does not
/// exist in the graph.
pub fn all_paths(
    graph: &OmtsGraph,
    from: &str,
    to: &str,
    max_depth: usize,
    direction: Direction,
    edge_filter: Option<&HashSet<EdgeTypeTag>>,
) -> Result<Vec<Vec<NodeIndex>>, QueryError> {
    let from_idx = *graph
        .node_index(from)
        .ok_or_else(|| QueryError::NodeNotFound(from.to_owned()))?;
    let to_idx = *graph
        .node_index(to)
        .ok_or_else(|| QueryError::NodeNotFound(to.to_owned()))?;

    let mut results: Vec<Vec<NodeIndex>> = Vec::new();

    // Handle trivial case: from == to.
    if from_idx == to_idx {
        results.push(vec![from_idx]);
        return Ok(results);
    }

    // Iterative-deepening DFS: run DFS for depth limit d = 1 ..= max_depth.
    // Each depth limit is independent; we collect all paths found at each
    // depth and deduplicate by re-running with increasing limits.
    //
    // We use an explicit stack to avoid recursive function calls (avoiding
    // any stack-overflow risk on large graphs), where each stack frame holds
    // the current node, the depth consumed so far, the current path, and the
    // visited-on-path set.
    //
    // We accumulate all unique paths into a HashSet of paths (as Vec<usize>)
    // to avoid duplicates across depth iterations.
    let mut seen_paths: HashSet<Vec<NodeIndex>> = HashSet::new();

    for depth_limit in 1..=max_depth {
        dfs_paths(
            graph,
            from_idx,
            to_idx,
            depth_limit,
            direction,
            edge_filter,
            &mut seen_paths,
        );
    }

    results.extend(seen_paths);

    Ok(results)
}

/// Runs a depth-limited DFS from `current` to `target`, collecting all simple
/// paths of exactly up to `depth_limit` hops into `results`.
///
/// `on_path` tracks nodes on the current DFS path to enforce simplicity.
fn dfs_paths(
    graph: &OmtsGraph,
    from: NodeIndex,
    target: NodeIndex,
    depth_limit: usize,
    direction: Direction,
    edge_filter: Option<&HashSet<EdgeTypeTag>>,
    results: &mut HashSet<Vec<NodeIndex>>,
) {
    // Explicit DFS stack. Each entry: (node, depth_used, path, on_path).
    // We represent a "frame" as the state at that point in the DFS.
    struct Frame {
        node: NodeIndex,
        depth_used: usize,
        path: Vec<NodeIndex>,
        on_path: HashSet<NodeIndex>,
    }

    let mut stack: Vec<Frame> = Vec::new();

    let mut initial_on_path = HashSet::new();
    initial_on_path.insert(from);

    stack.push(Frame {
        node: from,
        depth_used: 0,
        path: vec![from],
        on_path: initial_on_path,
    });

    while let Some(frame) = stack.pop() {
        if frame.node == target && frame.depth_used > 0 {
            // Found a complete path — record it.
            results.insert(frame.path.clone());
            // Do not extend beyond target.
            continue;
        }

        if frame.depth_used >= depth_limit {
            continue;
        }

        for neighbour in neighbours(graph, frame.node, direction, edge_filter) {
            if !frame.on_path.contains(&neighbour) {
                let mut new_path = frame.path.clone();
                new_path.push(neighbour);

                let mut new_on_path = frame.on_path.clone();
                new_on_path.insert(neighbour);

                stack.push(Frame {
                    node: neighbour,
                    depth_used: frame.depth_used + 1,
                    path: new_path,
                    on_path: new_on_path,
                });
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use std::collections::HashSet;

    use petgraph::stable_graph::NodeIndex;

    use super::*;
    use crate::enums::{EdgeType, EdgeTypeTag, NodeType, NodeTypeTag};
    use crate::file::OmtsFile;
    use crate::graph::build_graph;
    use crate::newtypes::{CalendarDate, EdgeId, FileSalt, NodeId, SemVer};
    use crate::structures::{Edge, EdgeProperties, Node};

    // -----------------------------------------------------------------------
    // Fixture helpers (duplicated from graph.rs tests)
    // -----------------------------------------------------------------------

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
            extra: serde_json::Map::new(),
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
            extra: serde_json::Map::new(),
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
            extra: serde_json::Map::new(),
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
            extra: serde_json::Map::new(),
        }
    }

    /// Resolve a local ID to a [`NodeIndex`] in `graph`, panicking if missing (test helper).
    fn idx(graph: &crate::graph::OmtsGraph, id: &str) -> NodeIndex {
        *graph.node_index(id).expect("node must exist")
    }

    // -----------------------------------------------------------------------
    // Helper: build a linear chain A → B → C → D
    // -----------------------------------------------------------------------

    fn linear_chain() -> crate::graph::OmtsGraph {
        let nodes = vec![org_node("a"), org_node("b"), org_node("c"), org_node("d")];
        let edges = vec![
            supplies_edge("e-ab", "a", "b"),
            supplies_edge("e-bc", "b", "c"),
            supplies_edge("e-cd", "c", "d"),
        ];
        build_graph(&minimal_file(nodes, edges)).expect("linear chain builds")
    }

    // -----------------------------------------------------------------------
    // reachable_from tests
    // -----------------------------------------------------------------------

    /// Forward reachability from the head of a linear chain returns all other nodes.
    #[test]
    fn test_reachable_forward_linear_chain() {
        let g = linear_chain();
        let reached = reachable_from(&g, "a", Direction::Forward, None).expect("should succeed");
        assert_eq!(reached.len(), 3);
        assert!(reached.contains(&idx(&g, "b")));
        assert!(reached.contains(&idx(&g, "c")));
        assert!(reached.contains(&idx(&g, "d")));
        // Start node excluded.
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
        let reached = reachable_from(&g, "a", Direction::Forward, None)
            .expect("should succeed without looping");
        // b, c, d are reachable; a itself excluded.
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

    // -----------------------------------------------------------------------
    // shortest_path tests
    // -----------------------------------------------------------------------

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
        // Backward direction from "a": no path back from d to a (forward direction)
        let result =
            shortest_path(&g, "d", "a", Direction::Forward, None).expect("should not error");
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
        // Shortest is a → b → d (3 nodes, 2 edges).
        assert_eq!(path.len(), 3);
        assert_eq!(path[0], idx(&g, "a"));
        assert_eq!(path[2], idx(&g, "d"));
    }

    /// Backward shortest path traverses incoming edges.
    #[test]
    fn test_shortest_path_backward_direction() {
        let g = linear_chain();
        // Backward from d to a should succeed.
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
        let result = shortest_path(&g, "a", "c", Direction::Forward, Some(&filter))
            .expect("should not error");
        assert!(result.is_none());
    }

    /// `NodeNotFound` is returned when the source node is absent.
    #[test]
    fn test_shortest_path_from_node_not_found() {
        let g = linear_chain();
        let err =
            shortest_path(&g, "ghost", "a", Direction::Forward, None).expect_err("should fail");
        assert_eq!(err, QueryError::NodeNotFound("ghost".to_owned()));
    }

    /// `NodeNotFound` is returned when the destination node is absent.
    #[test]
    fn test_shortest_path_to_node_not_found() {
        let g = linear_chain();
        let err =
            shortest_path(&g, "a", "ghost", Direction::Forward, None).expect_err("should fail");
        assert_eq!(err, QueryError::NodeNotFound("ghost".to_owned()));
    }

    // -----------------------------------------------------------------------
    // all_paths tests
    // -----------------------------------------------------------------------

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
        // Both paths start at a and end at d.
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
        let paths_to_b =
            all_paths(&g, "a", "b", 1, Direction::Forward, None).expect("should succeed");
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
        // Only a→c via ownership; a→b requires supplies which is filtered out.
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

        // Forward: no path from a to c (a has no outgoing edges).
        let fwd = shortest_path(&g, "a", "c", Direction::Forward, None).expect("should not error");
        assert!(fwd.is_none());

        // Both: a ← b → c, so treating undirected, a is connected to b and b to c.
        let both = shortest_path(&g, "a", "c", Direction::Both, None).expect("should not error");
        assert!(both.is_some());
        let path = both.expect("path exists");
        assert_eq!(path[0], idx(&g, "a"));
        assert_eq!(*path.last().expect("non-empty"), idx(&g, "c"));
    }
}
