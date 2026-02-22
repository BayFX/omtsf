#![allow(clippy::expect_used)]

use super::*;
use crate::enums::{EdgeType, NodeType};
use crate::file::OmtsFile;
use crate::newtypes::{CalendarDate, NodeId};
use crate::structures::{Edge, Node};
use crate::test_helpers::{minimal_file as make_file, typed_edge as edge, typed_node as node};
use crate::types::{DataQuality, Identifier};
use std::collections::BTreeMap;

fn node_with_operator(id: &str, operator_id: &str) -> Node {
    let mut n = node(id, NodeType::Facility);
    n.operator = Some(NodeId::try_from(operator_id).expect("valid operator"));
    n
}

fn node_with_identifiers(id: &str, node_type: NodeType, identifiers: Vec<Identifier>) -> Node {
    let mut n = node(id, node_type);
    n.identifiers = Some(identifiers);
    n
}

fn node_with_data_quality(id: &str, node_type: NodeType) -> Node {
    let mut n = node(id, node_type);
    n.data_quality = Some(DataQuality {
        confidence: None,
        source: Some("test".to_owned()),
        last_verified: None,
        extra: BTreeMap::new(),
    });
    n
}

fn edge_with_valid_from(id: &str, edge_type: EdgeType, source: &str, target: &str) -> Edge {
    let mut e = edge(id, edge_type, source, target);
    e.properties.valid_from = Some(CalendarDate::try_from("2020-01-01").expect("valid date"));
    e
}

fn edge_with_tier(id: &str, source: &str, target: &str, tier: u32) -> Edge {
    let mut e = edge(id, EdgeType::Supplies, source, target);
    e.properties.tier = Some(tier);
    e
}

fn edge_with_data_quality(id: &str, edge_type: EdgeType, source: &str, target: &str) -> Edge {
    let mut e = edge(id, edge_type, source, target);
    e.properties.data_quality = Some(DataQuality {
        confidence: None,
        source: Some("test".to_owned()),
        last_verified: None,
        extra: BTreeMap::new(),
    });
    e
}

fn identifier(scheme: &str, value: &str, authority: Option<&str>) -> Identifier {
    Identifier {
        scheme: scheme.to_owned(),
        value: value.to_owned(),
        authority: authority.map(str::to_owned),
        valid_from: None,
        valid_to: None,
        sensitivity: None,
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
fn iso3166_known_codes_accepted() {
    assert!(is_valid_iso3166_alpha2("DE"));
    assert!(is_valid_iso3166_alpha2("GB"));
    assert!(is_valid_iso3166_alpha2("US"));
    assert!(is_valid_iso3166_alpha2("FR"));
    assert!(is_valid_iso3166_alpha2("JP"));
    assert!(is_valid_iso3166_alpha2("CN"));
    assert!(is_valid_iso3166_alpha2("ZW")); // last code in the list
    assert!(is_valid_iso3166_alpha2("AD")); // first code in the list
}

#[test]
fn iso3166_invalid_codes_rejected() {
    assert!(!is_valid_iso3166_alpha2("XX")); // not assigned
    assert!(!is_valid_iso3166_alpha2("de")); // lowercase
    assert!(!is_valid_iso3166_alpha2("DEU")); // three letters
    assert!(!is_valid_iso3166_alpha2("")); // empty
    assert!(!is_valid_iso3166_alpha2("1A")); // digit prefix
    assert!(!is_valid_iso3166_alpha2("EU")); // political union, not ISO 3166-1
}

#[test]
fn gdm01_facility_with_operates_edge_passes() {
    let file = make_file(
        vec![
            node("org-1", NodeType::Organization),
            node("fac-1", NodeType::Facility),
        ],
        vec![edge("e-1", EdgeType::Operates, "org-1", "fac-1")],
    );
    let diags = run_rule(&L2Gdm01, &file);
    assert!(
        diags.is_empty(),
        "connected facility must produce no warning"
    );
}

#[test]
fn gdm01_facility_with_operator_property_passes() {
    let file = make_file(
        vec![
            node("org-1", NodeType::Organization),
            node_with_operator("fac-1", "org-1"),
        ],
        vec![],
    );
    let diags = run_rule(&L2Gdm01, &file);
    assert!(
        diags.is_empty(),
        "facility with operator property must produce no warning"
    );
}

#[test]
fn gdm01_isolated_facility_produces_warning() {
    let file = make_file(vec![node("fac-1", NodeType::Facility)], vec![]);
    let diags = run_rule(&L2Gdm01, &file);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].rule_id, RuleId::L2Gdm01);
    assert_eq!(diags[0].severity, Severity::Warning);
    assert!(diags[0].message.contains("fac-1"));
}

#[test]
fn gdm01_multiple_isolated_facilities_all_warned() {
    let file = make_file(
        vec![
            node("fac-1", NodeType::Facility),
            node("fac-2", NodeType::Facility),
            node("org-1", NodeType::Organization),
        ],
        vec![],
    );
    let diags = run_rule(&L2Gdm01, &file);
    assert_eq!(diags.len(), 2, "both facilities should be warned");
    let ids: Vec<_> = diags.iter().map(|d| &d.rule_id).collect();
    assert!(ids.iter().all(|id| **id == RuleId::L2Gdm01));
}

#[test]
fn gdm01_org_node_not_warned() {
    let file = make_file(vec![node("org-1", NodeType::Organization)], vec![]);
    let diags = run_rule(&L2Gdm01, &file);
    assert!(diags.is_empty(), "organisations are not subject to GDM-01");
}

#[test]
fn gdm01_facility_with_operational_control_edge_passes() {
    let file = make_file(
        vec![
            node("org-1", NodeType::Organization),
            node("fac-1", NodeType::Facility),
        ],
        vec![edge("e-1", EdgeType::OperationalControl, "org-1", "fac-1")],
    );
    let diags = run_rule(&L2Gdm01, &file);
    assert!(
        diags.is_empty(),
        "facility connected via operational_control must pass"
    );
}

#[test]
fn gdm01_empty_file_no_diagnostics() {
    let file = make_file(vec![], vec![]);
    let diags = run_rule(&L2Gdm01, &file);
    assert!(diags.is_empty());
}

#[test]
fn gdm02_ownership_with_valid_from_passes() {
    let file = make_file(
        vec![
            node("org-1", NodeType::Organization),
            node("org-2", NodeType::Organization),
        ],
        vec![edge_with_valid_from(
            "e-1",
            EdgeType::Ownership,
            "org-1",
            "org-2",
        )],
    );
    let diags = run_rule(&L2Gdm02, &file);
    assert!(diags.is_empty());
}

#[test]
fn gdm02_ownership_without_valid_from_warns() {
    let file = make_file(
        vec![
            node("org-1", NodeType::Organization),
            node("org-2", NodeType::Organization),
        ],
        vec![edge("e-1", EdgeType::Ownership, "org-1", "org-2")],
    );
    let diags = run_rule(&L2Gdm02, &file);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].rule_id, RuleId::L2Gdm02);
    assert_eq!(diags[0].severity, Severity::Warning);
    assert!(diags[0].message.contains("e-1"));
}

#[test]
fn gdm02_non_ownership_edge_without_valid_from_not_warned() {
    let file = make_file(
        vec![
            node("org-1", NodeType::Organization),
            node("org-2", NodeType::Organization),
        ],
        vec![edge("e-1", EdgeType::Supplies, "org-1", "org-2")],
    );
    let diags = run_rule(&L2Gdm02, &file);
    assert!(diags.is_empty(), "GDM-02 only applies to ownership edges");
}

#[test]
fn gdm02_multiple_ownership_edges_without_valid_from_all_warned() {
    let file = make_file(
        vec![
            node("org-1", NodeType::Organization),
            node("org-2", NodeType::Organization),
        ],
        vec![
            edge("e-1", EdgeType::Ownership, "org-1", "org-2"),
            edge("e-2", EdgeType::Ownership, "org-2", "org-1"),
        ],
    );
    let diags = run_rule(&L2Gdm02, &file);
    assert_eq!(diags.len(), 2);
}

#[test]
fn gdm02_empty_file_no_diagnostics() {
    let file = make_file(vec![], vec![]);
    let diags = run_rule(&L2Gdm02, &file);
    assert!(diags.is_empty());
}

#[test]
fn gdm03_org_with_data_quality_passes() {
    let file = make_file(
        vec![node_with_data_quality("org-1", NodeType::Organization)],
        vec![],
    );
    let diags = run_rule(&L2Gdm03, &file);
    assert!(diags.is_empty());
}

#[test]
fn gdm03_org_without_data_quality_warns() {
    let file = make_file(vec![node("org-1", NodeType::Organization)], vec![]);
    let diags = run_rule(&L2Gdm03, &file);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].rule_id, RuleId::L2Gdm03);
    assert_eq!(diags[0].severity, Severity::Warning);
    assert!(diags[0].message.contains("org-1"));
}

#[test]
fn gdm03_facility_without_data_quality_warns() {
    let file = make_file(vec![node("fac-1", NodeType::Facility)], vec![]);
    let diags = run_rule(&L2Gdm03, &file);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].rule_id, RuleId::L2Gdm03);
    assert!(diags[0].message.contains("fac-1"));
}

#[test]
fn gdm03_good_node_without_data_quality_not_warned() {
    let file = make_file(vec![node("good-1", NodeType::Good)], vec![]);
    let diags = run_rule(&L2Gdm03, &file);
    assert!(diags.is_empty(), "good nodes are not subject to GDM-03");
}

#[test]
fn gdm03_supplies_edge_without_data_quality_warns() {
    let file = make_file(
        vec![
            node("org-1", NodeType::Organization),
            node("org-2", NodeType::Organization),
        ],
        vec![edge("e-1", EdgeType::Supplies, "org-1", "org-2")],
    );
    let diags: Vec<_> = run_rule(&L2Gdm03, &file)
        .into_iter()
        .filter(|d| matches!(&d.location, Location::Edge { .. }))
        .collect();
    assert_eq!(diags.len(), 1);
    assert!(diags[0].message.contains("e-1"));
}

#[test]
fn gdm03_supplies_edge_with_data_quality_passes() {
    let file = make_file(
        vec![
            node("org-1", NodeType::Organization),
            node("org-2", NodeType::Organization),
        ],
        vec![edge_with_data_quality(
            "e-1",
            EdgeType::Supplies,
            "org-1",
            "org-2",
        )],
    );
    let edge_diags: Vec<_> = run_rule(&L2Gdm03, &file)
        .into_iter()
        .filter(|d| matches!(&d.location, Location::Edge { .. }))
        .collect();
    assert!(edge_diags.is_empty());
}

#[test]
fn gdm03_ownership_edge_without_data_quality_not_warned() {
    let file = make_file(
        vec![
            node("org-1", NodeType::Organization),
            node("org-2", NodeType::Organization),
        ],
        vec![edge("e-1", EdgeType::Ownership, "org-1", "org-2")],
    );
    let edge_diags: Vec<_> = run_rule(&L2Gdm03, &file)
        .into_iter()
        .filter(|d| matches!(&d.location, Location::Edge { .. }))
        .collect();
    assert!(edge_diags.is_empty());
}

#[test]
fn gdm04_supplies_tier_with_reporting_entity_passes() {
    let mut file = make_file(
        vec![
            node("org-1", NodeType::Organization),
            node("org-2", NodeType::Organization),
        ],
        vec![edge_with_tier("e-1", "org-1", "org-2", 1)],
    );
    file.reporting_entity = Some(NodeId::try_from("org-1").expect("valid"));
    let diags = run_rule(&L2Gdm04, &file);
    assert!(diags.is_empty());
}

#[test]
fn gdm04_supplies_tier_without_reporting_entity_warns() {
    let file = make_file(
        vec![
            node("org-1", NodeType::Organization),
            node("org-2", NodeType::Organization),
        ],
        vec![edge_with_tier("e-1", "org-1", "org-2", 1)],
    );
    let diags = run_rule(&L2Gdm04, &file);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].rule_id, RuleId::L2Gdm04);
    assert_eq!(diags[0].severity, Severity::Warning);
    assert!(diags[0].message.contains("e-1"));
}

#[test]
fn gdm04_supplies_without_tier_no_warning() {
    let file = make_file(
        vec![
            node("org-1", NodeType::Organization),
            node("org-2", NodeType::Organization),
        ],
        vec![edge("e-1", EdgeType::Supplies, "org-1", "org-2")],
    );
    let diags = run_rule(&L2Gdm04, &file);
    assert!(diags.is_empty());
}

#[test]
fn gdm04_multiple_tier_edges_without_reporting_entity_all_warned() {
    let file = make_file(
        vec![
            node("org-1", NodeType::Organization),
            node("org-2", NodeType::Organization),
            node("org-3", NodeType::Organization),
        ],
        vec![
            edge_with_tier("e-1", "org-1", "org-2", 1),
            edge_with_tier("e-2", "org-2", "org-3", 2),
        ],
    );
    let diags = run_rule(&L2Gdm04, &file);
    assert_eq!(diags.len(), 2);
}

#[test]
fn eid01_org_with_external_identifier_passes() {
    let file = make_file(
        vec![node_with_identifiers(
            "org-1",
            NodeType::Organization,
            vec![identifier("lei", "5493006MHB84DD0ZWV18", None)],
        )],
        vec![],
    );
    let diags = run_rule(&L2Eid01, &file);
    assert!(diags.is_empty());
}

#[test]
fn eid01_org_with_only_internal_identifier_warns() {
    let file = make_file(
        vec![node_with_identifiers(
            "org-1",
            NodeType::Organization,
            vec![identifier("internal", "V-100234", Some("sap-mm-prod"))],
        )],
        vec![],
    );
    let diags = run_rule(&L2Eid01, &file);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].rule_id, RuleId::L2Eid01);
    assert_eq!(diags[0].severity, Severity::Warning);
    assert!(diags[0].message.contains("org-1"));
}

#[test]
fn eid01_org_with_no_identifiers_warns() {
    let file = make_file(vec![node("org-1", NodeType::Organization)], vec![]);
    let diags = run_rule(&L2Eid01, &file);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].rule_id, RuleId::L2Eid01);
    assert_eq!(diags[0].severity, Severity::Warning);
}

#[test]
fn eid01_org_with_mixed_identifiers_passes() {
    let file = make_file(
        vec![node_with_identifiers(
            "org-1",
            NodeType::Organization,
            vec![
                identifier("internal", "V-100234", Some("sap-mm-prod")),
                identifier("duns", "081466849", None),
            ],
        )],
        vec![],
    );
    let diags = run_rule(&L2Eid01, &file);
    assert!(diags.is_empty());
}

#[test]
fn eid01_facility_node_not_subject_to_rule() {
    let file = make_file(vec![node("fac-1", NodeType::Facility)], vec![]);
    let diags = run_rule(&L2Eid01, &file);
    assert!(diags.is_empty(), "facility nodes are not subject to EID-01");
}

#[test]
fn eid01_multiple_orgs_without_identifiers_all_warned() {
    let file = make_file(
        vec![
            node("org-1", NodeType::Organization),
            node("org-2", NodeType::Organization),
        ],
        vec![],
    );
    let diags = run_rule(&L2Eid01, &file);
    assert_eq!(diags.len(), 2);
}

#[test]
fn eid04_valid_vat_authority_passes() {
    let file = make_file(
        vec![node_with_identifiers(
            "org-1",
            NodeType::Organization,
            vec![identifier("vat", "DE123456789", Some("DE"))],
        )],
        vec![],
    );
    let diags = run_rule(&L2Eid04, &file);
    assert!(diags.is_empty());
}

#[test]
fn eid04_invalid_vat_authority_warns() {
    let file = make_file(
        vec![node_with_identifiers(
            "org-1",
            NodeType::Organization,
            vec![identifier("vat", "XX123456789", Some("XX"))],
        )],
        vec![],
    );
    let diags = run_rule(&L2Eid04, &file);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].rule_id, RuleId::L2Eid04);
    assert_eq!(diags[0].severity, Severity::Warning);
    assert!(diags[0].message.contains("XX"));
}

#[test]
fn eid04_missing_vat_authority_not_warned_here() {
    let file = make_file(
        vec![node_with_identifiers(
            "org-1",
            NodeType::Organization,
            vec![identifier("vat", "DE123456789", None)],
        )],
        vec![],
    );
    let diags = run_rule(&L2Eid04, &file);
    assert!(diags.is_empty(), "missing authority is L1-EID-03's concern");
}

#[test]
fn eid04_non_vat_scheme_not_warned() {
    let file = make_file(
        vec![node_with_identifiers(
            "org-1",
            NodeType::Organization,
            vec![identifier("nat-reg", "HRB86891", Some("RA000548"))],
        )],
        vec![],
    );
    let diags = run_rule(&L2Eid04, &file);
    assert!(diags.is_empty(), "nat-reg is not subject to EID-04");
}

#[test]
fn eid04_multiple_invalid_vat_authorities_all_warned() {
    let file = make_file(
        vec![node_with_identifiers(
            "org-1",
            NodeType::Organization,
            vec![
                identifier("vat", "XX123456789", Some("XX")),
                identifier("vat", "EU123456789", Some("EU")),
            ],
        )],
        vec![],
    );
    let diags = run_rule(&L2Eid04, &file);
    assert_eq!(diags.len(), 2);
}

#[test]
fn eid04_lowercase_country_code_warns() {
    let file = make_file(
        vec![node_with_identifiers(
            "org-1",
            NodeType::Organization,
            vec![identifier("vat", "de123456789", Some("de"))],
        )],
        vec![],
    );
    let diags = run_rule(&L2Eid04, &file);
    assert_eq!(diags.len(), 1);
}

#[test]
fn all_l2_rules_produce_warnings_only() {
    // Construct a file that triggers at least one diagnostic from every rule.
    let file = make_file(
        vec![
            node("fac-1", NodeType::Facility),
            node("org-1", NodeType::Organization),
        ],
        vec![
            edge("e-own", EdgeType::Ownership, "org-1", "org-1"),
            edge_with_tier("e-sup", "org-1", "org-1", 1),
        ],
    );

    let rules: Vec<Box<dyn ValidationRule>> = vec![
        Box::new(L2Gdm01),
        Box::new(L2Gdm02),
        Box::new(L2Gdm03),
        Box::new(L2Gdm04),
        Box::new(L2Eid01),
    ];

    for rule in &rules {
        let mut diags = Vec::new();
        rule.check(&file, &mut diags, None);
        for d in &diags {
            assert_eq!(
                d.severity,
                Severity::Warning,
                "rule {} produced a non-Warning diagnostic: {:?}",
                rule.id().code(),
                d
            );
        }
    }

    let file_eid04 = make_file(
        vec![node_with_identifiers(
            "org-2",
            NodeType::Organization,
            vec![identifier("vat", "XX123", Some("XX"))],
        )],
        vec![],
    );
    let mut diags = Vec::new();
    L2Eid04.check(&file_eid04, &mut diags, None);
    for d in &diags {
        assert_eq!(d.severity, Severity::Warning);
    }
}
