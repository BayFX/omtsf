#![allow(clippy::expect_used)]

use crate::dynvalue::DynValue;
use crate::enums::{EdgeType, EdgeTypeTag, NodeType, NodeTypeTag, Sensitivity};
use crate::newtypes::{EdgeId, NodeId};
use crate::redaction::{NodeAction, classify_node};
use crate::structures::{Edge, EdgeProperties};
use crate::types::Identifier;
use crate::{enums::DisclosureScope, structures::Node};
use std::collections::BTreeMap;

pub(super) fn org_node(id: &str) -> Node {
    make_node(id, NodeTypeTag::Known(NodeType::Organization), None)
}

pub(super) fn person_node(id: &str) -> Node {
    make_node(id, NodeTypeTag::Known(NodeType::Person), None)
}

pub(super) fn boundary_ref_node(id: &str) -> Node {
    make_node(id, NodeTypeTag::Known(NodeType::BoundaryRef), None)
}

pub(super) fn make_node(
    id: &str,
    node_type: NodeTypeTag,
    identifiers: Option<Vec<Identifier>>,
) -> Node {
    Node {
        id: NodeId::try_from(id).expect("valid NodeId"),
        node_type,
        identifiers,
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

pub(super) fn make_identifier(scheme: &str, sensitivity: Option<Sensitivity>) -> Identifier {
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

pub(super) fn make_edge(edge_type: EdgeType, source: &str, target: &str) -> Edge {
    Edge {
        id: EdgeId::try_from("e-test").expect("valid EdgeId"),
        edge_type: EdgeTypeTag::Known(edge_type),
        source: NodeId::try_from(source).expect("valid NodeId"),
        target: NodeId::try_from(target).expect("valid NodeId"),
        identifiers: None,
        properties: EdgeProperties::default(),
        extra: BTreeMap::new(),
    }
}

pub(super) fn make_edge_with_properties(
    edge_type: EdgeType,
    source: &str,
    target: &str,
    extra_props: BTreeMap<String, DynValue>,
) -> Edge {
    Edge {
        id: EdgeId::try_from("e-props").expect("valid EdgeId"),
        edge_type: EdgeTypeTag::Known(edge_type),
        source: NodeId::try_from(source).expect("valid NodeId"),
        target: NodeId::try_from(target).expect("valid NodeId"),
        identifiers: None,
        properties: EdgeProperties {
            extra: extra_props,
            ..Default::default()
        },
        extra: BTreeMap::new(),
    }
}

#[test]
fn classify_org_partner_scope_retain() {
    let node = org_node("org-1");
    assert_eq!(
        classify_node(&node, &DisclosureScope::Partner),
        NodeAction::Retain
    );
}

#[test]
fn classify_facility_partner_scope_retain() {
    let node = make_node("fac-1", NodeTypeTag::Known(NodeType::Facility), None);
    assert_eq!(
        classify_node(&node, &DisclosureScope::Partner),
        NodeAction::Retain
    );
}

#[test]
fn classify_good_partner_scope_retain() {
    let node = make_node("good-1", NodeTypeTag::Known(NodeType::Good), None);
    assert_eq!(
        classify_node(&node, &DisclosureScope::Partner),
        NodeAction::Retain
    );
}

#[test]
fn classify_consignment_partner_scope_retain() {
    let node = make_node("cons-1", NodeTypeTag::Known(NodeType::Consignment), None);
    assert_eq!(
        classify_node(&node, &DisclosureScope::Partner),
        NodeAction::Retain
    );
}

#[test]
fn classify_attestation_partner_scope_retain() {
    let node = make_node("attest-1", NodeTypeTag::Known(NodeType::Attestation), None);
    assert_eq!(
        classify_node(&node, &DisclosureScope::Partner),
        NodeAction::Retain
    );
}

#[test]
fn classify_person_partner_scope_retain() {
    // Person nodes are retained in partner scope (identifiers get filtered).
    let node = person_node("person-1");
    assert_eq!(
        classify_node(&node, &DisclosureScope::Partner),
        NodeAction::Retain
    );
}

#[test]
fn classify_boundary_ref_partner_scope_retain() {
    let node = boundary_ref_node("ref-1");
    assert_eq!(
        classify_node(&node, &DisclosureScope::Partner),
        NodeAction::Retain
    );
}

#[test]
fn classify_extension_node_partner_scope_retain() {
    let node = make_node(
        "ext-1",
        NodeTypeTag::Extension("com.example.custom".to_owned()),
        None,
    );
    assert_eq!(
        classify_node(&node, &DisclosureScope::Partner),
        NodeAction::Retain
    );
}

#[test]
fn classify_org_public_scope_retain() {
    let node = org_node("org-1");
    assert_eq!(
        classify_node(&node, &DisclosureScope::Public),
        NodeAction::Retain
    );
}

#[test]
fn classify_facility_public_scope_retain() {
    let node = make_node("fac-1", NodeTypeTag::Known(NodeType::Facility), None);
    assert_eq!(
        classify_node(&node, &DisclosureScope::Public),
        NodeAction::Retain
    );
}

#[test]
fn classify_good_public_scope_retain() {
    let node = make_node("good-1", NodeTypeTag::Known(NodeType::Good), None);
    assert_eq!(
        classify_node(&node, &DisclosureScope::Public),
        NodeAction::Retain
    );
}

#[test]
fn classify_consignment_public_scope_retain() {
    let node = make_node("cons-1", NodeTypeTag::Known(NodeType::Consignment), None);
    assert_eq!(
        classify_node(&node, &DisclosureScope::Public),
        NodeAction::Retain
    );
}

#[test]
fn classify_attestation_public_scope_retain() {
    let node = make_node("attest-1", NodeTypeTag::Known(NodeType::Attestation), None);
    assert_eq!(
        classify_node(&node, &DisclosureScope::Public),
        NodeAction::Retain
    );
}

#[test]
fn classify_person_public_scope_omit() {
    // Person nodes are OMITTED in public scope.
    let node = person_node("person-1");
    assert_eq!(
        classify_node(&node, &DisclosureScope::Public),
        NodeAction::Omit
    );
}

#[test]
fn classify_boundary_ref_public_scope_retain() {
    let node = boundary_ref_node("ref-1");
    assert_eq!(
        classify_node(&node, &DisclosureScope::Public),
        NodeAction::Retain
    );
}

#[test]
fn classify_extension_node_public_scope_retain() {
    let node = make_node(
        "ext-1",
        NodeTypeTag::Extension("com.acme.custom".to_owned()),
        None,
    );
    assert_eq!(
        classify_node(&node, &DisclosureScope::Public),
        NodeAction::Retain
    );
}

#[test]
fn classify_all_node_types_internal_scope_retain() {
    let nodes = vec![
        make_node("org-1", NodeTypeTag::Known(NodeType::Organization), None),
        make_node("fac-1", NodeTypeTag::Known(NodeType::Facility), None),
        make_node("good-1", NodeTypeTag::Known(NodeType::Good), None),
        make_node("person-1", NodeTypeTag::Known(NodeType::Person), None),
        make_node("attest-1", NodeTypeTag::Known(NodeType::Attestation), None),
        make_node("cons-1", NodeTypeTag::Known(NodeType::Consignment), None),
        make_node("ref-1", NodeTypeTag::Known(NodeType::BoundaryRef), None),
    ];
    for node in &nodes {
        assert_eq!(
            classify_node(node, &DisclosureScope::Internal),
            NodeAction::Retain,
            "node {:?} should be Retain in internal scope",
            node.id
        );
    }
}
