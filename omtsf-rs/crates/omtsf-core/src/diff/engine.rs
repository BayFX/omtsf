use std::collections::HashSet;

use crate::file::OmtsFile;
use crate::types::{Identifier, Label};

use super::helpers::{edge_type_str, node_type_str};
use super::matching::{match_edges, match_nodes};
use super::props::{
    compare_edge_props, compare_identifiers, compare_labels, compare_node_properties,
};
use super::types::{
    DiffFilter, DiffResult, EdgeDiff, EdgeRef, EdgesDiff, NodeDiff, NodeRef, NodesDiff,
};

/// Compares two parsed OMTSF files and returns a description of the differences.
///
/// File A is the baseline ("before"); file B is the target ("after").
/// Additions are elements present in B but not A; deletions are elements
/// present in A but not B.
///
/// # Algorithm
///
/// 1. Build canonical identifier indices for both files.
/// 2. Run union-find transitive closure to match nodes by shared identifiers.
/// 3. Detect ambiguous match groups (multiple nodes from the same file in one group).
/// 4. Match edges by resolved endpoints, type, and identity properties.
/// 5. Compare properties of matched pairs and classify as `modified` or `unchanged`.
pub fn diff(a: &OmtsFile, b: &OmtsFile) -> DiffResult {
    diff_filtered(a, b, None)
}

/// Compares two parsed OMTSF files with an optional filter.
///
/// When `filter` is `None`, behaves identically to [`diff`].
pub fn diff_filtered(a: &OmtsFile, b: &OmtsFile, filter: Option<&DiffFilter>) -> DiffResult {
    let mut warnings: Vec<String> = Vec::new();

    let empty_ignore: HashSet<String> = HashSet::new();

    if a.omtsf_version != b.omtsf_version {
        warnings.push(format!(
            "Version mismatch: A has {}, B has {}",
            a.omtsf_version, b.omtsf_version
        ));
    }

    let node_match = match_nodes(&a.nodes, &b.nodes, filter);
    warnings.extend(node_match.warnings);

    let mut nodes_diff = NodesDiff::default();

    for ai in node_match.unmatched_a {
        nodes_diff.removed.push(NodeRef::from_node(&a.nodes[ai]));
    }
    for bi in node_match.unmatched_b {
        nodes_diff.added.push(NodeRef::from_node(&b.nodes[bi]));
    }
    let ignore = filter.map_or(&empty_ignore, |f| &f.ignore_fields);
    for (ai, bi, matched_by) in &node_match.matched {
        let node_a = &a.nodes[*ai];
        let node_b = &b.nodes[*bi];

        let property_changes = compare_node_properties(node_a, node_b, ignore);

        let empty_ids: &[Identifier] = &[];
        let identifier_changes = compare_identifiers(
            node_a.identifiers.as_deref().unwrap_or(empty_ids),
            node_b.identifiers.as_deref().unwrap_or(empty_ids),
        );

        let empty_labels: &[Label] = &[];
        let label_changes = compare_labels(
            node_a.labels.as_deref().unwrap_or(empty_labels),
            node_b.labels.as_deref().unwrap_or(empty_labels),
        );

        let is_modified = !property_changes.is_empty()
            || !identifier_changes.added.is_empty()
            || !identifier_changes.removed.is_empty()
            || !identifier_changes.modified.is_empty()
            || !label_changes.added.is_empty()
            || !label_changes.removed.is_empty();

        let nd = NodeDiff {
            id_a: node_a.id.to_string(),
            id_b: node_b.id.to_string(),
            node_type: node_type_str(&node_a.node_type).to_owned(),
            matched_by: matched_by.clone(),
            property_changes,
            identifier_changes,
            label_changes,
        };
        if is_modified {
            nodes_diff.modified.push(nd);
        } else {
            nodes_diff.unchanged.push(nd);
        }
    }

    let (matched_edge_pairs, unmatched_a_edges, unmatched_b_edges) = match_edges(
        &a.edges,
        &b.edges,
        &a.nodes,
        &b.nodes,
        &node_match.matched,
        filter,
    );

    let mut edges_diff = EdgesDiff::default();

    for ai in unmatched_a_edges {
        edges_diff.removed.push(EdgeRef::from_edge(&a.edges[ai]));
    }
    for bi in unmatched_b_edges {
        edges_diff.added.push(EdgeRef::from_edge(&b.edges[bi]));
    }
    for (ai, bi) in matched_edge_pairs {
        let edge_a = &a.edges[ai];
        let edge_b = &b.edges[bi];

        let property_changes = compare_edge_props(&edge_a.properties, &edge_b.properties, ignore);

        let empty_ids: &[Identifier] = &[];
        let identifier_changes = compare_identifiers(
            edge_a.identifiers.as_deref().unwrap_or(empty_ids),
            edge_b.identifiers.as_deref().unwrap_or(empty_ids),
        );

        let empty_labels: &[Label] = &[];
        let label_changes = compare_labels(
            edge_a.properties.labels.as_deref().unwrap_or(empty_labels),
            edge_b.properties.labels.as_deref().unwrap_or(empty_labels),
        );

        let is_modified = !property_changes.is_empty()
            || !identifier_changes.added.is_empty()
            || !identifier_changes.removed.is_empty()
            || !identifier_changes.modified.is_empty()
            || !label_changes.added.is_empty()
            || !label_changes.removed.is_empty();

        let ed = EdgeDiff {
            id_a: edge_a.id.to_string(),
            id_b: edge_b.id.to_string(),
            edge_type: edge_type_str(&edge_a.edge_type).to_owned(),
            property_changes,
            identifier_changes,
            label_changes,
        };
        if is_modified {
            edges_diff.modified.push(ed);
        } else {
            edges_diff.unchanged.push(ed);
        }
    }

    DiffResult {
        nodes: nodes_diff,
        edges: edges_diff,
        warnings,
    }
}
