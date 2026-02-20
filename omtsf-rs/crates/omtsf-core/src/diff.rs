/// Structural diff engine for OMTSF graph files.
///
/// Implements the matching algorithm and classification described in diff.md
/// Sections 2–4. Two parsed [`OmtsFile`] values are compared; the result
/// describes which nodes and edges were added, removed, or modified.
///
/// # Scope
///
/// - Node matching via canonical identifier indices and union-find transitive closure.
/// - Ambiguity detection (warning when a match group contains multiple nodes from
///   the same file).
/// - Edge matching using resolved endpoints, type, and per-type identity properties.
/// - Property comparison: scalar fields, identifiers set, labels set.
/// - Classification of matched pairs as `modified` or `unchanged` based on
///   whether any property changed.
use std::collections::{HashMap, HashSet};

use serde::Serialize;

use crate::canonical::{CanonicalId, build_identifier_index};
use crate::file::OmtsFile;
use crate::identity::{edges_match, identifiers_match};
use crate::newtypes::{EdgeId, NodeId};
use crate::structures::{Edge, EdgeProperties, Node};
use crate::types::{DataQuality, Identifier, Label};
use crate::union_find::UnionFind;

// ---------------------------------------------------------------------------
// Internal serialization helper
// ---------------------------------------------------------------------------

/// Serializes a type-tag enum (which implements `Serialize` to a JSON string)
/// and returns the unquoted string value.
///
/// Falls back to the `Debug` representation if serialization fails, which
/// should never happen for the well-defined enums in this crate.
fn tag_to_string<T: Serialize>(tag: &T) -> String {
    match serde_json::to_value(tag) {
        Ok(serde_json::Value::String(s)) => s,
        Ok(other) => format!("{other:?}"),
        Err(_) => "<unknown>".to_owned(),
    }
}

// ---------------------------------------------------------------------------
// Public types — lightweight references
// ---------------------------------------------------------------------------

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
    fn from_node(node: &Node) -> Self {
        Self {
            id: node.id.clone(),
            node_type: tag_to_string(&node.node_type),
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
    fn from_edge(edge: &Edge) -> Self {
        Self {
            id: edge.id.clone(),
            edge_type: tag_to_string(&edge.edge_type),
            source: edge.source.clone(),
            target: edge.target.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// Property/identifier/label change types
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// NodeDiff / EdgeDiff
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// NodesDiff / EdgesDiff
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// DiffFilter
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// DiffSummary / DiffResult
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Internal: node matching
// ---------------------------------------------------------------------------

/// Result of node matching.
///
/// Contains pairs of (`a_idx`, `b_idx`) that matched, the canonical identifier
/// strings that caused each pair to match, and diagnostic warnings.
struct NodeMatchResult {
    /// Matched pairs: (index in a.nodes, index in b.nodes, matched-by strings).
    matched: Vec<(usize, usize, Vec<String>)>,
    /// Node indices in A that were not matched.
    unmatched_a: Vec<usize>,
    /// Node indices in B that were not matched.
    unmatched_b: Vec<usize>,
    /// Diagnostic warnings for ambiguous match groups.
    warnings: Vec<String>,
}

/// Performs node matching for a diff.
///
/// Builds canonical identifier indices for both files, finds matching pairs
/// via shared identifiers, computes transitive closure using union-find,
/// detects ambiguous groups, and classifies unmatched nodes.
fn match_nodes(nodes_a: &[Node], nodes_b: &[Node], filter: Option<&DiffFilter>) -> NodeMatchResult {
    // --- filter helpers ---

    let node_type_allowed = |node: &Node| -> bool {
        match filter.and_then(|f| f.node_types.as_ref()) {
            None => true,
            Some(allowed) => allowed.contains(&tag_to_string(&node.node_type)),
        }
    };

    // Build sets of active (filter-passing) indices.
    let active_a: Vec<usize> = (0..nodes_a.len())
        .filter(|&i| node_type_allowed(&nodes_a[i]))
        .collect();
    let active_b: Vec<usize> = (0..nodes_b.len())
        .filter(|&i| node_type_allowed(&nodes_b[i]))
        .collect();

    // Build canonical identifier indices for each file.
    // The index maps CanonicalId → list of node ordinals within the file's slice.
    let index_a = build_identifier_index(nodes_a);
    let index_b = build_identifier_index(nodes_b);

    // We need a unified node space for union-find.
    // Assign ordinals: A nodes get [0, len_a), B nodes get [len_a, len_a+len_b).
    let len_a = nodes_a.len();
    let len_b = nodes_b.len();
    let total = len_a + len_b;

    let mut uf = UnionFind::new(total);

    // Track which canonical keys caused each pair to match, keyed by
    // (a_idx, b_idx) pair for later reporting.
    let mut pair_matched_by: HashMap<(usize, usize), Vec<String>> = HashMap::new();

    // For each canonical key present in both indices, union all A-nodes and
    // B-nodes that share the key.
    for (canonical_id, a_nodes) in &index_a {
        let Some(b_nodes) = index_b.get(canonical_id) else {
            continue;
        };

        // We need to check identifiers_match for each (a_id, b_id) pair because
        // the canonical index groups by key string but identifiers_match also
        // checks authority and temporal compatibility.
        for &ai in a_nodes {
            for &bi in b_nodes {
                // Only process active (filter-passing) nodes.
                if !active_a.contains(&ai) || !active_b.contains(&bi) {
                    continue;
                }

                // Check if any identifier pair on these two nodes actually matches.
                let a_node = &nodes_a[ai];
                let b_node = &nodes_b[bi];

                let a_ids = a_node.identifiers.as_deref().unwrap_or(&[]);
                let b_ids = b_node.identifiers.as_deref().unwrap_or(&[]);

                let mut found_match = false;
                for id_a in a_ids {
                    if id_a.scheme == "internal" {
                        continue;
                    }
                    let cid_a = CanonicalId::from_identifier(id_a);
                    if cid_a != *canonical_id {
                        continue;
                    }
                    for id_b in b_ids {
                        if identifiers_match(id_a, id_b) {
                            found_match = true;
                            break;
                        }
                    }
                    if found_match {
                        break;
                    }
                }

                if found_match {
                    // B node ordinal is offset by len_a in the unified space.
                    uf.union(ai, len_a + bi);
                    pair_matched_by
                        .entry((ai, bi))
                        .or_default()
                        .push(canonical_id.as_str().to_owned());
                }
            }
        }
    }

    // Group all elements by union-find representative.
    // Each group maps representative → (list of a_indices, list of b_indices).
    let mut groups: HashMap<usize, (Vec<usize>, Vec<usize>)> = HashMap::new();

    for &ai in &active_a {
        let rep = uf.find(ai);
        groups.entry(rep).or_default().0.push(ai);
    }
    for &bi in &active_b {
        let rep = uf.find(len_a + bi);
        groups.entry(rep).or_default().1.push(bi);
    }

    let mut matched: Vec<(usize, usize, Vec<String>)> = Vec::new();
    let mut unmatched_a: Vec<usize> = Vec::new();
    let mut unmatched_b: Vec<usize> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    for (rep, (a_members, b_members)) in &groups {
        let _ = rep; // representative not used directly

        match (a_members.as_slice(), b_members.as_slice()) {
            // No match on either side — shouldn't happen since we only form groups
            // from active nodes, but handle defensively.
            ([], []) => {}

            // Only A nodes — deletions.
            (a_list, []) => {
                unmatched_a.extend_from_slice(a_list);
            }

            // Only B nodes — additions.
            ([], b_list) => {
                unmatched_b.extend_from_slice(b_list);
            }

            // Both sides present — matched (possibly ambiguous).
            (a_list, b_list) => {
                // Emit ambiguity warning if more than one node from the same file.
                if a_list.len() > 1 || b_list.len() > 1 {
                    let a_ids: Vec<&str> = a_list.iter().map(|&i| &*nodes_a[i].id).collect();
                    let b_ids: Vec<&str> = b_list.iter().map(|&i| &*nodes_b[i].id).collect();
                    warnings.push(format!(
                        "Ambiguous match group: A=[{}] B=[{}]",
                        a_ids.join(", "),
                        b_ids.join(", ")
                    ));
                }

                // Produce matched pairs: pair each A node with each B node in the group.
                for &ai in a_list {
                    for &bi in b_list {
                        let matched_by =
                            pair_matched_by.get(&(ai, bi)).cloned().unwrap_or_default();
                        matched.push((ai, bi, matched_by));
                    }
                }
            }
        }
    }

    NodeMatchResult {
        matched,
        unmatched_a,
        unmatched_b,
        warnings,
    }
}

// ---------------------------------------------------------------------------
// Internal: edge matching
// ---------------------------------------------------------------------------

/// Builds a map from `NodeId` string to a representative index in the unified
/// node space `[0, len_a + len_b)`.
///
/// For matched nodes, both the A-side and B-side `NodeId` strings map to the
/// same representative. For unmatched nodes, each maps to its own ordinal.
fn build_node_rep_map(
    nodes_a: &[Node],
    nodes_b: &[Node],
    matched_pairs: &[(usize, usize, Vec<String>)],
) -> (HashMap<String, usize>, UnionFind) {
    let len_a = nodes_a.len();
    let len_b = nodes_b.len();
    let total = len_a + len_b;

    let mut uf = UnionFind::new(total);

    // Union matched pairs.
    for &(ai, bi, _) in matched_pairs {
        uf.union(ai, len_a + bi);
    }

    let mut map: HashMap<String, usize> = HashMap::new();

    // Map A-node IDs to their representatives.
    for (ai, node) in nodes_a.iter().enumerate() {
        let rep = uf.find(ai);
        map.insert(node.id.to_string(), rep);
    }
    // Map B-node IDs to their representatives.
    for (bi, node) in nodes_b.iter().enumerate() {
        let rep = uf.find(len_a + bi);
        map.insert(node.id.to_string(), rep);
    }

    (map, uf)
}

/// Performs edge matching after node matching is complete.
///
/// Edges are matched by:
/// 1. Their source nodes belonging to the same match group.
/// 2. Their target nodes belonging to the same match group.
/// 3. Their type values being equal.
/// 4. Sharing an external identifier, OR lacking external identifiers and having
///    equal type-specific identity properties.
///
/// When multiple A-edges match the same B-edge bucket, they are paired by
/// order of appearance. Excess edges are reported as additions or deletions.
fn match_edges(
    edges_a: &[Edge],
    edges_b: &[Edge],
    nodes_a: &[Node],
    nodes_b: &[Node],
    matched_node_pairs: &[(usize, usize, Vec<String>)],
    filter: Option<&DiffFilter>,
) -> (Vec<(usize, usize)>, Vec<usize>, Vec<usize>) {
    // Build representative map.
    let (node_rep_map, _) = build_node_rep_map(nodes_a, nodes_b, matched_node_pairs);

    let edge_type_allowed = |edge: &Edge| -> bool {
        match filter.and_then(|f| f.edge_types.as_ref()) {
            None => true,
            Some(allowed) => allowed.contains(&tag_to_string(&edge.edge_type)),
        }
    };

    // Additionally, edges whose source or target has a filtered-out node type
    // are excluded when node_types filter is active.
    let node_type_allowed_for_id = |node_id: &NodeId| -> bool {
        match filter.and_then(|f| f.node_types.as_ref()) {
            None => true,
            Some(allowed) => {
                // Check in A nodes first, then B nodes.
                let id_str: &str = node_id;
                if let Some(node) = nodes_a.iter().find(|n| &*n.id == id_str) {
                    return allowed.contains(&tag_to_string(&node.node_type));
                }
                if let Some(node) = nodes_b.iter().find(|n| &*n.id == id_str) {
                    return allowed.contains(&tag_to_string(&node.node_type));
                }
                // Unknown node ID — exclude.
                false
            }
        }
    };

    let edge_is_active = |edge: &Edge| -> bool {
        edge_type_allowed(edge)
            && node_type_allowed_for_id(&edge.source)
            && node_type_allowed_for_id(&edge.target)
    };

    let active_a_edges: Vec<usize> = (0..edges_a.len())
        .filter(|&i| edge_is_active(&edges_a[i]))
        .collect();
    let active_b_edges: Vec<usize> = (0..edges_b.len())
        .filter(|&i| edge_is_active(&edges_b[i]))
        .collect();

    // Resolve representatives for each edge's endpoints.
    let resolve_rep = |node_id: &NodeId| -> Option<usize> {
        let key: &str = node_id;
        node_rep_map.get(key).copied()
    };

    // Group A-edges by composite key (src_rep, tgt_rep, edge_type).
    // For same_as edges, edges_match returns false, so they won't be paired.
    type EdgeKey = (usize, usize, String);

    let mut a_buckets: HashMap<EdgeKey, Vec<usize>> = HashMap::new();
    for &ai in &active_a_edges {
        let edge = &edges_a[ai];
        let Some(src_rep) = resolve_rep(&edge.source) else {
            continue;
        };
        let Some(tgt_rep) = resolve_rep(&edge.target) else {
            continue;
        };
        let key = (src_rep, tgt_rep, tag_to_string(&edge.edge_type));
        a_buckets.entry(key).or_default().push(ai);
    }

    let mut matched_pairs: Vec<(usize, usize)> = Vec::new();
    let mut unmatched_b_edges: Vec<usize> = Vec::new();
    // Track which A-edges were consumed.
    let mut matched_a_set: HashSet<usize> = HashSet::new();

    for &bi in &active_b_edges {
        let edge_b = &edges_b[bi];
        let Some(src_rep_b) = resolve_rep(&edge_b.source) else {
            unmatched_b_edges.push(bi);
            continue;
        };
        let Some(tgt_rep_b) = resolve_rep(&edge_b.target) else {
            unmatched_b_edges.push(bi);
            continue;
        };
        let key_b = (src_rep_b, tgt_rep_b, tag_to_string(&edge_b.edge_type));

        let Some(bucket) = a_buckets.get_mut(&key_b) else {
            unmatched_b_edges.push(bi);
            continue;
        };

        // Find the first unmatched A-edge in this bucket that matches edge_b.
        let mut found = false;
        for &ai in bucket.iter() {
            if matched_a_set.contains(&ai) {
                continue;
            }
            let edge_a = &edges_a[ai];
            let Some(src_rep_a) = resolve_rep(&edge_a.source) else {
                continue;
            };
            let Some(tgt_rep_a) = resolve_rep(&edge_a.target) else {
                continue;
            };
            if edges_match(src_rep_a, tgt_rep_a, src_rep_b, tgt_rep_b, edge_a, edge_b) {
                matched_pairs.push((ai, bi));
                matched_a_set.insert(ai);
                found = true;
                break;
            }
        }

        if !found {
            unmatched_b_edges.push(bi);
        }
    }

    // Any active A-edges not in matched_a_set are deletions.
    let unmatched_a_edges: Vec<usize> = active_a_edges
        .into_iter()
        .filter(|ai| !matched_a_set.contains(ai))
        .collect();

    (matched_pairs, unmatched_a_edges, unmatched_b_edges)
}

// ---------------------------------------------------------------------------
// Internal: property comparison
// ---------------------------------------------------------------------------

/// Floating-point epsilon for numeric field comparisons (diff.md Section 3.1).
const NUMERIC_EPSILON: f64 = 1e-9;

/// Normalises a date string to `YYYY-MM-DD` by zero-padding month and day if
/// they are written without leading zeros (e.g. `"2026-2-9"` → `"2026-02-09"`).
///
/// A conformant `CalendarDate` is already zero-padded, but the spec says the
/// diff engine should normalise before comparing to avoid false positives
/// (diff.md Section 3.1).
fn normalise_date(s: &str) -> String {
    let parts: Vec<&str> = s.splitn(3, '-').collect();
    if parts.len() == 3 {
        let year = parts[0];
        let month = parts[1];
        let day = parts[2];
        format!(
            "{}-{:0>2}-{:0>2}",
            year,
            month
                .trim_start_matches('0')
                .parse::<u32>()
                .map_or_else(|_| month.to_owned(), |n| format!("{n:02}")),
            day.trim_start_matches('0')
                .parse::<u32>()
                .map_or_else(|_| day.to_owned(), |n| format!("{n:02}"))
        )
    } else {
        s.to_owned()
    }
}

/// Converts a `serde_json::Value` that might represent a date string to its
/// normalised form. Non-string values are returned as-is.
fn normalise_date_value(v: &serde_json::Value) -> serde_json::Value {
    if let Some(s) = v.as_str() {
        // Only normalise if it looks like a date (contains hyphens).
        if s.contains('-') {
            return serde_json::Value::String(normalise_date(s));
        }
    }
    v.clone()
}

/// Returns `true` if two `serde_json::Value`s are semantically equal under the
/// diff rules:
/// - For strings that look like dates, normalise before comparing.
/// - For numbers, use epsilon comparison.
/// - Otherwise, use structural equality.
fn values_equal(a: &serde_json::Value, b: &serde_json::Value) -> bool {
    use serde_json::Value;
    match (a, b) {
        // Both numbers: compare with epsilon.
        (Value::Number(na), Value::Number(nb)) => {
            match (na.as_f64(), nb.as_f64()) {
                (Some(fa), Some(fb)) => (fa - fb).abs() < NUMERIC_EPSILON,
                // If one can't be represented as f64 (very rare for valid JSON),
                // fall back to structural equality of the Number tokens.
                _ => na == nb,
            }
        }
        // Both strings that look like dates: normalise before comparing.
        (Value::String(sa), Value::String(sb)) => {
            if sa.contains('-') && sb.contains('-') {
                normalise_date(sa) == normalise_date(sb)
            } else {
                sa == sb
            }
        }
        // Everything else: structural equality.
        _ => a == b,
    }
}

/// Converts an `Option<T>` to `Option<serde_json::Value>` by serialising.
/// Returns `None` if the input is `None` or if serialization fails.
fn to_value<T: serde::Serialize>(v: &Option<T>) -> Option<serde_json::Value> {
    let inner = v.as_ref()?;
    serde_json::to_value(inner).ok()
}

/// Emits a `PropertyChange` if `old_value` and `new_value` differ (or if one
/// is `None` and the other is not), using semantic equality.
///
/// Uses `values_equal` for the comparison so that date normalisation and
/// numeric epsilon are applied.
fn maybe_change(
    field: &str,
    old_value: Option<serde_json::Value>,
    new_value: Option<serde_json::Value>,
    out: &mut Vec<PropertyChange>,
) {
    let equal = match (&old_value, &new_value) {
        (None, None) => true,
        (Some(a), Some(b)) => values_equal(a, b),
        _ => false,
    };
    if !equal {
        out.push(PropertyChange {
            field: field.to_owned(),
            old_value,
            new_value,
        });
    }
}

/// Compares the scalar fields of a [`DataQuality`] object.
///
/// Produces changes for `confidence`, `source`, and `last_verified`.
fn compare_data_quality(
    field_prefix: &str,
    a: Option<&DataQuality>,
    b: Option<&DataQuality>,
    ignore: &HashSet<String>,
    out: &mut Vec<PropertyChange>,
) {
    match (a, b) {
        (None, None) => {}
        (Some(aq), Some(bq)) => {
            // Compare each sub-field individually.
            let sub =
                |sub_name: &str, av: Option<serde_json::Value>, bv: Option<serde_json::Value>| {
                    let name = format!("{field_prefix}.{sub_name}");
                    (name, av, bv)
                };
            let checks = [
                sub(
                    "confidence",
                    to_value(&aq.confidence),
                    to_value(&bq.confidence),
                ),
                sub(
                    "source",
                    aq.source
                        .as_deref()
                        .map(|s| serde_json::Value::String(s.to_owned())),
                    bq.source
                        .as_deref()
                        .map(|s| serde_json::Value::String(s.to_owned())),
                ),
                sub(
                    "last_verified",
                    to_value(&aq.last_verified),
                    to_value(&bq.last_verified),
                ),
            ];
            for (name, av, bv) in checks {
                if !ignore.contains(&name) {
                    maybe_change(&name, av, bv, out);
                }
            }
            // Extra fields in data_quality are compared as raw JSON values.
            // Collect all keys from both sides into a set so that keys present
            // in both maps are visited exactly once (avoiding duplicate entries).
            let mut extra_keys: HashSet<&str> = HashSet::new();
            for k in aq.extra.keys() {
                extra_keys.insert(k.as_str());
            }
            for k in bq.extra.keys() {
                extra_keys.insert(k.as_str());
            }
            for key in &extra_keys {
                let name = format!("{field_prefix}.{key}");
                if ignore.contains(&name) {
                    continue;
                }
                let av = aq.extra.get(*key).cloned();
                let bv = bq.extra.get(*key).cloned();
                maybe_change(&name, av, bv, out);
            }
        }
        // One side has data_quality, the other doesn't.
        (Some(aq), None) => {
            let name = field_prefix.to_owned();
            if !ignore.contains(&name) {
                out.push(PropertyChange {
                    field: name,
                    old_value: serde_json::to_value(aq).ok(),
                    new_value: None,
                });
            }
        }
        (None, Some(bq)) => {
            let name = field_prefix.to_owned();
            if !ignore.contains(&name) {
                out.push(PropertyChange {
                    field: name,
                    old_value: None,
                    new_value: serde_json::to_value(bq).ok(),
                });
            }
        }
    }
}

/// Compares the scalar properties of two matched [`Node`]s.
///
/// Fields listed in `ignore` are skipped. Date fields are normalised before
/// comparison. Numeric fields use epsilon comparison.
fn compare_node_properties(a: &Node, b: &Node, ignore: &HashSet<String>) -> Vec<PropertyChange> {
    let mut changes: Vec<PropertyChange> = Vec::new();

    macro_rules! check {
        ($field:expr, $a:expr, $b:expr) => {
            if !ignore.contains($field) {
                maybe_change($field, $a, $b, &mut changes);
            }
        };
    }

    // String fields — simple equality after trimming.
    check!(
        "name",
        a.name
            .as_deref()
            .map(|s| serde_json::Value::String(s.to_owned())),
        b.name
            .as_deref()
            .map(|s| serde_json::Value::String(s.to_owned()))
    );
    check!(
        "jurisdiction",
        to_value(&a.jurisdiction),
        to_value(&b.jurisdiction)
    );
    check!("status", to_value(&a.status), to_value(&b.status));
    check!(
        "governance_structure",
        a.governance_structure.clone(),
        b.governance_structure.clone()
    );
    check!(
        "operator",
        a.operator
            .as_ref()
            .map(|id| serde_json::Value::String(id.to_string())),
        b.operator
            .as_ref()
            .map(|id| serde_json::Value::String(id.to_string()))
    );
    check!(
        "address",
        a.address
            .as_deref()
            .map(|s| serde_json::Value::String(s.to_owned())),
        b.address
            .as_deref()
            .map(|s| serde_json::Value::String(s.to_owned()))
    );
    check!("geo", a.geo.clone(), b.geo.clone());
    check!(
        "commodity_code",
        a.commodity_code
            .as_deref()
            .map(|s| serde_json::Value::String(s.to_owned())),
        b.commodity_code
            .as_deref()
            .map(|s| serde_json::Value::String(s.to_owned()))
    );
    check!(
        "unit",
        a.unit
            .as_deref()
            .map(|s| serde_json::Value::String(s.to_owned())),
        b.unit
            .as_deref()
            .map(|s| serde_json::Value::String(s.to_owned()))
    );
    check!(
        "role",
        a.role
            .as_deref()
            .map(|s| serde_json::Value::String(s.to_owned())),
        b.role
            .as_deref()
            .map(|s| serde_json::Value::String(s.to_owned()))
    );
    check!(
        "attestation_type",
        to_value(&a.attestation_type),
        to_value(&b.attestation_type)
    );
    check!(
        "standard",
        a.standard
            .as_deref()
            .map(|s| serde_json::Value::String(s.to_owned())),
        b.standard
            .as_deref()
            .map(|s| serde_json::Value::String(s.to_owned()))
    );
    check!(
        "issuer",
        a.issuer
            .as_deref()
            .map(|s| serde_json::Value::String(s.to_owned())),
        b.issuer
            .as_deref()
            .map(|s| serde_json::Value::String(s.to_owned()))
    );
    // Date fields — normalise before comparing.
    check!(
        "valid_from",
        to_value(&a.valid_from).map(|v| normalise_date_value(&v)),
        to_value(&b.valid_from).map(|v| normalise_date_value(&v))
    );
    // valid_to: Option<Option<CalendarDate>> — serialise as nullable JSON.
    {
        let av = match &a.valid_to {
            None => None,
            Some(None) => Some(serde_json::Value::Null),
            Some(Some(d)) => Some(serde_json::Value::String(normalise_date(d))),
        };
        let bv = match &b.valid_to {
            None => None,
            Some(None) => Some(serde_json::Value::Null),
            Some(Some(d)) => Some(serde_json::Value::String(normalise_date(d))),
        };
        check!("valid_to", av, bv);
    }
    check!("outcome", to_value(&a.outcome), to_value(&b.outcome));
    check!(
        "attestation_status",
        to_value(&a.attestation_status),
        to_value(&b.attestation_status)
    );
    check!(
        "reference",
        a.reference
            .as_deref()
            .map(|s| serde_json::Value::String(s.to_owned())),
        b.reference
            .as_deref()
            .map(|s| serde_json::Value::String(s.to_owned()))
    );
    check!(
        "risk_severity",
        to_value(&a.risk_severity),
        to_value(&b.risk_severity)
    );
    check!(
        "risk_likelihood",
        to_value(&a.risk_likelihood),
        to_value(&b.risk_likelihood)
    );
    check!(
        "lot_id",
        a.lot_id
            .as_deref()
            .map(|s| serde_json::Value::String(s.to_owned())),
        b.lot_id
            .as_deref()
            .map(|s| serde_json::Value::String(s.to_owned()))
    );
    // Numeric fields — epsilon comparison via values_equal.
    check!(
        "quantity",
        a.quantity
            .and_then(serde_json::Number::from_f64)
            .map(serde_json::Value::Number),
        b.quantity
            .and_then(serde_json::Number::from_f64)
            .map(serde_json::Value::Number)
    );
    check!(
        "production_date",
        to_value(&a.production_date).map(|v| normalise_date_value(&v)),
        to_value(&b.production_date).map(|v| normalise_date_value(&v))
    );
    check!(
        "origin_country",
        to_value(&a.origin_country),
        to_value(&b.origin_country)
    );
    check!(
        "direct_emissions_co2e",
        a.direct_emissions_co2e
            .and_then(serde_json::Number::from_f64)
            .map(serde_json::Value::Number),
        b.direct_emissions_co2e
            .and_then(serde_json::Number::from_f64)
            .map(serde_json::Value::Number)
    );
    check!(
        "indirect_emissions_co2e",
        a.indirect_emissions_co2e
            .and_then(serde_json::Number::from_f64)
            .map(serde_json::Value::Number),
        b.indirect_emissions_co2e
            .and_then(serde_json::Number::from_f64)
            .map(serde_json::Value::Number)
    );
    check!(
        "emission_factor_source",
        to_value(&a.emission_factor_source),
        to_value(&b.emission_factor_source)
    );
    check!(
        "installation_id",
        a.installation_id
            .as_ref()
            .map(|id| serde_json::Value::String(id.to_string())),
        b.installation_id
            .as_ref()
            .map(|id| serde_json::Value::String(id.to_string()))
    );

    // data_quality nested object.
    if !ignore.contains("data_quality") {
        compare_data_quality(
            "data_quality",
            a.data_quality.as_ref(),
            b.data_quality.as_ref(),
            ignore,
            &mut changes,
        );
    }

    // Extra (unknown) fields — compare by key.
    {
        let mut extra_keys: HashSet<&str> = HashSet::new();
        for k in a.extra.keys() {
            extra_keys.insert(k.as_str());
        }
        for k in b.extra.keys() {
            extra_keys.insert(k.as_str());
        }
        for key in &extra_keys {
            if ignore.contains(*key) {
                continue;
            }
            let av = a.extra.get(*key).cloned();
            let bv = b.extra.get(*key).cloned();
            maybe_change(key, av, bv, &mut changes);
        }
    }

    changes
}

/// Compares the properties of two matched [`EdgeProperties`] values.
///
/// Returns scalar `PropertyChange`s, ignoring fields in `ignore`.
fn compare_edge_props(
    a: &EdgeProperties,
    b: &EdgeProperties,
    ignore: &HashSet<String>,
) -> Vec<PropertyChange> {
    let mut changes: Vec<PropertyChange> = Vec::new();

    macro_rules! check {
        ($field:expr, $a:expr, $b:expr) => {
            if !ignore.contains($field) {
                maybe_change($field, $a, $b, &mut changes);
            }
        };
    }

    // Date fields.
    check!(
        "valid_from",
        to_value(&a.valid_from).map(|v| normalise_date_value(&v)),
        to_value(&b.valid_from).map(|v| normalise_date_value(&v))
    );
    {
        let av = match &a.valid_to {
            None => None,
            Some(None) => Some(serde_json::Value::Null),
            Some(Some(d)) => Some(serde_json::Value::String(normalise_date(d))),
        };
        let bv = match &b.valid_to {
            None => None,
            Some(None) => Some(serde_json::Value::Null),
            Some(Some(d)) => Some(serde_json::Value::String(normalise_date(d))),
        };
        check!("valid_to", av, bv);
    }

    // Numeric fields.
    check!(
        "percentage",
        a.percentage
            .and_then(serde_json::Number::from_f64)
            .map(serde_json::Value::Number),
        b.percentage
            .and_then(serde_json::Number::from_f64)
            .map(serde_json::Value::Number)
    );
    check!(
        "volume",
        a.volume
            .and_then(serde_json::Number::from_f64)
            .map(serde_json::Value::Number),
        b.volume
            .and_then(serde_json::Number::from_f64)
            .map(serde_json::Value::Number)
    );
    check!(
        "annual_value",
        a.annual_value
            .and_then(serde_json::Number::from_f64)
            .map(serde_json::Value::Number),
        b.annual_value
            .and_then(serde_json::Number::from_f64)
            .map(serde_json::Value::Number)
    );
    check!(
        "share_of_buyer_demand",
        a.share_of_buyer_demand
            .and_then(serde_json::Number::from_f64)
            .map(serde_json::Value::Number),
        b.share_of_buyer_demand
            .and_then(serde_json::Number::from_f64)
            .map(serde_json::Value::Number)
    );
    check!(
        "quantity",
        a.quantity
            .and_then(serde_json::Number::from_f64)
            .map(serde_json::Value::Number),
        b.quantity
            .and_then(serde_json::Number::from_f64)
            .map(serde_json::Value::Number)
    );

    // Boolean fields.
    check!(
        "direct",
        a.direct.map(serde_json::Value::Bool),
        b.direct.map(serde_json::Value::Bool)
    );

    // String / enum fields.
    check!(
        "control_type",
        a.control_type.clone(),
        b.control_type.clone()
    );
    check!(
        "consolidation_basis",
        to_value(&a.consolidation_basis),
        to_value(&b.consolidation_basis)
    );
    check!(
        "event_type",
        to_value(&a.event_type),
        to_value(&b.event_type)
    );
    check!(
        "effective_date",
        to_value(&a.effective_date).map(|v| normalise_date_value(&v)),
        to_value(&b.effective_date).map(|v| normalise_date_value(&v))
    );
    check!(
        "description",
        a.description
            .as_deref()
            .map(|s| serde_json::Value::String(s.to_owned())),
        b.description
            .as_deref()
            .map(|s| serde_json::Value::String(s.to_owned()))
    );
    check!(
        "commodity",
        a.commodity
            .as_deref()
            .map(|s| serde_json::Value::String(s.to_owned())),
        b.commodity
            .as_deref()
            .map(|s| serde_json::Value::String(s.to_owned()))
    );
    check!(
        "contract_ref",
        a.contract_ref
            .as_deref()
            .map(|s| serde_json::Value::String(s.to_owned())),
        b.contract_ref
            .as_deref()
            .map(|s| serde_json::Value::String(s.to_owned()))
    );
    check!(
        "volume_unit",
        a.volume_unit
            .as_deref()
            .map(|s| serde_json::Value::String(s.to_owned())),
        b.volume_unit
            .as_deref()
            .map(|s| serde_json::Value::String(s.to_owned()))
    );
    check!(
        "value_currency",
        a.value_currency
            .as_deref()
            .map(|s| serde_json::Value::String(s.to_owned())),
        b.value_currency
            .as_deref()
            .map(|s| serde_json::Value::String(s.to_owned()))
    );
    check!(
        "tier",
        a.tier
            .map(|n| serde_json::Value::Number(serde_json::Number::from(n))),
        b.tier
            .map(|n| serde_json::Value::Number(serde_json::Number::from(n)))
    );
    check!(
        "service_type",
        to_value(&a.service_type),
        to_value(&b.service_type)
    );
    check!(
        "unit",
        a.unit
            .as_deref()
            .map(|s| serde_json::Value::String(s.to_owned())),
        b.unit
            .as_deref()
            .map(|s| serde_json::Value::String(s.to_owned()))
    );
    check!(
        "scope",
        a.scope
            .as_deref()
            .map(|s| serde_json::Value::String(s.to_owned())),
        b.scope
            .as_deref()
            .map(|s| serde_json::Value::String(s.to_owned()))
    );

    // data_quality nested object.
    if !ignore.contains("data_quality") {
        compare_data_quality(
            "data_quality",
            a.data_quality.as_ref(),
            b.data_quality.as_ref(),
            ignore,
            &mut changes,
        );
    }

    // Extra fields.
    {
        let mut extra_keys: HashSet<&str> = HashSet::new();
        for k in a.extra.keys() {
            extra_keys.insert(k.as_str());
        }
        for k in b.extra.keys() {
            extra_keys.insert(k.as_str());
        }
        for key in &extra_keys {
            if ignore.contains(*key) {
                continue;
            }
            let av = a.extra.get(*key).cloned();
            let bv = b.extra.get(*key).cloned();
            maybe_change(key, av, bv, &mut changes);
        }
    }

    changes
}

/// Compares two `identifiers` slices and returns the set diff.
///
/// Identifiers are keyed by their canonical string (scheme:value or
/// scheme:authority:value). Identifiers with the same key in both slices are
/// checked for field-level changes to `valid_from`, `valid_to`, `sensitivity`,
/// `verification_status`, and `verification_date`.
fn compare_identifiers(a_ids: &[Identifier], b_ids: &[Identifier]) -> IdentifierSetDiff {
    // Build canonical-key → identifier maps.
    let mut a_map: HashMap<CanonicalId, &Identifier> = HashMap::new();
    for id in a_ids {
        if id.scheme != "internal" {
            a_map.insert(CanonicalId::from_identifier(id), id);
        }
    }
    let mut b_map: HashMap<CanonicalId, &Identifier> = HashMap::new();
    for id in b_ids {
        if id.scheme != "internal" {
            b_map.insert(CanonicalId::from_identifier(id), id);
        }
    }

    let mut added: Vec<Identifier> = Vec::new();
    let mut removed: Vec<Identifier> = Vec::new();
    let mut modified: Vec<IdentifierFieldDiff> = Vec::new();

    // Identifiers in A but not B → removed.
    for (cid, id) in &a_map {
        if !b_map.contains_key(cid) {
            removed.push((*id).clone());
        }
    }

    // Identifiers in B but not A → added.
    for (cid, id) in &b_map {
        if !a_map.contains_key(cid) {
            added.push((*id).clone());
        }
    }

    // Identifiers in both → compare field-by-field.
    for (cid, id_a) in &a_map {
        let Some(id_b) = b_map.get(cid) else {
            continue;
        };
        let mut field_changes: Vec<PropertyChange> = Vec::new();
        // valid_from
        maybe_change(
            "valid_from",
            to_value(&id_a.valid_from).map(|v| normalise_date_value(&v)),
            to_value(&id_b.valid_from).map(|v| normalise_date_value(&v)),
            &mut field_changes,
        );
        // valid_to
        {
            let av = match &id_a.valid_to {
                None => None,
                Some(None) => Some(serde_json::Value::Null),
                Some(Some(d)) => Some(serde_json::Value::String(normalise_date(d))),
            };
            let bv = match &id_b.valid_to {
                None => None,
                Some(None) => Some(serde_json::Value::Null),
                Some(Some(d)) => Some(serde_json::Value::String(normalise_date(d))),
            };
            maybe_change("valid_to", av, bv, &mut field_changes);
        }
        // sensitivity
        maybe_change(
            "sensitivity",
            to_value(&id_a.sensitivity),
            to_value(&id_b.sensitivity),
            &mut field_changes,
        );
        // verification_status
        maybe_change(
            "verification_status",
            to_value(&id_a.verification_status),
            to_value(&id_b.verification_status),
            &mut field_changes,
        );
        // verification_date
        maybe_change(
            "verification_date",
            to_value(&id_a.verification_date).map(|v| normalise_date_value(&v)),
            to_value(&id_b.verification_date).map(|v| normalise_date_value(&v)),
            &mut field_changes,
        );
        // authority (even though it's part of the canonical key for some schemes,
        // a change to a non-authority-required scheme's authority is a field change).
        let av_auth = id_a
            .authority
            .as_deref()
            .map(|s| serde_json::Value::String(s.to_owned()));
        let bv_auth = id_b
            .authority
            .as_deref()
            .map(|s| serde_json::Value::String(s.to_owned()));
        maybe_change("authority", av_auth, bv_auth, &mut field_changes);

        // Extra fields.
        let mut extra_keys: HashSet<&str> = HashSet::new();
        for k in id_a.extra.keys() {
            extra_keys.insert(k.as_str());
        }
        for k in id_b.extra.keys() {
            extra_keys.insert(k.as_str());
        }
        for key in &extra_keys {
            let av = id_a.extra.get(*key).cloned();
            let bv = id_b.extra.get(*key).cloned();
            maybe_change(key, av, bv, &mut field_changes);
        }

        if !field_changes.is_empty() {
            modified.push(IdentifierFieldDiff {
                canonical_key: cid.clone(),
                field_changes,
            });
        }
    }

    IdentifierSetDiff {
        added,
        removed,
        modified,
    }
}

/// Compares two `labels` slices and returns the set diff.
///
/// Labels are matched by `(key, value)` pair. A change in value for a given
/// key appears as a deletion of the old pair and an addition of the new one
/// (diff.md Section 3.3).
fn compare_labels(a_labels: &[Label], b_labels: &[Label]) -> LabelSetDiff {
    // Build (key, value) sets.
    // We use a normalised representation: (key, Option<value>).
    let a_set: HashSet<(&str, Option<&str>)> = a_labels
        .iter()
        .map(|l| (l.key.as_str(), l.value.as_deref()))
        .collect();
    let b_set: HashSet<(&str, Option<&str>)> = b_labels
        .iter()
        .map(|l| (l.key.as_str(), l.value.as_deref()))
        .collect();

    let mut removed: Vec<Label> = Vec::new();
    for label in a_labels {
        let pair = (label.key.as_str(), label.value.as_deref());
        if !b_set.contains(&pair) {
            removed.push(label.clone());
        }
    }

    let mut added: Vec<Label> = Vec::new();
    for label in b_labels {
        let pair = (label.key.as_str(), label.value.as_deref());
        if !a_set.contains(&pair) {
            added.push(label.clone());
        }
    }

    LabelSetDiff { added, removed }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

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

    // Shared empty ignore set used when no filter is provided.
    let empty_ignore: HashSet<String> = HashSet::new();

    // Emit a version mismatch warning if applicable.
    if a.omtsf_version != b.omtsf_version {
        warnings.push(format!(
            "Version mismatch: A has {}, B has {}",
            a.omtsf_version, b.omtsf_version
        ));
    }

    // --- Node matching ---
    let node_match = match_nodes(&a.nodes, &b.nodes, filter);
    warnings.extend(node_match.warnings);

    // --- Build node diffs ---
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
            node_type: tag_to_string(&node_a.node_type),
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

    // --- Edge matching ---
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
            edge_type: tag_to_string(&edge_a.edge_type),
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    #![allow(clippy::doc_markdown)]
    #![allow(clippy::field_reassign_with_default)]

    use super::*;
    use crate::enums::{EdgeType, EdgeTypeTag, NodeType, NodeTypeTag};
    use crate::newtypes::{CalendarDate, EdgeId, FileSalt, NodeId, SemVer};
    use crate::structures::EdgeProperties;
    use crate::types::Identifier;

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    const SALT_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const SALT_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    fn semver(s: &str) -> SemVer {
        SemVer::try_from(s).expect("semver")
    }

    fn date(s: &str) -> CalendarDate {
        CalendarDate::try_from(s).expect("date")
    }

    fn node_id(s: &str) -> NodeId {
        NodeId::try_from(s).expect("node id")
    }

    fn edge_id(s: &str) -> EdgeId {
        EdgeId::try_from(s).expect("edge id")
    }

    fn file_salt(s: &str) -> FileSalt {
        FileSalt::try_from(s).expect("salt")
    }

    fn make_file(nodes: Vec<Node>, edges: Vec<Edge>) -> OmtsFile {
        OmtsFile {
            omtsf_version: semver("1.0.0"),
            snapshot_date: date("2026-02-20"),
            file_salt: file_salt(SALT_A),
            disclosure_scope: None,
            previous_snapshot_ref: None,
            snapshot_sequence: None,
            reporting_entity: None,
            nodes,
            edges,
            extra: serde_json::Map::new(),
        }
    }

    fn make_file_b(nodes: Vec<Node>, edges: Vec<Edge>) -> OmtsFile {
        OmtsFile {
            file_salt: file_salt(SALT_B),
            ..make_file(nodes, edges)
        }
    }

    fn org_node(id: &str) -> Node {
        Node {
            id: node_id(id),
            node_type: NodeTypeTag::Known(NodeType::Organization),
            identifiers: None,
            data_quality: None,
            labels: None,
            name: Some(id.to_owned()),
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

    fn with_lei(mut node: Node, lei: &str) -> Node {
        let id = Identifier {
            scheme: "lei".to_owned(),
            value: lei.to_owned(),
            authority: None,
            valid_from: None,
            valid_to: None,
            sensitivity: None,
            verification_status: None,
            verification_date: None,
            extra: serde_json::Map::new(),
        };
        node.identifiers = Some(vec![id]);
        node
    }

    fn with_duns(mut node: Node, duns: &str) -> Node {
        let id = Identifier {
            scheme: "duns".to_owned(),
            value: duns.to_owned(),
            authority: None,
            valid_from: None,
            valid_to: None,
            sensitivity: None,
            verification_status: None,
            verification_date: None,
            extra: serde_json::Map::new(),
        };
        let ids = node.identifiers.get_or_insert_with(Vec::new);
        ids.push(id);
        node
    }

    fn supplies_edge(id: &str, src: &str, tgt: &str) -> Edge {
        Edge {
            id: edge_id(id),
            edge_type: EdgeTypeTag::Known(EdgeType::Supplies),
            source: node_id(src),
            target: node_id(tgt),
            identifiers: None,
            properties: EdgeProperties::default(),
            extra: serde_json::Map::new(),
        }
    }

    fn ownership_edge(id: &str, src: &str, tgt: &str) -> Edge {
        Edge {
            id: edge_id(id),
            edge_type: EdgeTypeTag::Known(EdgeType::Ownership),
            source: node_id(src),
            target: node_id(tgt),
            identifiers: None,
            properties: EdgeProperties::default(),
            extra: serde_json::Map::new(),
        }
    }

    // -----------------------------------------------------------------------
    // Node matching tests
    // -----------------------------------------------------------------------

    /// Two empty files produce an empty diff.
    #[test]
    fn diff_two_empty_files() {
        let a = make_file(vec![], vec![]);
        let b = make_file_b(vec![], vec![]);
        let result = diff(&a, &b);
        assert!(result.is_empty());
        assert!(result.warnings.is_empty());
        let summary = result.summary();
        assert_eq!(summary.nodes_added, 0);
        assert_eq!(summary.nodes_removed, 0);
        assert_eq!(summary.nodes_modified, 0);
        assert_eq!(summary.nodes_unchanged, 0);
    }

    /// Nodes in B with no match in A are additions.
    #[test]
    fn diff_all_nodes_added() {
        let a = make_file(vec![], vec![]);
        let b = make_file_b(vec![org_node("org-1"), org_node("org-2")], vec![]);
        let result = diff(&a, &b);
        assert_eq!(result.nodes.added.len(), 2);
        assert!(result.nodes.removed.is_empty());
        assert!(result.nodes.unchanged.is_empty());
        assert!(result.nodes.modified.is_empty());
    }

    /// Nodes in A with no match in B are deletions.
    #[test]
    fn diff_all_nodes_removed() {
        let a = make_file(vec![org_node("org-1"), org_node("org-2")], vec![]);
        let b = make_file_b(vec![], vec![]);
        let result = diff(&a, &b);
        assert_eq!(result.nodes.removed.len(), 2);
        assert!(result.nodes.added.is_empty());
        assert!(result.nodes.unchanged.is_empty());
    }

    /// Nodes without external identifiers are never matched (no match group forms).
    #[test]
    fn diff_nodes_without_identifiers_are_unmatched() {
        // Neither node has identifiers — they cannot match each other.
        let a = make_file(vec![org_node("org-a")], vec![]);
        let b = make_file_b(vec![org_node("org-b")], vec![]);
        let result = diff(&a, &b);
        assert_eq!(result.nodes.removed.len(), 1, "org-a is a deletion");
        assert_eq!(result.nodes.added.len(), 1, "org-b is an addition");
        assert!(result.nodes.unchanged.is_empty());
    }

    /// Nodes that share a LEI are matched.
    #[test]
    fn diff_nodes_matched_by_lei() {
        let node_a = with_lei(org_node("org-a"), "LEI0000000000000001");
        let node_b = with_lei(org_node("org-b"), "LEI0000000000000001");
        let a = make_file(vec![node_a], vec![]);
        let b = make_file_b(vec![node_b], vec![]);
        let result = diff(&a, &b);
        assert!(result.nodes.removed.is_empty(), "no deletions expected");
        assert!(result.nodes.added.is_empty(), "no additions expected");
        // The nodes differ in name ("org-a" vs "org-b"), so the pair is modified.
        let total_matched = result.nodes.unchanged.len() + result.nodes.modified.len();
        assert_eq!(total_matched, 1, "one matched pair");
        // Grab the diff from whichever bucket it landed in.
        let nd = if result.nodes.modified.is_empty() {
            &result.nodes.unchanged[0]
        } else {
            &result.nodes.modified[0]
        };
        assert_eq!(nd.id_a, "org-a");
        assert_eq!(nd.id_b, "org-b");
        assert!(
            nd.matched_by
                .iter()
                .any(|k| k.contains("LEI0000000000000001"))
        );
    }

    /// Nodes matched via transitive closure (A1↔B1 via LEI, A1↔B2 via DUNS).
    #[test]
    fn diff_node_transitive_match() {
        // org-a carries both LEI and DUNS.
        // org-b1 in B carries only the LEI.
        // org-b2 in B carries only the DUNS.
        // Result: org-a matches both org-b1 and org-b2 (one group of 3 = ambiguous).
        let node_a = with_duns(with_lei(org_node("org-a"), "LEI_TRANS"), "DUNS_TRANS");
        let node_b1 = with_lei(org_node("org-b1"), "LEI_TRANS");
        let node_b2 = with_duns(org_node("org-b2"), "DUNS_TRANS");
        let a = make_file(vec![node_a], vec![]);
        let b = make_file_b(vec![node_b1, node_b2], vec![]);
        let result = diff(&a, &b);
        // Two B nodes in the group → ambiguity warning.
        assert!(
            !result.warnings.is_empty(),
            "expected ambiguity warning for 1 A node matching 2 B nodes"
        );
        // Both pairs should be reported as matched (modified because names differ).
        let total_matched = result.nodes.unchanged.len() + result.nodes.modified.len();
        assert_eq!(total_matched, 2);
        assert!(result.nodes.added.is_empty());
        assert!(result.nodes.removed.is_empty());
    }

    /// Ambiguity: two nodes in A match the same node in B.
    #[test]
    fn diff_ambiguous_match_two_a_nodes_same_b() {
        let node_a1 = with_lei(org_node("org-a1"), "LEI_SHARED");
        let node_a2 = with_lei(org_node("org-a2"), "LEI_SHARED");
        let node_b = with_lei(org_node("org-b"), "LEI_SHARED");
        let a = make_file(vec![node_a1, node_a2], vec![]);
        let b = make_file_b(vec![node_b], vec![]);
        let result = diff(&a, &b);
        // A has 2 nodes in the same group → warning.
        assert!(!result.warnings.is_empty(), "expected ambiguity warning");
        // Both A nodes are reported as matched to the one B node (names differ → modified).
        let total_matched = result.nodes.unchanged.len() + result.nodes.modified.len();
        assert_eq!(total_matched, 2);
        assert!(result.nodes.removed.is_empty());
        assert!(result.nodes.added.is_empty());
    }

    /// Nodes with only `internal` scheme identifiers are never matched.
    #[test]
    fn diff_internal_identifiers_do_not_cause_match() {
        let mut node_a = org_node("org-a");
        node_a.identifiers = Some(vec![Identifier {
            scheme: "internal".to_owned(),
            value: "sap:001".to_owned(),
            authority: None,
            valid_from: None,
            valid_to: None,
            sensitivity: None,
            verification_status: None,
            verification_date: None,
            extra: serde_json::Map::new(),
        }]);
        let mut node_b = org_node("org-b");
        node_b.identifiers = Some(vec![Identifier {
            scheme: "internal".to_owned(),
            value: "sap:001".to_owned(),
            authority: None,
            valid_from: None,
            valid_to: None,
            sensitivity: None,
            verification_status: None,
            verification_date: None,
            extra: serde_json::Map::new(),
        }]);
        let a = make_file(vec![node_a], vec![]);
        let b = make_file_b(vec![node_b], vec![]);
        let result = diff(&a, &b);
        // Internal identifiers must not cause a match.
        assert_eq!(result.nodes.removed.len(), 1);
        assert_eq!(result.nodes.added.len(), 1);
        assert!(result.nodes.unchanged.is_empty());
    }

    // -----------------------------------------------------------------------
    // Edge matching tests
    // -----------------------------------------------------------------------

    /// Edges are matched when both endpoints match and type is the same.
    #[test]
    fn diff_edges_matched_exact() {
        let node_a1 = with_lei(org_node("org-a1"), "LEI_0001");
        let node_a2 = with_lei(org_node("org-a2"), "LEI_0002");
        let node_b1 = with_lei(org_node("org-b1"), "LEI_0001");
        let node_b2 = with_lei(org_node("org-b2"), "LEI_0002");

        let edge_a = supplies_edge("e-a", "org-a1", "org-a2");
        let edge_b = supplies_edge("e-b", "org-b1", "org-b2");

        let a = make_file(vec![node_a1, node_a2], vec![edge_a]);
        let b = make_file_b(vec![node_b1, node_b2], vec![edge_b]);

        let result = diff(&a, &b);
        assert!(result.edges.added.is_empty(), "no additions");
        assert!(result.edges.removed.is_empty(), "no deletions");
        assert_eq!(result.edges.unchanged.len(), 1, "one matched edge pair");
        assert_eq!(result.edges.unchanged[0].id_a, "e-a");
        assert_eq!(result.edges.unchanged[0].id_b, "e-b");
    }

    /// Edges in A with no match in B are deletions.
    #[test]
    fn diff_edge_deletion() {
        let node_a1 = with_lei(org_node("org-a1"), "LEI_0001");
        let node_a2 = with_lei(org_node("org-a2"), "LEI_0002");
        let node_b1 = with_lei(org_node("org-b1"), "LEI_0001");
        let node_b2 = with_lei(org_node("org-b2"), "LEI_0002");

        let edge_a = supplies_edge("e-a", "org-a1", "org-a2");
        // B has no edges.

        let a = make_file(vec![node_a1, node_a2], vec![edge_a]);
        let b = make_file_b(vec![node_b1, node_b2], vec![]);

        let result = diff(&a, &b);
        assert_eq!(result.edges.removed.len(), 1, "e-a is a deletion");
        assert!(result.edges.added.is_empty());
        assert!(result.edges.unchanged.is_empty());
    }

    /// Edges in B with no match in A are additions.
    #[test]
    fn diff_edge_addition() {
        let node_a1 = with_lei(org_node("org-a1"), "LEI_0001");
        let node_a2 = with_lei(org_node("org-a2"), "LEI_0002");
        let node_b1 = with_lei(org_node("org-b1"), "LEI_0001");
        let node_b2 = with_lei(org_node("org-b2"), "LEI_0002");

        let edge_b = supplies_edge("e-b", "org-b1", "org-b2");
        // A has no edges.

        let a = make_file(vec![node_a1, node_a2], vec![]);
        let b = make_file_b(vec![node_b1, node_b2], vec![edge_b]);

        let result = diff(&a, &b);
        assert_eq!(result.edges.added.len(), 1, "e-b is an addition");
        assert!(result.edges.removed.is_empty());
        assert!(result.edges.unchanged.is_empty());
    }

    /// Edges with different types are not matched.
    #[test]
    fn diff_edges_different_type_not_matched() {
        let node_a1 = with_lei(org_node("org-a1"), "LEI_0001");
        let node_a2 = with_lei(org_node("org-a2"), "LEI_0002");
        let node_b1 = with_lei(org_node("org-b1"), "LEI_0001");
        let node_b2 = with_lei(org_node("org-b2"), "LEI_0002");

        let edge_a = supplies_edge("e-a", "org-a1", "org-a2");
        let edge_b = ownership_edge("e-b", "org-b1", "org-b2");

        let a = make_file(vec![node_a1, node_a2], vec![edge_a]);
        let b = make_file_b(vec![node_b1, node_b2], vec![edge_b]);

        let result = diff(&a, &b);
        assert_eq!(
            result.edges.removed.len(),
            1,
            "e-a is a deletion (type mismatch)"
        );
        assert_eq!(
            result.edges.added.len(),
            1,
            "e-b is an addition (type mismatch)"
        );
        assert!(result.edges.unchanged.is_empty());
    }

    /// Edges whose nodes are unmatched are reported as additions/deletions.
    #[test]
    fn diff_edges_with_unmatched_nodes() {
        // Node in A has no counterpart in B. The edge is therefore a deletion.
        let node_a1 = org_node("org-a1"); // no identifiers → no match
        let node_a2 = org_node("org-a2");
        let node_b1 = org_node("org-b1");
        let node_b2 = org_node("org-b2");

        let edge_a = supplies_edge("e-a", "org-a1", "org-a2");
        let edge_b = supplies_edge("e-b", "org-b1", "org-b2");

        let a = make_file(vec![node_a1, node_a2], vec![edge_a]);
        let b = make_file_b(vec![node_b1, node_b2], vec![edge_b]);

        let result = diff(&a, &b);
        // Nodes don't match, so edges can't match.
        assert_eq!(result.edges.removed.len(), 1);
        assert_eq!(result.edges.added.len(), 1);
        assert!(result.edges.unchanged.is_empty());
    }

    /// same_as edges are never matched; they appear as deletions/additions.
    #[test]
    fn diff_same_as_edges_not_matched() {
        let node_a1 = with_lei(org_node("org-a1"), "LEI_X");
        let node_a2 = with_lei(org_node("org-a2"), "LEI_Y");
        let node_b1 = with_lei(org_node("org-b1"), "LEI_X");
        let node_b2 = with_lei(org_node("org-b2"), "LEI_Y");

        let same_as_a = Edge {
            id: edge_id("same-a"),
            edge_type: EdgeTypeTag::Known(EdgeType::SameAs),
            source: node_id("org-a1"),
            target: node_id("org-a2"),
            identifiers: None,
            properties: EdgeProperties::default(),
            extra: serde_json::Map::new(),
        };
        let same_as_b = Edge {
            id: edge_id("same-b"),
            edge_type: EdgeTypeTag::Known(EdgeType::SameAs),
            source: node_id("org-b1"),
            target: node_id("org-b2"),
            identifiers: None,
            properties: EdgeProperties::default(),
            extra: serde_json::Map::new(),
        };

        let a = make_file(vec![node_a1, node_a2], vec![same_as_a]);
        let b = make_file_b(vec![node_b1, node_b2], vec![same_as_b]);

        let result = diff(&a, &b);
        // same_as edges are never matched — both appear as deletion and addition.
        assert_eq!(result.edges.removed.len(), 1, "same_as in A is a deletion");
        assert_eq!(result.edges.added.len(), 1, "same_as in B is an addition");
        assert!(result.edges.unchanged.is_empty());
    }

    /// DiffSummary reflects counts correctly.
    #[test]
    fn diff_summary_counts() {
        let node_a = with_lei(org_node("org-a"), "LEI_AA");
        let node_b_matched = with_lei(org_node("org-b-match"), "LEI_AA");
        let node_b_added = org_node("org-b-new");

        let a = make_file(vec![node_a], vec![]);
        let b = make_file_b(vec![node_b_matched, node_b_added], vec![]);

        let result = diff(&a, &b);
        let summary = result.summary();
        assert_eq!(summary.nodes_added, 1, "org-b-new is added");
        assert_eq!(summary.nodes_removed, 0);
        // Nodes are matched; names differ ("org-a" vs "org-b-match"), so pair is modified.
        assert_eq!(
            summary.nodes_modified + summary.nodes_unchanged,
            1,
            "one matched pair (modified or unchanged)"
        );
        assert_eq!(summary.edges_added, 0);
        assert_eq!(summary.edges_removed, 0);
    }

    /// is_empty returns true only when there are no changes at all.
    #[test]
    fn diff_is_empty_with_identical_files() {
        let node = with_lei(org_node("org-a"), "LEI_EQ");
        let a = make_file(vec![node.clone()], vec![]);
        let mut b = make_file_b(vec![node], vec![]);
        // B node has same LEI, so it matches. Both are unchanged.
        b.nodes[0].id = node_id("org-b");
        b.nodes[0].identifiers = Some(vec![Identifier {
            scheme: "lei".to_owned(),
            value: "LEI_EQ".to_owned(),
            authority: None,
            valid_from: None,
            valid_to: None,
            sensitivity: None,
            verification_status: None,
            verification_date: None,
            extra: serde_json::Map::new(),
        }]);
        let result = diff(&a, &b);
        // Only matched (unchanged) nodes — is_empty checks additions/removals/modified only.
        assert!(result.is_empty(), "matched-only result should be empty");
    }

    /// Version mismatch emits a warning but proceeds.
    #[test]
    fn diff_version_mismatch_warning() {
        let mut a = make_file(vec![], vec![]);
        a.omtsf_version = SemVer::try_from("1.0.0").expect("semver");
        let mut b = make_file_b(vec![], vec![]);
        b.omtsf_version = SemVer::try_from("1.1.0").expect("semver");
        let result = diff(&a, &b);
        assert!(
            result
                .warnings
                .iter()
                .any(|w| w.contains("Version mismatch")),
            "expected version mismatch warning; got: {:?}",
            result.warnings
        );
    }

    /// diff_filtered with node_type filter excludes other types.
    #[test]
    fn diff_filtered_by_node_type() {
        use crate::enums::NodeType;
        let org = with_lei(org_node("org-a"), "LEI_ORG");
        let mut fac_a = org_node("fac-a");
        fac_a.node_type = NodeTypeTag::Known(NodeType::Facility);

        let org_b = with_lei(org_node("org-b"), "LEI_ORG");
        let mut fac_b = org_node("fac-b");
        fac_b.node_type = NodeTypeTag::Known(NodeType::Facility);

        let a = make_file(vec![org, fac_a], vec![]);
        let b = make_file_b(vec![org_b, fac_b], vec![]);

        let mut filter = DiffFilter::default();
        filter.node_types = Some(HashSet::from(["organization".to_owned()]));

        let result = diff_filtered(&a, &b, Some(&filter));
        // Only organization nodes are diffed; facility nodes are excluded.
        // org-a and org-b match via LEI_ORG; names differ so pair is modified.
        assert!(result.nodes.added.is_empty());
        assert!(result.nodes.removed.is_empty());
        let total_matched = result.nodes.unchanged.len() + result.nodes.modified.len();
        assert_eq!(total_matched, 1);
    }

    /// Supplies edges matched by identity properties (no external identifiers).
    #[test]
    fn diff_edges_matched_by_identity_properties() {
        let node_a1 = with_lei(org_node("org-a1"), "LEI_P");
        let node_a2 = with_lei(org_node("org-a2"), "LEI_Q");
        let node_b1 = with_lei(org_node("org-b1"), "LEI_P");
        let node_b2 = with_lei(org_node("org-b2"), "LEI_Q");

        // Both edges: supplies with commodity "steel", no external identifier.
        let mut props = EdgeProperties::default();
        props.commodity = Some("steel".to_owned());

        let mut edge_a = supplies_edge("e-a", "org-a1", "org-a2");
        edge_a.properties = props.clone();
        let mut edge_b = supplies_edge("e-b", "org-b1", "org-b2");
        edge_b.properties = props;

        let a = make_file(vec![node_a1, node_a2], vec![edge_a]);
        let b = make_file_b(vec![node_b1, node_b2], vec![edge_b]);

        let result = diff(&a, &b);
        assert!(result.edges.added.is_empty());
        assert!(result.edges.removed.is_empty());
        assert_eq!(result.edges.unchanged.len(), 1);
    }

    /// Two edges with different identity properties are not matched.
    #[test]
    fn diff_edges_not_matched_different_identity_properties() {
        let node_a1 = with_lei(org_node("org-a1"), "LEI_P");
        let node_a2 = with_lei(org_node("org-a2"), "LEI_Q");
        let node_b1 = with_lei(org_node("org-b1"), "LEI_P");
        let node_b2 = with_lei(org_node("org-b2"), "LEI_Q");

        let mut props_a = EdgeProperties::default();
        props_a.commodity = Some("steel".to_owned());

        let mut props_b = EdgeProperties::default();
        props_b.commodity = Some("aluminum".to_owned());

        let mut edge_a = supplies_edge("e-a", "org-a1", "org-a2");
        edge_a.properties = props_a;
        let mut edge_b = supplies_edge("e-b", "org-b1", "org-b2");
        edge_b.properties = props_b;

        let a = make_file(vec![node_a1, node_a2], vec![edge_a]);
        let b = make_file_b(vec![node_b1, node_b2], vec![edge_b]);

        let result = diff(&a, &b);
        assert_eq!(result.edges.removed.len(), 1);
        assert_eq!(result.edges.added.len(), 1);
        assert!(result.edges.unchanged.is_empty());
    }

    // -----------------------------------------------------------------------
    // T-030 Property comparison tests
    // -----------------------------------------------------------------------

    /// Helper: two matched org nodes with same LEI and same name → identical pair.
    fn make_identical_pair(id: &str, lei: &str, name: &str) -> (Node, Node) {
        let mut node_a = org_node(id);
        node_a.name = Some(name.to_owned());
        let node_a = with_lei(node_a, lei);
        let node_b = node_a.clone();
        (node_a, node_b)
    }

    /// Two files with identical content produce an empty diff.
    #[test]
    fn diff_identical_files_empty_diff() {
        let (node_a, node_b) = make_identical_pair("org-x", "LEI_IDENTICAL", "Acme Corp");

        let edge_a = {
            let mut e = supplies_edge("e-1", "org-x", "org-x");
            e.properties.commodity = Some("steel".to_owned());
            e
        };
        let edge_b = edge_a.clone();

        let a = make_file(vec![node_a], vec![edge_a]);
        let b = make_file_b(vec![node_b], vec![edge_b]);
        let result = diff(&a, &b);
        assert!(
            result.is_empty(),
            "identical files should produce empty diff"
        );
        assert_eq!(result.nodes.unchanged.len(), 1);
        assert_eq!(result.edges.unchanged.len(), 1);
        let summary = result.summary();
        assert_eq!(summary.nodes_modified, 0);
        assert_eq!(summary.edges_modified, 0);
    }

    /// A scalar property change (name) is detected as modified.
    #[test]
    fn diff_node_name_change_is_modified() {
        let mut node_a = org_node("org-nm");
        node_a.name = Some("Old Name".to_owned());
        let node_a = with_lei(node_a, "LEI_NM");

        let mut node_b = org_node("org-nm");
        node_b.name = Some("New Name".to_owned());
        let node_b = with_lei(node_b, "LEI_NM");

        let a = make_file(vec![node_a], vec![]);
        let b = make_file_b(vec![node_b], vec![]);
        let result = diff(&a, &b);

        assert_eq!(result.nodes.modified.len(), 1, "name change → modified");
        assert!(result.nodes.unchanged.is_empty());
        let nd = &result.nodes.modified[0];
        assert!(!nd.property_changes.is_empty());
        let name_change = nd.property_changes.iter().find(|c| c.field == "name");
        assert!(name_change.is_some(), "should have a 'name' change");
        let nc = name_change.expect("name change exists");
        assert_eq!(
            nc.old_value,
            Some(serde_json::Value::String("Old Name".to_owned()))
        );
        assert_eq!(
            nc.new_value,
            Some(serde_json::Value::String("New Name".to_owned()))
        );
    }

    /// Numeric comparison uses epsilon via node fields: quantity 1000.0 vs
    /// 1000.0 + 1e-10 should be equal (within epsilon).
    #[test]
    fn diff_numeric_epsilon_comparison() {
        let (mut node_a, mut node_b) = make_identical_pair("org-qty", "LEI_QTY", "QtyOrg");
        node_a.quantity = Some(1000.0_f64);
        // Within epsilon — should not be detected as a change.
        node_b.quantity = Some(1000.0_f64 + 1e-10_f64);

        let a = make_file(vec![node_a], vec![]);
        let b = make_file_b(vec![node_b], vec![]);
        let result = diff(&a, &b);

        // Node should be unchanged — quantity diff is below epsilon.
        assert_eq!(
            result.nodes.unchanged.len(),
            1,
            "within epsilon → unchanged"
        );
        assert!(result.nodes.modified.is_empty());
    }

    /// Quantity 1000.0 vs 2000.0 is outside epsilon and produces a property change.
    #[test]
    fn diff_numeric_change_detected() {
        let (mut node_a, mut node_b) = make_identical_pair("org-qty2", "LEI_QTY2", "QtyOrg2");
        node_a.quantity = Some(1000.0_f64);
        node_b.quantity = Some(2000.0_f64);

        let a = make_file(vec![node_a], vec![]);
        let b = make_file_b(vec![node_b], vec![]);
        let result = diff(&a, &b);

        assert_eq!(result.nodes.modified.len(), 1, "quantity change → modified");
        let nd = &result.nodes.modified[0];
        let qty_change = nd.property_changes.iter().find(|c| c.field == "quantity");
        assert!(qty_change.is_some(), "should have a 'quantity' change");
    }

    /// Date normalisation: "2026-2-9" and "2026-02-09" are treated as equal.
    #[test]
    fn diff_date_normalisation_no_false_positive() {
        let (mut node_a, node_b) = make_identical_pair("org-dt", "LEI_DT", "DateOrg");
        // Set valid_from with a non-padded date variant in the CalendarDate
        // (CalendarDate enforces YYYY-MM-DD format, so we test via the extra field
        // which accepts raw JSON values).
        node_a.extra.insert(
            "x_test_date".to_owned(),
            serde_json::Value::String("2026-2-9".to_owned()),
        );
        let mut node_b_mut = node_b;
        node_b_mut.extra.insert(
            "x_test_date".to_owned(),
            serde_json::Value::String("2026-02-09".to_owned()),
        );

        let a = make_file(vec![node_a], vec![]);
        let b = make_file_b(vec![node_b_mut], vec![]);
        let result = diff(&a, &b);

        // x_test_date should be treated as equal after normalisation.
        let any_modified = result
            .nodes
            .modified
            .iter()
            .any(|nd| nd.property_changes.iter().any(|c| c.field == "x_test_date"));
        assert!(
            !any_modified,
            "normalised dates should not produce a change; got: {:?}",
            result
                .nodes
                .modified
                .iter()
                .flat_map(|nd| nd.property_changes.iter())
                .collect::<Vec<_>>()
        );
    }

    /// Adding an identifier to a node is detected.
    #[test]
    fn diff_identifier_added() {
        let (node_a, mut node_b) = make_identical_pair("org-id", "LEI_ID", "IdOrg");
        // Add a DUNS to B.
        let duns = Identifier {
            scheme: "duns".to_owned(),
            value: "123456789".to_owned(),
            authority: None,
            valid_from: None,
            valid_to: None,
            sensitivity: None,
            verification_status: None,
            verification_date: None,
            extra: serde_json::Map::new(),
        };
        node_b.identifiers.get_or_insert_with(Vec::new).push(duns);

        let a = make_file(vec![node_a], vec![]);
        let b = make_file_b(vec![node_b], vec![]);
        let result = diff(&a, &b);

        assert_eq!(
            result.nodes.modified.len(),
            1,
            "identifier added → modified"
        );
        let nd = &result.nodes.modified[0];
        assert_eq!(nd.identifier_changes.added.len(), 1);
        assert!(nd.identifier_changes.removed.is_empty());
        assert_eq!(nd.identifier_changes.added[0].scheme, "duns");
    }

    /// Removing an identifier from a node is detected.
    #[test]
    fn diff_identifier_removed() {
        let (mut node_a, node_b) = make_identical_pair("org-idr", "LEI_IDR", "IdROrg");
        // Add a DUNS only to A.
        let duns = Identifier {
            scheme: "duns".to_owned(),
            value: "987654321".to_owned(),
            authority: None,
            valid_from: None,
            valid_to: None,
            sensitivity: None,
            verification_status: None,
            verification_date: None,
            extra: serde_json::Map::new(),
        };
        node_a.identifiers.get_or_insert_with(Vec::new).push(duns);

        let a = make_file(vec![node_a], vec![]);
        let b = make_file_b(vec![node_b], vec![]);
        let result = diff(&a, &b);

        assert_eq!(
            result.nodes.modified.len(),
            1,
            "identifier removed → modified"
        );
        let nd = &result.nodes.modified[0];
        assert!(nd.identifier_changes.added.is_empty());
        assert_eq!(nd.identifier_changes.removed.len(), 1);
        assert_eq!(nd.identifier_changes.removed[0].scheme, "duns");
    }

    /// Adding a label to a node is detected.
    #[test]
    fn diff_label_added() {
        use crate::types::Label;
        let (node_a, mut node_b) = make_identical_pair("org-lb", "LEI_LB", "LabelOrg");
        node_b.labels = Some(vec![Label {
            key: "tier".to_owned(),
            value: Some("1".to_owned()),
            extra: serde_json::Map::new(),
        }]);

        let a = make_file(vec![node_a], vec![]);
        let b = make_file_b(vec![node_b], vec![]);
        let result = diff(&a, &b);

        assert_eq!(result.nodes.modified.len(), 1, "label added → modified");
        let nd = &result.nodes.modified[0];
        assert_eq!(nd.label_changes.added.len(), 1);
        assert!(nd.label_changes.removed.is_empty());
        assert_eq!(nd.label_changes.added[0].key, "tier");
    }

    /// Removing a label from a node is detected.
    #[test]
    fn diff_label_removed() {
        use crate::types::Label;
        let (mut node_a, node_b) = make_identical_pair("org-lbr", "LEI_LBR", "LabelRmOrg");
        node_a.labels = Some(vec![Label {
            key: "risk-tier".to_owned(),
            value: Some("high".to_owned()),
            extra: serde_json::Map::new(),
        }]);

        let a = make_file(vec![node_a], vec![]);
        let b = make_file_b(vec![node_b], vec![]);
        let result = diff(&a, &b);

        assert_eq!(result.nodes.modified.len(), 1, "label removed → modified");
        let nd = &result.nodes.modified[0];
        assert!(nd.label_changes.added.is_empty());
        assert_eq!(nd.label_changes.removed.len(), 1);
        assert_eq!(nd.label_changes.removed[0].key, "risk-tier");
    }

    /// A label value change appears as a removal of the old and addition of the new.
    #[test]
    fn diff_label_value_change_is_remove_plus_add() {
        use crate::types::Label;
        let (mut node_a, mut node_b) = make_identical_pair("org-lbv", "LEI_LBV", "LabelValOrg");
        node_a.labels = Some(vec![Label {
            key: "risk-tier".to_owned(),
            value: Some("low".to_owned()),
            extra: serde_json::Map::new(),
        }]);
        node_b.labels = Some(vec![Label {
            key: "risk-tier".to_owned(),
            value: Some("medium".to_owned()),
            extra: serde_json::Map::new(),
        }]);

        let a = make_file(vec![node_a], vec![]);
        let b = make_file_b(vec![node_b], vec![]);
        let result = diff(&a, &b);

        assert_eq!(
            result.nodes.modified.len(),
            1,
            "label value change → modified"
        );
        let nd = &result.nodes.modified[0];
        assert_eq!(nd.label_changes.added.len(), 1, "new value added");
        assert_eq!(nd.label_changes.removed.len(), 1, "old value removed");
        assert_eq!(nd.label_changes.added[0].value.as_deref(), Some("medium"));
        assert_eq!(nd.label_changes.removed[0].value.as_deref(), Some("low"));
    }

    /// DiffFilter ignore_fields excludes specified fields from comparison.
    #[test]
    fn diff_filter_ignore_fields() {
        let (mut node_a, node_b) = make_identical_pair("org-ign", "LEI_IGN", "IgnOrg");
        // Change address in A only.
        node_a.address = Some("Old Address".to_owned());

        let a = make_file(vec![node_a], vec![]);
        let b = make_file_b(vec![node_b], vec![]);

        // Without ignore: should be modified (address differs).
        let result_all = diff(&a, &b);
        assert_eq!(
            result_all.nodes.modified.len(),
            1,
            "without ignore: address change detected"
        );

        // With ignore on "address": should be unchanged.
        let mut filter = DiffFilter::default();
        filter.ignore_fields.insert("address".to_owned());
        let result_filtered = diff_filtered(&a, &b, Some(&filter));
        assert_eq!(
            result_filtered.nodes.unchanged.len(),
            1,
            "with address ignored: should be unchanged"
        );
        assert!(result_filtered.nodes.modified.is_empty());
    }

    /// DiffSummary.is_empty is false when there are modifications.
    #[test]
    fn diff_is_empty_false_when_modified() {
        let (mut node_a, node_b) = make_identical_pair("org-mod", "LEI_MOD", "ModOrg");
        node_a.address = Some("Different".to_owned());

        let a = make_file(vec![node_a], vec![]);
        let b = make_file_b(vec![node_b], vec![]);
        let result = diff(&a, &b);
        assert!(
            !result.is_empty(),
            "diff with modifications should not be empty"
        );
        let summary = result.summary();
        assert_eq!(summary.nodes_modified, 1);
    }

    /// Edge property change (volume) is detected.
    #[test]
    fn diff_edge_property_change() {
        let na = with_lei(org_node("org-ep-a"), "LEI_EPA");
        let nb = na.clone();
        let nc = with_lei(org_node("org-ep-b"), "LEI_EPB");
        let nd = nc.clone();

        let mut edge_a = supplies_edge("e-vol", "org-ep-a", "org-ep-b");
        edge_a.properties.commodity = Some("coal".to_owned());
        edge_a.properties.volume = Some(1000.0_f64);

        let mut edge_b = edge_a.clone();
        edge_b.properties.volume = Some(1500.0_f64);

        let a = make_file(vec![na, nc], vec![edge_a]);
        let b = make_file_b(vec![nb, nd], vec![edge_b]);
        let result = diff(&a, &b);

        assert_eq!(
            result.edges.modified.len(),
            1,
            "volume change → modified edge"
        );
        let ed = &result.edges.modified[0];
        let vol_change = ed.property_changes.iter().find(|c| c.field == "volume");
        assert!(vol_change.is_some(), "should find a 'volume' change");
    }

    /// DiffFilter with edge_type restricts edge diffing.
    #[test]
    fn diff_filter_edge_type() {
        let na = with_lei(org_node("org-fet-a"), "LEI_FET_A");
        let nb = na.clone();
        let nc = with_lei(org_node("org-fet-b"), "LEI_FET_B");
        let nd = nc.clone();

        // One supplies edge (identity prop: commodity) and one ownership edge.
        let mut sup = supplies_edge("e-sup", "org-fet-a", "org-fet-b");
        sup.properties.commodity = Some("iron".to_owned());
        let own = ownership_edge("e-own", "org-fet-a", "org-fet-b");

        let a = make_file(vec![na, nc], vec![sup.clone(), own.clone()]);
        let b = make_file_b(vec![nb, nd], vec![sup, own]);

        let mut filter = DiffFilter::default();
        filter.edge_types = Some(HashSet::from(["supplies".to_owned()]));

        let result = diff_filtered(&a, &b, Some(&filter));
        // Only supplies edge should be diffed; ownership excluded.
        let total_edges = result.edges.unchanged.len()
            + result.edges.modified.len()
            + result.edges.added.len()
            + result.edges.removed.len();
        assert_eq!(total_edges, 1, "only one edge type considered");
    }
}
