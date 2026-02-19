/// Graph construction from an [`OmtsFile`] using `petgraph`.
///
/// This module implements Section 2 of the graph-engine technical specification:
/// wrapping a `StableDiGraph` with typed node and edge weights, building from
/// an in-memory [`OmtsFile`], and exposing accessors for traversal algorithms.
///
/// # Two-Pass Construction
///
/// [`build_graph`] runs two passes over the file:
/// 1. **Node pass** — inserts all nodes into the `StableDiGraph` and records
///    the `local_id → NodeIndex` mapping. Fails on duplicate IDs.
/// 2. **Edge pass** — resolves `source`/`target` IDs and inserts edges.
///    Fails if either endpoint is not present in the node map.
use std::collections::HashMap;

use petgraph::stable_graph::{EdgeIndex, NodeIndex, StableDiGraph};

use crate::enums::{EdgeTypeTag, NodeTypeTag};
use crate::file::OmtsFile;

// ---------------------------------------------------------------------------
// Weight types
// ---------------------------------------------------------------------------

/// Weight stored inline on each petgraph node.
///
/// Designed for cache-friendly traversal: the struct is small (≈56 bytes on
/// 64-bit) so that BFS/DFS loops over the petgraph node slab stay within a
/// few cache lines. Full property data is accessed via `data_index` into the
/// parallel `Vec<Node>` in the originating [`OmtsFile`].
#[derive(Debug, Clone)]
pub struct NodeWeight {
    /// Graph-local identifier copied from the `.omts` node's `id` field.
    pub local_id: String,
    /// Node subtype: known built-in or extension string.
    pub node_type: NodeTypeTag,
    /// Index into the `OmtsFile::nodes` vector for the full deserialized node.
    pub data_index: usize,
}

/// Weight stored inline on each petgraph edge.
///
/// Mirrors [`NodeWeight`] in design rationale: small struct for cache-friendly
/// traversal, with `data_index` pointing into `OmtsFile::edges` for full data.
#[derive(Debug, Clone)]
pub struct EdgeWeight {
    /// Graph-local identifier copied from the `.omts` edge's `id` field.
    pub local_id: String,
    /// Edge subtype: known built-in or extension string.
    pub edge_type: EdgeTypeTag,
    /// Index into the `OmtsFile::edges` vector for the full deserialized edge.
    pub data_index: usize,
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can occur during graph construction from an [`OmtsFile`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphBuildError {
    /// Two nodes in the file share the same `local_id`.
    ///
    /// The contained string is the duplicate ID.
    DuplicateNodeId(String),
    /// An edge references a `source` or `target` node ID that is not present
    /// in the node set.
    ///
    /// The first field is the edge's `local_id`; the second is the missing
    /// node ID.
    DanglingEdgeRef {
        /// The ID of the edge that contains the dangling reference.
        edge_id: String,
        /// The node ID that could not be resolved.
        missing_node_id: String,
    },
}

impl std::fmt::Display for GraphBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GraphBuildError::DuplicateNodeId(id) => {
                write!(f, "duplicate node ID: {id:?}")
            }
            GraphBuildError::DanglingEdgeRef {
                edge_id,
                missing_node_id,
            } => {
                write!(
                    f,
                    "edge {edge_id:?} references unknown node {missing_node_id:?}"
                )
            }
        }
    }
}

impl std::error::Error for GraphBuildError {}

// ---------------------------------------------------------------------------
// OmtsGraph
// ---------------------------------------------------------------------------

/// A directed labeled property multigraph built from an [`OmtsFile`].
///
/// Wraps a `petgraph` [`StableDiGraph`] with typed [`NodeWeight`] and
/// [`EdgeWeight`] structs, and maintains a `HashMap<String, NodeIndex>` for
/// O(1) lookup of nodes by their graph-local ID.
///
/// Indices remain valid after node or edge removal because [`StableDiGraph`]
/// uses tombstones rather than compacting on removal.
///
/// Construct with [`build_graph`].
#[derive(Debug)]
pub struct OmtsGraph {
    graph: StableDiGraph<NodeWeight, EdgeWeight>,
    id_to_index: HashMap<String, NodeIndex>,
}

impl OmtsGraph {
    /// Returns the number of nodes currently in the graph.
    pub fn node_count(&self) -> usize {
        self.graph.node_count()
    }

    /// Returns the number of edges currently in the graph.
    pub fn edge_count(&self) -> usize {
        self.graph.edge_count()
    }

    /// Looks up the [`NodeIndex`] for a graph-local node ID string.
    ///
    /// Returns `None` if no node with that ID exists in the graph.
    pub fn node_index(&self, id: &str) -> Option<&NodeIndex> {
        self.id_to_index.get(id)
    }

    /// Returns the [`NodeWeight`] for the given index, or `None` if the index
    /// is out of bounds or refers to a removed node.
    pub fn node_weight(&self, idx: NodeIndex) -> Option<&NodeWeight> {
        self.graph.node_weight(idx)
    }

    /// Returns the [`EdgeWeight`] for the given index, or `None` if the index
    /// is out of bounds or refers to a removed edge.
    pub fn edge_weight(&self, idx: EdgeIndex) -> Option<&EdgeWeight> {
        self.graph.edge_weight(idx)
    }

    /// Returns a reference to the underlying [`StableDiGraph`] for use by
    /// traversal and query algorithms.
    pub fn graph(&self) -> &StableDiGraph<NodeWeight, EdgeWeight> {
        &self.graph
    }
}

// ---------------------------------------------------------------------------
// Construction
// ---------------------------------------------------------------------------

/// Constructs an [`OmtsGraph`] from a deserialized [`OmtsFile`].
///
/// Construction is O(N + E) where N is node count and E is edge count.
///
/// # Two-Pass Process
///
/// **Pass 1 — Nodes:** iterates `file.nodes`, inserts each node into the
/// `StableDiGraph` with a [`NodeWeight`], and records the
/// `local_id → NodeIndex` mapping. Returns
/// [`GraphBuildError::DuplicateNodeId`] immediately if a local ID appears
/// more than once.
///
/// **Pass 2 — Edges:** iterates `file.edges`, resolves `source` and `target`
/// IDs through the index map, then inserts each edge with an [`EdgeWeight`].
/// Returns [`GraphBuildError::DanglingEdgeRef`] immediately if either
/// endpoint ID is not present in the node map.
///
/// # Errors
///
/// - [`GraphBuildError::DuplicateNodeId`] — two nodes share the same ID.
/// - [`GraphBuildError::DanglingEdgeRef`] — an edge references a node that
///   does not exist.
pub fn build_graph(file: &OmtsFile) -> Result<OmtsGraph, GraphBuildError> {
    let node_count = file.nodes.len();
    let edge_count = file.edges.len();

    let mut graph: StableDiGraph<NodeWeight, EdgeWeight> =
        StableDiGraph::with_capacity(node_count, edge_count);
    let mut id_to_index: HashMap<String, NodeIndex> = HashMap::with_capacity(node_count);

    // Pass 1: insert all nodes.
    for (data_index, node) in file.nodes.iter().enumerate() {
        let local_id = node.id.to_string();

        if id_to_index.contains_key(&local_id) {
            return Err(GraphBuildError::DuplicateNodeId(local_id));
        }

        let weight = NodeWeight {
            local_id: local_id.clone(),
            node_type: node.node_type.clone(),
            data_index,
        };

        let idx = graph.add_node(weight);
        id_to_index.insert(local_id, idx);
    }

    // Pass 2: insert all edges.
    for (data_index, edge) in file.edges.iter().enumerate() {
        let edge_id = edge.id.to_string();
        let source_id = edge.source.to_string();
        let target_id = edge.target.to_string();

        let source_idx = id_to_index.get(&source_id).copied().ok_or_else(|| {
            GraphBuildError::DanglingEdgeRef {
                edge_id: edge_id.clone(),
                missing_node_id: source_id,
            }
        })?;

        let target_idx = id_to_index.get(&target_id).copied().ok_or_else(|| {
            GraphBuildError::DanglingEdgeRef {
                edge_id: edge_id.clone(),
                missing_node_id: target_id,
            }
        })?;

        let weight = EdgeWeight {
            local_id: edge_id,
            edge_type: edge.edge_type.clone(),
            data_index,
        };

        graph.add_edge(source_idx, target_idx, weight);
    }

    Ok(OmtsGraph { graph, id_to_index })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use crate::enums::{EdgeType, EdgeTypeTag, NodeType, NodeTypeTag};
    use crate::file::OmtsFile;
    use crate::newtypes::{CalendarDate, EdgeId, FileSalt, NodeId, SemVer};
    use crate::structures::{Edge, EdgeProperties, Node};

    // -----------------------------------------------------------------------
    // Fixture helpers
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

    fn extension_node(id: &str, type_str: &str) -> Node {
        Node {
            id: node_id(id),
            node_type: NodeTypeTag::Extension(type_str.to_owned()),
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

    fn extension_edge(id: &str, source: &str, target: &str, type_str: &str) -> Edge {
        Edge {
            id: edge_id(id),
            edge_type: EdgeTypeTag::Extension(type_str.to_owned()),
            source: node_id(source),
            target: node_id(target),
            identifiers: None,
            properties: EdgeProperties::default(),
            extra: serde_json::Map::new(),
        }
    }

    // -----------------------------------------------------------------------
    // Tests
    // -----------------------------------------------------------------------

    /// An empty file (no nodes, no edges) builds successfully.
    #[test]
    fn test_empty_file_builds_successfully() {
        let file = minimal_file(vec![], vec![]);
        let g = build_graph(&file).expect("empty file should build");
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    /// A simple graph with nodes and edges builds correctly; verify counts.
    #[test]
    fn test_simple_graph_node_and_edge_counts() {
        let nodes = vec![org_node("org-1"), org_node("org-2"), org_node("org-3")];
        let edges = vec![
            supplies_edge("e-1", "org-1", "org-2"),
            supplies_edge("e-2", "org-2", "org-3"),
        ];
        let file = minimal_file(nodes, edges);
        let g = build_graph(&file).expect("should build");
        assert_eq!(g.node_count(), 3);
        assert_eq!(g.edge_count(), 2);
    }

    /// Duplicate node ID causes `DuplicateNodeId` error.
    #[test]
    fn test_duplicate_node_id_returns_error() {
        let nodes = vec![org_node("org-1"), org_node("org-1")];
        let file = minimal_file(nodes, vec![]);
        let err = build_graph(&file).expect_err("should fail on duplicate node");
        assert_eq!(err, GraphBuildError::DuplicateNodeId("org-1".to_owned()));
    }

    /// Dangling edge source causes `DanglingEdgeRef` error.
    #[test]
    fn test_dangling_edge_source_returns_error() {
        let nodes = vec![org_node("org-2")];
        let edges = vec![supplies_edge("e-1", "missing-source", "org-2")];
        let file = minimal_file(nodes, edges);
        let err = build_graph(&file).expect_err("should fail on missing source");
        assert_eq!(
            err,
            GraphBuildError::DanglingEdgeRef {
                edge_id: "e-1".to_owned(),
                missing_node_id: "missing-source".to_owned(),
            }
        );
    }

    /// Dangling edge target causes `DanglingEdgeRef` error.
    #[test]
    fn test_dangling_edge_target_returns_error() {
        let nodes = vec![org_node("org-1")];
        let edges = vec![supplies_edge("e-1", "org-1", "missing-target")];
        let file = minimal_file(nodes, edges);
        let err = build_graph(&file).expect_err("should fail on missing target");
        assert_eq!(
            err,
            GraphBuildError::DanglingEdgeRef {
                edge_id: "e-1".to_owned(),
                missing_node_id: "missing-target".to_owned(),
            }
        );
    }

    /// ID lookup returns the correct `NodeIndex` and resolves back to node weight.
    #[test]
    fn test_id_lookup_returns_correct_node_index() {
        let nodes = vec![org_node("org-alpha"), org_node("org-beta")];
        let file = minimal_file(nodes, vec![]);
        let g = build_graph(&file).expect("should build");

        let idx_alpha = g
            .node_index("org-alpha")
            .expect("org-alpha must be present");
        let idx_beta = g.node_index("org-beta").expect("org-beta must be present");
        assert_ne!(
            idx_alpha, idx_beta,
            "distinct nodes must have distinct indices"
        );

        let weight_alpha = g.node_weight(*idx_alpha).expect("weight must exist");
        assert_eq!(weight_alpha.local_id, "org-alpha");

        let weight_beta = g.node_weight(*idx_beta).expect("weight must exist");
        assert_eq!(weight_beta.local_id, "org-beta");
    }

    /// Extension node types are accepted and stored correctly in [`NodeWeight`].
    #[test]
    fn test_extension_node_type_handled_correctly() {
        let nodes = vec![extension_node("ext-1", "com.example.custom_node")];
        let file = minimal_file(nodes, vec![]);
        let g = build_graph(&file).expect("should build with extension node type");
        assert_eq!(g.node_count(), 1);

        let idx = g.node_index("ext-1").expect("ext-1 must be present");
        let weight = g.node_weight(*idx).expect("weight must exist");
        assert_eq!(
            weight.node_type,
            NodeTypeTag::Extension("com.example.custom_node".to_owned())
        );
    }

    /// Extension edge types are accepted and stored correctly in [`EdgeWeight`].
    #[test]
    fn test_extension_edge_type_handled_correctly() {
        let nodes = vec![org_node("node-a"), org_node("node-b")];
        let edges = vec![extension_edge(
            "e-ext-1",
            "node-a",
            "node-b",
            "com.acme.custom_rel",
        )];
        let file = minimal_file(nodes, edges);
        let g = build_graph(&file).expect("should build with extension edge type");
        assert_eq!(g.edge_count(), 1);
    }

    /// `data_index` in `NodeWeight` matches the node's position in `file.nodes`.
    #[test]
    fn test_node_weight_contains_correct_data_index() {
        let nodes = vec![org_node("first"), org_node("second"), org_node("third")];
        let file = minimal_file(nodes, vec![]);
        let g = build_graph(&file).expect("should build");

        for (node_id_str, expected_data_index) in [("first", 0usize), ("second", 1), ("third", 2)] {
            let idx = g.node_index(node_id_str).expect("node must be present");
            let weight = g.node_weight(*idx).expect("weight must exist");
            assert_eq!(
                weight.data_index, expected_data_index,
                "data_index for {node_id_str} should be {expected_data_index}"
            );
        }
    }

    /// `graph()` exposes the underlying `StableDiGraph` for traversal.
    #[test]
    fn test_graph_accessor_returns_stable_di_graph() {
        let nodes = vec![org_node("n1"), org_node("n2")];
        let edges = vec![supplies_edge("e1", "n1", "n2")];
        let file = minimal_file(nodes, edges);
        let g = build_graph(&file).expect("should build");
        assert_eq!(g.graph().node_count(), 2);
        assert_eq!(g.graph().edge_count(), 1);
    }

    /// `GraphBuildError` Display output contains the relevant ID strings.
    #[test]
    fn test_graph_build_error_display() {
        let dup_err = GraphBuildError::DuplicateNodeId("org-dup".to_owned());
        assert!(dup_err.to_string().contains("org-dup"));

        let dangle_err = GraphBuildError::DanglingEdgeRef {
            edge_id: "e-bad".to_owned(),
            missing_node_id: "ghost-node".to_owned(),
        };
        let msg = dangle_err.to_string();
        assert!(msg.contains("e-bad"));
        assert!(msg.contains("ghost-node"));
    }
}
