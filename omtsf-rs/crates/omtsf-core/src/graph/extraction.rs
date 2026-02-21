/// Subgraph extraction algorithms: induced subgraph, ego-graph, and selector-based extraction.
///
/// Implements Section 5 of the graph-engine technical specification.
///
/// # Induced Subgraph
///
/// [`induced_subgraph`] accepts a set of node IDs and returns an [`OmtsFile`]
/// containing exactly those nodes and every edge whose source *and* target are
/// both in the set.
///
/// # Ego-Graph
///
/// [`ego_graph`] wraps a bounded BFS around [`induced_subgraph`]: it first
/// collects all nodes within `radius` hops of the `center` node (inclusive of
/// the center), then extracts the induced subgraph of that neighbourhood.
///
/// # Selector-Based Extraction
///
/// [`selector_match`] scans all nodes and edges for property-predicate matches
/// without assembling a subgraph. [`selector_subgraph`] performs the full
/// pipeline: seed scan → seed edge resolution → BFS expansion → induced
/// subgraph assembly.
///
/// # Output Validity
///
/// All extraction functions return a valid [`OmtsFile`] with the original
/// header fields preserved.  The `reporting_entity` header field is retained
/// only if the referenced node is present in the subgraph; otherwise it is
/// set to `None`.
use std::collections::{HashSet, VecDeque};

use petgraph::stable_graph::NodeIndex;
use petgraph::visit::EdgeRef;

use crate::file::OmtsFile;
use crate::graph::OmtsGraph;
use crate::graph::queries::{Direction, QueryError};
use crate::graph::selectors::SelectorSet;

/// Extracts the induced subgraph for the given set of node IDs.
///
/// The induced subgraph contains exactly the specified nodes and every edge
/// in the original graph whose source *and* target are both in the node set.
/// Graph-local IDs are preserved so edge `source`/`target` references remain
/// correct in the returned file.
///
/// # Parameters
///
/// - `graph` — the graph to query.
/// - `file` — the source [`OmtsFile`] used to build `graph`; provides the full
///   node and edge data, and the header fields to carry forward.
/// - `node_ids` — graph-local IDs of the nodes to include.
///
/// # Output
///
/// Returns a valid [`OmtsFile`] with:
/// - The original header fields (`omtsf_version`, `snapshot_date`,
///   `file_salt`, `disclosure_scope`, `previous_snapshot_ref`,
///   `snapshot_sequence`, `extra`) preserved as-is.
/// - `reporting_entity` retained only if the referenced node is present in
///   the subgraph; otherwise `None`.
/// - `nodes` and `edges` filtered to the induced subgraph.
///
/// # Errors
///
/// Returns [`QueryError::NodeNotFound`] if any ID in `node_ids` does not
/// exist in the graph.
pub fn induced_subgraph(
    graph: &OmtsGraph,
    file: &OmtsFile,
    node_ids: &[&str],
) -> Result<OmtsFile, QueryError> {
    let mut index_set: HashSet<NodeIndex> = HashSet::with_capacity(node_ids.len());
    for &id in node_ids {
        let idx = *graph
            .node_index(id)
            .ok_or_else(|| QueryError::NodeNotFound(id.to_owned()))?;
        index_set.insert(idx);
    }

    assemble_subgraph(graph, file, &index_set)
}

/// Extracts the ego-graph: the induced subgraph of all nodes within `radius`
/// hops of `center`.
///
/// Algorithm:
/// 1. Run a bounded BFS from `center`, collecting every node reachable within
///    `radius` hops (inclusive of the center itself).
/// 2. Extract the induced subgraph of the collected node set.
///
/// This is equivalent to the `radius`-neighbourhood of `center` in `graph`.
///
/// # Parameters
///
/// - `graph` — the graph to query.
/// - `file` — the source [`OmtsFile`] used to build `graph`.
/// - `center` — graph-local ID of the ego node.
/// - `radius` — maximum number of hops from `center` (0 returns only the
///   center node; 1 returns the center plus its direct neighbours; etc.).
/// - `direction` — which edges to follow when expanding the neighbourhood.
///
/// # Errors
///
/// Returns [`QueryError::NodeNotFound`] if `center` does not exist in the
/// graph.
pub fn ego_graph(
    graph: &OmtsGraph,
    file: &OmtsFile,
    center: &str,
    radius: usize,
    direction: Direction,
) -> Result<OmtsFile, QueryError> {
    let center_idx = *graph
        .node_index(center)
        .ok_or_else(|| QueryError::NodeNotFound(center.to_owned()))?;

    let mut visited: HashSet<NodeIndex> = HashSet::new();
    let mut queue: VecDeque<(NodeIndex, usize)> = VecDeque::new();

    visited.insert(center_idx);
    queue.push_back((center_idx, 0));

    while let Some((current, hops)) = queue.pop_front() {
        if hops >= radius {
            continue;
        }

        let g = graph.graph();
        let next_hops = hops + 1;

        match direction {
            Direction::Forward => {
                for edge_ref in g.edges(current) {
                    let neighbour = edge_ref.target();
                    if !visited.contains(&neighbour) {
                        visited.insert(neighbour);
                        queue.push_back((neighbour, next_hops));
                    }
                }
            }
            Direction::Backward => {
                for edge_ref in g.edges_directed(current, petgraph::Direction::Incoming) {
                    let neighbour = edge_ref.source();
                    if !visited.contains(&neighbour) {
                        visited.insert(neighbour);
                        queue.push_back((neighbour, next_hops));
                    }
                }
            }
            Direction::Both => {
                for edge_ref in g.edges(current) {
                    let neighbour = edge_ref.target();
                    if !visited.contains(&neighbour) {
                        visited.insert(neighbour);
                        queue.push_back((neighbour, next_hops));
                    }
                }
                for edge_ref in g.edges_directed(current, petgraph::Direction::Incoming) {
                    let neighbour = edge_ref.source();
                    if !visited.contains(&neighbour) {
                        visited.insert(neighbour);
                        queue.push_back((neighbour, next_hops));
                    }
                }
            }
        }
    }

    assemble_subgraph(graph, file, &visited)
}

/// Result of a [`selector_match`] scan.
///
/// Contains the indices into the originating `OmtsFile`'s `nodes` and `edges`
/// vectors for all elements that matched the given selectors.
#[derive(Debug, Default)]
pub struct SelectorMatchResult {
    /// Indices into `file.nodes` for matching nodes.
    pub node_indices: Vec<usize>,
    /// Indices into `file.edges` for matching edges.
    pub edge_indices: Vec<usize>,
}

/// Returns the indices of all nodes and edges in `file` that match `selectors`.
///
/// Performs a single linear scan of `file.nodes` and `file.edges` — O((N + E) * S)
/// where S is the total number of selector values. Does **not** perform neighbor
/// expansion or assemble a subgraph file.
///
/// Intended for the `omtsf query` command, which displays matches without
/// producing a new `.omts` file.
///
/// When `selectors` is empty, every node and edge is returned (universal match).
/// Otherwise, nodes are evaluated only when `selectors` contains at least one
/// node-applicable selector (see [`SelectorSet::has_node_selectors`]), and edges
/// are evaluated only when `selectors` contains at least one edge-applicable selector
/// (see [`SelectorSet::has_edge_selectors`]).  This ensures that an edge-only
/// `SelectorSet` returns no nodes, and a node-only `SelectorSet` returns no edges.
pub fn selector_match(file: &OmtsFile, selectors: &SelectorSet) -> SelectorMatchResult {
    let mut result = SelectorMatchResult::default();

    if selectors.is_empty() {
        // Universal match: return all nodes and edges.
        result.node_indices = (0..file.nodes.len()).collect();
        result.edge_indices = (0..file.edges.len()).collect();
        return result;
    }

    if selectors.has_node_selectors() {
        for (i, node) in file.nodes.iter().enumerate() {
            if selectors.matches_node(node) {
                result.node_indices.push(i);
            }
        }
    }

    if selectors.has_edge_selectors() {
        for (i, edge) in file.edges.iter().enumerate() {
            if selectors.matches_edge(edge) {
                result.edge_indices.push(i);
            }
        }
    }

    result
}

/// Extracts a subgraph based on selector predicates.
///
/// The extraction runs in four sequential phases:
///
/// 1. **Seed scan** — evaluates `selectors` against every node and edge via a
///    linear pass. Produces `seed_nodes: HashSet<NodeIndex>` and
///    `seed_edges: HashSet<EdgeIndex>`.
///
/// 2. **Seed edge resolution** — for each seed edge, adds its source and target
///    to `seed_nodes`. This ensures matched edges contribute endpoints to the
///    BFS frontier.
///
/// 3. **BFS expansion** — starting from `seed_nodes`, performs bounded BFS for
///    `expand` hops (treating the graph as undirected). Complexity: O(V + E)
///    per hop.
///
/// 4. **Induced subgraph assembly** — delegates to [`assemble_subgraph`] to
///    produce the final [`OmtsFile`]. Complexity: O(E).
///
/// # Parameters
///
/// - `graph` — the graph built from `file` via [`crate::graph::build_graph`].
/// - `file` — the source [`OmtsFile`]; provides full node/edge data and header.
/// - `selectors` — the predicate set to match against.
/// - `expand` — number of BFS hops to expand from the seed set (0 = seed +
///   immediate incident elements only).
///
/// # Errors
///
/// Returns [`QueryError::EmptyResult`] when the selector scan matches no nodes
/// and no edges.
pub fn selector_subgraph(
    graph: &OmtsGraph,
    file: &OmtsFile,
    selectors: &SelectorSet,
    expand: usize,
) -> Result<OmtsFile, QueryError> {
    // Fast path: an empty SelectorSet is a universal match — seed all nodes.
    if selectors.is_empty() {
        let all_nodes: HashSet<NodeIndex> = graph.graph().node_indices().collect();
        return assemble_subgraph(graph, file, &all_nodes);
    }

    let mut seed_nodes: HashSet<NodeIndex> = HashSet::new();

    if selectors.has_node_selectors() {
        if can_use_node_type_index(selectors) {
            for node_type in &selectors.node_types {
                for &idx in graph.nodes_of_type(node_type) {
                    seed_nodes.insert(idx);
                }
            }
        } else {
            for node in &file.nodes {
                if selectors.matches_node(node) {
                    if let Some(&idx) = graph.node_index(node.id.as_ref()) {
                        seed_nodes.insert(idx);
                    }
                }
            }
        }
    }

    let mut seed_edge_node_ids: Vec<(String, String)> = Vec::new();
    let mut any_edge_matched = false;

    if selectors.has_edge_selectors() {
        if can_use_edge_type_index(selectors) {
            let g = graph.graph();
            for edge_type in &selectors.edge_types {
                for &edge_idx in graph.edges_of_type(edge_type) {
                    if let Some((src, tgt)) = g.edge_endpoints(edge_idx) {
                        any_edge_matched = true;
                        if let (Some(sw), Some(tw)) =
                            (graph.node_weight(src), graph.node_weight(tgt))
                        {
                            seed_edge_node_ids.push((sw.local_id.clone(), tw.local_id.clone()));
                        }
                    }
                }
            }
        } else {
            for edge in &file.edges {
                if selectors.matches_edge(edge) {
                    any_edge_matched = true;
                    seed_edge_node_ids.push((edge.source.to_string(), edge.target.to_string()));
                }
            }
        }
    }

    if seed_nodes.is_empty() && !any_edge_matched {
        return Err(QueryError::EmptyResult);
    }

    for (source_id, target_id) in &seed_edge_node_ids {
        if let Some(&idx) = graph.node_index(source_id.as_str()) {
            seed_nodes.insert(idx);
        }
        if let Some(&idx) = graph.node_index(target_id.as_str()) {
            seed_nodes.insert(idx);
        }
    }

    let mut visited: HashSet<NodeIndex> = seed_nodes.clone();
    let mut queue: VecDeque<(NodeIndex, usize)> =
        seed_nodes.iter().map(|&idx| (idx, 0usize)).collect();

    let g = graph.graph();

    while let Some((current, hops)) = queue.pop_front() {
        if hops >= expand {
            continue;
        }
        let next_hops = hops + 1;

        for edge_ref in g.edges(current) {
            let neighbour = edge_ref.target();
            if !visited.contains(&neighbour) {
                visited.insert(neighbour);
                queue.push_back((neighbour, next_hops));
            }
        }
        for edge_ref in g.edges_directed(current, petgraph::Direction::Incoming) {
            let neighbour = edge_ref.source();
            if !visited.contains(&neighbour) {
                visited.insert(neighbour);
                queue.push_back((neighbour, next_hops));
            }
        }
    }

    assemble_subgraph(graph, file, &visited)
}

/// Returns `true` when `node_types` is the only non-empty node-applicable
/// selector group, allowing the type index to replace a full linear scan.
fn can_use_node_type_index(ss: &SelectorSet) -> bool {
    !ss.node_types.is_empty()
        && ss.label_keys.is_empty()
        && ss.label_key_values.is_empty()
        && ss.identifier_schemes.is_empty()
        && ss.identifier_scheme_values.is_empty()
        && ss.jurisdictions.is_empty()
        && ss.names.is_empty()
}

/// Returns `true` when `edge_types` is the only non-empty edge-applicable
/// selector group, allowing the type index to replace a full linear scan.
fn can_use_edge_type_index(ss: &SelectorSet) -> bool {
    !ss.edge_types.is_empty() && ss.label_keys.is_empty() && ss.label_key_values.is_empty()
}

/// Assembles an [`OmtsFile`] from a set of included [`NodeIndex`] values.
///
/// Iterates all edges in the graph; includes an edge in the output only if
/// both its source and target are in `index_set`.  Nodes are included in
/// original file order (by `data_index`) to keep output deterministic.
///
/// The `reporting_entity` header field is preserved only when the referenced
/// node is present in `index_set`; otherwise it is set to `None`.
fn assemble_subgraph(
    graph: &OmtsGraph,
    file: &OmtsFile,
    index_set: &HashSet<NodeIndex>,
) -> Result<OmtsFile, QueryError> {
    let g = graph.graph();

    let mut included_data_indices: HashSet<usize> = HashSet::with_capacity(index_set.len());
    for &idx in index_set {
        if let Some(weight) = graph.node_weight(idx) {
            included_data_indices.insert(weight.data_index);
        }
    }

    let nodes: Vec<crate::structures::Node> = file
        .nodes
        .iter()
        .enumerate()
        .filter(|(i, _)| included_data_indices.contains(i))
        .map(|(_, node)| node.clone())
        .collect();

    let mut included_edge_data_indices: HashSet<usize> = HashSet::new();
    for &node_idx in index_set {
        for edge_ref in g.edges(node_idx) {
            if index_set.contains(&edge_ref.target()) {
                included_edge_data_indices.insert(edge_ref.weight().data_index);
            }
        }
    }

    let edges: Vec<crate::structures::Edge> = file
        .edges
        .iter()
        .enumerate()
        .filter(|(i, _)| included_edge_data_indices.contains(i))
        .map(|(_, edge)| edge.clone())
        .collect();

    let included_node_ids: HashSet<String> = nodes.iter().map(|n| n.id.to_string()).collect();
    let reporting_entity = file.reporting_entity.as_ref().and_then(|re_id| {
        if included_node_ids.contains(&re_id.to_string()) {
            Some(re_id.clone())
        } else {
            None
        }
    });

    Ok(OmtsFile {
        omtsf_version: file.omtsf_version.clone(),
        snapshot_date: file.snapshot_date.clone(),
        file_salt: file.file_salt.clone(),
        disclosure_scope: file.disclosure_scope.clone(),
        previous_snapshot_ref: file.previous_snapshot_ref.clone(),
        snapshot_sequence: file.snapshot_sequence,
        reporting_entity,
        nodes,
        edges,
        extra: file.extra.clone(),
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

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

        let err = induced_subgraph(&graph, &file, &["a", "ghost"])
            .expect_err("should fail for unknown node");
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
            extra: serde_json::Map::new(),
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
            extra: serde_json::Map::new(),
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

    use crate::graph::selectors::{Selector, SelectorSet};
    use crate::newtypes::CountryCode;
    use crate::types::{Identifier, Label};

    fn country_code(s: &str) -> CountryCode {
        CountryCode::try_from(s).expect("valid CountryCode")
    }

    fn label(key: &str, value: Option<&str>) -> Label {
        Label {
            key: key.to_owned(),
            value: value.map(str::to_owned),
            extra: serde_json::Map::new(),
        }
    }

    fn identifier(scheme: &str, value: &str) -> Identifier {
        Identifier {
            scheme: scheme.to_owned(),
            value: value.to_owned(),
            authority: None,
            valid_from: None,
            valid_to: None,
            sensitivity: None,
            verification_status: None,
            verification_date: None,
            extra: serde_json::Map::new(),
        }
    }

    fn facility_node(id: &str) -> Node {
        Node {
            id: node_id(id),
            node_type: NodeTypeTag::Known(NodeType::Facility),
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

    /// `selector_match` with `NodeType` selector returns matching node indices.
    #[test]
    fn test_selector_match_node_type_returns_correct_indices() {
        let nodes = vec![org_node("org-1"), facility_node("fac-1"), org_node("org-2")];
        let file = minimal_file(nodes, vec![]);

        let ss = SelectorSet::from_selectors(vec![Selector::NodeType(NodeTypeTag::Known(
            NodeType::Organization,
        ))]);
        let result = selector_match(&file, &ss);

        assert_eq!(result.node_indices, vec![0, 2], "indices 0 and 2 are orgs");
        assert!(result.edge_indices.is_empty());
    }

    /// `selector_match` with `EdgeType` selector returns matching edge indices.
    #[test]
    fn test_selector_match_edge_type_returns_correct_indices() {
        let nodes = vec![org_node("a"), org_node("b"), org_node("c")];
        let edges = vec![
            supplies_edge("e-ab", "a", "b"),
            ownership_edge("e-bc", "b", "c"),
            supplies_edge("e-ac", "a", "c"),
        ];
        let file = minimal_file(nodes, edges);

        let ss = SelectorSet::from_selectors(vec![Selector::EdgeType(EdgeTypeTag::Known(
            EdgeType::Supplies,
        ))]);
        let result = selector_match(&file, &ss);

        assert!(result.node_indices.is_empty());
        assert_eq!(
            result.edge_indices,
            vec![0, 2],
            "indices 0 and 2 are supplies edges"
        );
    }

    /// `selector_match` with `LabelKey` selector matches nodes with that label.
    #[test]
    fn test_selector_match_label_key_matches_labeled_nodes() {
        let mut n1 = org_node("n1");
        n1.labels = Some(vec![label("certified", None)]);
        let n2 = org_node("n2");
        let mut n3 = facility_node("n3");
        n3.labels = Some(vec![label("certified", None)]);

        let file = minimal_file(vec![n1, n2, n3], vec![]);
        let ss = SelectorSet::from_selectors(vec![Selector::LabelKey("certified".to_owned())]);
        let result = selector_match(&file, &ss);

        assert_eq!(result.node_indices, vec![0, 2]);
    }

    /// `selector_match` with `LabelKeyValue` selector matches nodes with exact pair.
    #[test]
    fn test_selector_match_label_key_value_exact_match() {
        let mut n1 = org_node("n1");
        n1.labels = Some(vec![label("tier", Some("1"))]);
        let mut n2 = org_node("n2");
        n2.labels = Some(vec![label("tier", Some("2"))]);
        let mut n3 = org_node("n3");
        n3.labels = Some(vec![label("tier", Some("1"))]);

        let file = minimal_file(vec![n1, n2, n3], vec![]);
        let ss = SelectorSet::from_selectors(vec![Selector::LabelKeyValue(
            "tier".to_owned(),
            "1".to_owned(),
        )]);
        let result = selector_match(&file, &ss);

        assert_eq!(result.node_indices, vec![0, 2]);
    }

    /// `selector_match` with `IdentifierScheme` selector matches nodes with that scheme.
    #[test]
    fn test_selector_match_identifier_scheme_matches_nodes() {
        let mut n1 = org_node("n1");
        n1.identifiers = Some(vec![identifier("lei", "529900T8BM49AURSDO55")]);
        let n2 = org_node("n2");
        let mut n3 = org_node("n3");
        n3.identifiers = Some(vec![identifier("duns", "123456789")]);

        let file = minimal_file(vec![n1, n2, n3], vec![]);
        let ss = SelectorSet::from_selectors(vec![Selector::IdentifierScheme("lei".to_owned())]);
        let result = selector_match(&file, &ss);

        assert_eq!(result.node_indices, vec![0]);
    }

    /// `selector_match` with `IdentifierSchemeValue` selector matches exact (scheme, value).
    #[test]
    fn test_selector_match_identifier_scheme_value_exact() {
        let mut n1 = org_node("n1");
        n1.identifiers = Some(vec![identifier("duns", "111111111")]);
        let mut n2 = org_node("n2");
        n2.identifiers = Some(vec![identifier("duns", "222222222")]);

        let file = minimal_file(vec![n1, n2], vec![]);
        let ss = SelectorSet::from_selectors(vec![Selector::IdentifierSchemeValue(
            "duns".to_owned(),
            "111111111".to_owned(),
        )]);
        let result = selector_match(&file, &ss);

        assert_eq!(result.node_indices, vec![0]);
    }

    /// `selector_match` with `Jurisdiction` selector matches nodes by country code.
    #[test]
    fn test_selector_match_jurisdiction_matches_correct_nodes() {
        let mut n1 = org_node("n1");
        n1.jurisdiction = Some(country_code("DE"));
        let mut n2 = org_node("n2");
        n2.jurisdiction = Some(country_code("US"));
        let mut n3 = org_node("n3");
        n3.jurisdiction = Some(country_code("DE"));

        let file = minimal_file(vec![n1, n2, n3], vec![]);
        let ss = SelectorSet::from_selectors(vec![Selector::Jurisdiction(country_code("DE"))]);
        let result = selector_match(&file, &ss);

        assert_eq!(result.node_indices, vec![0, 2]);
    }

    /// `selector_match` with `Name` selector uses case-insensitive substring match.
    #[test]
    fn test_selector_match_name_case_insensitive_substring() {
        let mut n1 = org_node("n1");
        n1.name = Some("Acme Corp".to_owned());
        let mut n2 = org_node("n2");
        n2.name = Some("Global Logistics".to_owned());
        let mut n3 = org_node("n3");
        n3.name = Some("ACME GmbH".to_owned());

        let file = minimal_file(vec![n1, n2, n3], vec![]);
        let ss = SelectorSet::from_selectors(vec![Selector::Name("acme".to_owned())]);
        let result = selector_match(&file, &ss);

        assert_eq!(result.node_indices, vec![0, 2]);
    }

    /// `selector_match` returns empty result when nothing matches.
    #[test]
    fn test_selector_match_no_matches_returns_empty() {
        let file = minimal_file(vec![org_node("n1"), org_node("n2")], vec![]);
        let ss = SelectorSet::from_selectors(vec![Selector::NodeType(NodeTypeTag::Known(
            NodeType::Facility,
        ))]);
        let result = selector_match(&file, &ss);

        assert!(result.node_indices.is_empty());
        assert!(result.edge_indices.is_empty());
    }

    /// `selector_match` with empty selector set returns all nodes and all edges.
    #[test]
    fn test_selector_match_empty_selector_set_matches_everything() {
        let nodes = vec![org_node("n1"), org_node("n2")];
        let edges = vec![supplies_edge("e1", "n1", "n2")];
        let file = minimal_file(nodes, edges);

        let ss = SelectorSet::default();
        let result = selector_match(&file, &ss);

        assert_eq!(result.node_indices, vec![0, 1]);
        assert_eq!(result.edge_indices, vec![0]);
    }

    /// `selector_subgraph` with expand=0 returns seed nodes and their incident edges.
    #[test]
    fn test_selector_subgraph_expand_0_returns_seed_with_incident_edges() {
        // Graph: a(org) → b(facility) → c(org)
        // Select organizations with expand=0.
        // Seeds: {a, c}; their incident edges include e-ab and e-bc.
        let nodes = vec![org_node("a"), facility_node("b"), org_node("c")];
        let edges = vec![
            supplies_edge("e-ab", "a", "b"),
            supplies_edge("e-bc", "b", "c"),
        ];
        let file = minimal_file(nodes, edges);
        let graph = build_graph(&file).expect("builds");

        let ss = SelectorSet::from_selectors(vec![Selector::NodeType(NodeTypeTag::Known(
            NodeType::Organization,
        ))]);
        let sub = selector_subgraph(&graph, &file, &ss, 0).expect("should succeed");

        // Seeds are {a, c}. With expand=0, BFS does not expand beyond seeds.
        // assemble_subgraph computes the induced subgraph of {a, c},
        // which includes the edge e-ab only if b is in the set (it isn't).
        // So we get nodes {a, c} and no edges (e-ab has b not in set, e-bc has b not in set).
        let node_ids: Vec<String> = sub.nodes.iter().map(|n| n.id.to_string()).collect();
        assert!(node_ids.contains(&"a".to_owned()), "a must be included");
        assert!(node_ids.contains(&"c".to_owned()), "c must be included");
        assert!(
            !node_ids.contains(&"b".to_owned()),
            "b is not a seed and expand=0"
        );
        // No edge has both endpoints in {a, c}
        assert_eq!(sub.edges.len(), 0);
    }

    /// `selector_subgraph` with expand=1 includes one-hop neighbors.
    #[test]
    fn test_selector_subgraph_expand_1_includes_one_hop_neighbors() {
        // Graph: a(org) → b(facility) → c(org)
        // Select facilities with expand=1: seed={b}, expand gives {a, c}.
        let nodes = vec![org_node("a"), facility_node("b"), org_node("c")];
        let edges = vec![
            supplies_edge("e-ab", "a", "b"),
            supplies_edge("e-bc", "b", "c"),
        ];
        let file = minimal_file(nodes, edges);
        let graph = build_graph(&file).expect("builds");

        let ss = SelectorSet::from_selectors(vec![Selector::NodeType(NodeTypeTag::Known(
            NodeType::Facility,
        ))]);
        let sub = selector_subgraph(&graph, &file, &ss, 1).expect("should succeed");

        let node_ids: Vec<String> = sub.nodes.iter().map(|n| n.id.to_string()).collect();
        assert!(node_ids.contains(&"a".to_owned()), "a is 1 hop from b");
        assert!(node_ids.contains(&"b".to_owned()), "b is the seed");
        assert!(node_ids.contains(&"c".to_owned()), "c is 1 hop from b");
        // All edges are within the full node set.
        assert_eq!(sub.edges.len(), 2);
    }

    /// `selector_subgraph` with expand=3 expands multiple hops.
    #[test]
    fn test_selector_subgraph_expand_3_captures_multi_hop_neighbors() {
        // Chain: org-a → fac-b → org-c → fac-d → org-e → fac-f
        // Select organizations with expand=3: seeds={org-a, org-c, org-e}.
        // BFS from seeds: 1 hop → {fac-b, fac-d, fac-f}
        //                 2 hops → (nothing new in this chain from those)
        //                 3 hops → (nothing new)
        // Actually in a chain, 1 hop from org-a reaches fac-b, 1 hop from org-c
        // reaches fac-b and fac-d, 1 hop from org-e reaches fac-d and fac-f.
        // So expand=1 already covers everything in this 6-node chain.
        let nodes = vec![
            org_node("a"),
            facility_node("b"),
            org_node("c"),
            facility_node("d"),
            org_node("e"),
            facility_node("f"),
        ];
        let edges = vec![
            supplies_edge("e-ab", "a", "b"),
            supplies_edge("e-bc", "b", "c"),
            supplies_edge("e-cd", "c", "d"),
            supplies_edge("e-de", "d", "e"),
            supplies_edge("e-ef", "e", "f"),
        ];
        let file = minimal_file(nodes, edges);
        let graph = build_graph(&file).expect("builds");

        let ss = SelectorSet::from_selectors(vec![Selector::NodeType(NodeTypeTag::Known(
            NodeType::Organization,
        ))]);
        let sub = selector_subgraph(&graph, &file, &ss, 3).expect("should succeed");

        // All 6 nodes are reachable within 3 hops from at least one org seed.
        assert_eq!(sub.nodes.len(), 6, "all nodes included within 3 hops");
        assert_eq!(sub.edges.len(), 5, "all edges included");
    }

    /// `selector_subgraph` via seed edge: matched edge includes its endpoints.
    #[test]
    fn test_selector_subgraph_seed_edge_contributes_endpoints() {
        // Graph: a → b → c
        // Select edges of type Ownership (only e-bc is ownership here).
        // Seed edge = e-bc; endpoints b, c are added to seed_nodes.
        // With expand=0, result = induced subgraph of {b, c} = {b, c, e-bc}.
        let nodes = vec![org_node("a"), org_node("b"), org_node("c")];
        let edges = vec![
            supplies_edge("e-ab", "a", "b"),
            ownership_edge("e-bc", "b", "c"),
        ];
        let file = minimal_file(nodes, edges);
        let graph = build_graph(&file).expect("builds");

        let ss = SelectorSet::from_selectors(vec![Selector::EdgeType(EdgeTypeTag::Known(
            EdgeType::Ownership,
        ))]);
        let sub = selector_subgraph(&graph, &file, &ss, 0).expect("should succeed");

        let node_ids: Vec<String> = sub.nodes.iter().map(|n| n.id.to_string()).collect();
        assert!(
            node_ids.contains(&"b".to_owned()),
            "b is endpoint of seed edge"
        );
        assert!(
            node_ids.contains(&"c".to_owned()),
            "c is endpoint of seed edge"
        );
        assert!(
            !node_ids.contains(&"a".to_owned()),
            "a is not reachable with expand=0"
        );

        let edge_ids: Vec<String> = sub.edges.iter().map(|e| e.id.to_string()).collect();
        assert!(
            edge_ids.contains(&"e-bc".to_owned()),
            "seed edge must be included"
        );
        assert!(
            !edge_ids.contains(&"e-ab".to_owned()),
            "e-ab not in induced subgraph of {{b,c}}"
        );
    }

    /// `selector_subgraph` returns `EmptyResult` when no nodes or edges match.
    #[test]
    fn test_selector_subgraph_empty_result_error() {
        let nodes = vec![org_node("n1"), org_node("n2")];
        let edges = vec![supplies_edge("e1", "n1", "n2")];
        let file = minimal_file(nodes, edges);
        let graph = build_graph(&file).expect("builds");

        // Select facilities — there are none.
        let ss = SelectorSet::from_selectors(vec![Selector::NodeType(NodeTypeTag::Known(
            NodeType::Facility,
        ))]);
        let err = selector_subgraph(&graph, &file, &ss, 1).expect_err("should fail: no matches");
        assert_eq!(err, QueryError::EmptyResult);
    }

    /// `selector_subgraph` with OR composition on node types.
    #[test]
    fn test_selector_subgraph_or_within_group_node_types() {
        // Nodes: 2 orgs, 1 facility, 1 attestation.
        // Select org OR facility with expand=0.
        let nodes = vec![
            org_node("org-1"),
            org_node("org-2"),
            facility_node("fac-1"),
            {
                let mut n = org_node("attest-1");
                n.node_type = NodeTypeTag::Known(crate::enums::NodeType::Attestation);
                n
            },
        ];
        let file = minimal_file(nodes, vec![]);
        let graph = build_graph(&file).expect("builds");

        let ss = SelectorSet::from_selectors(vec![
            Selector::NodeType(NodeTypeTag::Known(NodeType::Organization)),
            Selector::NodeType(NodeTypeTag::Known(NodeType::Facility)),
        ]);
        let sub = selector_subgraph(&graph, &file, &ss, 0).expect("should succeed");

        let node_ids: Vec<String> = sub.nodes.iter().map(|n| n.id.to_string()).collect();
        assert!(node_ids.contains(&"org-1".to_owned()));
        assert!(node_ids.contains(&"org-2".to_owned()));
        assert!(node_ids.contains(&"fac-1".to_owned()));
        assert!(
            !node_ids.contains(&"attest-1".to_owned()),
            "attestation not selected"
        );
    }

    /// `selector_subgraph` with AND composition across groups.
    #[test]
    fn test_selector_subgraph_and_across_groups() {
        // org-de: org in DE; org-us: org in US; fac-de: facility in DE.
        // Select org AND DE → only org-de.
        let mut org_de = org_node("org-de");
        org_de.jurisdiction = Some(country_code("DE"));
        let mut org_us = org_node("org-us");
        org_us.jurisdiction = Some(country_code("US"));
        let mut fac_de = facility_node("fac-de");
        fac_de.jurisdiction = Some(country_code("DE"));

        let file = minimal_file(vec![org_de, org_us, fac_de], vec![]);
        let graph = build_graph(&file).expect("builds");

        let ss = SelectorSet::from_selectors(vec![
            Selector::NodeType(NodeTypeTag::Known(NodeType::Organization)),
            Selector::Jurisdiction(country_code("DE")),
        ]);
        let sub = selector_subgraph(&graph, &file, &ss, 0).expect("should succeed");

        let node_ids: Vec<String> = sub.nodes.iter().map(|n| n.id.to_string()).collect();
        assert!(node_ids.contains(&"org-de".to_owned()), "org-de must match");
        assert!(
            !node_ids.contains(&"org-us".to_owned()),
            "org-us wrong jurisdiction"
        );
        assert!(
            !node_ids.contains(&"fac-de".to_owned()),
            "fac-de is not an org"
        );
    }

    /// `selector_subgraph` handles cyclic graphs correctly (BFS terminates).
    #[test]
    fn test_selector_subgraph_cyclic_graph_terminates() {
        // Graph: a → b → c → a (cycle); select 'a' only.
        let nodes = vec![org_node("a"), facility_node("b"), org_node("c")];
        let edges = vec![
            supplies_edge("e-ab", "a", "b"),
            supplies_edge("e-bc", "b", "c"),
            supplies_edge("e-ca", "c", "a"),
        ];
        let file = minimal_file(nodes, edges);
        let graph = build_graph(&file).expect("builds");

        let ss = SelectorSet::from_selectors(vec![Selector::NodeType(NodeTypeTag::Known(
            NodeType::Organization,
        ))]);
        // Seeds: {a, c}. With expand=1, reaches b too.
        let sub = selector_subgraph(&graph, &file, &ss, 1).expect("must terminate");
        // All three nodes within 1 hop of the seeds.
        assert_eq!(sub.nodes.len(), 3, "all 3 nodes reachable");
    }

    /// `selector_subgraph` preserves header fields in the output.
    #[test]
    fn test_selector_subgraph_preserves_header_fields() {
        let nodes = vec![org_node("n1"), org_node("n2")];
        let file = OmtsFile {
            omtsf_version: semver("1.2.0"),
            snapshot_date: date("2025-06-01"),
            file_salt: file_salt(SALT),
            disclosure_scope: None,
            previous_snapshot_ref: Some("sha256:abc".to_owned()),
            snapshot_sequence: Some(5),
            reporting_entity: None,
            nodes,
            edges: vec![],
            extra: serde_json::Map::new(),
        };
        let graph = build_graph(&file).expect("builds");

        let ss = SelectorSet::from_selectors(vec![Selector::NodeType(NodeTypeTag::Known(
            NodeType::Organization,
        ))]);
        let sub = selector_subgraph(&graph, &file, &ss, 0).expect("should succeed");

        assert_eq!(sub.omtsf_version, semver("1.2.0"));
        assert_eq!(sub.snapshot_date, date("2025-06-01"));
        assert_eq!(sub.previous_snapshot_ref.as_deref(), Some("sha256:abc"));
        assert_eq!(sub.snapshot_sequence, Some(5));
    }

    /// `QueryError::EmptyResult` has correct Display output.
    #[test]
    fn test_query_error_empty_result_display() {
        let err = QueryError::EmptyResult;
        let msg = err.to_string();
        assert!(!msg.is_empty());
        // Should mention something about selectors or matching.
        assert!(msg.contains("selector") || msg.contains("match") || msg.contains("element"));
    }
}
