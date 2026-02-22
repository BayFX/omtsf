use std::collections::HashSet;

use crate::canonical::CanonicalId;
use crate::newtypes::{EdgeId, NodeId};
use crate::structures::{Edge, Node};
use crate::types::{Identifier, Label};

use super::helpers::{edge_type_str, node_type_str};

/// A lightweight reference to a node, carrying just enough information for
/// readable diff output without cloning the full [`Node`].
#[derive(Debug, Clone, PartialEq)]
pub struct NodeRef {
    /// Graph-local node identifier.
    pub id: NodeId,
    /// Node type string (from the file).
    pub node_type: String,
    /// Display name of the node, if present.
    pub name: Option<String>,
}

impl NodeRef {
    pub(super) fn from_node(node: &Node) -> Self {
        Self {
            id: node.id.clone(),
            node_type: node_type_str(&node.node_type).to_owned(),
            name: node.name.clone(),
        }
    }
}

/// A lightweight reference to an edge.
#[derive(Debug, Clone, PartialEq)]
pub struct EdgeRef {
    /// Graph-local edge identifier.
    pub id: EdgeId,
    /// Edge type string.
    pub edge_type: String,
    /// Source node identifier.
    pub source: NodeId,
    /// Target node identifier.
    pub target: NodeId,
}

impl EdgeRef {
    pub(super) fn from_edge(edge: &Edge) -> Self {
        Self {
            id: edge.id.clone(),
            edge_type: edge_type_str(&edge.edge_type).to_owned(),
            source: edge.source.clone(),
            target: edge.target.clone(),
        }
    }
}

/// A change to a single scalar property field.
#[derive(Debug, Clone, PartialEq)]
pub struct PropertyChange {
    /// Name of the field that changed.
    pub field: String,
    /// Value in file A (baseline), or `None` if the field was absent.
    pub old_value: Option<serde_json::Value>,
    /// Value in file B (target), or `None` if the field is absent.
    pub new_value: Option<serde_json::Value>,
}

/// Diff of the `identifiers` set between two matched elements.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct IdentifierSetDiff {
    /// Identifiers present in B but not A.
    pub added: Vec<Identifier>,
    /// Identifiers present in A but not B.
    pub removed: Vec<Identifier>,
    /// Identifiers present in both with field-level changes.
    pub modified: Vec<IdentifierFieldDiff>,
}

/// Field-level changes on a single identifier that exists in both A and B.
#[derive(Debug, Clone, PartialEq)]
pub struct IdentifierFieldDiff {
    /// Canonical key identifying which identifier changed.
    pub canonical_key: CanonicalId,
    /// Scalar field changes within this identifier.
    pub field_changes: Vec<PropertyChange>,
}

/// Diff of the `labels` set between two matched elements.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LabelSetDiff {
    /// Labels present in B but not A.
    pub added: Vec<Label>,
    /// Labels present in A but not B.
    pub removed: Vec<Label>,
}

/// Differences found between a matched pair of nodes.
#[derive(Debug, Clone, PartialEq)]
pub struct NodeDiff {
    /// Graph-local ID of the node in file A.
    pub id_a: String,
    /// Graph-local ID of the node in file B.
    pub id_b: String,
    /// Node type (expected to be identical; if not, the most-specific value is used).
    pub node_type: String,
    /// Canonical identifier strings that caused the match.
    pub matched_by: Vec<String>,
    /// Scalar property changes detected between the two nodes.
    pub property_changes: Vec<PropertyChange>,
    /// Identifier set differences: identifiers added, removed, or modified.
    pub identifier_changes: IdentifierSetDiff,
    /// Label set differences: labels added or removed.
    pub label_changes: LabelSetDiff,
}

/// Differences found between a matched pair of edges.
#[derive(Debug, Clone, PartialEq)]
pub struct EdgeDiff {
    /// Graph-local ID of the edge in file A.
    pub id_a: String,
    /// Graph-local ID of the edge in file B.
    pub id_b: String,
    /// Edge type.
    pub edge_type: String,
    /// Scalar property changes detected between the two edges.
    pub property_changes: Vec<PropertyChange>,
    /// Identifier set differences: identifiers added, removed, or modified.
    pub identifier_changes: IdentifierSetDiff,
    /// Label set differences: labels added or removed.
    pub label_changes: LabelSetDiff,
}

/// Classification of node differences between two files.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct NodesDiff {
    /// Nodes present in B but not A (additions).
    pub added: Vec<NodeRef>,
    /// Nodes present in A but not B (deletions).
    pub removed: Vec<NodeRef>,
    /// Nodes present in both files (matched pairs) with any field-level differences.
    pub modified: Vec<NodeDiff>,
    /// Nodes present in both files with no differences.
    pub unchanged: Vec<NodeDiff>,
}

/// Classification of edge differences between two files.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EdgesDiff {
    /// Edges present in B but not A (additions).
    pub added: Vec<EdgeRef>,
    /// Edges present in A but not B (deletions).
    pub removed: Vec<EdgeRef>,
    /// Edges present in both files (matched pairs) with any field-level differences.
    pub modified: Vec<EdgeDiff>,
    /// Edges present in both files with no differences.
    pub unchanged: Vec<EdgeDiff>,
}

/// Optional filter to restrict which nodes and edges are compared.
///
/// Filtering by node type also filters edges: edges whose source or target
/// has a filtered-out node type are excluded from the diff.
#[derive(Debug, Clone, Default)]
pub struct DiffFilter {
    /// If set, only diff nodes of these types; `None` means all types.
    pub node_types: Option<HashSet<String>>,
    /// If set, only diff edges of these types; `None` means all types.
    pub edge_types: Option<HashSet<String>>,
    /// Property names to exclude from comparison.
    pub ignore_fields: HashSet<String>,
}

/// Summary statistics for a diff result.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DiffSummary {
    /// Number of nodes added (in B, not in A).
    pub nodes_added: usize,
    /// Number of nodes removed (in A, not in B).
    pub nodes_removed: usize,
    /// Number of matched node pairs with at least one changed field.
    pub nodes_modified: usize,
    /// Number of matched node pairs with no changed fields.
    pub nodes_unchanged: usize,
    /// Number of edges added (in B, not in A).
    pub edges_added: usize,
    /// Number of edges removed (in A, not in B).
    pub edges_removed: usize,
    /// Number of matched edge pairs with at least one changed field.
    pub edges_modified: usize,
    /// Number of matched edge pairs with no changed fields.
    pub edges_unchanged: usize,
}

/// The complete result of a structural diff between two OMTSF files.
#[derive(Debug, Clone, PartialEq)]
pub struct DiffResult {
    /// Node-level classification.
    pub nodes: NodesDiff,
    /// Edge-level classification.
    pub edges: EdgesDiff,
    /// Diagnostic warnings (e.g. ambiguous match groups).
    pub warnings: Vec<String>,
}

impl DiffResult {
    /// Returns a summary of the diff result.
    pub fn summary(&self) -> DiffSummary {
        DiffSummary {
            nodes_added: self.nodes.added.len(),
            nodes_removed: self.nodes.removed.len(),
            nodes_modified: self.nodes.modified.len(),
            nodes_unchanged: self.nodes.unchanged.len(),
            edges_added: self.edges.added.len(),
            edges_removed: self.edges.removed.len(),
            edges_modified: self.edges.modified.len(),
            edges_unchanged: self.edges.unchanged.len(),
        }
    }

    /// Returns `true` if there are no additions, removals, or modifications.
    pub fn is_empty(&self) -> bool {
        self.nodes.added.is_empty()
            && self.nodes.removed.is_empty()
            && self.nodes.modified.is_empty()
            && self.edges.added.is_empty()
            && self.edges.removed.is_empty()
            && self.edges.modified.is_empty()
    }
}

/// Result of node matching.
///
/// Contains pairs of (`a_idx`, `b_idx`) that matched, the canonical identifier
/// strings that caused each pair to match, and diagnostic warnings.
pub(super) struct NodeMatchResult {
    /// Matched pairs: (index in a.nodes, index in b.nodes, matched-by strings).
    pub matched: Vec<(usize, usize, Vec<String>)>,
    /// Node indices in A that were not matched.
    pub unmatched_a: Vec<usize>,
    /// Node indices in B that were not matched.
    pub unmatched_b: Vec<usize>,
    /// Diagnostic warnings for ambiguous match groups.
    pub warnings: Vec<String>,
}
