/// Top-level OMTSF file representation.
///
/// [`OmtsFile`] is the root type for a serialised/deserialised OMTSF snapshot.
/// It corresponds to the JSON top-level object defined in SPEC-001 Section 2
/// and data-model.md Section 2.
///
/// # Field ordering
///
/// `serde_json` serialises struct fields in declaration order.  The spec
/// (data-model.md Section 8.5) requires `omtsf_version` to be the first key in
/// the JSON object.  Do **not** reorder the fields without considering that
/// constraint.
///
/// # Unknown field preservation
///
/// The `extra` catch-all (`#[serde(flatten)]`) absorbs every JSON key that is
/// not explicitly declared on the struct.  This enables forward-compatible
/// consumers: a file written by a newer spec version will round-trip without
/// data loss through an older `omtsf-core`.  **Never** add
/// `#[serde(deny_unknown_fields)]` here or on any child struct.
use serde::{Deserialize, Serialize};

use crate::dynvalue::DynMap;
use crate::enums::DisclosureScope;
use crate::newtypes::{CalendarDate, FileSalt, NodeId, SemVer};
use crate::structures::{Edge, Node};

/// The top-level OMTSF file representation.
///
/// Deserialise from JSON with [`serde_json::from_str`] /
/// [`serde_json::from_reader`]; serialise back with [`serde_json::to_string`]
/// etc.  The round-trip preserves all unknown fields via [`OmtsFile::extra`].
///
/// # Required fields
///
/// - `omtsf_version` — specification version this file conforms to
/// - `snapshot_date` — ISO 8601 date of the snapshot
/// - `file_salt` — 64 lowercase hex characters ensuring global uniqueness
/// - `nodes` — graph node list (may be empty)
/// - `edges` — graph edge list (may be empty)
///
/// # Optional fields
///
/// All other fields may be omitted and will be absent from the serialised
/// output when `None` (via `#[serde(skip_serializing_if = "Option::is_none")]`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct OmtsFile {
    /// OMTSF specification version this file conforms to (e.g. `"1.0.0"`).
    ///
    /// MUST be the first key in the serialised JSON object (data-model.md
    /// Section 8.5).  Field declaration order here is therefore load-bearing.
    pub omtsf_version: SemVer,

    /// ISO 8601 calendar date (`YYYY-MM-DD`) on which this snapshot was taken.
    pub snapshot_date: CalendarDate,

    /// Exactly 64 lowercase hexadecimal characters providing global uniqueness
    /// and supporting privacy-preserving hash derivation (SPEC-001 Section 2).
    pub file_salt: FileSalt,

    /// Disclosure scope classification for this file (SPEC-004 Section 3).
    ///
    /// Absent when not explicitly declared.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disclosure_scope: Option<DisclosureScope>,

    /// Content-addressable reference to the immediately preceding snapshot.
    ///
    /// Typically a hex digest or URI.  Absent for the initial snapshot.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_snapshot_ref: Option<String>,

    /// Monotonically increasing snapshot sequence number within a series.
    ///
    /// `0` or absent for the first snapshot in a series.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot_sequence: Option<u64>,

    /// [`NodeId`] of the organisation that produced this file.
    ///
    /// Must reference a node present in [`OmtsFile::nodes`] when set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reporting_entity: Option<NodeId>,

    /// Ordered list of all supply-chain graph nodes.
    pub nodes: Vec<Node>,

    /// Ordered list of all supply-chain graph edges.
    pub edges: Vec<Edge>,

    /// Unknown top-level JSON fields preserved for round-trip fidelity.
    ///
    /// Any key not explicitly declared above is collected here during
    /// deserialisation and re-emitted last during serialisation.
    /// This is the primary mechanism for forward compatibility with future
    /// spec versions (SPEC-001 Section 2.2 / data-model.md Section 8.2).
    #[serde(flatten)]
    pub extra: DynMap,
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use serde_json::json;
    use std::collections::BTreeMap;

    use super::*;
    use crate::dynvalue::DynValue;
    use crate::enums::{EdgeType, EdgeTypeTag, NodeType, NodeTypeTag};
    use crate::newtypes::EdgeId;
    use crate::structures::EdgeProperties;

    /// A valid 64-char lowercase hex string for use as `file_salt`.
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

    /// Serialise and immediately re-parse, asserting structural equality.
    fn round_trip(f: &OmtsFile) -> OmtsFile {
        let json = serde_json::to_string(f).expect("serialize");
        let back: OmtsFile = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(*f, back, "round-trip mismatch:\n{json}");
        back
    }

    /// Build a minimal valid [`OmtsFile`] (required fields only, no nodes/edges).
    fn minimal_file() -> OmtsFile {
        OmtsFile {
            omtsf_version: semver("1.0.0"),
            snapshot_date: date("2026-02-19"),
            file_salt: file_salt(SALT),
            disclosure_scope: None,
            previous_snapshot_ref: None,
            snapshot_sequence: None,
            reporting_entity: None,
            nodes: vec![],
            edges: vec![],
            extra: BTreeMap::new(),
        }
    }

    /// Build a minimal organisation [`Node`] for fixture use.
    fn org_node(id: &str, name: &str) -> Node {
        Node {
            id: node_id(id),
            node_type: NodeTypeTag::Known(NodeType::Organization),
            identifiers: None,
            data_quality: None,
            labels: None,
            name: Some(name.to_owned()),
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

    /// Build a minimal [`Edge`] for fixture use.
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

    /// Parse a minimal JSON fixture (no optional fields, empty arrays).
    #[test]
    fn omts_file_minimal_parse() {
        let json = format!(
            r#"{{
                "omtsf_version": "1.0.0",
                "snapshot_date": "2026-02-19",
                "file_salt": "{SALT}",
                "nodes": [],
                "edges": []
            }}"#
        );
        let f: OmtsFile = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(f.omtsf_version, semver("1.0.0"));
        assert_eq!(f.snapshot_date, date("2026-02-19"));
        assert_eq!(&*f.file_salt, SALT);
        assert!(f.disclosure_scope.is_none());
        assert!(f.previous_snapshot_ref.is_none());
        assert!(f.snapshot_sequence.is_none());
        assert!(f.reporting_entity.is_none());
        assert!(f.nodes.is_empty());
        assert!(f.edges.is_empty());
        assert!(f.extra.is_empty());
    }

    /// Minimal file round-trips without data loss.
    #[test]
    fn omts_file_minimal_round_trip() {
        round_trip(&minimal_file());
    }

    /// Parse a fixture that sets every optional top-level field.
    #[test]
    fn omts_file_full_optional_fields() {
        let json = format!(
            r#"{{
                "omtsf_version": "1.2.0",
                "snapshot_date": "2026-01-15",
                "file_salt": "{SALT}",
                "disclosure_scope": "partner",
                "previous_snapshot_ref": "sha256:abcdef1234567890",
                "snapshot_sequence": 3,
                "reporting_entity": "org-acme",
                "nodes": [{{"id":"org-acme","type":"organization"}}],
                "edges": []
            }}"#
        );
        let f: OmtsFile = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(f.omtsf_version, semver("1.2.0"));
        assert_eq!(f.disclosure_scope, Some(DisclosureScope::Partner));
        assert_eq!(
            f.previous_snapshot_ref.as_deref(),
            Some("sha256:abcdef1234567890")
        );
        assert_eq!(f.snapshot_sequence, Some(3_u64));
        assert_eq!(f.reporting_entity, Some(node_id("org-acme")));
        assert_eq!(f.nodes.len(), 1);
        assert!(f.edges.is_empty());
    }

    /// Full optional-field file round-trips without data loss.
    #[test]
    fn omts_file_full_optional_fields_round_trip() {
        let f = OmtsFile {
            omtsf_version: semver("1.2.0"),
            snapshot_date: date("2026-01-15"),
            file_salt: file_salt(SALT),
            disclosure_scope: Some(DisclosureScope::Partner),
            previous_snapshot_ref: Some("sha256:abcdef1234567890".to_owned()),
            snapshot_sequence: Some(3),
            reporting_entity: Some(node_id("org-acme")),
            nodes: vec![org_node("org-acme", "Acme Corp")],
            edges: vec![],
            extra: BTreeMap::new(),
        };
        round_trip(&f);
    }

    /// Parse a complete JSON fixture with nodes and edges, re-serialise, and
    /// compare the re-parsed result for structural equality.
    #[test]
    fn omts_file_complete_round_trip() {
        let json = format!(
            r#"{{
                "omtsf_version": "1.0.0",
                "snapshot_date": "2026-02-19",
                "file_salt": "{SALT}",
                "disclosure_scope": "internal",
                "snapshot_sequence": 1,
                "reporting_entity": "org-1",
                "nodes": [
                    {{"id": "org-1", "type": "organization", "name": "Acme Corp"}},
                    {{"id": "org-2", "type": "organization", "name": "Beta Ltd"}},
                    {{"id": "fac-1", "type": "facility", "operator": "org-1", "address": "1 Main St"}}
                ],
                "edges": [
                    {{
                        "id": "e-1",
                        "type": "supplies",
                        "source": "org-2",
                        "target": "org-1",
                        "properties": {{"tier": 1}}
                    }}
                ]
            }}"#
        );
        let original: OmtsFile = serde_json::from_str(&json).expect("deserialize");
        let serialised = serde_json::to_string(&original).expect("serialize");
        let reparsed: OmtsFile = serde_json::from_str(&serialised).expect("re-deserialize");

        assert_eq!(original, reparsed);
        assert_eq!(reparsed.nodes.len(), 3);
        assert_eq!(reparsed.edges.len(), 1);
        assert_eq!(reparsed.disclosure_scope, Some(DisclosureScope::Internal));
    }

    /// Unknown top-level fields survive a deserialise → serialise cycle.
    #[test]
    fn omts_file_unknown_fields_preserved() {
        let json = format!(
            r#"{{
                "omtsf_version": "1.0.0",
                "snapshot_date": "2026-02-19",
                "file_salt": "{SALT}",
                "nodes": [],
                "edges": [],
                "x_custom_field": "hello",
                "x_version_hint": 42
            }}"#
        );
        let f: OmtsFile = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(
            f.extra.get("x_custom_field").and_then(|v| v.as_str()),
            Some("hello")
        );
        assert_eq!(
            f.extra.get("x_version_hint").and_then(DynValue::as_u64),
            Some(42)
        );

        let serialised = serde_json::to_string(&f).expect("serialize");
        assert!(
            serialised.contains("x_custom_field"),
            "unknown field missing from serialised output"
        );
        assert!(
            serialised.contains("x_version_hint"),
            "unknown field missing from serialised output"
        );

        round_trip(&f);
    }

    /// Unknown fields inside nested nodes are also preserved.
    #[test]
    fn omts_file_unknown_fields_in_nested_nodes_preserved() {
        let json = format!(
            r#"{{
                "omtsf_version": "1.0.0",
                "snapshot_date": "2026-02-19",
                "file_salt": "{SALT}",
                "nodes": [{{"id":"n1","type":"organization","x_nested":"value"}}],
                "edges": []
            }}"#
        );
        let f: OmtsFile = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(
            f.nodes[0].extra.get("x_nested").and_then(|v| v.as_str()),
            Some("value")
        );
        round_trip(&f);
    }

    /// The first JSON key in the serialised output must be `omtsf_version`.
    ///
    /// `serde_json` preserves struct field declaration order.  This test
    /// validates that the contract from data-model.md Section 8.5 is upheld.
    #[test]
    fn omts_file_omtsf_version_is_first_key() {
        let f = minimal_file();
        let json = serde_json::to_string(&f).expect("serialize");

        let first_key_pos = json.find("omtsf_version").expect("omtsf_version in output");
        for other_key in &[
            "snapshot_date",
            "file_salt",
            "disclosure_scope",
            "nodes",
            "edges",
        ] {
            if let Some(pos) = json.find(other_key) {
                assert!(
                    pos > first_key_pos,
                    "`omtsf_version` must precede `{other_key}` in serialised output"
                );
            }
        }
    }

    /// Verify the exact key order of all required fields in the serialised output.
    #[test]
    fn omts_file_field_order_required_fields() {
        let f = minimal_file();
        let json = serde_json::to_string(&f).expect("serialize");

        let pos_version = json.find("omtsf_version").expect("omtsf_version");
        let pos_date = json.find("snapshot_date").expect("snapshot_date");
        let pos_salt = json.find("file_salt").expect("file_salt");
        let pos_nodes = json.find("\"nodes\"").expect("nodes");
        let pos_edges = json.find("\"edges\"").expect("edges");

        assert!(
            pos_version < pos_date,
            "omtsf_version must precede snapshot_date"
        );
        assert!(pos_date < pos_salt, "snapshot_date must precede file_salt");
        assert!(pos_salt < pos_nodes, "file_salt must precede nodes");
        assert!(pos_nodes < pos_edges, "nodes must precede edges");
    }

    /// Missing `omtsf_version` must fail deserialization.
    #[test]
    fn omts_file_missing_omtsf_version_fails() {
        let json = format!(
            r#"{{
                "snapshot_date": "2026-02-19",
                "file_salt": "{SALT}",
                "nodes": [],
                "edges": []
            }}"#
        );
        let result: Result<OmtsFile, _> = serde_json::from_str(&json);
        assert!(result.is_err(), "missing omtsf_version should fail");
    }

    /// Missing `snapshot_date` must fail deserialization.
    #[test]
    fn omts_file_missing_snapshot_date_fails() {
        let json = format!(
            r#"{{
                "omtsf_version": "1.0.0",
                "file_salt": "{SALT}",
                "nodes": [],
                "edges": []
            }}"#
        );
        let result: Result<OmtsFile, _> = serde_json::from_str(&json);
        assert!(result.is_err(), "missing snapshot_date should fail");
    }

    /// Missing `file_salt` must fail deserialization.
    #[test]
    fn omts_file_missing_file_salt_fails() {
        let json = r#"{
            "omtsf_version": "1.0.0",
            "snapshot_date": "2026-02-19",
            "nodes": [],
            "edges": []
        }"#;
        let result: Result<OmtsFile, _> = serde_json::from_str(json);
        assert!(result.is_err(), "missing file_salt should fail");
    }

    /// Missing `nodes` must fail deserialization.
    #[test]
    fn omts_file_missing_nodes_fails() {
        let json = format!(
            r#"{{
                "omtsf_version": "1.0.0",
                "snapshot_date": "2026-02-19",
                "file_salt": "{SALT}",
                "edges": []
            }}"#
        );
        let result: Result<OmtsFile, _> = serde_json::from_str(&json);
        assert!(result.is_err(), "missing nodes should fail");
    }

    /// Missing `edges` must fail deserialization.
    #[test]
    fn omts_file_missing_edges_fails() {
        let json = format!(
            r#"{{
                "omtsf_version": "1.0.0",
                "snapshot_date": "2026-02-19",
                "file_salt": "{SALT}",
                "nodes": []
            }}"#
        );
        let result: Result<OmtsFile, _> = serde_json::from_str(&json);
        assert!(result.is_err(), "missing edges should fail");
    }

    /// An `OmtsFile` with empty `nodes` and `edges` arrays is valid.
    #[test]
    fn omts_file_empty_arrays_valid() {
        let f = minimal_file();
        assert!(f.nodes.is_empty());
        assert!(f.edges.is_empty());
        let json = serde_json::to_string(&f).expect("serialize");
        assert!(json.contains(r#""nodes":[]"#));
        assert!(json.contains(r#""edges":[]"#));
        round_trip(&f);
    }

    /// Optional fields absent from a minimal file do not appear in the JSON.
    #[test]
    fn omts_file_none_optionals_not_serialised() {
        let f = minimal_file();
        let json = serde_json::to_string(&f).expect("serialize");
        for absent_key in &[
            "disclosure_scope",
            "previous_snapshot_ref",
            "snapshot_sequence",
            "reporting_entity",
        ] {
            assert!(
                !json.contains(absent_key),
                "`{absent_key}` must not appear when None"
            );
        }
    }

    #[test]
    fn omts_file_disclosure_scope_variants() {
        for (variant_str, expected) in &[
            ("internal", DisclosureScope::Internal),
            ("partner", DisclosureScope::Partner),
            ("public", DisclosureScope::Public),
        ] {
            let json = format!(
                r#"{{
                    "omtsf_version": "1.0.0",
                    "snapshot_date": "2026-02-19",
                    "file_salt": "{SALT}",
                    "disclosure_scope": "{variant_str}",
                    "nodes": [],
                    "edges": []
                }}"#
            );
            let f: OmtsFile = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(f.disclosure_scope.as_ref(), Some(expected));
            round_trip(&f);
        }
    }

    #[test]
    fn omts_file_snapshot_sequence_zero_valid() {
        let f = OmtsFile {
            snapshot_sequence: Some(0),
            ..minimal_file()
        };
        let json = serde_json::to_string(&f).expect("serialize");
        assert!(json.contains(r#""snapshot_sequence":0"#));
        round_trip(&f);
    }

    /// Programmatically construct an [`OmtsFile`] with nodes and edges and
    /// verify structural round-trip equality.
    #[test]
    fn omts_file_programmatic_round_trip() {
        let f = OmtsFile {
            omtsf_version: semver("1.0.0"),
            snapshot_date: date("2026-02-19"),
            file_salt: file_salt(SALT),
            disclosure_scope: Some(DisclosureScope::Internal),
            previous_snapshot_ref: None,
            snapshot_sequence: Some(1),
            reporting_entity: Some(node_id("org-supplier")),
            nodes: vec![
                org_node("org-supplier", "Supplier Inc"),
                org_node("org-buyer", "Buyer Corp"),
            ],
            edges: vec![supplies_edge("e-1", "org-supplier", "org-buyer")],
            extra: BTreeMap::new(),
        };
        let rt = round_trip(&f);
        assert_eq!(rt.nodes.len(), 2);
        assert_eq!(rt.edges.len(), 1);
        assert_eq!(rt.nodes[0].id, node_id("org-supplier"));
        assert_eq!(rt.edges[0].source, node_id("org-supplier"));
    }

    /// An invalid `omtsf_version` format fails deserialization.
    #[test]
    fn omts_file_invalid_omtsf_version_fails() {
        let json = format!(
            r#"{{
                "omtsf_version": "not-semver",
                "snapshot_date": "2026-02-19",
                "file_salt": "{SALT}",
                "nodes": [],
                "edges": []
            }}"#
        );
        let result: Result<OmtsFile, _> = serde_json::from_str(&json);
        assert!(result.is_err(), "invalid omtsf_version format should fail");
    }

    /// An invalid `snapshot_date` format fails deserialization.
    #[test]
    fn omts_file_invalid_snapshot_date_fails() {
        let json = format!(
            r#"{{
                "omtsf_version": "1.0.0",
                "snapshot_date": "19-Feb-2026",
                "file_salt": "{SALT}",
                "nodes": [],
                "edges": []
            }}"#
        );
        let result: Result<OmtsFile, _> = serde_json::from_str(&json);
        assert!(result.is_err(), "invalid snapshot_date format should fail");
    }

    /// An invalid `file_salt` (too short) fails deserialization.
    #[test]
    fn omts_file_invalid_file_salt_fails() {
        let json = r#"{
            "omtsf_version": "1.0.0",
            "snapshot_date": "2026-02-19",
            "file_salt": "tooshort",
            "nodes": [],
            "edges": []
        }"#;
        let result: Result<OmtsFile, _> = serde_json::from_str(json);
        assert!(result.is_err(), "invalid file_salt should fail");
    }

    #[test]
    fn omts_file_extra_empty_for_known_fixture() {
        let f = minimal_file();
        let json = serde_json::to_string(&f).expect("serialize");
        let back: OmtsFile = serde_json::from_str(&json).expect("deserialize");
        assert!(
            back.extra.is_empty(),
            "no extra fields expected for a clean fixture"
        );
    }

    /// Multiple unknown fields of different JSON types all survive round-trip.
    #[test]
    fn omts_file_multiple_unknown_fields_round_trip() {
        let json = format!(
            r#"{{
                "omtsf_version": "1.0.0",
                "snapshot_date": "2026-02-19",
                "file_salt": "{SALT}",
                "nodes": [],
                "edges": [],
                "x_string": "hello",
                "x_number": 99,
                "x_bool": true,
                "x_null": null,
                "x_array": [1,2,3],
                "x_object": {{"nested": "value"}}
            }}"#
        );
        let f: OmtsFile = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(f.extra.len(), 6);

        let serialised = serde_json::to_string(&f).expect("serialize");
        let back: OmtsFile = serde_json::from_str(&serialised).expect("re-deserialize");
        assert_eq!(f, back);

        assert_eq!(
            back.extra
                .get("x_object")
                .and_then(|v| v.get("nested"))
                .and_then(|v| v.as_str()),
            Some("value")
        );
    }

    /// Constructing via [`serde_json::to_value`] / [`serde_json::from_value`]
    /// also round-trips correctly.
    #[test]
    fn omts_file_serde_json_value_round_trip() {
        let f = OmtsFile {
            omtsf_version: semver("1.0.0"),
            snapshot_date: date("2026-02-19"),
            file_salt: file_salt(SALT),
            disclosure_scope: Some(DisclosureScope::Public),
            previous_snapshot_ref: None,
            snapshot_sequence: None,
            reporting_entity: None,
            nodes: vec![org_node("org-1", "Test Corp")],
            edges: vec![],
            extra: {
                let mut m = BTreeMap::new();
                m.insert("x_extra".to_owned(), DynValue::from(json!("present")));
                m
            },
        };

        let value = serde_json::to_value(&f).expect("to_value");
        let back: OmtsFile = serde_json::from_value(value).expect("from_value");
        assert_eq!(f, back);
    }
}
