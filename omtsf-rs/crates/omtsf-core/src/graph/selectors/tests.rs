#![allow(clippy::expect_used)]

use super::*;
use crate::enums::{EdgeType, NodeType};
use crate::structures::Node;
use crate::test_helpers::{facility_node, node_id, org_node, ownership_edge, supplies_edge};
use crate::types::{Identifier, Label};
use std::collections::BTreeMap;

fn country_code(s: &str) -> CountryCode {
    CountryCode::try_from(s).expect("valid CountryCode")
}

fn bare_node(id: &str, node_type: NodeTypeTag) -> Node {
    Node {
        id: node_id(id),
        node_type,
        ..crate::test_helpers::org_node(id)
    }
}

fn label(key: &str, value: Option<&str>) -> Label {
    Label {
        key: key.to_owned(),
        value: value.map(str::to_owned),
        extra: BTreeMap::new(),
    }
}

fn identifier(scheme: &str, value: &str) -> Identifier {
    Identifier {
        scheme: scheme.to_owned(),
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

/// A default `SelectorSet` is empty.
#[test]
fn test_selector_set_default_is_empty() {
    let ss = SelectorSet::default();
    assert!(ss.is_empty());
}

/// A `SelectorSet` with at least one selector is not empty.
#[test]
fn test_selector_set_with_one_selector_is_not_empty() {
    let ss = SelectorSet::from_selectors(vec![Selector::NodeType(NodeTypeTag::Known(
        NodeType::Organization,
    ))]);
    assert!(!ss.is_empty());
}

/// `from_selectors` distributes each variant into the correct field.
#[test]
fn test_from_selectors_distributes_correctly() {
    let selectors = vec![
        Selector::NodeType(NodeTypeTag::Known(NodeType::Organization)),
        Selector::EdgeType(EdgeTypeTag::Known(EdgeType::Supplies)),
        Selector::LabelKey("certified".to_owned()),
        Selector::LabelKeyValue("tier".to_owned(), "1".to_owned()),
        Selector::IdentifierScheme("lei".to_owned()),
        Selector::IdentifierSchemeValue("duns".to_owned(), "123456789".to_owned()),
        Selector::Jurisdiction(country_code("DE")),
        Selector::Name("Acme".to_owned()),
    ];
    let ss = SelectorSet::from_selectors(selectors);
    assert_eq!(ss.node_types.len(), 1);
    assert_eq!(ss.edge_types.len(), 1);
    assert_eq!(ss.label_keys.len(), 1);
    assert_eq!(ss.label_key_values.len(), 1);
    assert_eq!(ss.identifier_schemes.len(), 1);
    assert_eq!(ss.identifier_scheme_values.len(), 1);
    assert_eq!(ss.jurisdictions.len(), 1);
    assert_eq!(ss.names.len(), 1);
}

/// A matching node type passes the `NodeType` selector.
#[test]
fn test_matches_node_node_type_match() {
    let ss = SelectorSet::from_selectors(vec![Selector::NodeType(NodeTypeTag::Known(
        NodeType::Organization,
    ))]);
    let node = org_node("n1");
    assert!(ss.matches_node(&node));
}

/// A non-matching node type fails the `NodeType` selector.
#[test]
fn test_matches_node_node_type_no_match() {
    let ss = SelectorSet::from_selectors(vec![Selector::NodeType(NodeTypeTag::Known(
        NodeType::Facility,
    ))]);
    let node = org_node("n1");
    assert!(!ss.matches_node(&node));
}

/// Multiple `NodeType` values compose with OR: node matches if it equals any.
#[test]
fn test_matches_node_node_type_or_composition() {
    let ss = SelectorSet::from_selectors(vec![
        Selector::NodeType(NodeTypeTag::Known(NodeType::Organization)),
        Selector::NodeType(NodeTypeTag::Known(NodeType::Facility)),
    ]);
    assert!(ss.matches_node(&org_node("n1")));
    assert!(ss.matches_node(&facility_node("n2")));
    assert!(!ss.matches_node(&bare_node("n3", NodeTypeTag::Known(NodeType::Good))));
}

/// Node with matching label key passes `LabelKey` selector.
#[test]
fn test_matches_node_label_key_match() {
    let ss = SelectorSet::from_selectors(vec![Selector::LabelKey("certified".to_owned())]);
    let mut node = org_node("n1");
    node.labels = Some(vec![label("certified", None)]);
    assert!(ss.matches_node(&node));
}

/// Node without labels fails `LabelKey` selector.
#[test]
fn test_matches_node_label_key_no_labels() {
    let ss = SelectorSet::from_selectors(vec![Selector::LabelKey("certified".to_owned())]);
    let node = org_node("n1");
    assert!(!ss.matches_node(&node));
}

/// Node with labels but wrong key fails `LabelKey` selector.
#[test]
fn test_matches_node_label_key_wrong_key() {
    let ss = SelectorSet::from_selectors(vec![Selector::LabelKey("certified".to_owned())]);
    let mut node = org_node("n1");
    node.labels = Some(vec![label("tier", Some("1"))]);
    assert!(!ss.matches_node(&node));
}

/// Multiple `LabelKey` values compose with OR.
#[test]
fn test_matches_node_label_key_or_composition() {
    let ss = SelectorSet::from_selectors(vec![
        Selector::LabelKey("certified".to_owned()),
        Selector::LabelKey("audited".to_owned()),
    ]);
    let mut node = org_node("n1");
    node.labels = Some(vec![label("audited", None)]);
    assert!(ss.matches_node(&node));
}

/// Node with matching (key, value) label pair passes `LabelKeyValue` selector.
#[test]
fn test_matches_node_label_key_value_match() {
    let ss = SelectorSet::from_selectors(vec![Selector::LabelKeyValue(
        "tier".to_owned(),
        "1".to_owned(),
    )]);
    let mut node = org_node("n1");
    node.labels = Some(vec![label("tier", Some("1"))]);
    assert!(ss.matches_node(&node));
}

/// Node with matching key but wrong value fails `LabelKeyValue` selector.
#[test]
fn test_matches_node_label_key_value_wrong_value() {
    let ss = SelectorSet::from_selectors(vec![Selector::LabelKeyValue(
        "tier".to_owned(),
        "1".to_owned(),
    )]);
    let mut node = org_node("n1");
    node.labels = Some(vec![label("tier", Some("2"))]);
    assert!(!ss.matches_node(&node));
}

/// Node with matching key but no value (key-only label) fails `LabelKeyValue`.
#[test]
fn test_matches_node_label_key_value_key_only_label_no_match() {
    let ss = SelectorSet::from_selectors(vec![Selector::LabelKeyValue(
        "certified".to_owned(),
        "yes".to_owned(),
    )]);
    let mut node = org_node("n1");
    node.labels = Some(vec![label("certified", None)]);
    assert!(!ss.matches_node(&node));
}

/// Node with matching identifier scheme passes `IdentifierScheme` selector.
#[test]
fn test_matches_node_identifier_scheme_match() {
    let ss = SelectorSet::from_selectors(vec![Selector::IdentifierScheme("lei".to_owned())]);
    let mut node = org_node("n1");
    node.identifiers = Some(vec![identifier("lei", "529900T8BM49AURSDO55")]);
    assert!(ss.matches_node(&node));
}

/// Node without identifiers fails `IdentifierScheme` selector.
#[test]
fn test_matches_node_identifier_scheme_no_identifiers() {
    let ss = SelectorSet::from_selectors(vec![Selector::IdentifierScheme("lei".to_owned())]);
    let node = org_node("n1");
    assert!(!ss.matches_node(&node));
}

/// Multiple `IdentifierScheme` values compose with OR.
#[test]
fn test_matches_node_identifier_scheme_or_composition() {
    let ss = SelectorSet::from_selectors(vec![
        Selector::IdentifierScheme("lei".to_owned()),
        Selector::IdentifierScheme("duns".to_owned()),
    ]);
    let mut node = org_node("n1");
    node.identifiers = Some(vec![identifier("duns", "123456789")]);
    assert!(ss.matches_node(&node));
}

/// Node with matching (scheme, value) identifier passes `IdentifierSchemeValue`.
#[test]
fn test_matches_node_identifier_scheme_value_match() {
    let ss = SelectorSet::from_selectors(vec![Selector::IdentifierSchemeValue(
        "duns".to_owned(),
        "123456789".to_owned(),
    )]);
    let mut node = org_node("n1");
    node.identifiers = Some(vec![identifier("duns", "123456789")]);
    assert!(ss.matches_node(&node));
}

/// Node with matching scheme but different value fails `IdentifierSchemeValue`.
#[test]
fn test_matches_node_identifier_scheme_value_wrong_value() {
    let ss = SelectorSet::from_selectors(vec![Selector::IdentifierSchemeValue(
        "duns".to_owned(),
        "123456789".to_owned(),
    )]);
    let mut node = org_node("n1");
    node.identifiers = Some(vec![identifier("duns", "999999999")]);
    assert!(!ss.matches_node(&node));
}

/// Node with matching jurisdiction passes `Jurisdiction` selector.
#[test]
fn test_matches_node_jurisdiction_match() {
    let ss = SelectorSet::from_selectors(vec![Selector::Jurisdiction(country_code("DE"))]);
    let mut node = org_node("n1");
    node.jurisdiction = Some(country_code("DE"));
    assert!(ss.matches_node(&node));
}

/// Node with different jurisdiction fails `Jurisdiction` selector.
#[test]
fn test_matches_node_jurisdiction_no_match() {
    let ss = SelectorSet::from_selectors(vec![Selector::Jurisdiction(country_code("DE"))]);
    let mut node = org_node("n1");
    node.jurisdiction = Some(country_code("US"));
    assert!(!ss.matches_node(&node));
}

/// Node without jurisdiction fails `Jurisdiction` selector.
#[test]
fn test_matches_node_jurisdiction_absent() {
    let ss = SelectorSet::from_selectors(vec![Selector::Jurisdiction(country_code("DE"))]);
    let node = org_node("n1");
    assert!(!ss.matches_node(&node));
}

/// Multiple `Jurisdiction` values compose with OR.
#[test]
fn test_matches_node_jurisdiction_or_composition() {
    let ss = SelectorSet::from_selectors(vec![
        Selector::Jurisdiction(country_code("DE")),
        Selector::Jurisdiction(country_code("FR")),
    ]);
    let mut node = org_node("n1");
    node.jurisdiction = Some(country_code("FR"));
    assert!(ss.matches_node(&node));
}

/// Node with name containing the pattern (case-insensitive) passes `Name` selector.
#[test]
fn test_matches_node_name_substring_match() {
    let ss = SelectorSet::from_selectors(vec![Selector::Name("acme".to_owned())]);
    let mut node = org_node("n1");
    node.name = Some("Acme Corp".to_owned());
    assert!(ss.matches_node(&node));
}

/// Case-insensitive: uppercase pattern matches lowercase name.
#[test]
fn test_matches_node_name_case_insensitive() {
    let ss = SelectorSet::from_selectors(vec![Selector::Name("ACME".to_owned())]);
    let mut node = org_node("n1");
    node.name = Some("acme gmbh".to_owned());
    assert!(ss.matches_node(&node));
}

/// Node with name that does not contain the pattern fails `Name` selector.
#[test]
fn test_matches_node_name_no_match() {
    let ss = SelectorSet::from_selectors(vec![Selector::Name("acme".to_owned())]);
    let mut node = org_node("n1");
    node.name = Some("Global Logistics Ltd".to_owned());
    assert!(!ss.matches_node(&node));
}

/// Node without name fails `Name` selector.
#[test]
fn test_matches_node_name_absent() {
    let ss = SelectorSet::from_selectors(vec![Selector::Name("acme".to_owned())]);
    let node = org_node("n1");
    assert!(!ss.matches_node(&node));
}

/// `NodeType` AND `Jurisdiction`: both groups must pass.
#[test]
fn test_matches_node_and_across_groups_both_pass() {
    let ss = SelectorSet::from_selectors(vec![
        Selector::NodeType(NodeTypeTag::Known(NodeType::Organization)),
        Selector::Jurisdiction(country_code("DE")),
    ]);
    let mut node = org_node("n1");
    node.jurisdiction = Some(country_code("DE"));
    assert!(ss.matches_node(&node));
}

/// `NodeType` passes but `Jurisdiction` fails → overall false.
#[test]
fn test_matches_node_and_across_groups_one_fails() {
    let ss = SelectorSet::from_selectors(vec![
        Selector::NodeType(NodeTypeTag::Known(NodeType::Organization)),
        Selector::Jurisdiction(country_code("DE")),
    ]);
    let mut node = org_node("n1");
    node.jurisdiction = Some(country_code("US"));
    assert!(!ss.matches_node(&node));
}

/// An empty `SelectorSet` matches every node (no constraints).
#[test]
fn test_matches_node_empty_selector_set_matches_all() {
    let ss = SelectorSet::default();
    assert!(ss.matches_node(&org_node("n1")));
    assert!(ss.matches_node(&facility_node("n2")));
}

/// Matching edge type passes `EdgeType` selector.
#[test]
fn test_matches_edge_edge_type_match() {
    let ss = SelectorSet::from_selectors(vec![Selector::EdgeType(EdgeTypeTag::Known(
        EdgeType::Supplies,
    ))]);
    let edge = supplies_edge("e1", "a", "b");
    assert!(ss.matches_edge(&edge));
}

/// Non-matching edge type fails `EdgeType` selector.
#[test]
fn test_matches_edge_edge_type_no_match() {
    let ss = SelectorSet::from_selectors(vec![Selector::EdgeType(EdgeTypeTag::Known(
        EdgeType::Ownership,
    ))]);
    let edge = supplies_edge("e1", "a", "b");
    assert!(!ss.matches_edge(&edge));
}

/// Multiple `EdgeType` values compose with OR.
#[test]
fn test_matches_edge_edge_type_or_composition() {
    let ss = SelectorSet::from_selectors(vec![
        Selector::EdgeType(EdgeTypeTag::Known(EdgeType::Supplies)),
        Selector::EdgeType(EdgeTypeTag::Known(EdgeType::Ownership)),
    ]);
    assert!(ss.matches_edge(&supplies_edge("e1", "a", "b")));
    assert!(ss.matches_edge(&ownership_edge("e2", "a", "b")));
}

/// `NodeType` selector is ignored when evaluating an edge.
#[test]
fn test_matches_edge_node_type_selector_is_skipped() {
    let ss = SelectorSet::from_selectors(vec![Selector::NodeType(NodeTypeTag::Known(
        NodeType::Organization,
    ))]);
    let edge = supplies_edge("e1", "a", "b");
    assert!(ss.matches_edge(&edge));
}

/// Jurisdiction selector is ignored when evaluating an edge.
#[test]
fn test_matches_edge_jurisdiction_selector_is_skipped() {
    let ss = SelectorSet::from_selectors(vec![Selector::Jurisdiction(country_code("DE"))]);
    let edge = supplies_edge("e1", "a", "b");
    assert!(ss.matches_edge(&edge));
}

/// `IdentifierScheme` selector is ignored when evaluating an edge.
#[test]
fn test_matches_edge_identifier_scheme_selector_is_skipped() {
    let ss = SelectorSet::from_selectors(vec![Selector::IdentifierScheme("lei".to_owned())]);
    let edge = supplies_edge("e1", "a", "b");
    assert!(ss.matches_edge(&edge));
}

/// Name selector is ignored when evaluating an edge.
#[test]
fn test_matches_edge_name_selector_is_skipped() {
    let ss = SelectorSet::from_selectors(vec![Selector::Name("acme".to_owned())]);
    let edge = supplies_edge("e1", "a", "b");
    assert!(ss.matches_edge(&edge));
}

/// Edge with matching label key in properties passes `LabelKey` selector.
#[test]
fn test_matches_edge_label_key_match() {
    let ss = SelectorSet::from_selectors(vec![Selector::LabelKey("certified".to_owned())]);
    let mut edge = supplies_edge("e1", "a", "b");
    edge.properties.labels = Some(vec![label("certified", None)]);
    assert!(ss.matches_edge(&edge));
}

/// Edge without labels in properties fails `LabelKey` selector.
#[test]
fn test_matches_edge_label_key_no_labels() {
    let ss = SelectorSet::from_selectors(vec![Selector::LabelKey("certified".to_owned())]);
    let edge = supplies_edge("e1", "a", "b");
    assert!(!ss.matches_edge(&edge));
}

/// Edge with matching (key, value) label in properties passes `LabelKeyValue`.
#[test]
fn test_matches_edge_label_key_value_match() {
    let ss = SelectorSet::from_selectors(vec![Selector::LabelKeyValue(
        "tier".to_owned(),
        "1".to_owned(),
    )]);
    let mut edge = supplies_edge("e1", "a", "b");
    edge.properties.labels = Some(vec![label("tier", Some("1"))]);
    assert!(ss.matches_edge(&edge));
}

/// Edge with wrong value fails `LabelKeyValue` selector.
#[test]
fn test_matches_edge_label_key_value_wrong_value() {
    let ss = SelectorSet::from_selectors(vec![Selector::LabelKeyValue(
        "tier".to_owned(),
        "1".to_owned(),
    )]);
    let mut edge = supplies_edge("e1", "a", "b");
    edge.properties.labels = Some(vec![label("tier", Some("2"))]);
    assert!(!ss.matches_edge(&edge));
}

/// An empty `SelectorSet` matches every edge (no constraints).
#[test]
fn test_matches_edge_empty_selector_set_matches_all() {
    let ss = SelectorSet::default();
    assert!(ss.matches_edge(&supplies_edge("e1", "a", "b")));
    assert!(ss.matches_edge(&ownership_edge("e2", "a", "b")));
}
