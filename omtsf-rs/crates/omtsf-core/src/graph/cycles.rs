/// Cycle detection for the OMTSF graph engine.
///
/// Implements Section 6 of the graph-engine technical specification using
/// Kahn's algorithm (BFS-based topological sort). The primary consumer is
/// the validation engine, which calls [`detect_cycles`] on the
/// `legal_parentage` edge-type subgraph to enforce rule L3-MRG-02.
///
/// # Algorithm Overview
///
/// Kahn's algorithm computes an in-degree table for every node in the
/// filtered subgraph, seeds a BFS queue with all zero-in-degree nodes,
/// then repeatedly removes a node from the queue and decrements the
/// in-degrees of its successors. Any node whose in-degree falls to zero
/// during this process is added to the queue.
///
/// If the algorithm exhausts the queue before visiting every node in the
/// subgraph, the unvisited nodes form one or more strongly connected
/// components (SCCs) — i.e., cycles. A DFS from each unvisited node
/// extracts the individual cycles.
use std::collections::{HashMap, HashSet, VecDeque};

use petgraph::stable_graph::NodeIndex;
use petgraph::visit::{EdgeRef, IntoEdgeReferences};

use crate::enums::EdgeTypeTag;
use crate::graph::OmtsGraph;

/// Detects cycles in the edge-type-filtered subgraph of `graph`.
///
/// Uses Kahn's algorithm (BFS-based topological sort) to identify nodes that
/// participate in cycles. When the topological sort cannot consume all nodes
/// in the filtered subgraph, the remaining nodes are partitioned into
/// individual cycles via DFS.
///
/// # Parameters
///
/// - `graph`      — The full graph to inspect.
/// - `edge_types` — The set of edge types to include in the subgraph. Only
///   edges whose type is in this set are considered. Pass all types to check
///   the entire graph.
///
/// # Returns
///
/// - An empty `Vec` if the filtered subgraph is acyclic (or if the
///   filtered subgraph contains no edges).
/// - One or more `Vec<NodeIndex>` sequences — each describing a cycle —
///   when cycles are present. Each inner vector lists the nodes that form the
///   cycle in traversal order, with the first and last node being the same
///   (i.e. closed cycle representation).
pub fn detect_cycles(graph: &OmtsGraph, edge_types: &HashSet<EdgeTypeTag>) -> Vec<Vec<NodeIndex>> {
    let g = graph.graph();

    let mut in_degree: HashMap<NodeIndex, usize> = HashMap::new();

    for node_idx in g.node_indices() {
        in_degree.entry(node_idx).or_insert(0);
    }

    for edge_ref in g.edge_references() {
        if edge_types.contains(&edge_ref.weight().edge_type) {
            *in_degree.entry(edge_ref.target()).or_insert(0) += 1;
        }
    }

    let mut queue: VecDeque<NodeIndex> = in_degree
        .iter()
        .filter(|&(_, &deg)| deg == 0)
        .map(|(&idx, _)| idx)
        .collect();

    let mut visited_count: usize = 0;
    let total_nodes = in_degree.len();

    while let Some(node) = queue.pop_front() {
        visited_count += 1;

        for edge_ref in g.edges(node) {
            if !edge_types.contains(&edge_ref.weight().edge_type) {
                continue;
            }
            let target = edge_ref.target();
            if let Some(deg) = in_degree.get_mut(&target) {
                if *deg > 0 {
                    *deg -= 1;
                }
                if *deg == 0 {
                    queue.push_back(target);
                }
            }
        }
    }

    if visited_count == total_nodes {
        return Vec::new();
    }

    let cyclic_nodes: HashSet<NodeIndex> = in_degree
        .iter()
        .filter(|&(_, &deg)| deg > 0)
        .map(|(&idx, _)| idx)
        .collect();

    extract_cycles(graph, &cyclic_nodes, edge_types)
}

/// Extracts individual cycles from a set of nodes known to be in cycles.
///
/// Performs iterative DFS rooted at each unvisited node in `cyclic_nodes`,
/// restricted to the filtered subgraph. When the DFS back-edge detects a
/// revisit to a node on the current path, the path segment from that node
/// to the current position forms a cycle.
///
/// The returned cycles each include the start node at both the beginning and
/// end to make them self-contained cycle descriptions.
fn extract_cycles(
    graph: &OmtsGraph,
    cyclic_nodes: &HashSet<NodeIndex>,
    edge_types: &HashSet<EdgeTypeTag>,
) -> Vec<Vec<NodeIndex>> {
    let g = graph.graph();
    let mut all_cycles: Vec<Vec<NodeIndex>> = Vec::new();
    let mut globally_visited: HashSet<NodeIndex> = HashSet::new();

    for &start in cyclic_nodes {
        if globally_visited.contains(&start) {
            continue;
        }

        let mut path: Vec<NodeIndex> = Vec::new();
        let mut on_path: HashSet<NodeIndex> = HashSet::new();

        let mut stack: Vec<(NodeIndex, Vec<NodeIndex>, usize)> = Vec::new();

        let start_children = filtered_successors(g, start, cyclic_nodes, edge_types);
        stack.push((start, start_children, 0));
        path.push(start);
        on_path.insert(start);

        while let Some(frame) = stack.last_mut() {
            let (node, children, child_idx) = frame;
            let node = *node;

            if *child_idx >= children.len() {
                stack.pop();
                path.pop();
                on_path.remove(&node);
                globally_visited.insert(node);
                continue;
            }

            let child = children[*child_idx];
            *child_idx += 1;

            if on_path.contains(&child) {
                if let Some(cycle_start_pos) = path.iter().position(|&n| n == child) {
                    let mut cycle: Vec<NodeIndex> = path[cycle_start_pos..].to_vec();
                    cycle.push(child);
                    all_cycles.push(cycle);
                }
                continue;
            }

            if globally_visited.contains(&child) {
                continue;
            }

            let child_children = filtered_successors(g, child, cyclic_nodes, edge_types);
            path.push(child);
            on_path.insert(child);
            stack.push((child, child_children, 0));
        }
    }

    all_cycles
}

/// Returns the successors of `node` reachable via filtered edges that are
/// also in `cyclic_nodes`.
///
/// Restricts traversal to nodes known to be in cycles to keep the DFS
/// confined to the cyclic component.
fn filtered_successors(
    g: &petgraph::stable_graph::StableDiGraph<crate::graph::NodeWeight, crate::graph::EdgeWeight>,
    node: NodeIndex,
    cyclic_nodes: &HashSet<NodeIndex>,
    edge_types: &HashSet<EdgeTypeTag>,
) -> Vec<NodeIndex> {
    g.edges(node)
        .filter(|e| edge_types.contains(&e.weight().edge_type))
        .map(|e| e.target())
        .filter(|t| cyclic_nodes.contains(t))
        .collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use std::collections::{BTreeMap, HashSet};

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

    fn legal_parentage_edge(id: &str, source: &str, target: &str) -> Edge {
        Edge {
            id: edge_id(id),
            edge_type: EdgeTypeTag::Known(EdgeType::LegalParentage),
            source: node_id(source),
            target: node_id(target),
            identifiers: None,
            properties: EdgeProperties::default(),
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

    /// Build a filter set for a single known edge type.
    fn filter(edge_type: EdgeType) -> HashSet<EdgeTypeTag> {
        [EdgeTypeTag::Known(edge_type)].into_iter().collect()
    }

    /// Resolve a local ID to a [`NodeIndex`] in `graph` (test helper).
    fn idx(graph: &crate::graph::OmtsGraph, id: &str) -> NodeIndex {
        *graph.node_index(id).expect("node must exist")
    }

    /// A directed acyclic graph produces an empty cycle list.
    ///
    /// Graph: a → b → c → d (linear chain, `legal_parentage` edges)
    #[test]
    fn test_dag_no_cycles() {
        let nodes = vec![org_node("a"), org_node("b"), org_node("c"), org_node("d")];
        let edges = vec![
            legal_parentage_edge("e-ab", "a", "b"),
            legal_parentage_edge("e-bc", "b", "c"),
            legal_parentage_edge("e-cd", "c", "d"),
        ];
        let g = build_graph(&minimal_file(nodes, edges)).expect("builds");
        let cycles = detect_cycles(&g, &filter(EdgeType::LegalParentage));
        assert!(
            cycles.is_empty(),
            "DAG should have no cycles; got {cycles:?}"
        );
    }

    /// A tree (branching DAG) produces an empty cycle list.
    ///
    /// Graph:      a
    ///            / \
    ///           b   c
    ///          / \
    ///         d   e
    #[test]
    fn test_tree_no_cycles() {
        let nodes = vec![
            org_node("a"),
            org_node("b"),
            org_node("c"),
            org_node("d"),
            org_node("e"),
        ];
        let edges = vec![
            legal_parentage_edge("e-ab", "a", "b"),
            legal_parentage_edge("e-ac", "a", "c"),
            legal_parentage_edge("e-bd", "b", "d"),
            legal_parentage_edge("e-be", "b", "e"),
        ];
        let g = build_graph(&minimal_file(nodes, edges)).expect("builds");
        let cycles = detect_cycles(&g, &filter(EdgeType::LegalParentage));
        assert!(
            cycles.is_empty(),
            "tree should have no cycles; got {cycles:?}"
        );
    }

    /// An empty graph (no nodes, no edges) is acyclic.
    #[test]
    fn test_empty_graph_no_cycles() {
        let g = build_graph(&minimal_file(vec![], vec![])).expect("builds");
        let cycles = detect_cycles(&g, &filter(EdgeType::LegalParentage));
        assert!(cycles.is_empty());
    }

    /// A three-node cycle is detected.
    ///
    /// Graph: a → b → c → a
    #[test]
    fn test_simple_three_node_cycle() {
        let nodes = vec![org_node("a"), org_node("b"), org_node("c")];
        let edges = vec![
            legal_parentage_edge("e-ab", "a", "b"),
            legal_parentage_edge("e-bc", "b", "c"),
            legal_parentage_edge("e-ca", "c", "a"),
        ];
        let g = build_graph(&minimal_file(nodes, edges)).expect("builds");
        let cycles = detect_cycles(&g, &filter(EdgeType::LegalParentage));

        assert!(!cycles.is_empty(), "should detect a cycle");

        for cycle in &cycles {
            assert!(cycle.len() >= 2, "cycle must have at least 2 entries");
            assert_eq!(
                cycle.first(),
                cycle.last(),
                "cycle must be closed (first == last)"
            );
        }

        let cycle_node_set: HashSet<NodeIndex> =
            cycles.iter().flat_map(|c| c.iter().copied()).collect();
        assert!(cycle_node_set.contains(&idx(&g, "a")));
        assert!(cycle_node_set.contains(&idx(&g, "b")));
        assert!(cycle_node_set.contains(&idx(&g, "c")));
    }

    /// A self-loop on a single node is detected as a cycle.
    #[test]
    fn test_self_loop_detected() {
        let nodes = vec![org_node("a")];
        let edges = vec![legal_parentage_edge("e-aa", "a", "a")];
        let g = build_graph(&minimal_file(nodes, edges)).expect("builds");
        let cycles = detect_cycles(&g, &filter(EdgeType::LegalParentage));
        assert!(
            !cycles.is_empty(),
            "self-loop should be detected as a cycle"
        );
    }

    /// A two-node mutual cycle is detected.
    ///
    /// Graph: a → b → a
    #[test]
    fn test_two_node_cycle() {
        let nodes = vec![org_node("a"), org_node("b")];
        let edges = vec![
            legal_parentage_edge("e-ab", "a", "b"),
            legal_parentage_edge("e-ba", "b", "a"),
        ];
        let g = build_graph(&minimal_file(nodes, edges)).expect("builds");
        let cycles = detect_cycles(&g, &filter(EdgeType::LegalParentage));

        assert!(!cycles.is_empty(), "should detect a two-node cycle");

        let cycle_node_set: HashSet<NodeIndex> =
            cycles.iter().flat_map(|c| c.iter().copied()).collect();
        assert!(cycle_node_set.contains(&idx(&g, "a")));
        assert!(cycle_node_set.contains(&idx(&g, "b")));
    }

    /// Two separate disjoint cycles are both detected.
    ///
    /// Cycle 1: a → b → a
    /// Cycle 2: c → d → e → c
    #[test]
    fn test_two_disjoint_cycles() {
        let nodes = vec![
            org_node("a"),
            org_node("b"),
            org_node("c"),
            org_node("d"),
            org_node("e"),
        ];
        let edges = vec![
            legal_parentage_edge("e-ab", "a", "b"),
            legal_parentage_edge("e-ba", "b", "a"),
            legal_parentage_edge("e-cd", "c", "d"),
            legal_parentage_edge("e-de", "d", "e"),
            legal_parentage_edge("e-ec", "e", "c"),
        ];
        let g = build_graph(&minimal_file(nodes, edges)).expect("builds");
        let cycles = detect_cycles(&g, &filter(EdgeType::LegalParentage));

        assert!(!cycles.is_empty(), "should detect at least one cycle");

        let cycle_node_set: HashSet<NodeIndex> =
            cycles.iter().flat_map(|c| c.iter().copied()).collect();

        for id in ["a", "b", "c", "d", "e"] {
            assert!(
                cycle_node_set.contains(&idx(&g, id)),
                "node {id} should be in a cycle"
            );
        }
    }

    /// Three separate disjoint cycles are all detected.
    ///
    /// Cycle 1: a → b → a (2 nodes)
    /// Cycle 2: c → d → e → c (3 nodes)
    /// Cycle 3: f → g → f (2 nodes)
    #[test]
    fn test_three_disjoint_cycles() {
        let nodes = vec![
            org_node("a"),
            org_node("b"),
            org_node("c"),
            org_node("d"),
            org_node("e"),
            org_node("f"),
            org_node("g"),
        ];
        let edges = vec![
            legal_parentage_edge("e-ab", "a", "b"),
            legal_parentage_edge("e-ba", "b", "a"),
            legal_parentage_edge("e-cd", "c", "d"),
            legal_parentage_edge("e-de", "d", "e"),
            legal_parentage_edge("e-ec", "e", "c"),
            legal_parentage_edge("e-fg", "f", "g"),
            legal_parentage_edge("e-gf", "g", "f"),
        ];
        let g = build_graph(&minimal_file(nodes, edges)).expect("builds");
        let cycles = detect_cycles(&g, &filter(EdgeType::LegalParentage));

        assert!(!cycles.is_empty(), "should detect cycles");

        let cycle_node_set: HashSet<NodeIndex> =
            cycles.iter().flat_map(|c| c.iter().copied()).collect();

        for id in ["a", "b", "c", "d", "e", "f", "g"] {
            assert!(
                cycle_node_set.contains(&idx(&g, id)),
                "node {id} should be in a cycle"
            );
        }
    }

    /// A graph where some edges form a cycle and others form a DAG.
    ///
    /// Graph:
    ///   root → a → b → a  (cycle between a and b)
    ///   root → c → d      (acyclic branch)
    #[test]
    fn test_mixed_acyclic_and_cyclic() {
        let nodes = vec![
            org_node("root"),
            org_node("a"),
            org_node("b"),
            org_node("c"),
            org_node("d"),
        ];
        let edges = vec![
            legal_parentage_edge("e-root-a", "root", "a"),
            legal_parentage_edge("e-ab", "a", "b"),
            legal_parentage_edge("e-ba", "b", "a"), // cycle
            legal_parentage_edge("e-root-c", "root", "c"),
            legal_parentage_edge("e-cd", "c", "d"),
        ];
        let g = build_graph(&minimal_file(nodes, edges)).expect("builds");
        let cycles = detect_cycles(&g, &filter(EdgeType::LegalParentage));

        assert!(!cycles.is_empty(), "should detect the a-b cycle");

        let cycle_node_set: HashSet<NodeIndex> =
            cycles.iter().flat_map(|c| c.iter().copied()).collect();

        // a and b are in the cycle.
        assert!(cycle_node_set.contains(&idx(&g, "a")));
        assert!(cycle_node_set.contains(&idx(&g, "b")));

        // root, c, d are not in cycles (acyclic part).
        assert!(
            !cycle_node_set.contains(&idx(&g, "root")),
            "root should not be in a cycle"
        );
        assert!(
            !cycle_node_set.contains(&idx(&g, "c")),
            "c should not be in a cycle"
        );
        assert!(
            !cycle_node_set.contains(&idx(&g, "d")),
            "d should not be in a cycle"
        );
    }

    /// DAG with acyclic edges of a different type mixed in with cyclic edges.
    ///
    /// Only cycles in the `legal_parentage` subgraph matter; supply cycles
    /// should not affect the result.
    #[test]
    fn test_mixed_acyclic_cyclic_cycle_only_in_other_type() {
        // supplies: a → b → c → a (cycle in supplies subgraph)
        // legal_parentage: x → y → z (acyclic)
        let nodes = vec![
            org_node("a"),
            org_node("b"),
            org_node("c"),
            org_node("x"),
            org_node("y"),
            org_node("z"),
        ];
        let edges = vec![
            supplies_edge("e-ab", "a", "b"),
            supplies_edge("e-bc", "b", "c"),
            supplies_edge("e-ca", "c", "a"),
            legal_parentage_edge("e-xy", "x", "y"),
            legal_parentage_edge("e-yz", "y", "z"),
        ];
        let g = build_graph(&minimal_file(nodes, edges)).expect("builds");

        let lp_cycles = detect_cycles(&g, &filter(EdgeType::LegalParentage));
        assert!(
            lp_cycles.is_empty(),
            "legal_parentage subgraph is acyclic; got {lp_cycles:?}"
        );

        let sup_cycles = detect_cycles(&g, &filter(EdgeType::Supplies));
        assert!(
            !sup_cycles.is_empty(),
            "supplies subgraph has a cycle that should be detected"
        );
    }

    /// Filtering by a type absent from the graph returns no cycles.
    #[test]
    fn test_filter_absent_edge_type_no_cycles() {
        let nodes = vec![org_node("a"), org_node("b")];
        let edges = vec![
            legal_parentage_edge("e-ab", "a", "b"),
            legal_parentage_edge("e-ba", "b", "a"),
        ];
        let g = build_graph(&minimal_file(nodes, edges)).expect("builds");

        let cycles = detect_cycles(&g, &filter(EdgeType::Supplies));
        assert!(
            cycles.is_empty(),
            "no supplies edges means no cycles in supplies subgraph"
        );
    }

    /// Filtering for all edge types detects cycles across the full graph.
    #[test]
    fn test_full_graph_filter_detects_cycle() {
        let nodes = vec![org_node("a"), org_node("b"), org_node("c")];
        let edges = vec![
            legal_parentage_edge("e-ab", "a", "b"),
            supplies_edge("e-bc", "b", "c"),
            legal_parentage_edge("e-ca", "c", "a"),
        ];
        let g = build_graph(&minimal_file(nodes, edges)).expect("builds");

        let full_filter: HashSet<EdgeTypeTag> = [
            EdgeTypeTag::Known(EdgeType::LegalParentage),
            EdgeTypeTag::Known(EdgeType::Supplies),
        ]
        .into_iter()
        .collect();

        let cycles = detect_cycles(&g, &full_filter);
        assert!(!cycles.is_empty(), "cycle spans both edge types");

        let lp_only = detect_cycles(&g, &filter(EdgeType::LegalParentage));
        assert!(
            lp_only.is_empty(),
            "legal_parentage subgraph alone is acyclic"
        );
    }
}
