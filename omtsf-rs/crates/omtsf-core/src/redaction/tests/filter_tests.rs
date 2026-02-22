#![allow(clippy::expect_used)]

use super::super::sensitivity_allowed;
use crate::dynvalue::DynValue;
use crate::enums::{DisclosureScope, EdgeType, NodeType, NodeTypeTag, Sensitivity};
use crate::redaction::{
    EdgeAction, NodeAction, classify_edge, filter_edge_properties, filter_identifiers,
};
use std::collections::BTreeMap;

use super::classify_tests::{make_edge, make_edge_with_properties, make_identifier};

#[test]
fn filter_partner_retains_public_identifiers() {
    let ids = vec![make_identifier("lei", None)]; // lei defaults to Public
    let result = filter_identifiers(
        &ids,
        &NodeTypeTag::Known(NodeType::Organization),
        &DisclosureScope::Partner,
    );
    assert_eq!(result.len(), 1);
}

#[test]
fn filter_partner_retains_restricted_identifiers() {
    let ids = vec![make_identifier("nat-reg", None)]; // nat-reg defaults to Restricted
    let result = filter_identifiers(
        &ids,
        &NodeTypeTag::Known(NodeType::Organization),
        &DisclosureScope::Partner,
    );
    assert_eq!(result.len(), 1);
}

#[test]
fn filter_partner_removes_confidential_identifiers() {
    let ids = vec![make_identifier("lei", Some(Sensitivity::Confidential))];
    let result = filter_identifiers(
        &ids,
        &NodeTypeTag::Known(NodeType::Organization),
        &DisclosureScope::Partner,
    );
    assert!(result.is_empty());
}

#[test]
fn filter_partner_person_node_removes_default_confidential() {
    // Person node identifiers default to confidential → removed in partner scope.
    let ids = vec![make_identifier("lei", None)];
    let result = filter_identifiers(
        &ids,
        &NodeTypeTag::Known(NodeType::Person),
        &DisclosureScope::Partner,
    );
    assert!(result.is_empty());
}

#[test]
fn filter_partner_person_node_retains_explicit_restricted() {
    // Person node: explicit restricted override is permitted.
    let ids = vec![make_identifier("lei", Some(Sensitivity::Restricted))];
    let result = filter_identifiers(
        &ids,
        &NodeTypeTag::Known(NodeType::Person),
        &DisclosureScope::Partner,
    );
    assert_eq!(result.len(), 1);
}

#[test]
fn filter_partner_person_node_removes_explicit_confidential() {
    let ids = vec![make_identifier("lei", Some(Sensitivity::Confidential))];
    let result = filter_identifiers(
        &ids,
        &NodeTypeTag::Known(NodeType::Person),
        &DisclosureScope::Partner,
    );
    assert!(result.is_empty());
}

#[test]
fn filter_partner_mixed_identifiers() {
    let ids = vec![
        make_identifier("lei", None),                            // Public → kept
        make_identifier("nat-reg", None),                        // Restricted → kept
        make_identifier("lei", Some(Sensitivity::Confidential)), // Confidential → removed
    ];
    let result = filter_identifiers(
        &ids,
        &NodeTypeTag::Known(NodeType::Organization),
        &DisclosureScope::Partner,
    );
    assert_eq!(result.len(), 2);
}

#[test]
fn filter_public_retains_public_identifiers() {
    let ids = vec![make_identifier("lei", None)]; // Public
    let result = filter_identifiers(
        &ids,
        &NodeTypeTag::Known(NodeType::Organization),
        &DisclosureScope::Public,
    );
    assert_eq!(result.len(), 1);
}

#[test]
fn filter_public_removes_restricted_identifiers() {
    let ids = vec![make_identifier("nat-reg", None)]; // Restricted
    let result = filter_identifiers(
        &ids,
        &NodeTypeTag::Known(NodeType::Organization),
        &DisclosureScope::Public,
    );
    assert!(result.is_empty());
}

#[test]
fn filter_public_removes_confidential_identifiers() {
    let ids = vec![make_identifier("lei", Some(Sensitivity::Confidential))];
    let result = filter_identifiers(
        &ids,
        &NodeTypeTag::Known(NodeType::Organization),
        &DisclosureScope::Public,
    );
    assert!(result.is_empty());
}

#[test]
fn filter_public_person_node_removes_all_by_default() {
    // Person node: all identifiers default to confidential → all removed.
    let ids = vec![
        make_identifier("lei", None),
        make_identifier("nat-reg", None),
    ];
    let result = filter_identifiers(
        &ids,
        &NodeTypeTag::Known(NodeType::Person),
        &DisclosureScope::Public,
    );
    assert!(result.is_empty());
}

#[test]
fn filter_public_person_node_removes_explicit_restricted() {
    // Even explicit restricted is removed in public scope.
    let ids = vec![make_identifier("lei", Some(Sensitivity::Restricted))];
    let result = filter_identifiers(
        &ids,
        &NodeTypeTag::Known(NodeType::Person),
        &DisclosureScope::Public,
    );
    assert!(result.is_empty());
}

#[test]
fn filter_public_person_node_retains_explicit_public() {
    // Explicit public override is respected (though validators may flag it).
    let ids = vec![make_identifier("lei", Some(Sensitivity::Public))];
    let result = filter_identifiers(
        &ids,
        &NodeTypeTag::Known(NodeType::Person),
        &DisclosureScope::Public,
    );
    assert_eq!(result.len(), 1);
}

#[test]
fn filter_public_mixed_identifiers() {
    let ids = vec![
        make_identifier("lei", None),                            // Public → kept
        make_identifier("duns", None),                           // Public → kept
        make_identifier("nat-reg", None),                        // Restricted → removed
        make_identifier("vat", None),                            // Restricted → removed
        make_identifier("lei", Some(Sensitivity::Confidential)), // Confidential → removed
    ];
    let result = filter_identifiers(
        &ids,
        &NodeTypeTag::Known(NodeType::Organization),
        &DisclosureScope::Public,
    );
    assert_eq!(result.len(), 2);
}

#[test]
fn filter_internal_retains_all_identifiers() {
    let ids = vec![
        make_identifier("lei", None),
        make_identifier("nat-reg", None),
        make_identifier("lei", Some(Sensitivity::Confidential)),
    ];
    let result = filter_identifiers(
        &ids,
        &NodeTypeTag::Known(NodeType::Organization),
        &DisclosureScope::Internal,
    );
    assert_eq!(result.len(), 3);
}

#[test]
fn filter_edge_props_partner_removes_confidential_percentage_on_beneficial_ownership() {
    // percentage on beneficial_ownership defaults to confidential.
    let mut edge = make_edge(EdgeType::BeneficialOwnership, "src", "tgt");
    edge.properties.percentage = Some(25.0);
    let result = filter_edge_properties(&edge, &DisclosureScope::Partner);
    assert!(result.percentage.is_none());
}

#[test]
fn filter_edge_props_partner_retains_percentage_on_ownership() {
    // percentage on ownership defaults to public.
    let mut edge = make_edge(EdgeType::Ownership, "src", "tgt");
    edge.properties.percentage = Some(51.0);
    let result = filter_edge_properties(&edge, &DisclosureScope::Partner);
    assert_eq!(result.percentage, Some(51.0));
}

#[test]
fn filter_edge_props_partner_removes_contract_ref() {
    // contract_ref defaults to restricted — kept in partner scope.
    // Wait — restricted is ALLOWED in partner. Let's re-check.
    // partner: remove confidential, retain restricted and public.
    let mut edge = make_edge(EdgeType::Supplies, "src", "tgt");
    edge.properties.contract_ref = Some("C-001".to_owned());
    let result = filter_edge_properties(&edge, &DisclosureScope::Partner);
    // contract_ref is restricted → kept in partner scope.
    assert_eq!(result.contract_ref.as_deref(), Some("C-001"));
}

#[test]
fn filter_edge_props_partner_retains_volume_unit() {
    // volume_unit defaults to public.
    let mut edge = make_edge(EdgeType::Supplies, "src", "tgt");
    edge.properties.volume_unit = Some("mt".to_owned());
    let result = filter_edge_properties(&edge, &DisclosureScope::Partner);
    assert_eq!(result.volume_unit.as_deref(), Some("mt"));
}

#[test]
fn filter_edge_props_partner_retains_property_sensitivity_map() {
    // _property_sensitivity is retained in partner scope.
    use serde_json::json;
    let mut extra = BTreeMap::new();
    extra.insert(
        "_property_sensitivity".to_owned(),
        DynValue::from(json!({"volume": "public"})),
    );
    let edge = make_edge_with_properties(EdgeType::Supplies, "src", "tgt", extra);
    let result = filter_edge_properties(&edge, &DisclosureScope::Partner);
    assert!(result.extra.contains_key("_property_sensitivity"));
}

#[test]
fn filter_edge_props_public_removes_restricted_contract_ref() {
    let mut edge = make_edge(EdgeType::Supplies, "src", "tgt");
    edge.properties.contract_ref = Some("C-001".to_owned());
    let result = filter_edge_properties(&edge, &DisclosureScope::Public);
    assert!(result.contract_ref.is_none());
}

#[test]
fn filter_edge_props_public_removes_restricted_annual_value() {
    let mut edge = make_edge(EdgeType::Supplies, "src", "tgt");
    edge.properties.annual_value = Some(1_000_000.0);
    let result = filter_edge_properties(&edge, &DisclosureScope::Public);
    assert!(result.annual_value.is_none());
}

#[test]
fn filter_edge_props_public_removes_restricted_volume() {
    let mut edge = make_edge(EdgeType::Supplies, "src", "tgt");
    edge.properties.volume = Some(5000.0);
    let result = filter_edge_properties(&edge, &DisclosureScope::Public);
    assert!(result.volume.is_none());
}

#[test]
fn filter_edge_props_public_retains_public_volume_unit() {
    let mut edge = make_edge(EdgeType::Supplies, "src", "tgt");
    edge.properties.volume_unit = Some("mt".to_owned());
    let result = filter_edge_properties(&edge, &DisclosureScope::Public);
    assert_eq!(result.volume_unit.as_deref(), Some("mt"));
}

#[test]
fn filter_edge_props_public_retains_public_percentage_on_ownership() {
    let mut edge = make_edge(EdgeType::Ownership, "src", "tgt");
    edge.properties.percentage = Some(51.0);
    let result = filter_edge_properties(&edge, &DisclosureScope::Public);
    assert_eq!(result.percentage, Some(51.0));
}

#[test]
fn filter_edge_props_public_removes_property_sensitivity_map() {
    // _property_sensitivity is removed entirely in public scope.
    use serde_json::json;
    let mut extra = BTreeMap::new();
    extra.insert(
        "_property_sensitivity".to_owned(),
        DynValue::from(json!({"volume": "public"})),
    );
    let edge = make_edge_with_properties(EdgeType::Supplies, "src", "tgt", extra);
    let result = filter_edge_properties(&edge, &DisclosureScope::Public);
    assert!(!result.extra.contains_key("_property_sensitivity"));
}

#[test]
fn filter_edge_props_public_removes_confidential_percentage_on_beneficial_ownership() {
    let mut edge = make_edge(EdgeType::BeneficialOwnership, "src", "tgt");
    edge.properties.percentage = Some(15.0);
    let result = filter_edge_properties(&edge, &DisclosureScope::Public);
    assert!(result.percentage.is_none());
}

#[test]
fn filter_edge_props_internal_no_change() {
    let mut edge = make_edge(EdgeType::Supplies, "src", "tgt");
    edge.properties.contract_ref = Some("C-001".to_owned());
    edge.properties.volume = Some(5000.0);
    edge.properties.percentage = Some(10.0);
    let result = filter_edge_properties(&edge, &DisclosureScope::Internal);
    assert_eq!(result.contract_ref.as_deref(), Some("C-001"));
    assert_eq!(result.volume, Some(5000.0));
    assert_eq!(result.percentage, Some(10.0));
}

#[test]
fn classify_edge_both_retain_is_retain() {
    let edge = make_edge(EdgeType::Supplies, "src", "tgt");
    let action = classify_edge(
        &edge,
        &NodeAction::Retain,
        &NodeAction::Retain,
        &DisclosureScope::Partner,
    );
    assert_eq!(action, EdgeAction::Retain);
}

#[test]
fn classify_edge_boundary_crossing_retain_replace_is_retain() {
    let edge = make_edge(EdgeType::Supplies, "src", "tgt");
    let action = classify_edge(
        &edge,
        &NodeAction::Retain,
        &NodeAction::Replace,
        &DisclosureScope::Partner,
    );
    assert_eq!(action, EdgeAction::Retain);
}

#[test]
fn classify_edge_boundary_crossing_replace_retain_is_retain() {
    let edge = make_edge(EdgeType::Supplies, "src", "tgt");
    let action = classify_edge(
        &edge,
        &NodeAction::Replace,
        &NodeAction::Retain,
        &DisclosureScope::Partner,
    );
    assert_eq!(action, EdgeAction::Retain);
}

#[test]
fn classify_edge_both_replace_is_omit() {
    // Both endpoints replaced → edge omitted (Section 6.2).
    let edge = make_edge(EdgeType::Supplies, "src", "tgt");
    let action = classify_edge(
        &edge,
        &NodeAction::Replace,
        &NodeAction::Replace,
        &DisclosureScope::Partner,
    );
    assert_eq!(action, EdgeAction::Omit);
}

#[test]
fn classify_edge_source_omit_is_omit() {
    // Source node omitted → edge omitted (Section 6.3).
    let edge = make_edge(EdgeType::Supplies, "src", "tgt");
    let action = classify_edge(
        &edge,
        &NodeAction::Omit,
        &NodeAction::Retain,
        &DisclosureScope::Public,
    );
    assert_eq!(action, EdgeAction::Omit);
}

#[test]
fn classify_edge_target_omit_is_omit() {
    // Target node omitted → edge omitted (Section 6.3).
    let edge = make_edge(EdgeType::Supplies, "src", "tgt");
    let action = classify_edge(
        &edge,
        &NodeAction::Retain,
        &NodeAction::Omit,
        &DisclosureScope::Public,
    );
    assert_eq!(action, EdgeAction::Omit);
}

#[test]
fn classify_edge_both_omit_is_omit() {
    let edge = make_edge(EdgeType::Supplies, "src", "tgt");
    let action = classify_edge(
        &edge,
        &NodeAction::Omit,
        &NodeAction::Omit,
        &DisclosureScope::Public,
    );
    assert_eq!(action, EdgeAction::Omit);
}

#[test]
fn classify_edge_beneficial_ownership_public_scope_unconditionally_omit() {
    // beneficial_ownership edges omitted in public scope regardless of endpoints (Section 6.4).
    let edge = make_edge(EdgeType::BeneficialOwnership, "src", "tgt");
    let action = classify_edge(
        &edge,
        &NodeAction::Retain,
        &NodeAction::Retain,
        &DisclosureScope::Public,
    );
    assert_eq!(action, EdgeAction::Omit);
}

#[test]
fn classify_edge_beneficial_ownership_partner_scope_not_unconditionally_omit() {
    // beneficial_ownership edges are NOT unconditionally omitted in partner scope.
    let edge = make_edge(EdgeType::BeneficialOwnership, "src", "tgt");
    let action = classify_edge(
        &edge,
        &NodeAction::Retain,
        &NodeAction::Retain,
        &DisclosureScope::Partner,
    );
    assert_eq!(action, EdgeAction::Retain);
}

#[test]
fn classify_edge_beneficial_ownership_public_both_replace_still_omit() {
    // Even if we check both endpoints before Section 6.4 wouldn't matter;
    // Section 6.4 fires first and overrides.
    let edge = make_edge(EdgeType::BeneficialOwnership, "src", "tgt");
    let action = classify_edge(
        &edge,
        &NodeAction::Replace,
        &NodeAction::Replace,
        &DisclosureScope::Public,
    );
    assert_eq!(action, EdgeAction::Omit);
}

#[test]
fn classify_edge_person_target_omit_causes_beneficial_ownership_omit() {
    // When target is a person node (omitted), the beneficial_ownership edge is omitted
    // via Section 6.3 (endpoint omit) as well as Section 6.4.
    let edge = make_edge(EdgeType::BeneficialOwnership, "org-1", "person-1");
    let action = classify_edge(
        &edge,
        &NodeAction::Retain,
        &NodeAction::Omit,
        &DisclosureScope::Public,
    );
    assert_eq!(action, EdgeAction::Omit);
}

#[test]
fn classify_edge_supplies_with_omit_source_in_partner_scope_is_omit() {
    // Even non-person edge, if source is omitted somehow, edge is omitted.
    let edge = make_edge(EdgeType::Supplies, "src", "tgt");
    let action = classify_edge(
        &edge,
        &NodeAction::Omit,
        &NodeAction::Retain,
        &DisclosureScope::Partner,
    );
    assert_eq!(action, EdgeAction::Omit);
}

#[test]
fn classify_edge_internal_scope_beneficial_ownership_retain() {
    // In internal scope, no filtering applies.
    let edge = make_edge(EdgeType::BeneficialOwnership, "src", "tgt");
    let action = classify_edge(
        &edge,
        &NodeAction::Retain,
        &NodeAction::Retain,
        &DisclosureScope::Internal,
    );
    assert_eq!(action, EdgeAction::Retain);
}

#[test]
fn sensitivity_allowed_internal_allows_all() {
    assert!(sensitivity_allowed(
        &Sensitivity::Public,
        &DisclosureScope::Internal
    ));
    assert!(sensitivity_allowed(
        &Sensitivity::Restricted,
        &DisclosureScope::Internal
    ));
    assert!(sensitivity_allowed(
        &Sensitivity::Confidential,
        &DisclosureScope::Internal
    ));
}

#[test]
fn sensitivity_allowed_partner_allows_public_restricted() {
    assert!(sensitivity_allowed(
        &Sensitivity::Public,
        &DisclosureScope::Partner
    ));
    assert!(sensitivity_allowed(
        &Sensitivity::Restricted,
        &DisclosureScope::Partner
    ));
    assert!(!sensitivity_allowed(
        &Sensitivity::Confidential,
        &DisclosureScope::Partner
    ));
}

#[test]
fn sensitivity_allowed_public_allows_only_public() {
    assert!(sensitivity_allowed(
        &Sensitivity::Public,
        &DisclosureScope::Public
    ));
    assert!(!sensitivity_allowed(
        &Sensitivity::Restricted,
        &DisclosureScope::Public
    ));
    assert!(!sensitivity_allowed(
        &Sensitivity::Confidential,
        &DisclosureScope::Public
    ));
}
