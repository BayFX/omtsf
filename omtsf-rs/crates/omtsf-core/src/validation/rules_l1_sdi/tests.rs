#![allow(clippy::expect_used)]

use super::*;
use crate::enums::{DisclosureScope, NodeType, NodeTypeTag, Sensitivity};
use crate::file::OmtsFile;
use crate::newtypes::{CalendarDate, FileSalt, NodeId, SemVer};
use crate::structures::Node;
use crate::types::Identifier;
use crate::validation::{Diagnostic, ValidationRule};
use std::collections::BTreeMap;

const SALT: &str = "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef";

fn make_file(nodes: Vec<Node>, disclosure_scope: Option<DisclosureScope>) -> OmtsFile {
    OmtsFile {
        omtsf_version: SemVer::try_from("1.0.0").expect("valid"),
        snapshot_date: CalendarDate::try_from("2026-02-19").expect("valid"),
        file_salt: FileSalt::try_from(SALT).expect("valid"),
        disclosure_scope,
        previous_snapshot_ref: None,
        snapshot_sequence: None,
        reporting_entity: None,
        nodes,
        edges: vec![],
        extra: BTreeMap::new(),
    }
}

fn node_no_identifiers(id: &str, node_type: NodeType) -> Node {
    Node {
        id: NodeId::try_from(id).expect("valid id"),
        node_type: NodeTypeTag::Known(node_type),
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
        extra: BTreeMap::new(),
    }
}

fn node_with_identifiers(id: &str, node_type: NodeType, identifiers: Vec<Identifier>) -> Node {
    Node {
        identifiers: Some(identifiers),
        ..node_no_identifiers(id, node_type)
    }
}

fn opaque_identifier(value: &str) -> Identifier {
    Identifier {
        scheme: "opaque".to_owned(),
        value: value.to_owned(),
        authority: None,
        valid_from: None,
        valid_to: None,
        sensitivity: None,
        verification_status: None,
        verification_date: None,
        extra: BTreeMap::new(),
    }
}

fn identifier_with_scheme(scheme: &str, sensitivity: Option<Sensitivity>) -> Identifier {
    Identifier {
        scheme: scheme.to_owned(),
        value: "test-value".to_owned(),
        authority: None,
        valid_from: None,
        valid_to: None,
        sensitivity,
        verification_status: None,
        verification_date: None,
        extra: BTreeMap::new(),
    }
}

fn run_rule(rule: &dyn ValidationRule, file: &OmtsFile) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    rule.check(file, &mut diags, None);
    diags
}

#[test]
fn sdi01_non_boundary_ref_nodes_ignored() {
    let file = make_file(
        vec![node_no_identifiers("org-1", NodeType::Organization)],
        None,
    );
    let diags = run_rule(&L1Sdi01, &file);
    assert!(
        diags.is_empty(),
        "non-boundary_ref nodes must not trigger L1-SDI-01"
    );
}

#[test]
fn sdi01_boundary_ref_exactly_one_opaque_passes() {
    let file = make_file(
        vec![node_with_identifiers(
            "br-1",
            NodeType::BoundaryRef,
            vec![opaque_identifier("abc123")],
        )],
        None,
    );
    let diags = run_rule(&L1Sdi01, &file);
    assert!(
        diags.is_empty(),
        "boundary_ref with exactly one opaque identifier must pass L1-SDI-01"
    );
}

#[test]
fn sdi01_empty_graph_no_diagnostics() {
    let file = make_file(vec![], None);
    let diags = run_rule(&L1Sdi01, &file);
    assert!(diags.is_empty());
}

#[test]
fn sdi01_multiple_valid_boundary_refs_pass() {
    let file = make_file(
        vec![
            node_with_identifiers(
                "br-1",
                NodeType::BoundaryRef,
                vec![opaque_identifier("hash1")],
            ),
            node_with_identifiers(
                "br-2",
                NodeType::BoundaryRef,
                vec![opaque_identifier("hash2")],
            ),
        ],
        None,
    );
    let diags = run_rule(&L1Sdi01, &file);
    assert!(
        diags.is_empty(),
        "multiple valid boundary_ref nodes must all pass"
    );
}

#[test]
fn sdi01_boundary_ref_no_identifiers_field_is_error() {
    let file = make_file(
        vec![node_no_identifiers("br-1", NodeType::BoundaryRef)],
        None,
    );
    let diags = run_rule(&L1Sdi01, &file);
    assert_eq!(diags.len(), 1, "missing identifiers field must be an error");
    assert_eq!(diags[0].rule_id, RuleId::L1Sdi01);
    assert_eq!(diags[0].severity, Severity::Error);
    assert!(diags[0].message.contains("br-1"));
    assert!(diags[0].message.contains("opaque"));
}

#[test]
fn sdi01_boundary_ref_empty_identifiers_is_error() {
    let file = make_file(
        vec![node_with_identifiers("br-1", NodeType::BoundaryRef, vec![])],
        None,
    );
    let diags = run_rule(&L1Sdi01, &file);
    assert_eq!(diags.len(), 1, "empty identifiers array must be an error");
    assert_eq!(diags[0].rule_id, RuleId::L1Sdi01);
    assert_eq!(diags[0].severity, Severity::Error);
    assert!(diags[0].message.contains("br-1"));
}

#[test]
fn sdi01_boundary_ref_non_opaque_scheme_is_error() {
    let file = make_file(
        vec![node_with_identifiers(
            "br-1",
            NodeType::BoundaryRef,
            vec![identifier_with_scheme("lei", None)],
        )],
        None,
    );
    let diags = run_rule(&L1Sdi01, &file);
    assert_eq!(
        diags.len(),
        1,
        "non-opaque scheme with no opaque identifier is an error"
    );
    assert_eq!(diags[0].rule_id, RuleId::L1Sdi01);
    assert!(diags[0].message.contains("br-1"));
    assert!(diags[0].message.contains("opaque"));
}

#[test]
fn sdi01_boundary_ref_two_opaque_identifiers_is_error() {
    let file = make_file(
        vec![node_with_identifiers(
            "br-1",
            NodeType::BoundaryRef,
            vec![opaque_identifier("hash1"), opaque_identifier("hash2")],
        )],
        None,
    );
    let diags = run_rule(&L1Sdi01, &file);
    // Expect two diagnostics: one for multiple opaque, one for total count > 1.
    assert!(
        !diags.is_empty(),
        "two opaque identifiers must produce at least one error"
    );
    assert!(diags.iter().all(|d| d.rule_id == RuleId::L1Sdi01));
    assert!(diags.iter().all(|d| d.severity == Severity::Error));
    assert!(diags.iter().any(|d| d.message.contains("br-1")));
}

#[test]
fn sdi01_boundary_ref_opaque_plus_extra_identifier_is_error() {
    let file = make_file(
        vec![node_with_identifiers(
            "br-1",
            NodeType::BoundaryRef,
            vec![
                opaque_identifier("hash1"),
                identifier_with_scheme("lei", None),
            ],
        )],
        None,
    );
    let diags = run_rule(&L1Sdi01, &file);
    assert!(
        !diags.is_empty(),
        "opaque + extra identifier must produce an error"
    );
    assert!(diags.iter().all(|d| d.rule_id == RuleId::L1Sdi01));
    assert!(diags.iter().all(|d| d.severity == Severity::Error));
}

#[test]
fn sdi01_multiple_boundary_refs_all_violations_collected() {
    let file = make_file(
        vec![
            node_with_identifiers(
                "br-ok",
                NodeType::BoundaryRef,
                vec![opaque_identifier("good")],
            ),
            node_no_identifiers("br-bad1", NodeType::BoundaryRef),
            node_with_identifiers(
                "br-bad2",
                NodeType::BoundaryRef,
                vec![identifier_with_scheme("lei", None)],
            ),
        ],
        None,
    );
    let diags = run_rule(&L1Sdi01, &file);
    // br-ok passes; br-bad1 and br-bad2 each produce one diagnostic.
    assert_eq!(diags.len(), 2, "each violation must produce a diagnostic");
    assert!(diags.iter().any(|d| d.message.contains("br-bad1")));
    assert!(diags.iter().any(|d| d.message.contains("br-bad2")));
    assert!(!diags.iter().any(|d| d.message.contains("br-ok")));
}

#[test]
fn sdi02_no_disclosure_scope_no_diagnostics() {
    // A confidential identifier on a partner-intended file without scope
    // declared must not be flagged — L1-SDI-02 only applies when scope is set.
    let file = make_file(
        vec![node_with_identifiers(
            "org-1",
            NodeType::Organization,
            vec![identifier_with_scheme(
                "lei",
                Some(Sensitivity::Confidential),
            )],
        )],
        None, // no disclosure_scope
    );
    let diags = run_rule(&L1Sdi02, &file);
    assert!(
        diags.is_empty(),
        "without disclosure_scope, L1-SDI-02 must not fire"
    );
}

#[test]
fn sdi02_internal_scope_allows_all_sensitivities() {
    let file = make_file(
        vec![node_with_identifiers(
            "org-1",
            NodeType::Organization,
            vec![
                identifier_with_scheme("lei", Some(Sensitivity::Confidential)),
                identifier_with_scheme("vat", Some(Sensitivity::Restricted)),
                identifier_with_scheme("duns", Some(Sensitivity::Public)),
            ],
        )],
        Some(DisclosureScope::Internal),
    );
    let diags = run_rule(&L1Sdi02, &file);
    assert!(
        diags.is_empty(),
        "internal scope imposes no sensitivity constraints"
    );
}

#[test]
fn sdi02_partner_scope_public_identifier_passes() {
    let file = make_file(
        vec![node_with_identifiers(
            "org-1",
            NodeType::Organization,
            vec![identifier_with_scheme("lei", None)], // lei defaults to public
        )],
        Some(DisclosureScope::Partner),
    );
    let diags = run_rule(&L1Sdi02, &file);
    assert!(
        diags.is_empty(),
        "public identifier must pass partner scope"
    );
}

#[test]
fn sdi02_partner_scope_restricted_identifier_passes() {
    let file = make_file(
        vec![node_with_identifiers(
            "org-1",
            NodeType::Organization,
            vec![identifier_with_scheme("nat-reg", None)], // nat-reg defaults to restricted
        )],
        Some(DisclosureScope::Partner),
    );
    let diags = run_rule(&L1Sdi02, &file);
    assert!(
        diags.is_empty(),
        "restricted identifier must pass partner scope"
    );
}

#[test]
fn sdi02_partner_scope_confidential_identifier_is_error() {
    let file = make_file(
        vec![node_with_identifiers(
            "org-1",
            NodeType::Organization,
            vec![identifier_with_scheme(
                "lei",
                Some(Sensitivity::Confidential),
            )],
        )],
        Some(DisclosureScope::Partner),
    );
    let diags = run_rule(&L1Sdi02, &file);
    assert_eq!(
        diags.len(),
        1,
        "confidential identifier violates partner scope"
    );
    assert_eq!(diags[0].rule_id, RuleId::L1Sdi02);
    assert_eq!(diags[0].severity, Severity::Error);
    assert!(diags[0].message.contains("org-1"));
    assert!(diags[0].message.contains("confidential"));
    assert!(diags[0].message.contains("partner"));
}

#[test]
fn sdi02_partner_scope_person_node_implicit_confidential_is_error() {
    // Person nodes default to confidential sensitivity for all identifiers
    // when no explicit sensitivity is set — this must also be flagged.
    let file = make_file(
        vec![node_with_identifiers(
            "person-1",
            NodeType::Person,
            vec![identifier_with_scheme("lei", None)], // implicitly confidential on Person
        )],
        Some(DisclosureScope::Partner),
    );
    let diags = run_rule(&L1Sdi02, &file);
    assert_eq!(
        diags.len(),
        1,
        "person node implicit confidential violates partner scope"
    );
    assert_eq!(diags[0].rule_id, RuleId::L1Sdi02);
    assert_eq!(diags[0].severity, Severity::Error);
    assert!(diags[0].message.contains("person-1"));
    assert!(diags[0].message.contains("confidential"));
}

#[test]
fn sdi02_public_scope_public_identifier_passes() {
    let file = make_file(
        vec![node_with_identifiers(
            "org-1",
            NodeType::Organization,
            vec![identifier_with_scheme("lei", None)], // lei defaults to public
        )],
        Some(DisclosureScope::Public),
    );
    let diags = run_rule(&L1Sdi02, &file);
    assert!(diags.is_empty(), "public identifier must pass public scope");
}

#[test]
fn sdi02_public_scope_restricted_identifier_is_error() {
    let file = make_file(
        vec![node_with_identifiers(
            "org-1",
            NodeType::Organization,
            vec![identifier_with_scheme("nat-reg", None)], // nat-reg defaults to restricted
        )],
        Some(DisclosureScope::Public),
    );
    let diags = run_rule(&L1Sdi02, &file);
    assert_eq!(
        diags.len(),
        1,
        "restricted identifier violates public scope"
    );
    assert_eq!(diags[0].rule_id, RuleId::L1Sdi02);
    assert_eq!(diags[0].severity, Severity::Error);
    assert!(diags[0].message.contains("org-1"));
    assert!(diags[0].message.contains("restricted"));
    assert!(diags[0].message.contains("public"));
}

#[test]
fn sdi02_public_scope_confidential_identifier_is_error() {
    let file = make_file(
        vec![node_with_identifiers(
            "org-1",
            NodeType::Organization,
            vec![identifier_with_scheme(
                "lei",
                Some(Sensitivity::Confidential),
            )],
        )],
        Some(DisclosureScope::Public),
    );
    let diags = run_rule(&L1Sdi02, &file);
    assert_eq!(
        diags.len(),
        1,
        "confidential identifier violates public scope"
    );
    assert_eq!(diags[0].rule_id, RuleId::L1Sdi02);
    assert_eq!(diags[0].severity, Severity::Error);
    assert!(diags[0].message.contains("org-1"));
    assert!(diags[0].message.contains("confidential"));
}

#[test]
fn sdi02_public_scope_explicit_restricted_on_lei_is_error() {
    // Explicit restricted override on a normally-public scheme is still restricted.
    let file = make_file(
        vec![node_with_identifiers(
            "org-1",
            NodeType::Organization,
            vec![identifier_with_scheme("lei", Some(Sensitivity::Restricted))],
        )],
        Some(DisclosureScope::Public),
    );
    let diags = run_rule(&L1Sdi02, &file);
    assert_eq!(
        diags.len(),
        1,
        "explicit restricted on lei still violates public scope"
    );
}

#[test]
fn sdi02_public_scope_collects_all_violations() {
    let file = make_file(
        vec![
            node_with_identifiers(
                "org-1",
                NodeType::Organization,
                vec![
                    identifier_with_scheme("lei", None),     // public — ok
                    identifier_with_scheme("nat-reg", None), // restricted — violation
                    identifier_with_scheme("vat", Some(Sensitivity::Confidential)), // confidential — violation
                ],
            ),
            node_with_identifiers(
                "org-2",
                NodeType::Organization,
                vec![
                    identifier_with_scheme("duns", None),     // public — ok
                    identifier_with_scheme("internal", None), // restricted — violation
                ],
            ),
        ],
        Some(DisclosureScope::Public),
    );
    let diags = run_rule(&L1Sdi02, &file);
    // 3 violations total: nat-reg, confidential vat on org-1, internal on org-2.
    assert_eq!(diags.len(), 3, "all violations must be collected");
    assert!(diags.iter().all(|d| d.rule_id == RuleId::L1Sdi02));
    assert!(diags.iter().all(|d| d.severity == Severity::Error));
}

#[test]
fn sdi02_partner_scope_collects_all_violations() {
    let file = make_file(
        vec![node_with_identifiers(
            "org-1",
            NodeType::Organization,
            vec![
                identifier_with_scheme("lei", Some(Sensitivity::Confidential)), // violation
                identifier_with_scheme("nat-reg", None),                        // restricted — ok
                identifier_with_scheme("vat", Some(Sensitivity::Confidential)), // violation
            ],
        )],
        Some(DisclosureScope::Partner),
    );
    let diags = run_rule(&L1Sdi02, &file);
    assert_eq!(
        diags.len(),
        2,
        "both confidential identifiers must be flagged"
    );
    assert!(diags.iter().all(|d| d.rule_id == RuleId::L1Sdi02));
}

#[test]
fn sdi02_location_points_to_correct_identifier_index() {
    let file = make_file(
        vec![node_with_identifiers(
            "org-1",
            NodeType::Organization,
            vec![
                identifier_with_scheme("lei", None), // index 0 — public, ok
                identifier_with_scheme("vat", Some(Sensitivity::Confidential)), // index 1 — violation
                identifier_with_scheme("duns", None), // index 2 — public, ok
            ],
        )],
        Some(DisclosureScope::Partner),
    );
    let diags = run_rule(&L1Sdi02, &file);
    assert_eq!(diags.len(), 1);
    assert!(matches!(
        &diags[0].location,
        Location::Identifier { node_id, index: 1, .. } if node_id == "org-1"
    ));
}

#[test]
fn sdi02_node_with_no_identifiers_is_ignored() {
    let file = make_file(
        vec![node_no_identifiers("org-1", NodeType::Organization)],
        Some(DisclosureScope::Public),
    );
    let diags = run_rule(&L1Sdi02, &file);
    assert!(
        diags.is_empty(),
        "nodes without identifiers must be ignored by L1-SDI-02"
    );
}

#[test]
fn sdi02_boundary_ref_opaque_identifier_passes_public_scope() {
    // The opaque scheme defaults to public per the redaction spec.
    // A valid boundary_ref node must pass L1-SDI-02 under public scope.
    let file = make_file(
        vec![node_with_identifiers(
            "br-1",
            NodeType::BoundaryRef,
            vec![opaque_identifier("abc123")],
        )],
        Some(DisclosureScope::Public),
    );
    let diags = run_rule(&L1Sdi02, &file);
    assert!(
        diags.is_empty(),
        "boundary_ref with opaque identifier must pass public scope"
    );
}
