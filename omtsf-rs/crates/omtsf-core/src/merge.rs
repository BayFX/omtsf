/// Property merge, conflict recording, and `same_as` handling for the merge engine.
///
/// This module implements the per-property merge strategy described in merge.md
/// Sections 4.1–4.3 and the `same_as` edge processing described in Sections
/// 7.1–7.3.
///
/// # Responsibilities
///
/// - [`merge_scalars`] — compare N optional scalar values; produce the winner or
///   a conflict record.
/// - [`merge_identifiers`] — set-union of [`Identifier`] arrays, deduplicated by
///   canonical string and sorted.
/// - [`merge_labels`] — set-union of [`Label`] arrays, sorted by `(key, value)`.
/// - [`Conflict`] / [`ConflictEntry`] — deterministic conflict representation.
/// - [`MergeMetadata`] — provenance record written into the merged file header.
/// - [`SameAsThreshold`] — configurable confidence gate for `same_as` edges.
/// - [`apply_same_as_edges`] — feeds qualifying `same_as` edges into a
///   [`UnionFind`] structure after the identifier-based pass.
use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::canonical::CanonicalId;
use crate::enums::{EdgeType, EdgeTypeTag};
use crate::structures::Edge;
use crate::types::{Identifier, Label};
use crate::union_find::UnionFind;

// ---------------------------------------------------------------------------
// SameAsThreshold
// ---------------------------------------------------------------------------

/// Configures which `same_as` edges are honoured during union-find processing.
///
/// The spec defines three confidence levels for `same_as` edges (merge.md
/// Section 7.1). The threshold controls the minimum level that triggers a
/// `union` call on the underlying [`UnionFind`] structure.
///
/// ```text
/// Definite  → only "definite" edges are honoured  (most conservative)
/// Probable  → "definite" and "probable" edges are honoured
/// Possible  → all three levels are honoured        (most permissive)
/// ```
///
/// The default is [`SameAsThreshold::Definite`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SameAsThreshold {
    /// Honour only `same_as` edges with `confidence: "definite"` (default).
    Definite,
    /// Honour `same_as` edges with `confidence: "definite"` or `"probable"`.
    Probable,
    /// Honour all `same_as` edges regardless of confidence level.
    Possible,
}

impl Default for SameAsThreshold {
    fn default() -> Self {
        Self::Definite
    }
}

impl SameAsThreshold {
    /// Returns `true` when a `same_as` edge carrying the given `confidence`
    /// string should be honoured under this threshold.
    ///
    /// Unrecognised confidence strings are treated as `"possible"` (the weakest
    /// level), meaning they are honoured only when the threshold is
    /// [`SameAsThreshold::Possible`].
    ///
    /// # Parameters
    ///
    /// - `confidence`: the value of the `confidence` property on the `same_as`
    ///   edge (e.g. `"definite"`, `"probable"`, `"possible"`).  `None` (field
    ///   absent) is treated as `"possible"`.
    pub fn honours(&self, confidence: Option<&str>) -> bool {
        let level = SameAsLevel::from_str(confidence.unwrap_or("possible"));
        match self {
            SameAsThreshold::Definite => matches!(level, SameAsLevel::Definite),
            SameAsThreshold::Probable => {
                matches!(level, SameAsLevel::Definite | SameAsLevel::Probable)
            }
            SameAsThreshold::Possible => true,
        }
    }
}

/// Internal helper for the three `same_as` confidence levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SameAsLevel {
    Definite,
    Probable,
    Possible,
}

impl SameAsLevel {
    fn from_str(s: &str) -> Self {
        match s {
            "definite" => Self::Definite,
            "probable" => Self::Probable,
            _ => Self::Possible,
        }
    }
}

// ---------------------------------------------------------------------------
// ConflictEntry
// ---------------------------------------------------------------------------

/// A single conflicting value observed in a merge group, with its provenance.
///
/// Conflict entries are sorted by `(source_file, json_value)` to guarantee
/// deterministic output (merge.md Section 4.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConflictEntry {
    /// JSON-serialized form of the conflicting value.
    pub value: serde_json::Value,
    /// The source file that contributed this value.
    pub source_file: String,
}

// ---------------------------------------------------------------------------
// Conflict
// ---------------------------------------------------------------------------

/// A recorded conflict on a single property within a merge group.
///
/// When two or more source nodes/edges disagree on a scalar property, the
/// property is omitted from the merged output and a `Conflict` is appended to
/// the `_conflicts` array (merge.md Section 4.1).
///
/// Entries within a `Conflict` are sorted by `(source_file, json_value)`;
/// multiple `Conflict` records in a `_conflicts` array are sorted by `field`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Conflict {
    /// Name of the property that conflicted (e.g. `"name"`, `"status"`).
    pub field: String,
    /// All distinct values seen for this property, with provenance.
    pub values: Vec<ConflictEntry>,
}

// ---------------------------------------------------------------------------
// MergeMetadata
// ---------------------------------------------------------------------------

/// Provenance record written into the merged file header.
///
/// Corresponds to the `merge_metadata` object described in merge.md Section 4.3.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MergeMetadata {
    /// Sorted list of source file paths or identifiers that were merged.
    pub source_files: Vec<String>,
    /// Reporting entity values collected from all source files.
    ///
    /// When source files declare different `reporting_entity` values, the merged
    /// header omits `reporting_entity` and records all values here.
    pub reporting_entities: Vec<String>,
    /// ISO 8601 timestamp of when the merge was performed.
    pub timestamp: String,
    /// Number of merged output nodes.
    pub merged_node_count: usize,
    /// Number of merged output edges.
    pub merged_edge_count: usize,
    /// Total number of conflicts recorded across all nodes and edges.
    pub conflict_count: usize,
}

// ---------------------------------------------------------------------------
// Scalar merge
// ---------------------------------------------------------------------------

/// Result of merging N optional scalar values from a merge group.
///
/// Returned by [`merge_scalars`].
#[derive(Debug, Clone, PartialEq)]
pub enum ScalarMergeResult<T> {
    /// All sources agree on this value (or the value is absent in all sources).
    Agreed(Option<T>),
    /// Sources disagree; the caller should record a [`Conflict`].
    Conflict(Vec<ConflictEntry>),
}

/// Merges multiple optional scalar values into a single result.
///
/// The input is a slice of `(value, source_file)` pairs. The function:
///
/// 1. Collects all `Some` values, serialising each to a `serde_json::Value`
///    for comparison.
/// 2. If all `Some` values are JSON-equal (or there are no `Some` values at
///    all), returns [`ScalarMergeResult::Agreed`] with the common value.
/// 3. If any two `Some` values differ, returns [`ScalarMergeResult::Conflict`]
///    with one entry per distinct `(source_file, value)` pair, sorted by
///    `(source_file, json_value_as_string)`.
///
/// # Type parameters
///
/// - `T`: must implement [`Serialize`] (for JSON comparison) and [`Clone`].
pub fn merge_scalars<T>(inputs: &[(Option<T>, &str)]) -> ScalarMergeResult<T>
where
    T: Serialize + Clone,
{
    // Separate inputs that have a value from those that are absent.
    let mut present: Vec<(serde_json::Value, &str, &T)> = Vec::new();

    for (opt, source) in inputs {
        if let Some(val) = opt {
            let json_val = serde_json::to_value(val).unwrap_or(serde_json::Value::Null);
            present.push((json_val, source, val));
        }
    }

    if present.is_empty() {
        return ScalarMergeResult::Agreed(None);
    }

    // Check for consensus: all present values must be JSON-equal to the first.
    let first_json = &present[0].0;
    let all_equal = present.iter().all(|(v, _, _)| v == first_json);

    if all_equal {
        // Clone the first present T as the agreed value.
        return ScalarMergeResult::Agreed(Some(present[0].2.clone()));
    }

    // Build ConflictEntry list: one entry per distinct (source_file, value) pair.
    // Use a set of serialized values per source to avoid duplicating identical
    // values from the same source.
    let mut entries: Vec<ConflictEntry> = present
        .into_iter()
        .map(|(json_val, source, _)| ConflictEntry {
            value: json_val,
            source_file: source.to_owned(),
        })
        .collect();

    // Sort by (source_file, JSON value string) for determinism.
    entries.sort_by(|a, b| {
        let af = &a.source_file;
        let bf = &b.source_file;
        let av = a.value.to_string();
        let bv = b.value.to_string();
        af.cmp(bf).then_with(|| av.cmp(&bv))
    });

    // Deduplicate (same source_file + same json value is redundant).
    entries.dedup_by(|a, b| a.source_file == b.source_file && a.value == b.value);

    ScalarMergeResult::Conflict(entries)
}

// ---------------------------------------------------------------------------
// Identifier set-union merge
// ---------------------------------------------------------------------------

/// Merges multiple `Identifier` arrays into a deduplicated, sorted union.
///
/// Deduplication uses the [`CanonicalId`] string as the key: two identifiers
/// that produce the same canonical string are considered identical and only the
/// first occurrence (in input order) is retained.
///
/// The merged array is sorted by canonical string in lexicographic UTF-8 byte
/// order (merge.md Section 4.2).
///
/// # Parameters
///
/// - `inputs`: each element is an `Option<&[Identifier]>` (the `identifiers`
///   field from a source node/edge, which is `Option<Vec<Identifier>>`).
///
/// # Returns
///
/// A `Vec<Identifier>` that is the sorted set-union, or an empty `Vec` when
/// all inputs are `None` or empty.
pub fn merge_identifiers(inputs: &[Option<&[Identifier]>]) -> Vec<Identifier> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut result: Vec<(String, Identifier)> = Vec::new();

    for input in inputs {
        let Some(ids) = input else { continue };
        for id in *ids {
            let cid = CanonicalId::from_identifier(id);
            let key = cid.into_string();
            if seen.insert(key.clone()) {
                result.push((key, id.clone()));
            }
        }
    }

    // Sort by canonical string (the first element of the tuple).
    result.sort_by(|(a, _), (b, _)| a.cmp(b));

    result.into_iter().map(|(_, id)| id).collect()
}

// ---------------------------------------------------------------------------
// Label set-union merge
// ---------------------------------------------------------------------------

/// Merges multiple `Label` arrays into a deduplicated, sorted union.
///
/// Deduplication uses `(key, value)` as the composite key. Sorting follows
/// merge.md Section 4.2:
/// - Primary key: `key` ascending.
/// - Secondary key: `value` ascending, with `None` (absent value) sorting
///   before `Some(_)` (present value).
///
/// # Parameters
///
/// - `inputs`: each element is an `Option<&[Label]>`.
///
/// # Returns
///
/// A `Vec<Label>` that is the sorted set-union.
pub fn merge_labels(inputs: &[Option<&[Label]>]) -> Vec<Label> {
    // Canonicalise key: (key_string, value_option_string) for dedup.
    let mut seen: HashSet<(String, Option<String>)> = HashSet::new();
    let mut result: Vec<Label> = Vec::new();

    for input in inputs {
        let Some(labels) = input else { continue };
        for label in *labels {
            let key = (label.key.clone(), label.value.clone());
            if seen.insert(key) {
                result.push(label.clone());
            }
        }
    }

    // Sort: primary by key ascending, secondary by value (None before Some).
    result.sort_by(|a, b| {
        a.key.cmp(&b.key).then_with(|| match (&a.value, &b.value) {
            (None, None) => std::cmp::Ordering::Equal,
            (None, Some(_)) => std::cmp::Ordering::Less,
            (Some(_), None) => std::cmp::Ordering::Greater,
            (Some(av), Some(bv)) => av.cmp(bv),
        })
    });

    result
}

// ---------------------------------------------------------------------------
// same_as processing
// ---------------------------------------------------------------------------

/// Processes `same_as` edges and applies qualifying ones to a [`UnionFind`].
///
/// This function implements merge.md Section 7.1. It scans `edges` for edges of
/// type [`EdgeType::SameAs`] and, for each edge whose confidence level meets
/// `threshold`, calls `uf.union(src_ord, tgt_ord)`.
///
/// Node ordinals are resolved via `node_id_to_ordinal`, which maps a node's
/// graph-local `id` string to its index in the concatenated node slice.
///
/// `same_as` edges that are honoured are collected and returned so the caller
/// can record which merge groups were extended by `same_as` (merge.md
/// Section 7.3).  Edges that fail the threshold or whose source/target cannot
/// be resolved are silently skipped.
///
/// # Parameters
///
/// - `edges`: the full list of edges from all source files.
/// - `node_id_to_ordinal`: a function mapping a node ID string to its ordinal
///   index in the union-find structure.  Returns `None` for unknown IDs.
/// - `uf`: mutable reference to the [`UnionFind`] structure (already populated
///   by the identifier-based pass).
/// - `threshold`: the [`SameAsThreshold`] gate.
///
/// # Returns
///
/// A `Vec<&Edge>` containing the `same_as` edges that were honoured.
pub fn apply_same_as_edges<'a, F>(
    edges: &'a [Edge],
    node_id_to_ordinal: F,
    uf: &mut UnionFind,
    threshold: SameAsThreshold,
) -> Vec<&'a Edge>
where
    F: Fn(&str) -> Option<usize>,
{
    let mut honoured: Vec<&Edge> = Vec::new();

    for edge in edges {
        // Filter to same_as edges only.
        let is_same_as = match &edge.edge_type {
            EdgeTypeTag::Known(EdgeType::SameAs) => true,
            EdgeTypeTag::Known(_) | EdgeTypeTag::Extension(_) => false,
        };
        if !is_same_as {
            continue;
        }

        // Extract the confidence from the edge properties.
        // The confidence field lives inside `data_quality` on `EdgeProperties`,
        // but `same_as` edges carry a dedicated `confidence` property in their
        // `extra` map per the spec. We check `properties.extra["confidence"]`
        // first, then `data_quality.confidence` as a fallback.
        let confidence_str: Option<&str> = edge
            .properties
            .extra
            .get("confidence")
            .and_then(|v| v.as_str());

        // Also look in the edge-level extra map.
        let confidence_str =
            confidence_str.or_else(|| edge.extra.get("confidence").and_then(|v| v.as_str()));

        if !threshold.honours(confidence_str) {
            continue;
        }

        // Resolve source and target ordinals.
        let Some(src_ord) = node_id_to_ordinal(&edge.source) else {
            continue;
        };
        let Some(tgt_ord) = node_id_to_ordinal(&edge.target) else {
            continue;
        };

        uf.union(src_ord, tgt_ord);
        honoured.push(edge);
    }

    honoured
}

// ---------------------------------------------------------------------------
// Conflict builder helper
// ---------------------------------------------------------------------------

/// Builds a sorted `_conflicts` JSON array from a slice of [`Conflict`] records.
///
/// Conflicts are sorted by `field` name (merge.md Section 5, invariant 4).
/// This function is used when writing the merged node's `_conflicts` property.
///
/// Returns `None` when `conflicts` is empty (no `_conflicts` key should be
/// written in that case).
pub fn build_conflicts_value(mut conflicts: Vec<Conflict>) -> Option<serde_json::Value> {
    if conflicts.is_empty() {
        return None;
    }
    conflicts.sort_by(|a, b| a.field.cmp(&b.field));
    let val = serde_json::to_value(&conflicts).unwrap_or(serde_json::Value::Null);
    Some(val)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    #![allow(clippy::panic)]

    use serde_json::json;

    use super::*;
    use crate::enums::{EdgeType, EdgeTypeTag};
    use crate::newtypes::NodeId;
    use crate::structures::{Edge, EdgeProperties};
    use crate::types::{Identifier, Label};

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn make_identifier(scheme: &str, value: &str) -> Identifier {
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

    fn make_label(key: &str, value: Option<&str>) -> Label {
        Label {
            key: key.to_owned(),
            value: value.map(str::to_owned),
            extra: serde_json::Map::new(),
        }
    }

    fn make_same_as_edge(id: &str, src: &str, tgt: &str, confidence: Option<&str>) -> Edge {
        let mut props = EdgeProperties::default();
        if let Some(c) = confidence {
            props.extra.insert("confidence".to_owned(), json!(c));
        }
        Edge {
            id: NodeId::try_from(id).expect("valid edge id"),
            edge_type: EdgeTypeTag::Known(EdgeType::SameAs),
            source: NodeId::try_from(src).expect("valid node id"),
            target: NodeId::try_from(tgt).expect("valid node id"),
            identifiers: None,
            properties: props,
            extra: serde_json::Map::new(),
        }
    }

    fn make_supplies_edge(id: &str, src: &str, tgt: &str) -> Edge {
        Edge {
            id: NodeId::try_from(id).expect("valid edge id"),
            edge_type: EdgeTypeTag::Known(EdgeType::Supplies),
            source: NodeId::try_from(src).expect("valid node id"),
            target: NodeId::try_from(tgt).expect("valid node id"),
            identifiers: None,
            properties: EdgeProperties::default(),
            extra: serde_json::Map::new(),
        }
    }

    // -----------------------------------------------------------------------
    // SameAsThreshold::honours
    // -----------------------------------------------------------------------

    #[test]
    fn threshold_definite_honours_definite_only() {
        let t = SameAsThreshold::Definite;
        assert!(t.honours(Some("definite")));
        assert!(!t.honours(Some("probable")));
        assert!(!t.honours(Some("possible")));
        assert!(!t.honours(None));
    }

    #[test]
    fn threshold_probable_honours_definite_and_probable() {
        let t = SameAsThreshold::Probable;
        assert!(t.honours(Some("definite")));
        assert!(t.honours(Some("probable")));
        assert!(!t.honours(Some("possible")));
        assert!(!t.honours(None));
    }

    #[test]
    fn threshold_possible_honours_all() {
        let t = SameAsThreshold::Possible;
        assert!(t.honours(Some("definite")));
        assert!(t.honours(Some("probable")));
        assert!(t.honours(Some("possible")));
        assert!(t.honours(None)); // absent treated as "possible"
    }

    #[test]
    fn threshold_unrecognised_string_treated_as_possible() {
        let t = SameAsThreshold::Possible;
        assert!(t.honours(Some("unknown_level")));
        let t2 = SameAsThreshold::Definite;
        assert!(!t2.honours(Some("unknown_level")));
    }

    #[test]
    fn threshold_default_is_definite() {
        assert_eq!(SameAsThreshold::default(), SameAsThreshold::Definite);
    }

    // -----------------------------------------------------------------------
    // merge_scalars — identical properties
    // -----------------------------------------------------------------------

    #[test]
    fn scalars_both_none_agrees_on_none() {
        let inputs: Vec<(Option<String>, &str)> =
            vec![(None, "file_a.json"), (None, "file_b.json")];
        let result = merge_scalars(&inputs);
        assert_eq!(result, ScalarMergeResult::Agreed(None));
    }

    #[test]
    fn scalars_one_none_one_some_agrees_on_some() {
        let inputs: Vec<(Option<String>, &str)> = vec![
            (None, "file_a.json"),
            (Some("Acme Corp".to_owned()), "file_b.json"),
        ];
        let result = merge_scalars(&inputs);
        assert_eq!(
            result,
            ScalarMergeResult::Agreed(Some("Acme Corp".to_owned()))
        );
    }

    #[test]
    fn scalars_identical_values_agree() {
        let inputs: Vec<(Option<String>, &str)> = vec![
            (Some("Acme Corp".to_owned()), "file_a.json"),
            (Some("Acme Corp".to_owned()), "file_b.json"),
        ];
        let result = merge_scalars(&inputs);
        assert_eq!(
            result,
            ScalarMergeResult::Agreed(Some("Acme Corp".to_owned()))
        );
    }

    #[test]
    fn scalars_three_identical_values_agree() {
        let inputs: Vec<(Option<u64>, &str)> = vec![
            (Some(42), "a.json"),
            (Some(42), "b.json"),
            (Some(42), "c.json"),
        ];
        let result = merge_scalars(&inputs);
        assert_eq!(result, ScalarMergeResult::Agreed(Some(42u64)));
    }

    // -----------------------------------------------------------------------
    // merge_scalars — conflicting properties
    // -----------------------------------------------------------------------

    #[test]
    fn scalars_different_values_conflict() {
        let inputs: Vec<(Option<String>, &str)> = vec![
            (Some("Acme Corp".to_owned()), "file_a.json"),
            (Some("ACME Corporation".to_owned()), "file_b.json"),
        ];
        let result = merge_scalars(&inputs);
        match result {
            ScalarMergeResult::Conflict(entries) => {
                assert_eq!(entries.len(), 2);
                // Sorted by source_file
                assert_eq!(entries[0].source_file, "file_a.json");
                assert_eq!(entries[0].value, json!("Acme Corp"));
                assert_eq!(entries[1].source_file, "file_b.json");
                assert_eq!(entries[1].value, json!("ACME Corporation"));
            }
            ScalarMergeResult::Agreed(_) => panic!("expected Conflict"),
        }
    }

    #[test]
    fn scalars_conflict_sorted_by_source_file() {
        // Source files arrive out of order; output must be sorted.
        let inputs: Vec<(Option<String>, &str)> = vec![
            (Some("Z".to_owned()), "z_file.json"),
            (Some("A".to_owned()), "a_file.json"),
        ];
        let result = merge_scalars(&inputs);
        match result {
            ScalarMergeResult::Conflict(entries) => {
                assert_eq!(entries[0].source_file, "a_file.json");
                assert_eq!(entries[1].source_file, "z_file.json");
            }
            ScalarMergeResult::Agreed(_) => panic!("expected Conflict"),
        }
    }

    #[test]
    fn scalars_conflict_deduplicates_same_source_same_value() {
        let inputs: Vec<(Option<String>, &str)> = vec![
            (Some("X".to_owned()), "file_a.json"),
            (Some("X".to_owned()), "file_a.json"), // duplicate — should be merged with above
            (Some("Y".to_owned()), "file_b.json"),
        ];
        let result = merge_scalars(&inputs);
        match result {
            ScalarMergeResult::Conflict(entries) => {
                // "X" from file_a appears once despite two inputs
                let file_a_entries: Vec<_> = entries
                    .iter()
                    .filter(|e| e.source_file == "file_a.json")
                    .collect();
                assert_eq!(file_a_entries.len(), 1);
                assert_eq!(file_a_entries[0].value, json!("X"));
            }
            ScalarMergeResult::Agreed(_) => panic!("expected Conflict"),
        }
    }

    #[test]
    fn scalars_numeric_conflict() {
        let inputs: Vec<(Option<f64>, &str)> =
            vec![(Some(51.0_f64), "a.json"), (Some(49.0_f64), "b.json")];
        let result = merge_scalars(&inputs);
        assert!(matches!(result, ScalarMergeResult::Conflict(_)));
    }

    // -----------------------------------------------------------------------
    // merge_identifiers — deduplication and sorting
    // -----------------------------------------------------------------------

    #[test]
    fn identifiers_empty_inputs_produces_empty() {
        let result = merge_identifiers(&[]);
        assert!(result.is_empty());
    }

    #[test]
    fn identifiers_all_none_produces_empty() {
        let result = merge_identifiers(&[None, None]);
        assert!(result.is_empty());
    }

    #[test]
    fn identifiers_single_source_passthrough() {
        let ids = vec![
            make_identifier("lei", "LEI_A"),
            make_identifier("duns", "DUNS_A"),
        ];
        let result = merge_identifiers(&[Some(ids.as_slice())]);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn identifiers_dedup_by_canonical_string() {
        let ids_a = vec![make_identifier("lei", "SAME_LEI")];
        let ids_b = vec![make_identifier("lei", "SAME_LEI")];
        let result = merge_identifiers(&[Some(ids_a.as_slice()), Some(ids_b.as_slice())]);
        assert_eq!(result.len(), 1, "duplicate canonical id should be deduped");
        assert_eq!(result[0].scheme, "lei");
        assert_eq!(result[0].value, "SAME_LEI");
    }

    #[test]
    fn identifiers_union_non_overlapping() {
        let ids_a = vec![make_identifier("lei", "LEI_A")];
        let ids_b = vec![make_identifier("duns", "DUNS_B")];
        let result = merge_identifiers(&[Some(ids_a.as_slice()), Some(ids_b.as_slice())]);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn identifiers_sorted_by_canonical_string() {
        // "lei:Z" > "duns:A" lexicographically; output must sort them
        let ids_a = vec![make_identifier("lei", "Z")];
        let ids_b = vec![make_identifier("duns", "A")];
        let result = merge_identifiers(&[Some(ids_a.as_slice()), Some(ids_b.as_slice())]);
        assert_eq!(result.len(), 2);
        // "duns:A" < "lei:Z" lexicographically
        assert_eq!(result[0].scheme, "duns");
        assert_eq!(result[1].scheme, "lei");
    }

    #[test]
    fn identifiers_multiple_sources_union_deduplicated_sorted() {
        // Three sources: overlapping identifiers from different sources.
        let ids_a = vec![
            make_identifier("lei", "LEI_1"),
            make_identifier("duns", "DUNS_1"),
        ];
        let ids_b = vec![
            make_identifier("lei", "LEI_1"), // duplicate
            make_identifier("gln", "GLN_1"),
        ];
        let ids_c = vec![make_identifier("duns", "DUNS_1")]; // duplicate

        let result = merge_identifiers(&[
            Some(ids_a.as_slice()),
            Some(ids_b.as_slice()),
            Some(ids_c.as_slice()),
        ]);

        // Should have: duns:DUNS_1, gln:GLN_1, lei:LEI_1
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].scheme, "duns");
        assert_eq!(result[1].scheme, "gln");
        assert_eq!(result[2].scheme, "lei");
    }

    // -----------------------------------------------------------------------
    // merge_labels — set union and sorting
    // -----------------------------------------------------------------------

    #[test]
    fn labels_empty_inputs_produces_empty() {
        let result = merge_labels(&[]);
        assert!(result.is_empty());
    }

    #[test]
    fn labels_all_none_produces_empty() {
        let result = merge_labels(&[None, None]);
        assert!(result.is_empty());
    }

    #[test]
    fn labels_single_source_passthrough() {
        let labels = vec![make_label("env", Some("prod")), make_label("tier", None)];
        let result = merge_labels(&[Some(labels.as_slice())]);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn labels_dedup_exact_key_value_pair() {
        let labels_a = vec![make_label("env", Some("prod"))];
        let labels_b = vec![make_label("env", Some("prod"))];
        let result = merge_labels(&[Some(labels_a.as_slice()), Some(labels_b.as_slice())]);
        assert_eq!(
            result.len(),
            1,
            "duplicate (key, value) pair should be deduped"
        );
    }

    #[test]
    fn labels_same_key_different_values_both_kept() {
        let labels_a = vec![make_label("env", Some("prod"))];
        let labels_b = vec![make_label("env", Some("staging"))];
        let result = merge_labels(&[Some(labels_a.as_slice()), Some(labels_b.as_slice())]);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn labels_sorted_by_key_ascending() {
        let labels = vec![
            make_label("z_tag", Some("v")),
            make_label("a_tag", Some("v")),
        ];
        let result = merge_labels(&[Some(labels.as_slice())]);
        assert_eq!(result[0].key, "a_tag");
        assert_eq!(result[1].key, "z_tag");
    }

    #[test]
    fn labels_none_value_sorts_before_some_value() {
        // Same key, one label has no value and the other has a value.
        let labels = vec![
            make_label("flag", Some("present")),
            make_label("flag", None),
        ];
        let result = merge_labels(&[Some(labels.as_slice())]);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].value, None); // None sorts first
        assert_eq!(result[1].value, Some("present".to_owned()));
    }

    #[test]
    fn labels_sorted_by_key_then_value() {
        let labels_a = vec![make_label("env", Some("prod"))];
        let labels_b = vec![
            make_label("env", Some("dev")),
            make_label("app", Some("service-a")),
        ];
        let result = merge_labels(&[Some(labels_a.as_slice()), Some(labels_b.as_slice())]);
        // Expected order: ("app","service-a"), ("env","dev"), ("env","prod")
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].key, "app");
        assert_eq!(result[1].key, "env");
        assert_eq!(result[1].value.as_deref(), Some("dev"));
        assert_eq!(result[2].key, "env");
        assert_eq!(result[2].value.as_deref(), Some("prod"));
    }

    // -----------------------------------------------------------------------
    // apply_same_as_edges — threshold gating
    // -----------------------------------------------------------------------

    fn ordinal_lookup<'a>(ids: &'a [&'a str]) -> impl Fn(&str) -> Option<usize> + 'a {
        |id: &str| ids.iter().position(|&s| s == id)
    }

    #[test]
    fn same_as_definite_threshold_honours_definite_only() {
        let node_ids = ["n0", "n1", "n2"];
        let edges = vec![
            make_same_as_edge("e1", "n0", "n1", Some("definite")),
            make_same_as_edge("e2", "n1", "n2", Some("probable")),
        ];

        let mut uf = UnionFind::new(3);
        let lookup = ordinal_lookup(&node_ids);
        let honoured = apply_same_as_edges(&edges, lookup, &mut uf, SameAsThreshold::Definite);

        assert_eq!(honoured.len(), 1);
        assert_eq!(&*honoured[0].id, "e1");
        // n0 and n1 should be in the same set
        assert_eq!(uf.find(0), uf.find(1));
        // n2 should be separate
        assert_ne!(uf.find(0), uf.find(2));
    }

    #[test]
    fn same_as_probable_threshold_honours_definite_and_probable() {
        let node_ids = ["n0", "n1", "n2"];
        let edges = vec![
            make_same_as_edge("e1", "n0", "n1", Some("definite")),
            make_same_as_edge("e2", "n1", "n2", Some("probable")),
        ];

        let mut uf = UnionFind::new(3);
        let lookup = ordinal_lookup(&node_ids);
        let honoured = apply_same_as_edges(&edges, lookup, &mut uf, SameAsThreshold::Probable);

        assert_eq!(honoured.len(), 2);
        assert_eq!(uf.find(0), uf.find(1));
        assert_eq!(uf.find(1), uf.find(2));
    }

    #[test]
    fn same_as_possible_threshold_honours_all() {
        let node_ids = ["n0", "n1", "n2", "n3"];
        let edges = vec![
            make_same_as_edge("e1", "n0", "n1", Some("definite")),
            make_same_as_edge("e2", "n1", "n2", Some("probable")),
            make_same_as_edge("e3", "n2", "n3", Some("possible")),
        ];

        let mut uf = UnionFind::new(4);
        let lookup = ordinal_lookup(&node_ids);
        let honoured = apply_same_as_edges(&edges, lookup, &mut uf, SameAsThreshold::Possible);

        assert_eq!(honoured.len(), 3);
        let root = uf.find(0);
        assert_eq!(uf.find(1), root);
        assert_eq!(uf.find(2), root);
        assert_eq!(uf.find(3), root);
    }

    #[test]
    fn same_as_no_confidence_treated_as_possible() {
        let node_ids = ["n0", "n1"];
        let edges = vec![make_same_as_edge("e1", "n0", "n1", None)];

        let mut uf = UnionFind::new(2);
        let lookup = ordinal_lookup(&node_ids);

        // With Definite threshold: absent confidence = possible → not honoured
        let honoured = apply_same_as_edges(&edges, lookup, &mut uf, SameAsThreshold::Definite);
        assert!(honoured.is_empty());
        assert_ne!(uf.find(0), uf.find(1));

        // With Possible threshold: honoured
        let lookup2 = ordinal_lookup(&node_ids);
        let mut uf2 = UnionFind::new(2);
        let honoured2 = apply_same_as_edges(&edges, lookup2, &mut uf2, SameAsThreshold::Possible);
        assert_eq!(honoured2.len(), 1);
        assert_eq!(uf2.find(0), uf2.find(1));
    }

    #[test]
    fn same_as_non_same_as_edges_ignored() {
        let node_ids = ["n0", "n1"];
        let edges = vec![make_supplies_edge("e1", "n0", "n1")];

        let mut uf = UnionFind::new(2);
        let lookup = ordinal_lookup(&node_ids);
        let honoured = apply_same_as_edges(&edges, lookup, &mut uf, SameAsThreshold::Possible);

        assert!(honoured.is_empty());
        assert_ne!(uf.find(0), uf.find(1));
    }

    #[test]
    fn same_as_unknown_source_skipped() {
        let node_ids = ["n0", "n1"];
        let edges = vec![make_same_as_edge("e1", "UNKNOWN", "n1", Some("definite"))];

        let mut uf = UnionFind::new(2);
        let lookup = ordinal_lookup(&node_ids);
        let honoured = apply_same_as_edges(&edges, lookup, &mut uf, SameAsThreshold::Possible);

        assert!(honoured.is_empty());
    }

    #[test]
    fn same_as_cycle_handled_idempotently() {
        // A→B, B→C, C→A: union-find handles cycles as redundant unions.
        let node_ids = ["n0", "n1", "n2"];
        let edges = vec![
            make_same_as_edge("e1", "n0", "n1", Some("definite")),
            make_same_as_edge("e2", "n1", "n2", Some("definite")),
            make_same_as_edge("e3", "n2", "n0", Some("definite")),
        ];

        let mut uf = UnionFind::new(3);
        let lookup = ordinal_lookup(&node_ids);
        let honoured = apply_same_as_edges(&edges, lookup, &mut uf, SameAsThreshold::Definite);

        assert_eq!(honoured.len(), 3);
        let root = uf.find(0);
        assert_eq!(uf.find(1), root);
        assert_eq!(uf.find(2), root);
    }

    // -----------------------------------------------------------------------
    // build_conflicts_value
    // -----------------------------------------------------------------------

    #[test]
    fn build_conflicts_empty_returns_none() {
        let result = build_conflicts_value(vec![]);
        assert!(result.is_none());
    }

    #[test]
    fn build_conflicts_sorted_by_field() {
        let conflicts = vec![
            Conflict {
                field: "z_field".to_owned(),
                values: vec![ConflictEntry {
                    value: json!("z"),
                    source_file: "a.json".to_owned(),
                }],
            },
            Conflict {
                field: "a_field".to_owned(),
                values: vec![ConflictEntry {
                    value: json!("a"),
                    source_file: "a.json".to_owned(),
                }],
            },
        ];
        let val = build_conflicts_value(conflicts).expect("non-empty conflicts");
        let arr = val.as_array().expect("array");
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["field"].as_str(), Some("a_field"));
        assert_eq!(arr[1]["field"].as_str(), Some("z_field"));
    }

    #[test]
    fn build_conflicts_single_conflict_serialises() {
        let conflicts = vec![Conflict {
            field: "name".to_owned(),
            values: vec![
                ConflictEntry {
                    value: json!("Acme"),
                    source_file: "a.json".to_owned(),
                },
                ConflictEntry {
                    value: json!("ACME Corp"),
                    source_file: "b.json".to_owned(),
                },
            ],
        }];
        let val = build_conflicts_value(conflicts).expect("non-empty");
        let arr = val.as_array().expect("array");
        assert_eq!(arr[0]["field"].as_str(), Some("name"));
        let entries = arr[0]["values"].as_array().expect("values array");
        assert_eq!(entries.len(), 2);
    }

    // -----------------------------------------------------------------------
    // MergeMetadata — construction and serialisation
    // -----------------------------------------------------------------------

    #[test]
    fn merge_metadata_round_trip() {
        let meta = MergeMetadata {
            source_files: vec!["a.omts".to_owned(), "b.omts".to_owned()],
            reporting_entities: vec!["org-acme".to_owned()],
            timestamp: "2026-02-19T00:00:00Z".to_owned(),
            merged_node_count: 10,
            merged_edge_count: 5,
            conflict_count: 2,
        };
        let json = serde_json::to_string(&meta).expect("serialize");
        let back: MergeMetadata = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(meta, back);
    }
}
