//! WASM compatibility documentation tests for `omtsf-core`.
//!
//! These tests run on native targets and verify that the core types used by
//! `omtsf-core` can be constructed and exercised without any platform-specific
//! I/O APIs (`std::fs`, `std::net`, `std::process`).  The same code compiles
//! unchanged under `--target wasm32-unknown-unknown`.
//!
//! ## WASM build requirement
//!
//! `omtsf-core` must compile for `wasm32-unknown-unknown`.  The CI pipeline
//! enforces this via `just wasm-check`:
//!
//! ```text
//! cargo build -p omtsf-core --target wasm32-unknown-unknown
//! ```
//!
//! ## Forbidden imports
//!
//! Production code under `crates/omtsf-core/src/` must not import:
//! - `std::fs`
//! - `std::net`
//! - `std::process`
//!
//! All I/O belongs in `omtsf-cli`.
#![allow(clippy::expect_used)]

use std::collections::BTreeMap;

use omtsf_core::{
    CalendarDate, DisclosureScope, EdgeType, EdgeTypeTag, FileSalt, NodeType, NodeTypeTag,
    OmtsFile, SemVer,
    structures::{Edge, EdgeProperties, Node},
};

const SALT: &str = "cafebabecafebabecafebabecafebabecafebabecafebabecafebabecafebabe";

fn semver() -> SemVer {
    SemVer::try_from("1.0.0").expect("valid SemVer")
}

fn date() -> CalendarDate {
    CalendarDate::try_from("2026-02-20").expect("valid CalendarDate")
}

fn salt() -> FileSalt {
    FileSalt::try_from(SALT).expect("valid FileSalt")
}

fn node_id(s: &str) -> omtsf_core::NodeId {
    omtsf_core::NodeId::try_from(s).expect("valid NodeId")
}

fn edge_id(s: &str) -> omtsf_core::EdgeId {
    omtsf_core::EdgeId::try_from(s).expect("valid EdgeId")
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
        extra: BTreeMap::new(),
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
        extra: BTreeMap::new(),
    }
}

fn make_file(nodes: Vec<Node>, edges: Vec<Edge>) -> OmtsFile {
    OmtsFile {
        omtsf_version: semver(),
        snapshot_date: date(),
        file_salt: salt(),
        disclosure_scope: None,
        previous_snapshot_ref: None,
        snapshot_sequence: None,
        reporting_entity: None,
        nodes,
        edges,
        extra: BTreeMap::new(),
    }
}

/// [`OmtsFile`] and its nested types can be constructed and serialised using
/// only in-memory operations — no filesystem, network, or process APIs.
///
/// This is the same code path exercised when `omtsf-core` is loaded inside a
/// WASM module.
#[test]
fn wasm_compat_core_types_no_io() {
    let file = make_file(
        vec![org_node("org-a"), org_node("org-b")],
        vec![supplies_edge("e-1", "org-a", "org-b")],
    );

    let json = serde_json::to_string(&file).expect("serialise");
    let back: OmtsFile = serde_json::from_str(&json).expect("deserialise");

    assert_eq!(file, back);
    assert_eq!(back.nodes.len(), 2);
    assert_eq!(back.edges.len(), 1);
}

/// [`omtsf_core::validate`] accepts and validates an [`OmtsFile`] using only
/// in-memory data — compatible with a WASM sandbox that has no filesystem.
#[test]
fn wasm_compat_validation_no_io() {
    use omtsf_core::validation::{ValidationConfig, validate};

    let file = make_file(vec![org_node("org-x")], vec![]);

    let cfg = ValidationConfig {
        run_l1: true,
        run_l2: false,
        run_l3: false,
    };

    let result = validate(&file, &cfg, None);
    assert!(
        result.is_conformant(),
        "minimal file must pass L1 validation"
    );
}

/// [`omtsf_core::redact`] produces a redacted file using only in-memory
/// operations — no filesystem access required.
#[test]
fn wasm_compat_redaction_no_io() {
    use std::collections::HashSet;

    let file = make_file(
        vec![org_node("org-pub"), org_node("org-priv")],
        vec![supplies_edge("e-1", "org-pub", "org-priv")],
    );

    let mut retain: HashSet<omtsf_core::NodeId> = HashSet::new();
    retain.insert(node_id("org-pub"));

    let output =
        omtsf_core::redact(&file, DisclosureScope::Partner, &retain).expect("redact must succeed");

    assert_eq!(output.disclosure_scope, Some(DisclosureScope::Partner));
    assert_eq!(output.nodes.len(), 2);
}

/// The [`omtsf_core::boundary_ref_value`] function uses `sha2` under the hood.
/// `sha2` compiles for WASM32 and requires no platform entropy — it is a pure
/// hash function.
#[test]
fn wasm_compat_sha2_no_io() {
    use omtsf_core::boundary_ref_value;
    use omtsf_core::decode_salt;

    let file_salt = salt();
    let salt_bytes = decode_salt(&file_salt).expect("decode_salt must succeed");

    let id = omtsf_core::types::Identifier {
        scheme: "lei".to_owned(),
        value: "5493006MHB84DD0ZWV18".to_owned(),
        authority: None,
        valid_from: None,
        valid_to: None,
        sensitivity: None,
        verification_status: None,
        verification_date: None,
        extra: BTreeMap::new(),
    };
    let canonical_id = omtsf_core::CanonicalId::from_identifier(&id);

    let result = boundary_ref_value(&[canonical_id.clone()], &salt_bytes);
    assert!(
        result.is_ok(),
        "boundary_ref_value must succeed: {result:?}"
    );

    let hash = result.expect("boundary_ref_value ok");
    assert_eq!(hash.len(), 64, "SHA-256 hex digest must be 64 chars");
    assert!(
        hash.chars().all(|c| c.is_ascii_hexdigit()),
        "hash must be lowercase hex"
    );

    let hash2 = boundary_ref_value(&[canonical_id], &salt_bytes).expect("second call ok");
    assert_eq!(hash, hash2, "SHA-256 must be deterministic");
}
