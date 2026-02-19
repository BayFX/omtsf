/// Identity predicates for the merge engine.
///
/// Implements the node identity predicate and temporal compatibility check
/// described in merge.md Sections 2.2 and 3.1, and the edge identity predicate
/// described in merge.md Section 3.2 and diff.md Section 2.2.
///
/// All functions in this module are pure (no side-effects, no I/O).
use std::collections::HashMap;

use crate::enums::{EdgeType, EdgeTypeTag};
use crate::newtypes::CalendarDate;
use crate::structures::{Edge, EdgeProperties};
use crate::types::Identifier;

// ---------------------------------------------------------------------------
// identifiers_match
// ---------------------------------------------------------------------------

/// Returns `true` when two [`Identifier`] records should be considered the
/// same identifier for merge purposes.
///
/// The predicate is symmetric by construction; every comparison is symmetric
/// (string equality, case-insensitive equality, interval overlap), so
/// `identifiers_match(a, b) == identifiers_match(b, a)` always holds.
///
/// # Rules (applied in order)
///
/// 1. **Internal scheme excluded** — if either identifier uses the `"internal"`
///    scheme, return `false`. Internal identifiers are private to each
///    reporting entity and must never trigger a merge.
/// 2. **Schemes must match** — `a.scheme != b.scheme` → `false`.
/// 3. **Values must match (whitespace-trimmed)** — leading/trailing whitespace
///    in a stored value is normalised away before comparison.
/// 4. **Authority check** — if either record carries an `authority` field,
///    *both* must carry it and it must match case-insensitively. If one has
///    authority and the other does not, return `false`.
/// 5. **Temporal compatibility** — the validity intervals must overlap; see
///    [`temporal_compatible`] for the detailed rules.
pub fn identifiers_match(a: &Identifier, b: &Identifier) -> bool {
    // Rule 1: Exclude internal scheme.
    if a.scheme == "internal" || b.scheme == "internal" {
        return false;
    }

    // Rule 2: Schemes must match.
    if a.scheme != b.scheme {
        return false;
    }

    // Rule 3: Values must match (whitespace-trimmed).
    if a.value.trim() != b.value.trim() {
        return false;
    }

    // Rule 4: Authority check.
    if a.authority.is_some() || b.authority.is_some() {
        match (&a.authority, &b.authority) {
            (Some(aa), Some(ba)) => {
                if !aa.eq_ignore_ascii_case(ba) {
                    return false;
                }
            }
            // One has authority, the other does not.
            (Some(_), None) | (None, Some(_)) => return false,
            // Both None is handled by the outer `is_some()` guard above and
            // can never reach this arm, but the match must be exhaustive.
            (None, None) => {}
        }
    }

    // Rule 5: Temporal compatibility.
    temporal_compatible(a, b)
}

// ---------------------------------------------------------------------------
// temporal_compatible
// ---------------------------------------------------------------------------

/// Returns `true` when two identifier records' validity intervals overlap.
///
/// The full three-state semantics of `valid_to` on [`Identifier`] are:
/// - `None` — field absent (temporal bounds not supplied at all).
/// - `Some(None)` — explicit JSON `null` (identifier has no expiry; open-ended
///   into the future).
/// - `Some(Some(date))` — expires on the given date.
///
/// # Rules
///
/// - If *either* record omits **both** `valid_from` and `valid_to` entirely
///   (i.e. both fields are `None`), temporal compatibility is assumed.
/// - Two intervals overlap when it is *not* the case that one ends strictly
///   before the other begins. Specifically, incompatibility is declared only
///   when both records have a concrete `valid_to` date, one `valid_to` is
///   strictly less than the other's `valid_from`, and that `valid_from` is
///   present. An explicit `valid_to: null` (no-expiry) never causes
///   incompatibility.
pub fn temporal_compatible(a: &Identifier, b: &Identifier) -> bool {
    // If either record has no temporal information at all, assume compatible.
    let a_has_temporal = a.valid_from.is_some() || a.valid_to.is_some();
    let b_has_temporal = b.valid_from.is_some() || b.valid_to.is_some();
    if !a_has_temporal || !b_has_temporal {
        return true;
    }

    // Check whether interval A ends before interval B starts.
    if intervals_disjoint(a.valid_to.as_ref(), b.valid_from.as_ref()) {
        return false;
    }

    // Check whether interval B ends before interval A starts.
    if intervals_disjoint(b.valid_to.as_ref(), a.valid_from.as_ref()) {
        return false;
    }

    true
}

/// Returns `true` when an interval that ends at `end` is strictly before an
/// interval that starts at `start`.
///
/// - `end = None` — field absent; treated as open-ended (never disjoint on
///   this end).
/// - `end = Some(None)` — explicit no-expiry; open-ended (never disjoint).
/// - `end = Some(Some(date))` — concrete end date.
/// - `start = None` — field absent; treated as open-ended from the beginning.
///
/// Disjoint only when `end < start` with both values concrete.
fn intervals_disjoint(end: Option<&Option<CalendarDate>>, start: Option<&CalendarDate>) -> bool {
    // If start is absent, the interval is open-ended at the left; never
    // disjoint on that end.
    let Some(start_date) = start else {
        return false;
    };

    // Resolve the end value.
    match end {
        // end field absent → open-ended; not disjoint.
        None => false,
        // explicit null → no-expiry; not disjoint.
        Some(None) => false,
        // concrete end date: disjoint iff end < start
        Some(Some(end_date)) => end_date < start_date,
    }
}

// ---------------------------------------------------------------------------
// is_lei_annulled
// ---------------------------------------------------------------------------

/// Returns `true` when an LEI identifier is known to be in ANNULLED status.
///
/// The `VerificationStatus` enum does not include an `Annulled` variant; GLEIF
/// ANNULLED status is typically carried as enrichment data outside the core
/// schema. This function inspects the identifier's `extra` extension fields for
/// a best-effort detection of annulled LEIs.
///
/// # Detection strategy
///
/// The function checks:
/// 1. `id.scheme == "lei"`.
/// 2. The `extra` map contains `"entity_status"` with the string value
///    `"ANNULLED"` (case-sensitive, as GLEIF uses all-caps status codes).
///
/// If L2 enrichment data is unavailable, this function returns `false` (no
/// false-positive exclusions). Callers that have richer LEI data should apply
/// their own filtering before index construction.
pub fn is_lei_annulled(id: &Identifier) -> bool {
    if id.scheme != "lei" {
        return false;
    }
    matches!(
        id.extra.get("entity_status").and_then(|v| v.as_str()),
        Some("ANNULLED")
    )
}

// ---------------------------------------------------------------------------
// edge_composite_key
// ---------------------------------------------------------------------------

/// Composite key used to group edge merge candidates.
///
/// Two edges belong to the same candidate bucket when their source nodes
/// resolve to the same union-find representative, their target nodes resolve to
/// the same representative, and their edge types are equal.
///
/// This is an opaque key suitable for use as a `HashMap` key; it does not
/// expose the individual fields beyond equality and hashing.
///
/// Constructed via [`edge_composite_key`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EdgeCompositeKey {
    /// Union-find representative of the source node.
    pub source_rep: usize,
    /// Union-find representative of the target node.
    pub target_rep: usize,
    /// Edge type tag.
    pub edge_type: EdgeTypeTag,
}

/// Returns the composite key `(find(source_ordinal), find(target_ordinal), type)`
/// for an edge, using the provided union-find representatives.
///
/// The caller is responsible for resolving source and target node ordinals to
/// their union-find representatives before calling this function. This keeps
/// the function pure and the union-find mutation explicit in the caller.
///
/// # Parameters
///
/// - `source_rep`: `find(source_node_ordinal)` from the union-find structure.
/// - `target_rep`: `find(target_node_ordinal)` from the union-find structure.
/// - `edge`: the edge whose type field is used as the third component.
///
/// # Returns
///
/// Returns `None` when the edge type is `same_as` — such edges are excluded
/// from candidate grouping per merge.md Section 3.2.
pub fn edge_composite_key(
    source_rep: usize,
    target_rep: usize,
    edge: &Edge,
) -> Option<EdgeCompositeKey> {
    // same_as edges are never merged; exclude them entirely.
    if let EdgeTypeTag::Known(EdgeType::SameAs) = &edge.edge_type {
        return None;
    }

    Some(EdgeCompositeKey {
        source_rep,
        target_rep,
        edge_type: edge.edge_type.clone(),
    })
}

// ---------------------------------------------------------------------------
// build_edge_candidate_index
// ---------------------------------------------------------------------------

/// Builds a composite-key index that groups edge ordinals by their resolved
/// `(find(source), find(target), type)` triple.
///
/// The caller supplies a `node_ordinal` function that maps a node's graph-local
/// `NodeId` string to its position in the flat node array, and a `find`
/// closure that returns the union-find representative for a given ordinal.
/// Both may return `None` when the node is not found (dangling references).
///
/// Edges whose source or target is dangling — i.e. the node ordinal cannot be
/// resolved — are silently skipped. `same_as` edges are also skipped per the
/// spec (they are never merge candidates).
///
/// Construction is O(total edges) after the union-find is settled.
///
/// # Parameters
///
/// - `edges`: flat slice of all edges from the concatenated input files.
/// - `node_ordinal`: returns the ordinal of the node with the given id string,
///   or `None` if not found.
/// - `find`: returns the union-find representative of the given ordinal.
///
/// # Returns
///
/// A `HashMap` from [`EdgeCompositeKey`] to a `Vec<usize>` of edge ordinals
/// that share the same composite key.
pub fn build_edge_candidate_index<F, G>(
    edges: &[Edge],
    node_ordinal: F,
    find: G,
) -> HashMap<EdgeCompositeKey, Vec<usize>>
where
    F: Fn(&str) -> Option<usize>,
    G: Fn(usize) -> usize,
{
    let mut index: HashMap<EdgeCompositeKey, Vec<usize>> = HashMap::new();

    for (edge_idx, edge) in edges.iter().enumerate() {
        let Some(src_ord) = node_ordinal(edge.source.as_ref()) else {
            continue;
        };
        let Some(tgt_ord) = node_ordinal(edge.target.as_ref()) else {
            continue;
        };

        let src_rep = find(src_ord);
        let tgt_rep = find(tgt_ord);

        let Some(key) = edge_composite_key(src_rep, tgt_rep, edge) else {
            // same_as — skip
            continue;
        };

        index.entry(key).or_default().push(edge_idx);
    }

    index
}

// ---------------------------------------------------------------------------
// edge_identity_properties_match
// ---------------------------------------------------------------------------

/// Returns `true` when two edges' type-specific identity properties are equal
/// per the SPEC-003 Section 3.1 table.
///
/// This predicate is evaluated only for edges that **lack external identifiers**
/// (or whose external identifiers do not produce a match). When both edges have
/// no external identifiers (or only `internal`-scheme identifiers), this
/// property comparison is the sole basis for deciding whether the edges are
/// merge candidates.
///
/// # Per-type identity property table
///
/// | Edge type             | Identity properties beyond type + endpoints |
/// |-----------------------|---------------------------------------------|
/// | `ownership`           | `percentage`, `direct`                      |
/// | `operational_control` | `control_type`                              |
/// | `legal_parentage`     | `consolidation_basis`                       |
/// | `former_identity`     | `event_type`, `effective_date`              |
/// | `beneficial_ownership`| `control_type`, `percentage`                |
/// | `supplies`            | `commodity`, `contract_ref`                 |
/// | `subcontracts`        | `commodity`, `contract_ref`                 |
/// | `tolls`               | `commodity`                                 |
/// | `distributes`         | `service_type`                              |
/// | `brokers`             | `commodity`                                 |
/// | `operates`            | *(type + endpoints suffice)*                |
/// | `produces`            | *(type + endpoints suffice)*                |
/// | `composed_of`         | *(type + endpoints suffice)*                |
/// | `sells_to`            | `commodity`, `contract_ref`                 |
/// | `attested_by`         | `scope`                                     |
/// | `same_as`             | *(never matched — always unique)*           |
/// | Extension             | *(type + endpoints suffice)*                |
///
/// For edge types where "type + endpoints suffice," this function always
/// returns `true` (the composite key check already guarantees type and endpoint
/// identity).
pub fn edge_identity_properties_match(
    edge_type: &EdgeTypeTag,
    a: &EdgeProperties,
    b: &EdgeProperties,
) -> bool {
    match edge_type {
        EdgeTypeTag::Known(EdgeType::Ownership) => {
            options_eq(&a.percentage, &b.percentage) && a.direct == b.direct
        }
        EdgeTypeTag::Known(EdgeType::OperationalControl) => a.control_type == b.control_type,
        EdgeTypeTag::Known(EdgeType::LegalParentage) => {
            a.consolidation_basis == b.consolidation_basis
        }
        EdgeTypeTag::Known(EdgeType::FormerIdentity) => {
            a.event_type == b.event_type && a.effective_date == b.effective_date
        }
        EdgeTypeTag::Known(EdgeType::BeneficialOwnership) => {
            a.control_type == b.control_type && options_eq(&a.percentage, &b.percentage)
        }
        EdgeTypeTag::Known(EdgeType::Supplies) => {
            a.commodity == b.commodity && a.contract_ref == b.contract_ref
        }
        EdgeTypeTag::Known(EdgeType::Subcontracts) => {
            a.commodity == b.commodity && a.contract_ref == b.contract_ref
        }
        EdgeTypeTag::Known(EdgeType::Tolls) => a.commodity == b.commodity,
        EdgeTypeTag::Known(EdgeType::Distributes) => a.service_type == b.service_type,
        EdgeTypeTag::Known(EdgeType::Brokers) => a.commodity == b.commodity,
        EdgeTypeTag::Known(EdgeType::Operates) => true,
        EdgeTypeTag::Known(EdgeType::Produces) => true,
        EdgeTypeTag::Known(EdgeType::ComposedOf) => true,
        EdgeTypeTag::Known(EdgeType::SellsTo) => {
            a.commodity == b.commodity && a.contract_ref == b.contract_ref
        }
        EdgeTypeTag::Known(EdgeType::AttestedBy) => a.scope == b.scope,
        // same_as is never matched; this arm is unreachable in normal usage
        // because edge_composite_key returns None for same_as, but the
        // exhaustive match is required.
        EdgeTypeTag::Known(EdgeType::SameAs) => false,
        // Extension edge types: type + endpoints suffice.
        EdgeTypeTag::Extension(_) => true,
    }
}

/// Compares two `Option<f64>` values for identity-predicate equality.
///
/// `None == None` is `true`. `Some(a) == Some(b)` uses bitwise comparison
/// (same bit pattern), which is appropriate for identity purposes where the
/// values came from the same JSON representation. In particular, this treats
/// `NaN != NaN` (standard IEEE 754 semantics), which is correct: two edges
/// with NaN percentages should not be considered the same.
fn options_eq(a: &Option<f64>, b: &Option<f64>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(x), Some(y)) => x.to_bits() == y.to_bits(),
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// edges_match
// ---------------------------------------------------------------------------

/// Returns `true` when two edges are merge candidates.
///
/// Two edges are merge candidates when **all** of the following hold:
///
/// 1. Their source nodes belong to the same union-find group (`source_rep_a ==
///    source_rep_b`).
/// 2. Their target nodes belong to the same union-find group (`target_rep_a ==
///    target_rep_b`).
/// 3. Their `type` fields are equal.
/// 4. Either they share an external identifier (same predicate as nodes, per
///    [`identifiers_match`]) — or they lack external identifiers and their
///    type-specific identity properties match per
///    [`edge_identity_properties_match`].
///
/// Condition 4 is evaluated as: if *any* pair of external identifiers matches
/// (via [`identifiers_match`]), the edges are candidates. Otherwise, if both
/// edges have **no** external identifiers (or only `internal`-scheme ones), the
/// type-specific property table is consulted.
///
/// `same_as` edges are excluded: this function returns `false` for any edge
/// whose type is `same_as`, before any further checks.
///
/// # Parameters
///
/// - `source_rep_a`, `target_rep_a`: resolved union-find representatives for
///   the source and target of edge `a`.
/// - `source_rep_b`, `target_rep_b`: resolved union-find representatives for
///   the source and target of edge `b`.
/// - `a`, `b`: the two edges to compare.
#[allow(clippy::too_many_arguments)]
pub fn edges_match(
    source_rep_a: usize,
    target_rep_a: usize,
    source_rep_b: usize,
    target_rep_b: usize,
    a: &Edge,
    b: &Edge,
) -> bool {
    // same_as edges are never merge candidates.
    if let EdgeTypeTag::Known(EdgeType::SameAs) = &a.edge_type {
        return false;
    }
    if let EdgeTypeTag::Known(EdgeType::SameAs) = &b.edge_type {
        return false;
    }

    // Condition 1 & 2: resolved endpoints must match.
    if source_rep_a != source_rep_b || target_rep_a != target_rep_b {
        return false;
    }

    // Condition 3: types must match.
    if a.edge_type != b.edge_type {
        return false;
    }

    // Condition 4: shared external identifier OR property-table match.
    let a_external: Vec<&Identifier> = a
        .identifiers
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .filter(|id| id.scheme != "internal")
        .collect();

    let b_external: Vec<&Identifier> = b
        .identifiers
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .filter(|id| id.scheme != "internal")
        .collect();

    // If either edge has external identifiers, check for a matching pair.
    if !a_external.is_empty() || !b_external.is_empty() {
        // Check whether any pair of external identifiers matches.
        for id_a in &a_external {
            for id_b in &b_external {
                if identifiers_match(id_a, id_b) {
                    return true;
                }
            }
        }
        // At least one side had external identifiers but none matched.
        return false;
    }

    // Both edges have no external identifiers: fall back to type-specific
    // property comparison.
    edge_identity_properties_match(&a.edge_type, &a.properties, &b.properties)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    #![allow(clippy::field_reassign_with_default)]

    use serde_json::json;

    use crate::newtypes::CalendarDate;

    use super::*;

    // --- helpers ------------------------------------------------------------

    fn make_id(scheme: &str, value: &str) -> Identifier {
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

    fn with_authority(mut id: Identifier, authority: &str) -> Identifier {
        id.authority = Some(authority.to_owned());
        id
    }

    fn with_valid_from(mut id: Identifier, date: &str) -> Identifier {
        id.valid_from = Some(CalendarDate::try_from(date).expect("valid date"));
        id
    }

    fn with_valid_to_date(mut id: Identifier, date: &str) -> Identifier {
        id.valid_to = Some(Some(CalendarDate::try_from(date).expect("valid date")));
        id
    }

    fn with_valid_to_null(mut id: Identifier) -> Identifier {
        // Explicit no-expiry (JSON null)
        id.valid_to = Some(None);
        id
    }

    // --- identifiers_match --------------------------------------------------

    #[test]
    fn same_scheme_and_value_matches() {
        let a = make_id("lei", "LEI_ACME");
        let b = make_id("lei", "LEI_ACME");
        assert!(identifiers_match(&a, &b));
    }

    #[test]
    fn different_scheme_rejects() {
        let a = make_id("lei", "VALUE");
        let b = make_id("duns", "VALUE");
        assert!(!identifiers_match(&a, &b));
    }

    #[test]
    fn internal_scheme_on_a_rejects() {
        let a = make_id("internal", "sap:1234");
        let b = make_id("lei", "sap:1234");
        assert!(!identifiers_match(&a, &b));
    }

    #[test]
    fn internal_scheme_on_b_rejects() {
        let a = make_id("lei", "VAL");
        let b = make_id("internal", "VAL");
        assert!(!identifiers_match(&a, &b));
    }

    #[test]
    fn both_internal_rejects() {
        let a = make_id("internal", "X");
        let b = make_id("internal", "X");
        assert!(!identifiers_match(&a, &b));
    }

    #[test]
    fn whitespace_trimmed_values_match() {
        let a = make_id("lei", " LEI_ACME ");
        let b = make_id("lei", "LEI_ACME");
        assert!(identifiers_match(&a, &b));
    }

    #[test]
    fn whitespace_trimmed_values_both_padded_match() {
        let a = make_id("duns", "  123456789  ");
        let b = make_id("duns", "  123456789  ");
        assert!(identifiers_match(&a, &b));
    }

    #[test]
    fn different_values_rejects() {
        let a = make_id("lei", "LEI_A");
        let b = make_id("lei", "LEI_B");
        assert!(!identifiers_match(&a, &b));
    }

    #[test]
    fn authority_case_insensitive_match() {
        let a = with_authority(make_id("nat-reg", "HRB12345"), "DE");
        let b = with_authority(make_id("nat-reg", "HRB12345"), "de");
        assert!(identifiers_match(&a, &b));
    }

    #[test]
    fn authority_case_insensitive_mixed_case() {
        let a = with_authority(make_id("nat-reg", "HRB12345"), "GLEIF");
        let b = with_authority(make_id("nat-reg", "HRB12345"), "gleif");
        assert!(identifiers_match(&a, &b));
    }

    #[test]
    fn authority_mismatch_rejects() {
        let a = with_authority(make_id("nat-reg", "HRB12345"), "DE");
        let b = with_authority(make_id("nat-reg", "HRB12345"), "FR");
        assert!(!identifiers_match(&a, &b));
    }

    #[test]
    fn one_has_authority_other_does_not_rejects() {
        let a = with_authority(make_id("nat-reg", "HRB12345"), "DE");
        let b = make_id("nat-reg", "HRB12345");
        assert!(!identifiers_match(&a, &b));
        // Symmetric
        assert!(!identifiers_match(&b, &a));
    }

    #[test]
    fn no_authority_on_either_matches_without_authority_check() {
        // Both lack authority → authority check is skipped.
        let a = make_id("lei", "SAME_VAL");
        let b = make_id("lei", "SAME_VAL");
        assert!(identifiers_match(&a, &b));
    }

    // --- temporal_compatible ------------------------------------------------

    #[test]
    fn both_missing_temporal_is_compatible() {
        let a = make_id("lei", "X");
        let b = make_id("lei", "X");
        // Both have no valid_from and no valid_to → compatible by default.
        assert!(temporal_compatible(&a, &b));
    }

    #[test]
    fn one_missing_temporal_is_compatible() {
        // One record has temporal info, the other does not → compatible.
        let a = with_valid_from(make_id("lei", "X"), "2020-01-01");
        let b = make_id("lei", "X");
        assert!(temporal_compatible(&a, &b));
        assert!(temporal_compatible(&b, &a));
    }

    #[test]
    fn overlapping_intervals_are_compatible() {
        // a: [2020-01-01, 2022-12-31], b: [2021-01-01, 2023-12-31] — overlap in 2021-2022
        let a = with_valid_to_date(
            with_valid_from(make_id("lei", "X"), "2020-01-01"),
            "2022-12-31",
        );
        let b = with_valid_to_date(
            with_valid_from(make_id("lei", "X"), "2021-01-01"),
            "2023-12-31",
        );
        assert!(temporal_compatible(&a, &b));
    }

    #[test]
    fn non_overlapping_intervals_are_incompatible() {
        // a ends 2019-12-31, b starts 2020-01-01 → disjoint
        let a = with_valid_to_date(
            with_valid_from(make_id("lei", "X"), "2018-01-01"),
            "2019-12-31",
        );
        let b = with_valid_from(make_id("lei", "X"), "2020-01-01");
        assert!(!temporal_compatible(&a, &b));
        assert!(!temporal_compatible(&b, &a));
    }

    #[test]
    fn adjacent_intervals_on_same_date_are_compatible() {
        // a: valid_to 2020-12-31, b: valid_from 2020-12-31 — same date → not strictly less than
        let a = with_valid_to_date(make_id("lei", "X"), "2020-12-31");
        let b = with_valid_from(make_id("lei", "X"), "2020-12-31");
        assert!(temporal_compatible(&a, &b));
    }

    #[test]
    fn valid_to_null_no_expiry_is_compatible() {
        // Explicit no-expiry on one side, dated start on the other
        let a = with_valid_to_null(with_valid_from(make_id("lei", "X"), "2020-01-01"));
        let b = with_valid_from(make_id("lei", "X"), "2025-01-01");
        assert!(temporal_compatible(&a, &b));
    }

    #[test]
    fn valid_to_null_both_sides_are_compatible() {
        let a = with_valid_to_null(with_valid_from(make_id("lei", "X"), "2020-01-01"));
        let b = with_valid_to_null(with_valid_from(make_id("lei", "X"), "2021-01-01"));
        assert!(temporal_compatible(&a, &b));
    }

    #[test]
    fn identifiers_match_temporal_incompatibility_rejects() {
        // Same scheme/value/authority but non-overlapping temporal windows → reject
        let a = with_valid_to_date(make_id("lei", "LEI_ACME"), "2019-12-31");
        let b = with_valid_from(make_id("lei", "LEI_ACME"), "2020-06-01");
        assert!(!identifiers_match(&a, &b));
    }

    // --- is_lei_annulled ----------------------------------------------------

    #[test]
    fn non_lei_scheme_not_annulled() {
        let id = make_id("duns", "123");
        assert!(!is_lei_annulled(&id));
    }

    #[test]
    fn lei_without_entity_status_not_annulled() {
        let id = make_id("lei", "SOME_LEI");
        assert!(!is_lei_annulled(&id));
    }

    #[test]
    fn lei_with_annulled_status_is_annulled() {
        let mut id = make_id("lei", "SOME_LEI");
        id.extra
            .insert("entity_status".to_owned(), json!("ANNULLED"));
        assert!(is_lei_annulled(&id));
    }

    #[test]
    fn lei_with_active_status_not_annulled() {
        let mut id = make_id("lei", "SOME_LEI");
        id.extra.insert("entity_status".to_owned(), json!("ACTIVE"));
        assert!(!is_lei_annulled(&id));
    }

    #[test]
    fn lei_with_lowercase_annulled_not_annulled() {
        // GLEIF uses uppercase; lowercase is not a match (case-sensitive).
        let mut id = make_id("lei", "SOME_LEI");
        id.extra
            .insert("entity_status".to_owned(), json!("annulled"));
        assert!(!is_lei_annulled(&id));
    }

    #[test]
    fn internal_scheme_is_not_annulled_check() {
        let mut id = make_id("internal", "LEI_VAL");
        id.extra
            .insert("entity_status".to_owned(), json!("ANNULLED"));
        assert!(!is_lei_annulled(&id), "non-lei scheme must return false");
    }

    // --- edge helper construction -------------------------------------------

    fn make_edge(id: &str, edge_type: EdgeTypeTag, source: &str, target: &str) -> Edge {
        use crate::newtypes::{EdgeId, NodeId};
        use crate::structures::EdgeProperties;
        Edge {
            id: EdgeId::try_from(id).expect("valid EdgeId"),
            edge_type,
            source: NodeId::try_from(source).expect("valid NodeId"),
            target: NodeId::try_from(target).expect("valid NodeId"),
            identifiers: None,
            properties: EdgeProperties::default(),
            extra: serde_json::Map::new(),
        }
    }

    fn with_edge_identifiers(mut edge: Edge, ids: Vec<Identifier>) -> Edge {
        edge.identifiers = Some(ids);
        edge
    }

    fn with_edge_properties(mut edge: Edge, props: crate::structures::EdgeProperties) -> Edge {
        edge.properties = props;
        edge
    }

    // --- edge_composite_key -------------------------------------------------

    #[test]
    fn composite_key_same_as_excluded() {
        let edge = make_edge("e1", EdgeTypeTag::Known(EdgeType::SameAs), "org-1", "org-2");
        assert!(
            edge_composite_key(0, 1, &edge).is_none(),
            "same_as edges must return None"
        );
    }

    #[test]
    fn composite_key_supplies_included() {
        let edge = make_edge(
            "e1",
            EdgeTypeTag::Known(EdgeType::Supplies),
            "org-1",
            "org-2",
        );
        let key = edge_composite_key(0, 1, &edge);
        assert!(key.is_some());
        let key = key.expect("should be Some");
        assert_eq!(key.source_rep, 0);
        assert_eq!(key.target_rep, 1);
        assert_eq!(key.edge_type, EdgeTypeTag::Known(EdgeType::Supplies));
    }

    #[test]
    fn composite_key_extension_type_included() {
        let edge = make_edge(
            "e1",
            EdgeTypeTag::Extension("com.acme.custom".to_owned()),
            "n-1",
            "n-2",
        );
        let key = edge_composite_key(5, 7, &edge);
        assert!(key.is_some());
        let key = key.expect("Some");
        assert_eq!(key.source_rep, 5);
        assert_eq!(key.target_rep, 7);
        assert_eq!(
            key.edge_type,
            EdgeTypeTag::Extension("com.acme.custom".to_owned())
        );
    }

    #[test]
    fn composite_key_different_reps_produce_different_keys() {
        let edge = make_edge("e1", EdgeTypeTag::Known(EdgeType::Ownership), "s", "t");
        let key_01 = edge_composite_key(0, 1, &edge).expect("Some");
        let key_02 = edge_composite_key(0, 2, &edge).expect("Some");
        assert_ne!(key_01, key_02, "different target_rep must differ");
    }

    // --- build_edge_candidate_index -----------------------------------------

    #[test]
    fn candidate_index_empty_edges() {
        let edges: Vec<Edge> = vec![];
        let index = build_edge_candidate_index(&edges, |_| None, |x| x);
        assert!(index.is_empty());
    }

    #[test]
    fn candidate_index_same_as_edges_excluded() {
        // All edges are same_as — index should be empty.
        let edges = vec![make_edge(
            "e1",
            EdgeTypeTag::Known(EdgeType::SameAs),
            "org-1",
            "org-2",
        )];
        // node_ordinal resolves every node id to ordinal 0 or 1
        let index = build_edge_candidate_index(
            &edges,
            |id| if id == "org-1" { Some(0) } else { Some(1) },
            |x| x,
        );
        assert!(index.is_empty());
    }

    #[test]
    fn candidate_index_dangling_source_skipped() {
        let edges = vec![make_edge(
            "e1",
            EdgeTypeTag::Known(EdgeType::Supplies),
            "missing-node",
            "org-2",
        )];
        let index = build_edge_candidate_index(
            &edges,
            |id| if id == "org-2" { Some(1) } else { None },
            |x| x,
        );
        assert!(index.is_empty(), "dangling source should skip the edge");
    }

    #[test]
    fn candidate_index_dangling_target_skipped() {
        let edges = vec![make_edge(
            "e1",
            EdgeTypeTag::Known(EdgeType::Supplies),
            "org-1",
            "missing-node",
        )];
        let index = build_edge_candidate_index(
            &edges,
            |id| if id == "org-1" { Some(0) } else { None },
            |x| x,
        );
        assert!(index.is_empty(), "dangling target should skip the edge");
    }

    #[test]
    fn candidate_index_two_edges_same_bucket() {
        // Two supplies edges from resolved endpoints (0→1) go in the same bucket.
        let edges = vec![
            make_edge(
                "e1",
                EdgeTypeTag::Known(EdgeType::Supplies),
                "org-1",
                "org-2",
            ),
            make_edge(
                "e2",
                EdgeTypeTag::Known(EdgeType::Supplies),
                "org-1",
                "org-2",
            ),
        ];
        let index = build_edge_candidate_index(
            &edges,
            |id| match id {
                "org-1" => Some(0),
                "org-2" => Some(1),
                _ => None,
            },
            |x| x,
        );
        assert_eq!(index.len(), 1, "both edges share one composite key");
        let bucket = index.values().next().expect("one bucket");
        let mut bucket = bucket.clone();
        bucket.sort_unstable();
        assert_eq!(bucket, vec![0usize, 1]);
    }

    #[test]
    fn candidate_index_different_type_different_bucket() {
        let edges = vec![
            make_edge(
                "e1",
                EdgeTypeTag::Known(EdgeType::Supplies),
                "org-1",
                "org-2",
            ),
            make_edge(
                "e2",
                EdgeTypeTag::Known(EdgeType::Ownership),
                "org-1",
                "org-2",
            ),
        ];
        let index = build_edge_candidate_index(
            &edges,
            |id| match id {
                "org-1" => Some(0),
                "org-2" => Some(1),
                _ => None,
            },
            |x| x,
        );
        assert_eq!(index.len(), 2, "different types → different buckets");
    }

    #[test]
    fn candidate_index_union_find_rep_used() {
        // org-1 and org-3 are unioned → both should resolve to rep 0.
        // An edge from org-1 and an edge from org-3 to org-2 share a bucket.
        let edges = vec![
            make_edge(
                "e1",
                EdgeTypeTag::Known(EdgeType::Supplies),
                "org-1",
                "org-2",
            ),
            make_edge(
                "e2",
                EdgeTypeTag::Known(EdgeType::Supplies),
                "org-3",
                "org-2",
            ),
        ];
        let index = build_edge_candidate_index(
            &edges,
            |id| match id {
                "org-1" => Some(0),
                "org-2" => Some(1),
                "org-3" => Some(2),
                _ => None,
            },
            // org-3 (ordinal 2) is unioned with org-1 (ordinal 0) → rep is 0.
            |x| if x == 2 { 0 } else { x },
        );
        assert_eq!(index.len(), 1, "merged source nodes → same bucket");
        let bucket = index.values().next().expect("one bucket");
        let mut bucket = bucket.clone();
        bucket.sort_unstable();
        assert_eq!(bucket, vec![0usize, 1]);
    }

    // --- edge_identity_properties_match ------------------------------------

    #[test]
    fn ownership_same_percentage_and_direct_matches() {
        let mut a = crate::structures::EdgeProperties::default();
        a.percentage = Some(51.0);
        a.direct = Some(true);
        let b = a.clone();
        assert!(edge_identity_properties_match(
            &EdgeTypeTag::Known(EdgeType::Ownership),
            &a,
            &b
        ));
    }

    #[test]
    fn ownership_different_percentage_no_match() {
        let mut a = crate::structures::EdgeProperties::default();
        a.percentage = Some(51.0);
        a.direct = Some(true);
        let mut b = crate::structures::EdgeProperties::default();
        b.percentage = Some(49.0);
        b.direct = Some(true);
        assert!(!edge_identity_properties_match(
            &EdgeTypeTag::Known(EdgeType::Ownership),
            &a,
            &b
        ));
    }

    #[test]
    fn ownership_different_direct_no_match() {
        let mut a = crate::structures::EdgeProperties::default();
        a.percentage = Some(51.0);
        a.direct = Some(true);
        let mut b = crate::structures::EdgeProperties::default();
        b.percentage = Some(51.0);
        b.direct = Some(false);
        assert!(!edge_identity_properties_match(
            &EdgeTypeTag::Known(EdgeType::Ownership),
            &a,
            &b
        ));
    }

    #[test]
    fn ownership_both_none_matches() {
        // Both percentage and direct absent → match (same "unspecified" identity)
        let a = crate::structures::EdgeProperties::default();
        let b = crate::structures::EdgeProperties::default();
        assert!(edge_identity_properties_match(
            &EdgeTypeTag::Known(EdgeType::Ownership),
            &a,
            &b
        ));
    }

    #[test]
    fn operational_control_same_control_type_matches() {
        let mut a = crate::structures::EdgeProperties::default();
        a.control_type = Some(json!("franchise"));
        let mut b = crate::structures::EdgeProperties::default();
        b.control_type = Some(json!("franchise"));
        assert!(edge_identity_properties_match(
            &EdgeTypeTag::Known(EdgeType::OperationalControl),
            &a,
            &b
        ));
    }

    #[test]
    fn operational_control_different_control_type_no_match() {
        let mut a = crate::structures::EdgeProperties::default();
        a.control_type = Some(json!("franchise"));
        let mut b = crate::structures::EdgeProperties::default();
        b.control_type = Some(json!("management"));
        assert!(!edge_identity_properties_match(
            &EdgeTypeTag::Known(EdgeType::OperationalControl),
            &a,
            &b
        ));
    }

    #[test]
    fn legal_parentage_same_consolidation_basis_matches() {
        use crate::enums::ConsolidationBasis;
        let mut a = crate::structures::EdgeProperties::default();
        a.consolidation_basis = Some(ConsolidationBasis::Ifrs10);
        let b = a.clone();
        assert!(edge_identity_properties_match(
            &EdgeTypeTag::Known(EdgeType::LegalParentage),
            &a,
            &b
        ));
    }

    #[test]
    fn legal_parentage_different_consolidation_basis_no_match() {
        use crate::enums::ConsolidationBasis;
        let mut a = crate::structures::EdgeProperties::default();
        a.consolidation_basis = Some(ConsolidationBasis::Ifrs10);
        let mut b = crate::structures::EdgeProperties::default();
        b.consolidation_basis = Some(ConsolidationBasis::UsGaapAsc810);
        assert!(!edge_identity_properties_match(
            &EdgeTypeTag::Known(EdgeType::LegalParentage),
            &a,
            &b
        ));
    }

    #[test]
    fn former_identity_same_event_and_date_matches() {
        use crate::enums::EventType;
        let mut a = crate::structures::EdgeProperties::default();
        a.event_type = Some(EventType::Merger);
        a.effective_date = Some(CalendarDate::try_from("2022-07-01").expect("date"));
        let b = a.clone();
        assert!(edge_identity_properties_match(
            &EdgeTypeTag::Known(EdgeType::FormerIdentity),
            &a,
            &b
        ));
    }

    #[test]
    fn former_identity_different_event_type_no_match() {
        use crate::enums::EventType;
        let mut a = crate::structures::EdgeProperties::default();
        a.event_type = Some(EventType::Merger);
        a.effective_date = Some(CalendarDate::try_from("2022-07-01").expect("date"));
        let mut b = crate::structures::EdgeProperties::default();
        b.event_type = Some(EventType::Acquisition);
        b.effective_date = Some(CalendarDate::try_from("2022-07-01").expect("date"));
        assert!(!edge_identity_properties_match(
            &EdgeTypeTag::Known(EdgeType::FormerIdentity),
            &a,
            &b
        ));
    }

    #[test]
    fn beneficial_ownership_same_control_and_pct_matches() {
        let mut a = crate::structures::EdgeProperties::default();
        a.control_type = Some(json!("management"));
        a.percentage = Some(25.0);
        let b = a.clone();
        assert!(edge_identity_properties_match(
            &EdgeTypeTag::Known(EdgeType::BeneficialOwnership),
            &a,
            &b
        ));
    }

    #[test]
    fn beneficial_ownership_different_pct_no_match() {
        let mut a = crate::structures::EdgeProperties::default();
        a.control_type = Some(json!("management"));
        a.percentage = Some(25.0);
        let mut b = crate::structures::EdgeProperties::default();
        b.control_type = Some(json!("management"));
        b.percentage = Some(30.0);
        assert!(!edge_identity_properties_match(
            &EdgeTypeTag::Known(EdgeType::BeneficialOwnership),
            &a,
            &b
        ));
    }

    #[test]
    fn supplies_same_commodity_and_contract_matches() {
        let mut a = crate::structures::EdgeProperties::default();
        a.commodity = Some("7318.15".to_owned());
        a.contract_ref = Some("CTR-001".to_owned());
        let b = a.clone();
        assert!(edge_identity_properties_match(
            &EdgeTypeTag::Known(EdgeType::Supplies),
            &a,
            &b
        ));
    }

    #[test]
    fn supplies_different_contract_ref_no_match() {
        let mut a = crate::structures::EdgeProperties::default();
        a.commodity = Some("7318.15".to_owned());
        a.contract_ref = Some("CTR-001".to_owned());
        let mut b = crate::structures::EdgeProperties::default();
        b.commodity = Some("7318.15".to_owned());
        b.contract_ref = Some("CTR-002".to_owned());
        assert!(!edge_identity_properties_match(
            &EdgeTypeTag::Known(EdgeType::Supplies),
            &a,
            &b
        ));
    }

    #[test]
    fn subcontracts_commodity_and_contract_ref_matches() {
        let mut a = crate::structures::EdgeProperties::default();
        a.commodity = Some("8471".to_owned());
        a.contract_ref = Some("SC-100".to_owned());
        let b = a.clone();
        assert!(edge_identity_properties_match(
            &EdgeTypeTag::Known(EdgeType::Subcontracts),
            &a,
            &b
        ));
    }

    #[test]
    fn tolls_same_commodity_matches() {
        let mut a = crate::structures::EdgeProperties::default();
        a.commodity = Some("aluminum".to_owned());
        let b = a.clone();
        assert!(edge_identity_properties_match(
            &EdgeTypeTag::Known(EdgeType::Tolls),
            &a,
            &b
        ));
    }

    #[test]
    fn tolls_different_commodity_no_match() {
        let mut a = crate::structures::EdgeProperties::default();
        a.commodity = Some("aluminum".to_owned());
        let mut b = crate::structures::EdgeProperties::default();
        b.commodity = Some("steel".to_owned());
        assert!(!edge_identity_properties_match(
            &EdgeTypeTag::Known(EdgeType::Tolls),
            &a,
            &b
        ));
    }

    #[test]
    fn distributes_same_service_type_matches() {
        use crate::enums::ServiceType;
        let mut a = crate::structures::EdgeProperties::default();
        a.service_type = Some(ServiceType::Transport);
        let b = a.clone();
        assert!(edge_identity_properties_match(
            &EdgeTypeTag::Known(EdgeType::Distributes),
            &a,
            &b
        ));
    }

    #[test]
    fn distributes_different_service_type_no_match() {
        use crate::enums::ServiceType;
        let mut a = crate::structures::EdgeProperties::default();
        a.service_type = Some(ServiceType::Transport);
        let mut b = crate::structures::EdgeProperties::default();
        b.service_type = Some(ServiceType::Warehousing);
        assert!(!edge_identity_properties_match(
            &EdgeTypeTag::Known(EdgeType::Distributes),
            &a,
            &b
        ));
    }

    #[test]
    fn brokers_same_commodity_matches() {
        let mut a = crate::structures::EdgeProperties::default();
        a.commodity = Some("crude_oil".to_owned());
        let b = a.clone();
        assert!(edge_identity_properties_match(
            &EdgeTypeTag::Known(EdgeType::Brokers),
            &a,
            &b
        ));
    }

    #[test]
    fn operates_always_matches() {
        // type + endpoints suffice
        assert!(edge_identity_properties_match(
            &EdgeTypeTag::Known(EdgeType::Operates),
            &crate::structures::EdgeProperties::default(),
            &crate::structures::EdgeProperties::default()
        ));
    }

    #[test]
    fn produces_always_matches() {
        assert!(edge_identity_properties_match(
            &EdgeTypeTag::Known(EdgeType::Produces),
            &crate::structures::EdgeProperties::default(),
            &crate::structures::EdgeProperties::default()
        ));
    }

    #[test]
    fn composed_of_always_matches() {
        assert!(edge_identity_properties_match(
            &EdgeTypeTag::Known(EdgeType::ComposedOf),
            &crate::structures::EdgeProperties::default(),
            &crate::structures::EdgeProperties::default()
        ));
    }

    #[test]
    fn sells_to_same_commodity_and_contract_matches() {
        let mut a = crate::structures::EdgeProperties::default();
        a.commodity = Some("widgets".to_owned());
        a.contract_ref = Some("PO-999".to_owned());
        let b = a.clone();
        assert!(edge_identity_properties_match(
            &EdgeTypeTag::Known(EdgeType::SellsTo),
            &a,
            &b
        ));
    }

    #[test]
    fn attested_by_same_scope_matches() {
        let mut a = crate::structures::EdgeProperties::default();
        a.scope = Some("full site".to_owned());
        let b = a.clone();
        assert!(edge_identity_properties_match(
            &EdgeTypeTag::Known(EdgeType::AttestedBy),
            &a,
            &b
        ));
    }

    #[test]
    fn attested_by_different_scope_no_match() {
        let mut a = crate::structures::EdgeProperties::default();
        a.scope = Some("full site".to_owned());
        let mut b = crate::structures::EdgeProperties::default();
        b.scope = Some("partial".to_owned());
        assert!(!edge_identity_properties_match(
            &EdgeTypeTag::Known(EdgeType::AttestedBy),
            &a,
            &b
        ));
    }

    #[test]
    fn same_as_identity_always_false() {
        // same_as is never matched — property predicate returns false.
        assert!(!edge_identity_properties_match(
            &EdgeTypeTag::Known(EdgeType::SameAs),
            &crate::structures::EdgeProperties::default(),
            &crate::structures::EdgeProperties::default()
        ));
    }

    #[test]
    fn extension_type_always_matches_properties() {
        // Extension types: type + endpoints suffice.
        assert!(edge_identity_properties_match(
            &EdgeTypeTag::Extension("com.acme.custom".to_owned()),
            &crate::structures::EdgeProperties::default(),
            &crate::structures::EdgeProperties::default()
        ));
    }

    // --- edges_match --------------------------------------------------------

    #[test]
    fn edges_match_same_as_is_never_a_candidate() {
        let a = make_edge("e1", EdgeTypeTag::Known(EdgeType::SameAs), "s", "t");
        let b = make_edge("e2", EdgeTypeTag::Known(EdgeType::SameAs), "s", "t");
        assert!(!edges_match(0, 1, 0, 1, &a, &b));
    }

    #[test]
    fn edges_match_same_as_on_a_is_never_a_candidate() {
        let a = make_edge("e1", EdgeTypeTag::Known(EdgeType::SameAs), "s", "t");
        let b = make_edge("e2", EdgeTypeTag::Known(EdgeType::Supplies), "s", "t");
        assert!(!edges_match(0, 1, 0, 1, &a, &b));
    }

    #[test]
    fn edges_match_different_source_rep_no_match() {
        let a = make_edge("e1", EdgeTypeTag::Known(EdgeType::Supplies), "s", "t");
        let b = make_edge("e2", EdgeTypeTag::Known(EdgeType::Supplies), "s", "t");
        // source_rep_a=0 vs source_rep_b=1 — different
        assert!(!edges_match(0, 1, 1, 1, &a, &b));
    }

    #[test]
    fn edges_match_different_target_rep_no_match() {
        let a = make_edge("e1", EdgeTypeTag::Known(EdgeType::Supplies), "s", "t");
        let b = make_edge("e2", EdgeTypeTag::Known(EdgeType::Supplies), "s", "t");
        assert!(!edges_match(0, 1, 0, 2, &a, &b));
    }

    #[test]
    fn edges_match_different_type_no_match() {
        let a = make_edge("e1", EdgeTypeTag::Known(EdgeType::Supplies), "s", "t");
        let b = make_edge("e2", EdgeTypeTag::Known(EdgeType::Ownership), "s", "t");
        assert!(!edges_match(0, 1, 0, 1, &a, &b));
    }

    #[test]
    fn edges_match_by_shared_external_identifier() {
        let shared_id = make_id("lei", "EDGE_LEI_123");
        let a = with_edge_identifiers(
            make_edge("e1", EdgeTypeTag::Known(EdgeType::Supplies), "s", "t"),
            vec![shared_id.clone()],
        );
        let b = with_edge_identifiers(
            make_edge("e2", EdgeTypeTag::Known(EdgeType::Supplies), "s", "t"),
            vec![shared_id],
        );
        assert!(edges_match(0, 1, 0, 1, &a, &b));
    }

    #[test]
    fn edges_match_different_external_identifiers_no_match() {
        let a = with_edge_identifiers(
            make_edge("e1", EdgeTypeTag::Known(EdgeType::Supplies), "s", "t"),
            vec![make_id("lei", "ID_A")],
        );
        let b = with_edge_identifiers(
            make_edge("e2", EdgeTypeTag::Known(EdgeType::Supplies), "s", "t"),
            vec![make_id("lei", "ID_B")],
        );
        // Both have external identifiers but they don't match.
        assert!(!edges_match(0, 1, 0, 1, &a, &b));
    }

    #[test]
    fn edges_match_a_has_external_b_does_not_no_match() {
        // a has external identifiers but b has none → no match (spec says when
        // at least one side has external IDs, identifier matching is the gate).
        let a = with_edge_identifiers(
            make_edge("e1", EdgeTypeTag::Known(EdgeType::Supplies), "s", "t"),
            vec![make_id("lei", "EDGE_ID")],
        );
        let b = make_edge("e2", EdgeTypeTag::Known(EdgeType::Supplies), "s", "t");
        assert!(!edges_match(0, 1, 0, 1, &a, &b));
    }

    #[test]
    fn edges_match_internal_identifier_treated_as_no_external() {
        // Internal identifiers are excluded from the external-id check.
        // Both edges only have internal IDs → fall back to property matching.
        let internal_id_a = make_id("internal", "sap:A");
        let internal_id_b = make_id("internal", "sap:B");
        let mut props = crate::structures::EdgeProperties::default();
        props.commodity = Some("steel".to_owned());
        props.contract_ref = Some("CTR-001".to_owned());
        let a = with_edge_properties(
            with_edge_identifiers(
                make_edge("e1", EdgeTypeTag::Known(EdgeType::Supplies), "s", "t"),
                vec![internal_id_a],
            ),
            props.clone(),
        );
        let b = with_edge_properties(
            with_edge_identifiers(
                make_edge("e2", EdgeTypeTag::Known(EdgeType::Supplies), "s", "t"),
                vec![internal_id_b],
            ),
            props,
        );
        assert!(edges_match(0, 1, 0, 1, &a, &b));
    }

    #[test]
    fn edges_match_no_identifiers_falls_back_to_properties() {
        // No identifiers on either edge → property-table match.
        let mut props = crate::structures::EdgeProperties::default();
        props.commodity = Some("7318.15".to_owned());
        props.contract_ref = None;
        let a = with_edge_properties(
            make_edge("e1", EdgeTypeTag::Known(EdgeType::Supplies), "s", "t"),
            props.clone(),
        );
        let b = with_edge_properties(
            make_edge("e2", EdgeTypeTag::Known(EdgeType::Supplies), "s", "t"),
            props,
        );
        assert!(edges_match(0, 1, 0, 1, &a, &b));
    }

    #[test]
    fn edges_match_no_identifiers_property_mismatch_no_match() {
        let mut props_a = crate::structures::EdgeProperties::default();
        props_a.commodity = Some("7318.15".to_owned());
        let mut props_b = crate::structures::EdgeProperties::default();
        props_b.commodity = Some("8471".to_owned());
        let a = with_edge_properties(
            make_edge("e1", EdgeTypeTag::Known(EdgeType::Supplies), "s", "t"),
            props_a,
        );
        let b = with_edge_properties(
            make_edge("e2", EdgeTypeTag::Known(EdgeType::Supplies), "s", "t"),
            props_b,
        );
        assert!(!edges_match(0, 1, 0, 1, &a, &b));
    }

    #[test]
    fn edges_match_operates_no_identifiers_always_matches() {
        // operates: type + endpoints suffice; no properties needed.
        let a = make_edge("e1", EdgeTypeTag::Known(EdgeType::Operates), "s", "t");
        let b = make_edge("e2", EdgeTypeTag::Known(EdgeType::Operates), "s", "t");
        assert!(edges_match(0, 1, 0, 1, &a, &b));
    }

    #[test]
    fn edges_match_symmetry() {
        // edges_match must be symmetric.
        let mut props = crate::structures::EdgeProperties::default();
        props.commodity = Some("cotton".to_owned());
        let a = with_edge_properties(
            make_edge("e1", EdgeTypeTag::Known(EdgeType::Supplies), "s", "t"),
            props.clone(),
        );
        let b = with_edge_properties(
            make_edge("e2", EdgeTypeTag::Known(EdgeType::Supplies), "s", "t"),
            props,
        );
        let fwd = edges_match(0, 1, 0, 1, &a, &b);
        let rev = edges_match(0, 1, 0, 1, &b, &a);
        assert_eq!(fwd, rev, "edges_match must be symmetric");
    }

    #[test]
    fn edges_match_shared_identifier_with_temporal_compatibility() {
        // Two edges share an identifier that has overlapping valid windows → match.
        let id_a = with_valid_from(make_id("lei", "EDGE_LEI"), "2022-01-01");
        let id_b = with_valid_from(make_id("lei", "EDGE_LEI"), "2022-06-01");
        let a = with_edge_identifiers(
            make_edge("e1", EdgeTypeTag::Known(EdgeType::Ownership), "s", "t"),
            vec![id_a],
        );
        let b = with_edge_identifiers(
            make_edge("e2", EdgeTypeTag::Known(EdgeType::Ownership), "s", "t"),
            vec![id_b],
        );
        assert!(edges_match(0, 1, 0, 1, &a, &b));
    }

    #[test]
    fn edges_match_shared_identifier_temporal_incompatibility_no_match() {
        // Same identifier but non-overlapping valid windows → no match.
        let id_a = with_valid_to_date(make_id("lei", "EDGE_LEI"), "2019-12-31");
        let id_b = with_valid_from(make_id("lei", "EDGE_LEI"), "2020-06-01");
        let a = with_edge_identifiers(
            make_edge("e1", EdgeTypeTag::Known(EdgeType::Ownership), "s", "t"),
            vec![id_a],
        );
        let b = with_edge_identifiers(
            make_edge("e2", EdgeTypeTag::Known(EdgeType::Ownership), "s", "t"),
            vec![id_b],
        );
        assert!(!edges_match(0, 1, 0, 1, &a, &b));
    }
}
