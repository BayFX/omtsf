//! Integration tests for the full redaction pipeline (`redact` function).
//!
//! Each test constructs a source [`OmtsFile`], calls [`redact`], and asserts
//! post-redaction invariants:
//! - No dangling edges (L1-GDM-03 via post-redaction validation).
//! - `boundary_ref` nodes carry exactly one `opaque` identifier (L1-SDI-01).
//! - `disclosure_scope` is set to the target scope.
//! - `file_salt` is preserved unchanged.
//! - `person` nodes absent in `public` output.
//! - `beneficial_ownership` edges absent in `public` output.
//! - Replaced nodes produce exactly one `boundary_ref` stub (deduplication).
#![allow(clippy::expect_used)]

use std::collections::HashSet;

use omtsf_core::newtypes::{EdgeId, FileSalt, NodeId, SemVer};
use omtsf_core::structures::{Edge, EdgeProperties, Node};
use omtsf_core::types::Identifier;
use omtsf_core::validation::{RuleId, ValidationConfig, validate};
use omtsf_core::{
    CalendarDate, DisclosureScope, EdgeType, EdgeTypeTag, NodeType, NodeTypeTag, OmtsFile,
    Sensitivity, redact,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const SALT: &str = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";

fn salt() -> FileSalt {
    FileSalt::try_from(SALT).expect("valid salt")
}

fn semver() -> SemVer {
    SemVer::try_from("1.0.0").expect("valid semver")
}

fn date() -> CalendarDate {
    CalendarDate::try_from("2026-02-20").expect("valid date")
}

fn nid(s: &str) -> NodeId {
    NodeId::try_from(s).expect("valid NodeId")
}

fn eid(s: &str) -> EdgeId {
    EdgeId::try_from(s).expect("valid EdgeId")
}

fn make_org_node(id: &str, identifiers: Vec<Identifier>) -> Node {
    Node {
        id: nid(id),
        node_type: NodeTypeTag::Known(NodeType::Organization),
        identifiers: if identifiers.is_empty() {
            None
        } else {
            Some(identifiers)
        },
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

fn make_person_node(id: &str) -> Node {
    Node {
        id: nid(id),
        node_type: NodeTypeTag::Known(NodeType::Person),
        identifiers: None,
        data_quality: None,
        labels: None,
        name: Some("Jane Doe".to_owned()),
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

fn make_edge(id: &str, edge_type: EdgeType, source: &str, target: &str) -> Edge {
    Edge {
        id: eid(id),
        edge_type: EdgeTypeTag::Known(edge_type),
        source: nid(source),
        target: nid(target),
        identifiers: None,
        properties: EdgeProperties::default(),
        extra: serde_json::Map::new(),
    }
}

fn make_edge_with_props(
    id: &str,
    edge_type: EdgeType,
    source: &str,
    target: &str,
    props: EdgeProperties,
) -> Edge {
    Edge {
        id: eid(id),
        edge_type: EdgeTypeTag::Known(edge_type),
        source: nid(source),
        target: nid(target),
        identifiers: None,
        properties: props,
        extra: serde_json::Map::new(),
    }
}

fn lei_id(value: &str) -> Identifier {
    Identifier {
        scheme: "lei".to_owned(),
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

fn restricted_id(scheme: &str, value: &str) -> Identifier {
    Identifier {
        scheme: scheme.to_owned(),
        value: value.to_owned(),
        authority: None,
        valid_from: None,
        valid_to: None,
        sensitivity: Some(Sensitivity::Restricted),
        verification_status: None,
        verification_date: None,
        extra: serde_json::Map::new(),
    }
}

fn confidential_id(scheme: &str, value: &str) -> Identifier {
    Identifier {
        scheme: scheme.to_owned(),
        value: value.to_owned(),
        authority: None,
        valid_from: None,
        valid_to: None,
        sensitivity: Some(Sensitivity::Confidential),
        verification_status: None,
        verification_date: None,
        extra: serde_json::Map::new(),
    }
}

/// Build a minimal file with the given nodes and edges.
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
        extra: serde_json::Map::new(),
    }
}

/// Assert the output passes L1 validation.
fn assert_l1_valid(output: &OmtsFile) {
    let cfg = ValidationConfig {
        run_l1: true,
        run_l2: false,
        run_l3: false,
    };
    let result = validate(output, &cfg, None);
    assert!(
        result.is_conformant(),
        "output must pass L1 validation; errors: {:?}",
        result.errors().collect::<Vec<_>>()
    );
}

/// Find a node by id in the output.
fn find_node<'a>(output: &'a OmtsFile, id: &str) -> Option<&'a Node> {
    output.nodes.iter().find(|n| {
        let node_id: &str = n.id.as_ref();
        node_id == id
    })
}

// ---------------------------------------------------------------------------
// T-028-01: internal scope is a no-op
// ---------------------------------------------------------------------------

#[test]
fn redact_internal_scope_is_noop() {
    let nodes = vec![
        make_org_node("org-a", vec![lei_id("5493006MHB84DD0ZWV18")]),
        make_org_node("org-b", vec![lei_id("529900T8BM49AURSDO55")]),
    ];
    let edges = vec![make_edge("e-1", EdgeType::Supplies, "org-a", "org-b")];
    let file = make_file(nodes, edges);

    let retain_ids: HashSet<NodeId> = HashSet::new();
    let output = redact(&file, DisclosureScope::Internal, &retain_ids)
        .expect("internal redact must succeed");

    assert_eq!(output.disclosure_scope, Some(DisclosureScope::Internal));
    assert_eq!(output.file_salt, file.file_salt);
    assert_eq!(output.nodes.len(), file.nodes.len());
    assert_eq!(output.edges.len(), file.edges.len());
    // All identifiers preserved.
    let node_a = find_node(&output, "org-a").expect("org-a present");
    assert_eq!(node_a.identifiers.as_deref().unwrap_or(&[]).len(), 1);
    assert_l1_valid(&output);
}

// ---------------------------------------------------------------------------
// T-028-02: Full redaction to partner scope
// ---------------------------------------------------------------------------

#[test]
fn redact_to_partner_scope_full() {
    // Three orgs: org-a and org-b retained; org-c replaced.
    // org-a has a LEI (public) and a confidential id (stripped in partner).
    // org-c has only a LEI; its boundary_ref uses that for the hash.
    let nodes = vec![
        make_org_node(
            "org-a",
            vec![
                lei_id("5493006MHB84DD0ZWV18"),
                confidential_id("internal", "V-001"),
            ],
        ),
        make_org_node("org-b", vec![lei_id("529900T8BM49AURSDO55")]),
        make_org_node("org-c", vec![lei_id("3ERO3P1U3D2WQ9WLWA36")]),
    ];
    let edges = vec![
        make_edge("e-1", EdgeType::Supplies, "org-a", "org-b"),
        make_edge("e-2", EdgeType::Supplies, "org-b", "org-c"),
    ];
    let file = make_file(nodes, edges);

    let mut retain_ids: HashSet<NodeId> = HashSet::new();
    retain_ids.insert(nid("org-a"));
    retain_ids.insert(nid("org-b"));

    let output =
        redact(&file, DisclosureScope::Partner, &retain_ids).expect("partner redact must succeed");

    // disclosure_scope and salt.
    assert_eq!(output.disclosure_scope, Some(DisclosureScope::Partner));
    assert_eq!(output.file_salt, file.file_salt);

    // org-a retained, confidential id stripped.
    let node_a = find_node(&output, "org-a").expect("org-a present");
    assert!(
        matches!(
            &node_a.node_type,
            NodeTypeTag::Known(NodeType::Organization)
        ),
        "org-a must remain an organization node"
    );
    let ids_a = node_a.identifiers.as_deref().unwrap_or(&[]);
    assert_eq!(ids_a.len(), 1, "confidential id must be stripped");
    assert_eq!(ids_a[0].scheme, "lei");

    // org-b retained with its LEI.
    let node_b = find_node(&output, "org-b").expect("org-b present");
    assert!(matches!(
        &node_b.node_type,
        NodeTypeTag::Known(NodeType::Organization)
    ));

    // org-c replaced with boundary_ref.
    let node_c = find_node(&output, "org-c").expect("org-c present as boundary_ref");
    assert!(
        matches!(&node_c.node_type, NodeTypeTag::Known(NodeType::BoundaryRef)),
        "org-c must be a boundary_ref node"
    );
    let ids_c = node_c.identifiers.as_deref().unwrap_or(&[]);
    assert_eq!(
        ids_c.len(),
        1,
        "boundary_ref must have exactly one identifier"
    );
    assert_eq!(ids_c[0].scheme, "opaque");
    assert_eq!(
        ids_c[0].value.len(),
        64,
        "opaque value must be 64 hex chars"
    );

    // e-1 (org-a → org-b, both Retain) is kept.
    assert!(
        output.edges.iter().any(|e| e.id == eid("e-1")),
        "e-1 must be kept (both endpoints Retain)"
    );
    // e-2 (org-b → org-c, Retain → Replace) is kept as a boundary-crossing edge.
    assert!(
        output.edges.iter().any(|e| e.id == eid("e-2")),
        "e-2 must be kept (boundary-crossing edge)"
    );

    assert_l1_valid(&output);
}

// ---------------------------------------------------------------------------
// T-028-03: Full redaction to public scope
// ---------------------------------------------------------------------------

#[test]
fn redact_to_public_scope_full() {
    // org-a: retained; org-b: replaced; person-doe: omitted.
    // Edges: org-a → org-b (supplies, kept as boundary-crossing);
    //        person-doe → org-a (beneficial_ownership, omitted unconditionally).
    // org-a has a restricted id (stripped in public) and a LEI (kept).
    let nodes = vec![
        make_org_node(
            "org-a",
            vec![
                lei_id("5493006MHB84DD0ZWV18"),
                restricted_id("nat-reg", "HRB12345"),
            ],
        ),
        make_org_node("org-b", vec![lei_id("529900T8BM49AURSDO55")]),
        make_person_node("person-doe"),
    ];

    let bo_props = EdgeProperties {
        percentage: Some(60.0),
        ..EdgeProperties::default()
    };

    let edges = vec![
        make_edge("e-supplies", EdgeType::Supplies, "org-a", "org-b"),
        make_edge_with_props(
            "e-bo",
            EdgeType::BeneficialOwnership,
            "person-doe",
            "org-a",
            bo_props,
        ),
    ];
    let file = make_file(nodes, edges);

    let mut retain_ids: HashSet<NodeId> = HashSet::new();
    retain_ids.insert(nid("org-a"));

    let output =
        redact(&file, DisclosureScope::Public, &retain_ids).expect("public redact must succeed");

    // disclosure_scope and salt.
    assert_eq!(output.disclosure_scope, Some(DisclosureScope::Public));
    assert_eq!(output.file_salt, file.file_salt);

    // org-a retained; restricted id stripped.
    let node_a = find_node(&output, "org-a").expect("org-a present");
    assert!(matches!(
        &node_a.node_type,
        NodeTypeTag::Known(NodeType::Organization)
    ));
    let ids_a = node_a.identifiers.as_deref().unwrap_or(&[]);
    assert_eq!(
        ids_a.len(),
        1,
        "nat-reg (restricted) must be stripped in public scope"
    );
    assert_eq!(ids_a[0].scheme, "lei");

    // org-b replaced with boundary_ref.
    let node_b = find_node(&output, "org-b").expect("org-b present as boundary_ref");
    assert!(matches!(
        &node_b.node_type,
        NodeTypeTag::Known(NodeType::BoundaryRef)
    ));

    // person-doe absent entirely.
    assert!(
        find_node(&output, "person-doe").is_none(),
        "person-doe must be omitted in public scope"
    );

    // e-supplies kept (Retain → Replace boundary crossing).
    assert!(
        output.edges.iter().any(|e| e.id == eid("e-supplies")),
        "e-supplies must be kept"
    );

    // e-bo omitted (beneficial_ownership unconditionally omitted in public).
    assert!(
        !output.edges.iter().any(|e| e.id == eid("e-bo")),
        "e-bo must be omitted in public scope"
    );

    // No person nodes in output.
    assert!(
        output
            .nodes
            .iter()
            .all(|n| !matches!(&n.node_type, NodeTypeTag::Known(NodeType::Person))),
        "no person nodes in public output"
    );

    // No beneficial_ownership edges in output.
    assert!(
        output.edges.iter().all(|e| !matches!(
            &e.edge_type,
            EdgeTypeTag::Known(EdgeType::BeneficialOwnership)
        )),
        "no beneficial_ownership edges in public output"
    );

    assert_l1_valid(&output);
}

// ---------------------------------------------------------------------------
// T-028-04: L1-SDI-01 validity of boundary_ref nodes
// ---------------------------------------------------------------------------

#[test]
fn redact_boundary_ref_nodes_are_l1_sdi01_valid() {
    // Two orgs; one retained, one replaced. Verify boundary_ref structure.
    let nodes = vec![
        make_org_node("org-keep", vec![lei_id("5493006MHB84DD0ZWV18")]),
        make_org_node("org-replace", vec![lei_id("529900T8BM49AURSDO55")]),
    ];
    let file = make_file(nodes, vec![]);

    let mut retain_ids: HashSet<NodeId> = HashSet::new();
    retain_ids.insert(nid("org-keep"));

    let output = redact(&file, DisclosureScope::Partner, &retain_ids).expect("redact must succeed");

    // Check L1-SDI-01 explicitly.
    let cfg = ValidationConfig {
        run_l1: true,
        run_l2: false,
        run_l3: false,
    };
    let result = validate(&output, &cfg, None);
    let sdi01_errors: Vec<_> = result.by_rule(&RuleId::L1Sdi01).collect();
    assert!(
        sdi01_errors.is_empty(),
        "L1-SDI-01 must pass; errors: {sdi01_errors:?}"
    );

    // Check structure directly.
    let replaced = find_node(&output, "org-replace").expect("org-replace present");
    assert!(matches!(
        &replaced.node_type,
        NodeTypeTag::Known(NodeType::BoundaryRef)
    ));
    let ids = replaced.identifiers.as_deref().unwrap_or(&[]);
    assert_eq!(ids.len(), 1, "exactly one identifier");
    assert_eq!(ids[0].scheme, "opaque");
}

// ---------------------------------------------------------------------------
// T-028-05: Deduplication — single boundary_ref per node
// ---------------------------------------------------------------------------

#[test]
fn redact_one_boundary_ref_per_replaced_node() {
    // org-c is replaced and referenced by two edges. Only one boundary_ref
    // stub must appear in the output.
    let nodes = vec![
        make_org_node("org-a", vec![lei_id("5493006MHB84DD0ZWV18")]),
        make_org_node("org-b", vec![lei_id("529900T8BM49AURSDO55")]),
        make_org_node("org-c", vec![lei_id("3ERO3P1U3D2WQ9WLWA36")]),
    ];
    let edges = vec![
        make_edge("e-1", EdgeType::Supplies, "org-a", "org-c"),
        make_edge("e-2", EdgeType::Supplies, "org-b", "org-c"),
    ];
    let file = make_file(nodes, edges);

    let mut retain_ids: HashSet<NodeId> = HashSet::new();
    retain_ids.insert(nid("org-a"));
    retain_ids.insert(nid("org-b"));

    let output = redact(&file, DisclosureScope::Partner, &retain_ids).expect("redact must succeed");

    // Exactly one boundary_ref node for org-c.
    let bref_count = output
        .nodes
        .iter()
        .filter(|n| {
            let node_id: &str = n.id.as_ref();
            node_id == "org-c"
        })
        .count();
    assert_eq!(bref_count, 1, "exactly one boundary_ref node for org-c");

    // Both boundary-crossing edges are kept.
    assert!(output.edges.iter().any(|e| e.id == eid("e-1")));
    assert!(output.edges.iter().any(|e| e.id == eid("e-2")));

    assert_l1_valid(&output);
}

// ---------------------------------------------------------------------------
// T-028-06: Both-endpoints-replaced edges are omitted
// ---------------------------------------------------------------------------

#[test]
fn redact_both_endpoints_replaced_edge_omitted() {
    let nodes = vec![
        make_org_node("org-a", vec![lei_id("5493006MHB84DD0ZWV18")]),
        make_org_node("org-b", vec![lei_id("529900T8BM49AURSDO55")]),
        make_org_node("org-c", vec![lei_id("3ERO3P1U3D2WQ9WLWA36")]),
    ];
    // e-bc connects two replaced nodes; must be omitted.
    let edges = vec![
        make_edge("e-ac", EdgeType::Supplies, "org-a", "org-c"),
        make_edge("e-bc", EdgeType::Supplies, "org-b", "org-c"),
    ];
    let file = make_file(nodes, edges);

    // Only org-a retained; org-b and org-c replaced.
    let mut retain_ids: HashSet<NodeId> = HashSet::new();
    retain_ids.insert(nid("org-a"));

    let output = redact(&file, DisclosureScope::Partner, &retain_ids).expect("redact must succeed");

    // e-ac: org-a (Retain) → org-c (Replace) → kept.
    assert!(output.edges.iter().any(|e| e.id == eid("e-ac")));
    // e-bc: org-b (Replace) → org-c (Replace) → omitted.
    assert!(!output.edges.iter().any(|e| e.id == eid("e-bc")));

    assert_l1_valid(&output);
}

// ---------------------------------------------------------------------------
// T-028-07: Salt preserved
// ---------------------------------------------------------------------------

#[test]
fn redact_salt_preserved() {
    let file = make_file(
        vec![make_org_node("org-a", vec![lei_id("5493006MHB84DD0ZWV18")])],
        vec![],
    );
    let retain_ids: HashSet<NodeId> = std::iter::once(nid("org-a")).collect();

    for scope in [DisclosureScope::Partner, DisclosureScope::Public] {
        let scope_label = format!("{scope:?}");
        let output = redact(&file, scope, &retain_ids).expect("redact must succeed");
        assert_eq!(
            output.file_salt, file.file_salt,
            "file_salt must be preserved for scope {scope_label}"
        );
    }
}

// ---------------------------------------------------------------------------
// T-028-08: Edge property stripping — partner scope removes confidential only
// ---------------------------------------------------------------------------

#[test]
fn redact_partner_strips_confidential_edge_properties() {
    // beneficial_ownership edge with percentage (confidential) and valid_from (public).
    let nodes = vec![
        make_person_node("person-1"),
        make_org_node("org-a", vec![lei_id("5493006MHB84DD0ZWV18")]),
    ];
    let props = EdgeProperties {
        percentage: Some(60.0), // confidential on beneficial_ownership
        valid_from: Some(CalendarDate::try_from("2020-01-01").expect("valid date")),
        ..EdgeProperties::default()
    };

    let edges = vec![make_edge_with_props(
        "e-bo",
        EdgeType::BeneficialOwnership,
        "person-1",
        "org-a",
        props,
    )];
    let file = make_file(nodes, edges);

    // Retain both endpoints so the edge survives.
    let mut retain_ids: HashSet<NodeId> = HashSet::new();
    retain_ids.insert(nid("org-a"));
    // person-1 is retained in partner scope (filtered identifiers)
    retain_ids.insert(nid("person-1"));

    let output = redact(&file, DisclosureScope::Partner, &retain_ids).expect("redact must succeed");

    let edge = output
        .edges
        .iter()
        .find(|e| e.id == eid("e-bo"))
        .expect("e-bo must be present in partner scope");

    // percentage (confidential on beneficial_ownership) stripped.
    assert!(
        edge.properties.percentage.is_none(),
        "percentage must be stripped in partner scope"
    );
    // valid_from (public) kept.
    assert!(
        edge.properties.valid_from.is_some(),
        "valid_from must be kept in partner scope"
    );

    assert_l1_valid(&output);
}

// ---------------------------------------------------------------------------
// T-028-09: Edge property stripping — public scope removes restricted
// ---------------------------------------------------------------------------

#[test]
fn redact_public_strips_restricted_edge_properties() {
    let nodes = vec![
        make_org_node("org-a", vec![lei_id("5493006MHB84DD0ZWV18")]),
        make_org_node("org-b", vec![lei_id("529900T8BM49AURSDO55")]),
    ];
    let props = EdgeProperties {
        contract_ref: Some("C-001".to_owned()), // restricted
        volume: Some(5000.0),                   // restricted
        volume_unit: Some("mt".to_owned()),     // public
        ..EdgeProperties::default()
    };

    let edges = vec![make_edge_with_props(
        "e-supply",
        EdgeType::Supplies,
        "org-a",
        "org-b",
        props,
    )];
    let file = make_file(nodes, edges);

    let retain_ids: HashSet<NodeId> = [nid("org-a"), nid("org-b")].into_iter().collect();

    let output = redact(&file, DisclosureScope::Public, &retain_ids).expect("redact must succeed");

    let edge = output
        .edges
        .iter()
        .find(|e| e.id == eid("e-supply"))
        .expect("e-supply must be present");

    assert!(
        edge.properties.contract_ref.is_none(),
        "contract_ref stripped"
    );
    assert!(edge.properties.volume.is_none(), "volume stripped");
    assert_eq!(
        edge.properties.volume_unit.as_deref(),
        Some("mt"),
        "volume_unit kept"
    );

    assert_l1_valid(&output);
}

// ---------------------------------------------------------------------------
// T-028-10: No dangling edges after person node omission
// ---------------------------------------------------------------------------

#[test]
fn redact_no_dangling_edges_after_person_omission() {
    let nodes = vec![
        make_org_node("org-a", vec![lei_id("5493006MHB84DD0ZWV18")]),
        make_person_node("person-1"),
    ];
    let edges = vec![make_edge(
        "e-bo",
        EdgeType::BeneficialOwnership,
        "person-1",
        "org-a",
    )];
    let file = make_file(nodes, edges);

    let retain_ids: HashSet<NodeId> = std::iter::once(nid("org-a")).collect();

    let output = redact(&file, DisclosureScope::Public, &retain_ids).expect("redact must succeed");

    // person-1 is omitted; e-bo must be omitted too.
    assert!(find_node(&output, "person-1").is_none());
    assert!(!output.edges.iter().any(|e| e.id == eid("e-bo")));

    // Verify via L1 validation (L1-GDM-03 checks for dangling edges).
    assert_l1_valid(&output);
}

// ---------------------------------------------------------------------------
// T-028-11: Deterministic boundary_ref hash for same public ids
// ---------------------------------------------------------------------------

#[test]
fn redact_deterministic_boundary_ref_hash() {
    // Two identical files produce the same boundary_ref value for the same node.
    let nodes = vec![
        make_org_node("org-keep", vec![lei_id("5493006MHB84DD0ZWV18")]),
        make_org_node("org-replace", vec![lei_id("529900T8BM49AURSDO55")]),
    ];
    let file = make_file(nodes, vec![]);
    let retain_ids: HashSet<NodeId> = std::iter::once(nid("org-keep")).collect();

    let out1 = redact(&file, DisclosureScope::Partner, &retain_ids).expect("first redact");
    let out2 = redact(&file, DisclosureScope::Partner, &retain_ids).expect("second redact");

    let bref1 = find_node(&out1, "org-replace").expect("bref in out1");
    let bref2 = find_node(&out2, "org-replace").expect("bref in out2");

    let val1 = &bref1.identifiers.as_deref().unwrap_or(&[])[0].value;
    let val2 = &bref2.identifiers.as_deref().unwrap_or(&[])[0].value;
    assert_eq!(val1, val2, "boundary_ref hash must be deterministic");
}

// ---------------------------------------------------------------------------
// T-028-12: Full-featured fixture — redact to partner
// ---------------------------------------------------------------------------

#[test]
fn redact_full_featured_fixture_to_partner() {
    let fixture_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/full-featured.omts");
    let content = std::fs::read_to_string(&fixture_path).expect("fixture readable");
    let file: OmtsFile = serde_json::from_str(&content).expect("fixture parses");

    // Retain org-alpha, org-beta, fac-sheffield; replace good-bolts, att-sa8000,
    // con-batch-001; person-doe retained (partner) with stripped identifiers.
    let mut retain_ids: HashSet<NodeId> = HashSet::new();
    retain_ids.insert(nid("org-alpha"));
    retain_ids.insert(nid("org-beta"));
    retain_ids.insert(nid("fac-sheffield"));
    retain_ids.insert(nid("person-doe"));

    let output = redact(&file, DisclosureScope::Partner, &retain_ids)
        .expect("partner redact of full-featured fixture must succeed");

    assert_eq!(output.disclosure_scope, Some(DisclosureScope::Partner));
    assert_eq!(output.file_salt, file.file_salt);

    // org-alpha: confidential identifier stripped, public+restricted kept.
    let alpha = find_node(&output, "org-alpha").expect("org-alpha present");
    let alpha_ids = alpha.identifiers.as_deref().unwrap_or(&[]);
    // lei (public), duns (public), nat-reg (restricted), vat (restricted), internal (restricted)
    // — confidential explicitly marked ids would be stripped; here all are public/restricted.
    assert!(
        !alpha_ids
            .iter()
            .any(|id| id.sensitivity == Some(Sensitivity::Confidential)),
        "no confidential identifiers in partner output"
    );

    // bref-redacted passes through unchanged as boundary_ref.
    let bref = find_node(&output, "bref-redacted").expect("bref-redacted present");
    assert!(matches!(
        &bref.node_type,
        NodeTypeTag::Known(NodeType::BoundaryRef)
    ));

    // person-doe present (partner retains with filtered identifiers).
    assert!(
        find_node(&output, "person-doe").is_some(),
        "person-doe present in partner scope"
    );

    assert_l1_valid(&output);
}

// ---------------------------------------------------------------------------
// T-028-13: Full-featured fixture — redact to public
// ---------------------------------------------------------------------------

#[test]
fn redact_full_featured_fixture_to_public() {
    let fixture_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/full-featured.omts");
    let content = std::fs::read_to_string(&fixture_path).expect("fixture readable");
    let file: OmtsFile = serde_json::from_str(&content).expect("fixture parses");

    let mut retain_ids: HashSet<NodeId> = HashSet::new();
    retain_ids.insert(nid("org-alpha"));
    retain_ids.insert(nid("org-beta"));
    retain_ids.insert(nid("fac-sheffield"));

    let output = redact(&file, DisclosureScope::Public, &retain_ids)
        .expect("public redact of full-featured fixture must succeed");

    assert_eq!(output.disclosure_scope, Some(DisclosureScope::Public));
    assert_eq!(output.file_salt, file.file_salt);

    // No person nodes.
    assert!(
        output
            .nodes
            .iter()
            .all(|n| !matches!(&n.node_type, NodeTypeTag::Known(NodeType::Person))),
        "no person nodes in public output"
    );

    // No beneficial_ownership edges.
    assert!(
        output.edges.iter().all(|e| !matches!(
            &e.edge_type,
            EdgeTypeTag::Known(EdgeType::BeneficialOwnership)
        )),
        "no beneficial_ownership edges in public output"
    );

    // org-alpha: only public identifiers kept (lei, duns).
    let alpha = find_node(&output, "org-alpha").expect("org-alpha present");
    let alpha_ids = alpha.identifiers.as_deref().unwrap_or(&[]);
    for id in alpha_ids {
        assert!(
            matches!(id.scheme.as_str(), "lei" | "duns" | "gln" | "opaque"),
            "only public-scheme identifiers in public output; got scheme: {}",
            id.scheme
        );
    }

    // bref-redacted passes through.
    let bref = find_node(&output, "bref-redacted").expect("bref-redacted present");
    assert!(matches!(
        &bref.node_type,
        NodeTypeTag::Known(NodeType::BoundaryRef)
    ));

    assert_l1_valid(&output);
}
