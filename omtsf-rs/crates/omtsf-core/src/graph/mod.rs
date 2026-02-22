/// Graph construction from an [`OmtsFile`] using `petgraph`, plus query algorithms.
///
/// This module implements Sections 2–6 of the graph-engine technical specification:
/// wrapping a `StableDiGraph` with typed node and edge weights, building from
/// an in-memory [`OmtsFile`], exposing traversal, query, subgraph extraction,
/// and cycle-detection algorithms.
///
/// # Two-Pass Construction
///
/// [`build_graph`] runs two passes over the file:
/// 1. **Node pass** — inserts all nodes into the `StableDiGraph` and records
///    the `local_id → NodeIndex` mapping. Fails on duplicate IDs.
/// 2. **Edge pass** — resolves `source`/`target` IDs and inserts edges.
///    Fails if either endpoint is not present in the node map.
///
/// # Query Algorithms
///
/// See the [`queries`] submodule for reachability, shortest-path, and
/// all-paths enumeration.
///
/// # Subgraph Extraction
///
/// See the [`extraction`] submodule for [`induced_subgraph`], [`ego_graph`],
/// [`selector_match`], and [`selector_subgraph`].
///
/// # Selector-Based Queries
///
/// See the [`selectors`] submodule for [`selectors::Selector`] and
/// [`selectors::SelectorSet`], which implement property-based filtering of
/// nodes and edges.
///
/// # Cycle Detection
///
/// See the [`cycles`] submodule for Kahn's algorithm cycle detection, used by
/// the validation engine to enforce L3-MRG-02 (legal parentage must be a forest).
pub mod cycles;
pub mod extraction;
pub mod queries;
pub mod selectors;

pub use cycles::detect_cycles;
pub use extraction::{
    SelectorMatchResult, ego_graph, induced_subgraph, selector_match, selector_subgraph,
};
pub use queries::{
    DEFAULT_MAX_DEPTH, Direction, QueryError, all_paths, reachable_from, shortest_path,
};
pub use selectors::{Selector, SelectorSet};

use std::collections::HashMap;

use petgraph::stable_graph::{EdgeIndex, NodeIndex, StableDiGraph};

use crate::enums::{EdgeTypeTag, NodeTypeTag};
use crate::file::OmtsFile;

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
    nodes_by_type: HashMap<NodeTypeTag, Vec<NodeIndex>>,
    edges_by_type: HashMap<EdgeTypeTag, Vec<EdgeIndex>>,
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

    /// Returns the [`NodeIndex`] values for all nodes of the given type.
    ///
    /// Returns an empty slice if no nodes of that type exist.
    pub fn nodes_of_type(&self, t: &NodeTypeTag) -> &[NodeIndex] {
        self.nodes_by_type.get(t).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Returns the [`EdgeIndex`] values for all edges of the given type.
    ///
    /// Returns an empty slice if no edges of that type exist.
    pub fn edges_of_type(&self, t: &EdgeTypeTag) -> &[EdgeIndex] {
        self.edges_by_type.get(t).map(Vec::as_slice).unwrap_or(&[])
    }
}

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
    let mut nodes_by_type: HashMap<NodeTypeTag, Vec<NodeIndex>> = HashMap::new();
    let mut edges_by_type: HashMap<EdgeTypeTag, Vec<EdgeIndex>> = HashMap::new();

    for (data_index, node) in file.nodes.iter().enumerate() {
        if id_to_index.contains_key(&*node.id) {
            return Err(GraphBuildError::DuplicateNodeId(node.id.to_string()));
        }

        let local_id = node.id.to_string();
        let weight = NodeWeight {
            local_id: local_id.clone(),
            node_type: node.node_type.clone(),
            data_index,
        };

        let idx = graph.add_node(weight);
        id_to_index.insert(local_id, idx);
        nodes_by_type
            .entry(node.node_type.clone())
            .or_default()
            .push(idx);
    }

    for (data_index, edge) in file.edges.iter().enumerate() {
        let source_idx = id_to_index.get(&*edge.source).copied().ok_or_else(|| {
            GraphBuildError::DanglingEdgeRef {
                edge_id: edge.id.to_string(),
                missing_node_id: edge.source.to_string(),
            }
        })?;

        let target_idx = id_to_index.get(&*edge.target).copied().ok_or_else(|| {
            GraphBuildError::DanglingEdgeRef {
                edge_id: edge.id.to_string(),
                missing_node_id: edge.target.to_string(),
            }
        })?;

        let weight = EdgeWeight {
            local_id: edge.id.to_string(),
            edge_type: edge.edge_type.clone(),
            data_index,
        };

        let edge_idx = graph.add_edge(source_idx, target_idx, weight);
        edges_by_type
            .entry(edge.edge_type.clone())
            .or_default()
            .push(edge_idx);
    }

    Ok(OmtsGraph {
        graph,
        id_to_index,
        nodes_by_type,
        edges_by_type,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use crate::enums::NodeTypeTag;
    use crate::test_helpers::{
        extension_edge, extension_node, minimal_file, org_node, supplies_edge,
    };

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
