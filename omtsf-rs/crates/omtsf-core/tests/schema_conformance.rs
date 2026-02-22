#![allow(clippy::expect_used, clippy::panic, clippy::needless_pass_by_value)]

//! Schema conformance tests for `omts-v0.1.0.schema.json`.
//!
//! Validates that:
//! - The JSON Schema is valid draft 2020-12 (Group A)
//! - Schema changes are detected by checksum / structural fingerprint / enum tests (Group B)
//! - Existing fixture files conform to the schema (Group D)
//! - Auto-generated fixtures validate against the schema AND round-trip through Rust (Group C,
//!   continued in `schema_fixtures.rs`)

#[path = "schema_conformance/schema_fixtures.rs"]
mod schema_fixtures;

use std::path::PathBuf;

use jsonschema::Validator;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use omtsf_core::OmtsFile;

const SALT: &str = "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef";

fn schema_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../schema/omts-v0.1.0.schema.json")
}

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures")
        .canonicalize()
        .expect("fixtures directory should exist")
}

fn load_schema() -> Value {
    let raw = std::fs::read_to_string(schema_path()).expect("schema file should be readable");
    serde_json::from_str(&raw).expect("schema should be valid JSON")
}

fn compile_schema(schema: &Value) -> Validator {
    jsonschema::validator_for(schema).expect("schema should compile as valid JSON Schema")
}

/// Validate JSON against the schema, then parse as `OmtsFile`, then round-trip.
pub fn validate_and_parse(json: &Value, validator: &Validator) {
    let errors: Vec<_> = validator.iter_errors(json).collect();
    if !errors.is_empty() {
        let msgs: Vec<String> = errors
            .iter()
            .map(|e| format!("  - {} at {}", e, e.instance_path))
            .collect();
        panic!(
            "Schema validation failed:\n{}\nJSON:\n{}",
            msgs.join("\n"),
            serde_json::to_string_pretty(json).expect("serialize")
        );
    }

    let text = serde_json::to_string(json).expect("re-serialize");
    let parsed: OmtsFile = serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("Rust parse failed: {e}\nJSON:\n{text}"));

    let re_serialised = serde_json::to_string(&parsed).expect("round-trip serialize");
    let re_value: Value = serde_json::from_str(&re_serialised).expect("round-trip deserialize");

    let errors2: Vec<_> = validator.iter_errors(&re_value).collect();
    if !errors2.is_empty() {
        let msgs: Vec<String> = errors2
            .iter()
            .map(|e| format!("  - {} at {}", e, e.instance_path))
            .collect();
        panic!(
            "Round-trip schema validation failed:\n{}\nOriginal:\n{}\nRound-tripped:\n{}",
            msgs.join("\n"),
            serde_json::to_string_pretty(json).expect("serialize"),
            serde_json::to_string_pretty(&re_value).expect("serialize"),
        );
    }
}

/// Build a minimal valid file as `serde_json::Value`.
pub fn base_file() -> Value {
    json!({
        "omtsf_version": "0.1.0",
        "snapshot_date": "2026-01-01",
        "file_salt": SALT,
        "nodes": [],
        "edges": []
    })
}

/// Build a minimal valid file with specified nodes and edges.
pub fn base_file_with(nodes: Vec<Value>, edges: Vec<Value>) -> Value {
    json!({
        "omtsf_version": "0.1.0",
        "snapshot_date": "2026-01-01",
        "file_salt": SALT,
        "nodes": nodes,
        "edges": edges
    })
}

/// Build a minimal organization node.
pub fn org_node(id: &str, name: &str) -> Value {
    json!({"id": id, "type": "organization", "name": name})
}

/// Build a minimal facility node.
pub fn facility_node(id: &str, name: &str) -> Value {
    json!({"id": id, "type": "facility", "name": name})
}

/// Build a minimal good node.
pub fn good_node(id: &str, name: &str) -> Value {
    json!({"id": id, "type": "good", "name": name})
}

/// Build a minimal attestation node.
pub fn attestation_node(id: &str, name: &str) -> Value {
    json!({
        "id": id, "type": "attestation", "name": name,
        "attestation_type": "certification", "valid_from": "2025-01-01"
    })
}

/// Build a minimal edge with given type.
pub fn edge(id: &str, edge_type: &str, source: &str, target: &str, props: Value) -> Value {
    json!({
        "id": id, "type": edge_type, "source": source, "target": target,
        "properties": props
    })
}

// =========================================================================
// Group A — Schema Validity
// =========================================================================

#[test]
fn schema_is_valid_json_schema() {
    let schema = load_schema();
    let validator = compile_schema(&schema);
    // Verify it can validate a minimal doc without panicking
    let doc = base_file();
    assert!(validator.is_valid(&doc));
}

// =========================================================================
// Group B — Schema Change Detection
// =========================================================================

#[test]
fn schema_file_checksum() {
    let bytes = std::fs::read(schema_path()).expect("read schema bytes");
    let hash = Sha256::digest(&bytes);
    let hex = format!("{hash:x}");
    assert_eq!(
        hex, "64ea95c5c4eca84b91a436e7b2db536afaa2300790b1da355f16a8a54e99f65c",
        "Schema file has changed. Update the expected hash after reviewing all other test failures."
    );
}

#[test]
fn schema_structural_fingerprint() {
    let schema = load_schema();

    // Check $defs names
    let defs = schema.get("$defs").expect("$defs must exist");
    let defs_obj = defs.as_object().expect("$defs must be an object");
    let mut def_names: Vec<&String> = defs_obj.keys().collect();
    def_names.sort();
    assert_eq!(
        def_names,
        vec![
            "data_quality",
            "delta_operation",
            "disclosure_scope",
            "edge",
            "file_integrity",
            "geo_object",
            "geo_point",
            "identifier_record",
            "identifier_sensitivity",
            "iso_8601_date",
            "iso_8601_date_nullable",
            "merge_metadata",
            "node",
            "verification_status",
        ],
    );

    // Check top-level properties
    let props = schema.get("properties").expect("properties must exist");
    let props_obj = props.as_object().expect("properties must be an object");
    let mut prop_names: Vec<&String> = props_obj.keys().collect();
    prop_names.sort();
    assert_eq!(
        prop_names,
        vec![
            "base_snapshot_ref",
            "disclosure_scope",
            "edges",
            "file_integrity",
            "file_salt",
            "merge_metadata",
            "nodes",
            "omtsf_version",
            "previous_snapshot_ref",
            "snapshot_date",
            "snapshot_sequence",
            "update_type",
        ],
    );

    // Count node type conditionals (allOf entries in node definition)
    let node_def = defs.get("node").expect("node def");
    let node_all_of = node_def.get("allOf").expect("node allOf");
    let node_all_of_arr = node_all_of.as_array().expect("node allOf must be array");
    assert_eq!(
        node_all_of_arr.len(),
        7,
        "node allOf should have 7 entries (one per node type)"
    );

    // Count edge type conditionals
    let edge_def = defs.get("edge").expect("edge def");
    let edge_all_of = edge_def.get("allOf").expect("edge allOf");
    let edge_all_of_arr = edge_all_of.as_array().expect("edge allOf must be array");
    assert_eq!(
        edge_all_of_arr.len(),
        16,
        "edge allOf should have 16 entries (one per edge type)"
    );
}

#[test]
fn schema_enum_coverage() {
    let schema = load_schema();
    let defs = schema.get("$defs").expect("$defs");

    fn enum_vals(def: &Value) -> Vec<String> {
        def.get("enum")
            .expect("enum array")
            .as_array()
            .expect("array")
            .iter()
            .map(|v| v.as_str().expect("string").to_owned())
            .collect()
    }

    fn conditional_enum(all_of: &Value, type_val: &str, field: &str) -> Vec<String> {
        let arr = all_of.as_array().expect("array");
        for entry in arr {
            let if_block = entry.get("if");
            if let Some(ib) = if_block {
                let const_val = ib
                    .pointer("/properties/type/const")
                    .and_then(|v| v.as_str());
                if const_val == Some(type_val) {
                    let then_block = entry.get("then").expect("then");
                    if let Some(prop) = then_block.pointer(&format!("/properties/{field}")) {
                        return enum_vals(prop);
                    }
                    // Field may be inside properties/properties (for edges)
                    if let Some(prop) =
                        then_block.pointer(&format!("/properties/properties/properties/{field}"))
                    {
                        return enum_vals(prop);
                    }
                }
            }
        }
        panic!("conditional enum not found for type={type_val} field={field}");
    }

    // $defs-level enums
    assert_eq!(
        enum_vals(defs.get("disclosure_scope").expect("disclosure_scope")),
        vec!["internal", "partner", "public"]
    );
    assert_eq!(
        enum_vals(
            defs.get("identifier_sensitivity")
                .expect("identifier_sensitivity")
        ),
        vec!["public", "restricted", "confidential"]
    );
    assert_eq!(
        enum_vals(
            defs.get("verification_status")
                .expect("verification_status")
        ),
        vec!["verified", "reported", "inferred", "unverified"]
    );
    assert_eq!(
        enum_vals(defs.get("delta_operation").expect("delta_operation")),
        vec!["add", "modify", "remove"]
    );

    let dq_confidence = defs
        .get("data_quality")
        .expect("data_quality")
        .pointer("/properties/confidence")
        .expect("confidence prop");
    assert_eq!(
        enum_vals(dq_confidence),
        vec!["verified", "reported", "inferred", "estimated"]
    );

    // Top-level update_type
    let update_type = schema
        .pointer("/properties/update_type")
        .expect("update_type");
    assert_eq!(enum_vals(update_type), vec!["snapshot", "delta"]);

    // Node-level enums (via allOf conditionals)
    let node_all_of = defs.get("node").expect("node").get("allOf").expect("allOf");

    assert_eq!(
        conditional_enum(node_all_of, "organization", "status"),
        vec!["active", "dissolved", "merged", "suspended"]
    );
    assert_eq!(
        conditional_enum(node_all_of, "organization", "governance_structure"),
        vec![
            "sole_subsidiary",
            "joint_venture",
            "consortium",
            "cooperative"
        ]
    );
    assert_eq!(
        conditional_enum(node_all_of, "attestation", "attestation_type"),
        vec![
            "certification",
            "audit",
            "due_diligence_statement",
            "self_declaration",
            "other"
        ]
    );
    assert_eq!(
        conditional_enum(node_all_of, "attestation", "outcome"),
        vec![
            "pass",
            "conditional_pass",
            "fail",
            "pending",
            "not_applicable"
        ]
    );
    assert_eq!(
        conditional_enum(node_all_of, "attestation", "status"),
        vec!["active", "suspended", "revoked", "expired", "withdrawn"]
    );
    assert_eq!(
        conditional_enum(node_all_of, "attestation", "risk_severity"),
        vec!["critical", "high", "medium", "low"]
    );
    assert_eq!(
        conditional_enum(node_all_of, "attestation", "risk_likelihood"),
        vec!["very_likely", "likely", "possible", "unlikely"]
    );
    assert_eq!(
        conditional_enum(node_all_of, "consignment", "emission_factor_source"),
        vec!["actual", "default_eu", "default_country"]
    );

    // Edge-level enums (via allOf conditionals)
    let edge_all_of = defs.get("edge").expect("edge").get("allOf").expect("allOf");

    assert_eq!(
        conditional_enum(edge_all_of, "operational_control", "control_type"),
        vec![
            "franchise",
            "management",
            "tolling",
            "licensed_manufacturing",
            "other"
        ]
    );
    assert_eq!(
        conditional_enum(edge_all_of, "legal_parentage", "consolidation_basis"),
        vec!["ifrs10", "us_gaap_asc810", "other", "unknown"]
    );
    assert_eq!(
        conditional_enum(edge_all_of, "former_identity", "event_type"),
        vec!["merger", "acquisition", "rename", "demerger", "spin_off"]
    );
    assert_eq!(
        conditional_enum(edge_all_of, "beneficial_ownership", "control_type"),
        vec![
            "voting_rights",
            "capital",
            "other_means",
            "senior_management"
        ]
    );
    assert_eq!(
        conditional_enum(edge_all_of, "distributes", "service_type"),
        vec!["warehousing", "transport", "fulfillment", "other"]
    );
    assert_eq!(
        conditional_enum(edge_all_of, "same_as", "confidence"),
        vec!["definite", "probable", "possible"]
    );
}

// =========================================================================
// Group D — Existing Fixture Validation
// =========================================================================

#[test]
fn existing_valid_fixtures_pass_schema() {
    let schema = load_schema();
    let validator = compile_schema(&schema);
    for name in &["valid/minimal.omts", "valid/full-featured.omts"] {
        let path = fixtures_dir().join(name);
        let raw = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {name}: {e}"));
        let val: Value = serde_json::from_str(&raw).unwrap_or_else(|e| panic!("parse {name}: {e}"));
        validate_and_parse(&val, &validator);
    }
}

#[test]
fn existing_invalid_fixtures_documented() {
    let schema = load_schema();
    let validator = compile_schema(&schema);
    let invalid_dir = fixtures_dir().join("invalid");
    let mut entries: Vec<_> = std::fs::read_dir(&invalid_dir)
        .expect("read invalid dir")
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "omts"))
        .collect();
    entries.sort_by_key(std::fs::DirEntry::file_name);

    assert!(!entries.is_empty(), "expected at least one invalid fixture");

    let mut schema_rejects = Vec::new();
    let mut schema_accepts = Vec::new();

    for entry in &entries {
        let name = entry.file_name().to_string_lossy().to_string();
        let raw =
            std::fs::read_to_string(entry.path()).unwrap_or_else(|e| panic!("read {name}: {e}"));
        let val: Result<Value, _> = serde_json::from_str(&raw);
        match val {
            Ok(v) => {
                if validator.is_valid(&v) {
                    schema_accepts.push(name);
                } else {
                    schema_rejects.push(name);
                }
            }
            Err(_) => {
                schema_rejects.push(name);
            }
        }
    }

    // These fixtures have structural issues that the JSON Schema catches
    let expected_schema_rejects = ["missing-node-id.omts"];

    for name in &expected_schema_rejects {
        assert!(
            schema_rejects.contains(&name.to_string()),
            "{name} should be rejected by schema validation, but was accepted. \
             Schema accepts: {schema_accepts:?}"
        );
    }

    // Most invalid fixtures exercise L1/L2 validation rules (business logic)
    // that the schema intentionally does not enforce. This is expected — the
    // schema is a structural grammar, not a semantic validator.
    let expected_schema_accepts = [
        "broken-edge-ref.omts",
        "duplicate-node-id.omts",
        "duplicate-identifier.omts",
        "bad-lei-checksum.omts",
        "bad-duns-format.omts",
        "bad-gln-checksum.omts",
        "date-range-inverted.omts",
        "disclosure-violation.omts",
        "graph-type-violation.omts",
    ];

    for name in &expected_schema_accepts {
        assert!(
            schema_accepts.contains(&name.to_string()),
            "{name} should be accepted by schema but was rejected"
        );
    }
}
