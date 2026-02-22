#![allow(clippy::expect_used)]

use super::*;
use crate::enums::{EdgeType, NodeType};
use crate::file::OmtsFile;
use crate::newtypes::NodeId;
use crate::structures::{Edge, Node};
use crate::test_helpers::{extension_node, minimal_file, typed_edge as edge, typed_node as node};

fn make_file(nodes: Vec<Node>, edges: Vec<Edge>) -> OmtsFile {
    minimal_file(nodes, edges)
}

fn extension_edge(id: &str, type_str: &str, source: &str, target: &str) -> Edge {
    crate::test_helpers::extension_edge(id, source, target, type_str)
}

fn run_rule(rule: &dyn ValidationRule, file: &OmtsFile) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    rule.check(file, &mut diags, None);
    diags
}

fn rule_ids(diags: &[Diagnostic]) -> Vec<&RuleId> {
    diags.iter().map(|d| &d.rule_id).collect()
}

#[test]
fn gdm01_clean_no_diagnostics() {
    let file = make_file(
        vec![
            node("org-1", NodeType::Organization),
            node("org-2", NodeType::Organization),
        ],
        vec![],
    );
    let diags = run_rule(&GdmRule01, &file);
    assert!(diags.is_empty(), "no duplicate ids → no diagnostics");
}

#[test]
fn gdm01_duplicate_node_id_detected() {
    let file = make_file(
        vec![
            node("dup-id", NodeType::Organization),
            node("dup-id", NodeType::Facility),
        ],
        vec![],
    );
    let diags = run_rule(&GdmRule01, &file);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].rule_id, RuleId::L1Gdm01);
    assert_eq!(diags[0].severity, Severity::Error);
    assert!(diags[0].message.contains("dup-id"));
}

#[test]
fn gdm01_multiple_duplicates_all_collected() {
    let file = make_file(
        vec![
            node("same", NodeType::Organization),
            node("same", NodeType::Facility),
            node("same", NodeType::Good),
            node("unique", NodeType::Person),
        ],
        vec![],
    );
    let diags = run_rule(&GdmRule01, &file);
    assert_eq!(diags.len(), 2, "second and third occurrence each trigger");
    assert!(rule_ids(&diags).iter().all(|id| **id == RuleId::L1Gdm01));
}

#[test]
fn gdm01_empty_graph_no_diagnostics() {
    let file = make_file(vec![], vec![]);
    let diags = run_rule(&GdmRule01, &file);
    assert!(diags.is_empty());
}

#[test]
fn gdm02_clean_no_diagnostics() {
    let file = make_file(
        vec![
            node("org-1", NodeType::Organization),
            node("org-2", NodeType::Organization),
        ],
        vec![edge("e-1", EdgeType::Supplies, "org-1", "org-2")],
    );
    let diags = run_rule(&GdmRule02, &file);
    assert!(diags.is_empty());
}

#[test]
fn gdm02_duplicate_edge_id_detected() {
    let file = make_file(
        vec![
            node("org-1", NodeType::Organization),
            node("org-2", NodeType::Organization),
        ],
        vec![
            edge("dup-edge", EdgeType::Supplies, "org-1", "org-2"),
            edge("dup-edge", EdgeType::Ownership, "org-1", "org-2"),
        ],
    );
    let diags = run_rule(&GdmRule02, &file);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].rule_id, RuleId::L1Gdm02);
    assert_eq!(diags[0].severity, Severity::Error);
    assert!(diags[0].message.contains("dup-edge"));
}

#[test]
fn gdm02_multiple_duplicate_edge_ids_all_collected() {
    let file = make_file(
        vec![
            node("org-1", NodeType::Organization),
            node("org-2", NodeType::Organization),
        ],
        vec![
            edge("dup", EdgeType::Supplies, "org-1", "org-2"),
            edge("dup", EdgeType::Supplies, "org-1", "org-2"),
            edge("dup", EdgeType::Supplies, "org-1", "org-2"),
            edge("unique", EdgeType::Ownership, "org-1", "org-2"),
        ],
    );
    let diags = run_rule(&GdmRule02, &file);
    assert_eq!(diags.len(), 2);
    assert!(rule_ids(&diags).iter().all(|id| **id == RuleId::L1Gdm02));
}

#[test]
fn gdm02_empty_edges_no_diagnostics() {
    let file = make_file(vec![], vec![]);
    let diags = run_rule(&GdmRule02, &file);
    assert!(diags.is_empty());
}

#[test]
fn gdm03_clean_no_diagnostics() {
    let file = make_file(
        vec![
            node("org-1", NodeType::Organization),
            node("org-2", NodeType::Organization),
        ],
        vec![edge("e-1", EdgeType::Supplies, "org-1", "org-2")],
    );
    let diags = run_rule(&GdmRule03, &file);
    assert!(diags.is_empty());
}

#[test]
fn gdm03_dangling_source_detected() {
    let file = make_file(
        vec![node("org-1", NodeType::Organization)],
        vec![edge("e-1", EdgeType::Supplies, "missing-node", "org-1")],
    );
    let diags = run_rule(&GdmRule03, &file);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].rule_id, RuleId::L1Gdm03);
    assert!(matches!(
        &diags[0].location,
        Location::Edge { field: Some(f), .. } if f == "source"
    ));
    assert!(diags[0].message.contains("missing-node"));
}

#[test]
fn gdm03_dangling_target_detected() {
    let file = make_file(
        vec![node("org-1", NodeType::Organization)],
        vec![edge("e-1", EdgeType::Supplies, "org-1", "missing-node")],
    );
    let diags = run_rule(&GdmRule03, &file);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].rule_id, RuleId::L1Gdm03);
    assert!(matches!(
        &diags[0].location,
        Location::Edge { field: Some(f), .. } if f == "target"
    ));
    assert!(diags[0].message.contains("missing-node"));
}

#[test]
fn gdm03_both_dangling_two_diagnostics() {
    let file = make_file(
        vec![],
        vec![edge(
            "e-1",
            EdgeType::Supplies,
            "src-missing",
            "tgt-missing",
        )],
    );
    let diags = run_rule(&GdmRule03, &file);
    assert_eq!(diags.len(), 2);
    assert!(rule_ids(&diags).iter().all(|id| **id == RuleId::L1Gdm03));
}

#[test]
fn gdm03_all_violations_collected_no_early_exit() {
    let file = make_file(
        vec![node("org-1", NodeType::Organization)],
        vec![
            edge("e-1", EdgeType::Supplies, "org-1", "ghost-1"),
            edge("e-2", EdgeType::Supplies, "ghost-2", "org-1"),
            edge("e-3", EdgeType::Supplies, "ghost-3", "ghost-4"),
        ],
    );
    let diags = run_rule(&GdmRule03, &file);
    assert_eq!(diags.len(), 4);
}

#[test]
fn gdm04_clean_all_known_types_no_diagnostics() {
    let file = make_file(
        vec![
            node("org-1", NodeType::Organization),
            node("org-2", NodeType::Organization),
        ],
        vec![
            edge("e-1", EdgeType::Supplies, "org-1", "org-2"),
            edge("e-2", EdgeType::SameAs, "org-1", "org-2"),
        ],
    );
    let diags = run_rule(&GdmRule04, &file);
    assert!(diags.is_empty());
}

#[test]
fn gdm04_extension_type_with_dot_accepted() {
    let file = make_file(
        vec![
            node("org-1", NodeType::Organization),
            node("org-2", NodeType::Organization),
        ],
        vec![extension_edge(
            "e-1",
            "com.example.custom",
            "org-1",
            "org-2",
        )],
    );
    let diags = run_rule(&GdmRule04, &file);
    assert!(diags.is_empty(), "extension with dot must be accepted");
}

#[test]
fn gdm04_unknown_type_no_dot_rejected() {
    let file = make_file(
        vec![
            node("org-1", NodeType::Organization),
            node("org-2", NodeType::Organization),
        ],
        vec![extension_edge("e-1", "mystery_type", "org-1", "org-2")],
    );
    let diags = run_rule(&GdmRule04, &file);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].rule_id, RuleId::L1Gdm04);
    assert_eq!(diags[0].severity, Severity::Error);
    assert!(diags[0].message.contains("mystery_type"));
}

#[test]
fn gdm04_multiple_bad_types_all_collected() {
    let file = make_file(
        vec![
            node("n-1", NodeType::Organization),
            node("n-2", NodeType::Organization),
        ],
        vec![
            extension_edge("e-1", "bad_type_a", "n-1", "n-2"),
            extension_edge("e-2", "bad_type_b", "n-1", "n-2"),
            edge("e-3", EdgeType::Supplies, "n-1", "n-2"),
        ],
    );
    let diags = run_rule(&GdmRule04, &file);
    assert_eq!(diags.len(), 2);
}

#[test]
fn gdm05_no_reporting_entity_no_diagnostics() {
    let file = make_file(vec![], vec![]);
    let diags = run_rule(&GdmRule05, &file);
    assert!(diags.is_empty());
}

#[test]
fn gdm05_valid_reporting_entity_no_diagnostics() {
    let mut file = make_file(vec![node("org-acme", NodeType::Organization)], vec![]);
    file.reporting_entity = Some(NodeId::try_from("org-acme").expect("valid"));
    let diags = run_rule(&GdmRule05, &file);
    assert!(diags.is_empty());
}

#[test]
fn gdm05_reporting_entity_missing_node_detected() {
    let mut file = make_file(vec![], vec![]);
    file.reporting_entity = Some(NodeId::try_from("ghost-org").expect("valid"));
    let diags = run_rule(&GdmRule05, &file);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].rule_id, RuleId::L1Gdm05);
    assert_eq!(diags[0].severity, Severity::Error);
    assert!(diags[0].message.contains("ghost-org"));
    assert!(matches!(
        diags[0].location,
        Location::Header {
            field: "reporting_entity"
        }
    ));
}

#[test]
fn gdm05_reporting_entity_wrong_type_detected() {
    let mut file = make_file(vec![node("fac-1", NodeType::Facility)], vec![]);
    file.reporting_entity = Some(NodeId::try_from("fac-1").expect("valid"));
    let diags = run_rule(&GdmRule05, &file);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].rule_id, RuleId::L1Gdm05);
    assert!(diags[0].message.contains("fac-1"));
    assert!(diags[0].message.contains("organization"));
}

#[test]
fn gdm05_reporting_entity_references_person_rejected() {
    let mut file = make_file(vec![node("person-1", NodeType::Person)], vec![]);
    file.reporting_entity = Some(NodeId::try_from("person-1").expect("valid"));
    let diags = run_rule(&GdmRule05, &file);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].rule_id, RuleId::L1Gdm05);
}

#[test]
fn gdm06_clean_supplies_org_to_org_no_diagnostics() {
    let file = make_file(
        vec![
            node("org-1", NodeType::Organization),
            node("org-2", NodeType::Organization),
        ],
        vec![edge("e-1", EdgeType::Supplies, "org-1", "org-2")],
    );
    let diags = run_rule(&GdmRule06, &file);
    assert!(diags.is_empty());
}

#[test]
fn gdm06_ownership_wrong_target_type_detected() {
    let file = make_file(
        vec![
            node("org-1", NodeType::Organization),
            node("fac-1", NodeType::Facility),
        ],
        vec![edge("e-1", EdgeType::Ownership, "org-1", "fac-1")],
    );
    let diags = run_rule(&GdmRule06, &file);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].rule_id, RuleId::L1Gdm06);
    assert_eq!(diags[0].severity, Severity::Error);
    assert!(matches!(
        &diags[0].location,
        Location::Edge { field: Some(f), .. } if f == "target"
    ));
}

#[test]
fn gdm06_beneficial_ownership_person_to_org_accepted() {
    let file = make_file(
        vec![
            node("person-1", NodeType::Person),
            node("org-1", NodeType::Organization),
        ],
        vec![edge(
            "e-1",
            EdgeType::BeneficialOwnership,
            "person-1",
            "org-1",
        )],
    );
    let diags = run_rule(&GdmRule06, &file);
    assert!(diags.is_empty());
}

#[test]
fn gdm06_beneficial_ownership_wrong_source_detected() {
    let file = make_file(
        vec![
            node("org-1", NodeType::Organization),
            node("org-2", NodeType::Organization),
        ],
        vec![edge("e-1", EdgeType::BeneficialOwnership, "org-1", "org-2")],
    );
    let diags = run_rule(&GdmRule06, &file);
    assert_eq!(diags.len(), 1);
    assert!(matches!(
        &diags[0].location,
        Location::Edge { field: Some(f), .. } if f == "source"
    ));
}

#[test]
fn gdm06_operates_org_to_facility_accepted() {
    let file = make_file(
        vec![
            node("org-1", NodeType::Organization),
            node("fac-1", NodeType::Facility),
        ],
        vec![edge("e-1", EdgeType::Operates, "org-1", "fac-1")],
    );
    let diags = run_rule(&GdmRule06, &file);
    assert!(diags.is_empty());
}

#[test]
fn gdm06_produces_facility_to_good_accepted() {
    let file = make_file(
        vec![
            node("fac-1", NodeType::Facility),
            node("good-1", NodeType::Good),
        ],
        vec![edge("e-1", EdgeType::Produces, "fac-1", "good-1")],
    );
    let diags = run_rule(&GdmRule06, &file);
    assert!(diags.is_empty());
}

#[test]
fn gdm06_produces_facility_to_consignment_accepted() {
    let file = make_file(
        vec![
            node("fac-1", NodeType::Facility),
            node("cons-1", NodeType::Consignment),
        ],
        vec![edge("e-1", EdgeType::Produces, "fac-1", "cons-1")],
    );
    let diags = run_rule(&GdmRule06, &file);
    assert!(diags.is_empty());
}

#[test]
fn gdm06_attested_by_multiple_source_types_accepted() {
    let file = make_file(
        vec![
            node("org-1", NodeType::Organization),
            node("fac-1", NodeType::Facility),
            node("good-1", NodeType::Good),
            node("cons-1", NodeType::Consignment),
            node("att-1", NodeType::Attestation),
        ],
        vec![
            edge("e-1", EdgeType::AttestedBy, "org-1", "att-1"),
            edge("e-2", EdgeType::AttestedBy, "fac-1", "att-1"),
            edge("e-3", EdgeType::AttestedBy, "good-1", "att-1"),
            edge("e-4", EdgeType::AttestedBy, "cons-1", "att-1"),
        ],
    );
    let diags = run_rule(&GdmRule06, &file);
    assert!(
        diags.is_empty(),
        "all four source types are permitted for attested_by"
    );
}

#[test]
fn gdm06_attested_by_wrong_target_type_detected() {
    let file = make_file(
        vec![
            node("org-1", NodeType::Organization),
            node("org-2", NodeType::Organization),
        ],
        vec![edge("e-1", EdgeType::AttestedBy, "org-1", "org-2")],
    );
    let diags = run_rule(&GdmRule06, &file);
    assert_eq!(diags.len(), 1);
    assert!(matches!(
        &diags[0].location,
        Location::Edge { field: Some(f), .. } if f == "target"
    ));
}

#[test]
fn gdm06_same_as_any_types_accepted() {
    let file = make_file(
        vec![
            node("org-1", NodeType::Organization),
            node("fac-1", NodeType::Facility),
        ],
        vec![edge("e-1", EdgeType::SameAs, "org-1", "fac-1")],
    );
    let diags = run_rule(&GdmRule06, &file);
    assert!(
        diags.is_empty(),
        "same_as has no source/target type constraint"
    );
}

#[test]
fn gdm06_extension_edge_type_exempt() {
    let file = make_file(
        vec![
            node("org-1", NodeType::Organization),
            node("att-1", NodeType::Attestation),
        ],
        vec![extension_edge(
            "e-1",
            "com.example.custom",
            "org-1",
            "att-1",
        )],
    );
    let diags = run_rule(&GdmRule06, &file);
    assert!(diags.is_empty(), "extension edges are exempt from GDM-06");
}

#[test]
fn gdm06_dangling_ref_not_double_reported() {
    // When source doesn't exist, GDM-06 skips it (GDM-03 handles it).
    let file = make_file(
        vec![node("org-1", NodeType::Organization)],
        vec![edge("e-1", EdgeType::Supplies, "ghost", "org-1")],
    );
    let diags = run_rule(&GdmRule06, &file);
    assert!(
        diags.is_empty(),
        "missing nodes are GDM-03's concern, not GDM-06"
    );
}

#[test]
fn gdm06_extension_node_type_not_constrained() {
    let file = make_file(
        vec![
            extension_node("ext-1", "com.example.custom"),
            node("org-1", NodeType::Organization),
        ],
        vec![edge("e-1", EdgeType::Supplies, "ext-1", "org-1")],
    );
    let diags = run_rule(&GdmRule06, &file);
    assert!(
        diags.is_empty(),
        "extension node types are not constrained by GDM-06"
    );
}

#[test]
fn gdm06_all_violations_collected_no_early_exit() {
    let file = make_file(
        vec![
            node("fac-1", NodeType::Facility),
            node("fac-2", NodeType::Facility),
            node("org-1", NodeType::Organization),
        ],
        vec![
            edge("e-1", EdgeType::Ownership, "fac-1", "org-1"),
            edge("e-2", EdgeType::Ownership, "fac-2", "org-1"),
        ],
    );
    let diags = run_rule(&GdmRule06, &file);
    assert_eq!(diags.len(), 2);
    assert!(rule_ids(&diags).iter().all(|id| **id == RuleId::L1Gdm06));
}

#[test]
fn gdm06_composed_of_good_to_consignment_accepted() {
    let file = make_file(
        vec![
            node("good-1", NodeType::Good),
            node("cons-1", NodeType::Consignment),
        ],
        vec![edge("e-1", EdgeType::ComposedOf, "good-1", "cons-1")],
    );
    let diags = run_rule(&GdmRule06, &file);
    assert!(diags.is_empty());
}

#[test]
fn gdm06_operational_control_org_to_facility_accepted() {
    let file = make_file(
        vec![
            node("org-1", NodeType::Organization),
            node("fac-1", NodeType::Facility),
        ],
        vec![edge("e-1", EdgeType::OperationalControl, "org-1", "fac-1")],
    );
    let diags = run_rule(&GdmRule06, &file);
    assert!(diags.is_empty());
}

#[test]
fn gdm06_tolls_facility_source_accepted() {
    let file = make_file(
        vec![
            node("fac-1", NodeType::Facility),
            node("org-1", NodeType::Organization),
        ],
        vec![edge("e-1", EdgeType::Tolls, "fac-1", "org-1")],
    );
    let diags = run_rule(&GdmRule06, &file);
    assert!(diags.is_empty());
}
