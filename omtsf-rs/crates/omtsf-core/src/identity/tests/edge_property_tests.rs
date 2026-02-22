#![allow(clippy::expect_used)]
#![allow(clippy::field_reassign_with_default)]

use crate::dynvalue::DynValue;
use serde_json::json;

use crate::newtypes::CalendarDate;

use crate::identity::{edge_identity_properties_match, edges_match};

use super::edge_tests::{make_edge, with_edge_identifiers, with_edge_properties};
use super::identifier_tests::{make_id, with_valid_from, with_valid_to_date};

#[test]
fn ownership_same_percentage_and_direct_matches() {
    use crate::enums::{EdgeType, EdgeTypeTag};
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
    use crate::enums::{EdgeType, EdgeTypeTag};
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
    use crate::enums::{EdgeType, EdgeTypeTag};
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
    use crate::enums::{EdgeType, EdgeTypeTag};
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
    use crate::enums::{EdgeType, EdgeTypeTag};
    let mut a = crate::structures::EdgeProperties::default();
    a.control_type = Some(DynValue::from(json!("franchise")));
    let mut b = crate::structures::EdgeProperties::default();
    b.control_type = Some(DynValue::from(json!("franchise")));
    assert!(edge_identity_properties_match(
        &EdgeTypeTag::Known(EdgeType::OperationalControl),
        &a,
        &b
    ));
}

#[test]
fn operational_control_different_control_type_no_match() {
    use crate::enums::{EdgeType, EdgeTypeTag};
    let mut a = crate::structures::EdgeProperties::default();
    a.control_type = Some(DynValue::from(json!("franchise")));
    let mut b = crate::structures::EdgeProperties::default();
    b.control_type = Some(DynValue::from(json!("management")));
    assert!(!edge_identity_properties_match(
        &EdgeTypeTag::Known(EdgeType::OperationalControl),
        &a,
        &b
    ));
}

#[test]
fn legal_parentage_same_consolidation_basis_matches() {
    use crate::enums::{ConsolidationBasis, EdgeType, EdgeTypeTag};
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
    use crate::enums::{ConsolidationBasis, EdgeType, EdgeTypeTag};
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
    use crate::enums::{EdgeType, EdgeTypeTag, EventType};
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
    use crate::enums::{EdgeType, EdgeTypeTag, EventType};
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
    use crate::enums::{EdgeType, EdgeTypeTag};
    let mut a = crate::structures::EdgeProperties::default();
    a.control_type = Some(DynValue::from(json!("management")));
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
    use crate::enums::{EdgeType, EdgeTypeTag};
    let mut a = crate::structures::EdgeProperties::default();
    a.control_type = Some(DynValue::from(json!("management")));
    a.percentage = Some(25.0);
    let mut b = crate::structures::EdgeProperties::default();
    b.control_type = Some(DynValue::from(json!("management")));
    b.percentage = Some(30.0);
    assert!(!edge_identity_properties_match(
        &EdgeTypeTag::Known(EdgeType::BeneficialOwnership),
        &a,
        &b
    ));
}

#[test]
fn supplies_same_commodity_and_contract_matches() {
    use crate::enums::{EdgeType, EdgeTypeTag};
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
    use crate::enums::{EdgeType, EdgeTypeTag};
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
    use crate::enums::{EdgeType, EdgeTypeTag};
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
    use crate::enums::{EdgeType, EdgeTypeTag};
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
    use crate::enums::{EdgeType, EdgeTypeTag};
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
    use crate::enums::{EdgeType, EdgeTypeTag, ServiceType};
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
    use crate::enums::{EdgeType, EdgeTypeTag, ServiceType};
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
    use crate::enums::{EdgeType, EdgeTypeTag};
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
    use crate::enums::{EdgeType, EdgeTypeTag};
    // type + endpoints suffice
    assert!(edge_identity_properties_match(
        &EdgeTypeTag::Known(EdgeType::Operates),
        &crate::structures::EdgeProperties::default(),
        &crate::structures::EdgeProperties::default()
    ));
}

#[test]
fn produces_always_matches() {
    use crate::enums::{EdgeType, EdgeTypeTag};
    assert!(edge_identity_properties_match(
        &EdgeTypeTag::Known(EdgeType::Produces),
        &crate::structures::EdgeProperties::default(),
        &crate::structures::EdgeProperties::default()
    ));
}

#[test]
fn composed_of_always_matches() {
    use crate::enums::{EdgeType, EdgeTypeTag};
    assert!(edge_identity_properties_match(
        &EdgeTypeTag::Known(EdgeType::ComposedOf),
        &crate::structures::EdgeProperties::default(),
        &crate::structures::EdgeProperties::default()
    ));
}

#[test]
fn sells_to_same_commodity_and_contract_matches() {
    use crate::enums::{EdgeType, EdgeTypeTag};
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
    use crate::enums::{EdgeType, EdgeTypeTag};
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
    use crate::enums::{EdgeType, EdgeTypeTag};
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
    use crate::enums::{EdgeType, EdgeTypeTag};
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
        &crate::enums::EdgeTypeTag::Extension("com.acme.custom".to_owned()),
        &crate::structures::EdgeProperties::default(),
        &crate::structures::EdgeProperties::default()
    ));
}

#[test]
fn edges_match_same_as_is_never_a_candidate() {
    use crate::enums::{EdgeType, EdgeTypeTag};
    let a = make_edge("e1", EdgeTypeTag::Known(EdgeType::SameAs), "s", "t");
    let b = make_edge("e2", EdgeTypeTag::Known(EdgeType::SameAs), "s", "t");
    assert!(!edges_match(0, 1, 0, 1, &a, &b));
}

#[test]
fn edges_match_same_as_on_a_is_never_a_candidate() {
    use crate::enums::{EdgeType, EdgeTypeTag};
    let a = make_edge("e1", EdgeTypeTag::Known(EdgeType::SameAs), "s", "t");
    let b = make_edge("e2", EdgeTypeTag::Known(EdgeType::Supplies), "s", "t");
    assert!(!edges_match(0, 1, 0, 1, &a, &b));
}

#[test]
fn edges_match_different_source_rep_no_match() {
    use crate::enums::{EdgeType, EdgeTypeTag};
    let a = make_edge("e1", EdgeTypeTag::Known(EdgeType::Supplies), "s", "t");
    let b = make_edge("e2", EdgeTypeTag::Known(EdgeType::Supplies), "s", "t");
    // source_rep_a=0 vs source_rep_b=1 — different
    assert!(!edges_match(0, 1, 1, 1, &a, &b));
}

#[test]
fn edges_match_different_target_rep_no_match() {
    use crate::enums::{EdgeType, EdgeTypeTag};
    let a = make_edge("e1", EdgeTypeTag::Known(EdgeType::Supplies), "s", "t");
    let b = make_edge("e2", EdgeTypeTag::Known(EdgeType::Supplies), "s", "t");
    assert!(!edges_match(0, 1, 0, 2, &a, &b));
}

#[test]
fn edges_match_different_type_no_match() {
    use crate::enums::{EdgeType, EdgeTypeTag};
    let a = make_edge("e1", EdgeTypeTag::Known(EdgeType::Supplies), "s", "t");
    let b = make_edge("e2", EdgeTypeTag::Known(EdgeType::Ownership), "s", "t");
    assert!(!edges_match(0, 1, 0, 1, &a, &b));
}

#[test]
fn edges_match_by_shared_external_identifier() {
    use crate::enums::{EdgeType, EdgeTypeTag};
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
    use crate::enums::{EdgeType, EdgeTypeTag};
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
    use crate::enums::{EdgeType, EdgeTypeTag};
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
    use crate::enums::{EdgeType, EdgeTypeTag};
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
    use crate::enums::{EdgeType, EdgeTypeTag};
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
    use crate::enums::{EdgeType, EdgeTypeTag};
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
    use crate::enums::{EdgeType, EdgeTypeTag};
    // operates: type + endpoints suffice; no properties needed.
    let a = make_edge("e1", EdgeTypeTag::Known(EdgeType::Operates), "s", "t");
    let b = make_edge("e2", EdgeTypeTag::Known(EdgeType::Operates), "s", "t");
    assert!(edges_match(0, 1, 0, 1, &a, &b));
}

#[test]
fn edges_match_symmetry() {
    use crate::enums::{EdgeType, EdgeTypeTag};
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
    use crate::enums::{EdgeType, EdgeTypeTag};
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
    use crate::enums::{EdgeType, EdgeTypeTag};
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
