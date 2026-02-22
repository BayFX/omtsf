use crate::dynvalue::DynValue;
use std::collections::BTreeMap;
use std::collections::HashSet;

use crate::newtypes::{NodeId, SemVer};
use crate::types::Identifier;

use super::super::engine::{diff, diff_filtered};
use super::super::types::DiffFilter;
use super::{make_file, make_file_b, make_identical_pair, org_node, with_duns, with_lei};

/// Two empty files produce an empty diff.
#[test]
fn diff_two_empty_files() {
    let a = make_file(vec![], vec![]);
    let b = make_file_b(vec![], vec![]);
    let result = diff(&a, &b);
    assert!(result.is_empty());
    assert!(result.warnings.is_empty());
    let summary = result.summary();
    assert_eq!(summary.nodes_added, 0);
    assert_eq!(summary.nodes_removed, 0);
    assert_eq!(summary.nodes_modified, 0);
    assert_eq!(summary.nodes_unchanged, 0);
}

/// Nodes in B with no match in A are additions.
#[test]
fn diff_all_nodes_added() {
    let a = make_file(vec![], vec![]);
    let b = make_file_b(vec![org_node("org-1"), org_node("org-2")], vec![]);
    let result = diff(&a, &b);
    assert_eq!(result.nodes.added.len(), 2);
    assert!(result.nodes.removed.is_empty());
    assert!(result.nodes.unchanged.is_empty());
    assert!(result.nodes.modified.is_empty());
}

/// Nodes in A with no match in B are deletions.
#[test]
fn diff_all_nodes_removed() {
    let a = make_file(vec![org_node("org-1"), org_node("org-2")], vec![]);
    let b = make_file_b(vec![], vec![]);
    let result = diff(&a, &b);
    assert_eq!(result.nodes.removed.len(), 2);
    assert!(result.nodes.added.is_empty());
    assert!(result.nodes.unchanged.is_empty());
}

/// Nodes without external identifiers are never matched (no match group forms).
#[test]
fn diff_nodes_without_identifiers_are_unmatched() {
    // Neither node has identifiers — they cannot match each other.
    let a = make_file(vec![org_node("org-a")], vec![]);
    let b = make_file_b(vec![org_node("org-b")], vec![]);
    let result = diff(&a, &b);
    assert_eq!(result.nodes.removed.len(), 1, "org-a is a deletion");
    assert_eq!(result.nodes.added.len(), 1, "org-b is an addition");
    assert!(result.nodes.unchanged.is_empty());
}

/// Nodes that share a LEI are matched.
#[test]
fn diff_nodes_matched_by_lei() {
    let node_a = with_lei(org_node("org-a"), "LEI0000000000000001");
    let node_b = with_lei(org_node("org-b"), "LEI0000000000000001");
    let a = make_file(vec![node_a], vec![]);
    let b = make_file_b(vec![node_b], vec![]);
    let result = diff(&a, &b);
    assert!(result.nodes.removed.is_empty(), "no deletions expected");
    assert!(result.nodes.added.is_empty(), "no additions expected");
    // The nodes differ in name ("org-a" vs "org-b"), so the pair is modified.
    let total_matched = result.nodes.unchanged.len() + result.nodes.modified.len();
    assert_eq!(total_matched, 1, "one matched pair");
    // Grab the diff from whichever bucket it landed in.
    let nd = if result.nodes.modified.is_empty() {
        &result.nodes.unchanged[0]
    } else {
        &result.nodes.modified[0]
    };
    assert_eq!(nd.id_a, "org-a");
    assert_eq!(nd.id_b, "org-b");
    assert!(
        nd.matched_by
            .iter()
            .any(|k| k.contains("LEI0000000000000001"))
    );
}

/// Nodes matched via transitive closure (A1↔B1 via LEI, A1↔B2 via DUNS).
#[test]
fn diff_node_transitive_match() {
    // org-a carries both LEI and DUNS.
    // org-b1 in B carries only the LEI.
    // org-b2 in B carries only the DUNS.
    // Result: org-a matches both org-b1 and org-b2 (one group of 3 = ambiguous).
    let node_a = with_duns(with_lei(org_node("org-a"), "LEI_TRANS"), "DUNS_TRANS");
    let node_b1 = with_lei(org_node("org-b1"), "LEI_TRANS");
    let node_b2 = with_duns(org_node("org-b2"), "DUNS_TRANS");
    let a = make_file(vec![node_a], vec![]);
    let b = make_file_b(vec![node_b1, node_b2], vec![]);
    let result = diff(&a, &b);
    // Two B nodes in the group → ambiguity warning.
    assert!(
        !result.warnings.is_empty(),
        "expected ambiguity warning for 1 A node matching 2 B nodes"
    );
    // Both pairs should be reported as matched (modified because names differ).
    let total_matched = result.nodes.unchanged.len() + result.nodes.modified.len();
    assert_eq!(total_matched, 2);
    assert!(result.nodes.added.is_empty());
    assert!(result.nodes.removed.is_empty());
}

/// Ambiguity: two nodes in A match the same node in B.
#[test]
fn diff_ambiguous_match_two_a_nodes_same_b() {
    let node_a1 = with_lei(org_node("org-a1"), "LEI_SHARED");
    let node_a2 = with_lei(org_node("org-a2"), "LEI_SHARED");
    let node_b = with_lei(org_node("org-b"), "LEI_SHARED");
    let a = make_file(vec![node_a1, node_a2], vec![]);
    let b = make_file_b(vec![node_b], vec![]);
    let result = diff(&a, &b);
    // A has 2 nodes in the same group → warning.
    assert!(!result.warnings.is_empty(), "expected ambiguity warning");
    // Both A nodes are reported as matched to the one B node (names differ → modified).
    let total_matched = result.nodes.unchanged.len() + result.nodes.modified.len();
    assert_eq!(total_matched, 2);
    assert!(result.nodes.removed.is_empty());
    assert!(result.nodes.added.is_empty());
}

/// Nodes with only `internal` scheme identifiers are never matched.
#[test]
fn diff_internal_identifiers_do_not_cause_match() {
    let mut node_a = org_node("org-a");
    node_a.identifiers = Some(vec![Identifier {
        scheme: "internal".to_owned(),
        value: "sap:001".to_owned(),
        authority: None,
        valid_from: None,
        valid_to: None,
        sensitivity: None,
        verification_status: None,
        verification_date: None,
        extra: BTreeMap::new(),
    }]);
    let mut node_b = org_node("org-b");
    node_b.identifiers = Some(vec![Identifier {
        scheme: "internal".to_owned(),
        value: "sap:001".to_owned(),
        authority: None,
        valid_from: None,
        valid_to: None,
        sensitivity: None,
        verification_status: None,
        verification_date: None,
        extra: BTreeMap::new(),
    }]);
    let a = make_file(vec![node_a], vec![]);
    let b = make_file_b(vec![node_b], vec![]);
    let result = diff(&a, &b);
    // Internal identifiers must not cause a match.
    assert_eq!(result.nodes.removed.len(), 1);
    assert_eq!(result.nodes.added.len(), 1);
    assert!(result.nodes.unchanged.is_empty());
}

/// DiffSummary reflects counts correctly.
#[test]
fn diff_summary_counts() {
    let node_a = with_lei(org_node("org-a"), "LEI_AA");
    let node_b_matched = with_lei(org_node("org-b-match"), "LEI_AA");
    let node_b_added = org_node("org-b-new");

    let a = make_file(vec![node_a], vec![]);
    let b = make_file_b(vec![node_b_matched, node_b_added], vec![]);

    let result = diff(&a, &b);
    let summary = result.summary();
    assert_eq!(summary.nodes_added, 1, "org-b-new is added");
    assert_eq!(summary.nodes_removed, 0);
    // Nodes are matched; names differ ("org-a" vs "org-b-match"), so pair is modified.
    assert_eq!(
        summary.nodes_modified + summary.nodes_unchanged,
        1,
        "one matched pair (modified or unchanged)"
    );
    assert_eq!(summary.edges_added, 0);
    assert_eq!(summary.edges_removed, 0);
}

/// is_empty returns true only when there are no changes at all.
#[test]
fn diff_is_empty_with_identical_files() {
    let node = with_lei(org_node("org-a"), "LEI_EQ");
    let a = make_file(vec![node.clone()], vec![]);
    let mut b = make_file_b(vec![node], vec![]);
    // B node has same LEI, so it matches. Both are unchanged.
    b.nodes[0].id = NodeId::try_from("org-b").expect("node id");
    b.nodes[0].identifiers = Some(vec![Identifier {
        scheme: "lei".to_owned(),
        value: "LEI_EQ".to_owned(),
        authority: None,
        valid_from: None,
        valid_to: None,
        sensitivity: None,
        verification_status: None,
        verification_date: None,
        extra: BTreeMap::new(),
    }]);
    let result = diff(&a, &b);
    // Only matched (unchanged) nodes — is_empty checks additions/removals/modified only.
    assert!(result.is_empty(), "matched-only result should be empty");
}

/// Version mismatch emits a warning but proceeds.
#[test]
fn diff_version_mismatch_warning() {
    let mut a = make_file(vec![], vec![]);
    a.omtsf_version = SemVer::try_from("1.0.0").expect("semver");
    let mut b = make_file_b(vec![], vec![]);
    b.omtsf_version = SemVer::try_from("1.1.0").expect("semver");
    let result = diff(&a, &b);
    assert!(
        result
            .warnings
            .iter()
            .any(|w| w.contains("Version mismatch")),
        "expected version mismatch warning; got: {:?}",
        result.warnings
    );
}

/// diff_filtered with node_type filter excludes other types.
#[test]
fn diff_filtered_by_node_type() {
    use crate::enums::{NodeType, NodeTypeTag};
    let org = with_lei(org_node("org-a"), "LEI_ORG");
    let mut fac_a = org_node("fac-a");
    fac_a.node_type = NodeTypeTag::Known(NodeType::Facility);

    let org_b = with_lei(org_node("org-b"), "LEI_ORG");
    let mut fac_b = org_node("fac-b");
    fac_b.node_type = NodeTypeTag::Known(NodeType::Facility);

    let a = make_file(vec![org, fac_a], vec![]);
    let b = make_file_b(vec![org_b, fac_b], vec![]);

    let mut filter = DiffFilter::default();
    filter.node_types = Some(HashSet::from(["organization".to_owned()]));

    let result = diff_filtered(&a, &b, Some(&filter));
    // Only organization nodes are diffed; facility nodes are excluded.
    // org-a and org-b match via LEI_ORG; names differ so pair is modified.
    assert!(result.nodes.added.is_empty());
    assert!(result.nodes.removed.is_empty());
    let total_matched = result.nodes.unchanged.len() + result.nodes.modified.len();
    assert_eq!(total_matched, 1);
}

/// Two files with identical content produce an empty diff.
#[test]
fn diff_identical_files_empty_diff() {
    let (node_a, node_b) = make_identical_pair("org-x", "LEI_IDENTICAL", "Acme Corp");

    let edge_a = {
        let mut e = super::supplies_edge("e-1", "org-x", "org-x");
        e.properties.commodity = Some("steel".to_owned());
        e
    };
    let edge_b = edge_a.clone();

    let a = make_file(vec![node_a], vec![edge_a]);
    let b = make_file_b(vec![node_b], vec![edge_b]);
    let result = diff(&a, &b);
    assert!(
        result.is_empty(),
        "identical files should produce empty diff"
    );
    assert_eq!(result.nodes.unchanged.len(), 1);
    assert_eq!(result.edges.unchanged.len(), 1);
    let summary = result.summary();
    assert_eq!(summary.nodes_modified, 0);
    assert_eq!(summary.edges_modified, 0);
}

/// A scalar property change (name) is detected as modified.
#[test]
fn diff_node_name_change_is_modified() {
    let mut node_a = org_node("org-nm");
    node_a.name = Some("Old Name".to_owned());
    let node_a = with_lei(node_a, "LEI_NM");

    let mut node_b = org_node("org-nm");
    node_b.name = Some("New Name".to_owned());
    let node_b = with_lei(node_b, "LEI_NM");

    let a = make_file(vec![node_a], vec![]);
    let b = make_file_b(vec![node_b], vec![]);
    let result = diff(&a, &b);

    assert_eq!(result.nodes.modified.len(), 1, "name change → modified");
    assert!(result.nodes.unchanged.is_empty());
    let nd = &result.nodes.modified[0];
    assert!(!nd.property_changes.is_empty());
    let name_change = nd.property_changes.iter().find(|c| c.field == "name");
    assert!(name_change.is_some(), "should have a 'name' change");
    let nc = name_change.expect("name change exists");
    assert_eq!(
        nc.old_value,
        Some(serde_json::Value::String("Old Name".to_owned()))
    );
    assert_eq!(
        nc.new_value,
        Some(serde_json::Value::String("New Name".to_owned()))
    );
}

/// Numeric comparison uses epsilon via node fields: quantity 1000.0 vs
/// 1000.0 + 1e-10 should be equal (within epsilon).
#[test]
fn diff_numeric_epsilon_comparison() {
    let (mut node_a, mut node_b) = make_identical_pair("org-qty", "LEI_QTY", "QtyOrg");
    node_a.quantity = Some(1000.0_f64);
    // Within epsilon — should not be detected as a change.
    node_b.quantity = Some(1000.0_f64 + 1e-10_f64);

    let a = make_file(vec![node_a], vec![]);
    let b = make_file_b(vec![node_b], vec![]);
    let result = diff(&a, &b);

    // Node should be unchanged — quantity diff is below epsilon.
    assert_eq!(
        result.nodes.unchanged.len(),
        1,
        "within epsilon → unchanged"
    );
    assert!(result.nodes.modified.is_empty());
}

/// Quantity 1000.0 vs 2000.0 is outside epsilon and produces a property change.
#[test]
fn diff_numeric_change_detected() {
    let (mut node_a, mut node_b) = make_identical_pair("org-qty2", "LEI_QTY2", "QtyOrg2");
    node_a.quantity = Some(1000.0_f64);
    node_b.quantity = Some(2000.0_f64);

    let a = make_file(vec![node_a], vec![]);
    let b = make_file_b(vec![node_b], vec![]);
    let result = diff(&a, &b);

    assert_eq!(result.nodes.modified.len(), 1, "quantity change → modified");
    let nd = &result.nodes.modified[0];
    let qty_change = nd.property_changes.iter().find(|c| c.field == "quantity");
    assert!(qty_change.is_some(), "should have a 'quantity' change");
}

/// Date normalisation: "2026-2-9" and "2026-02-09" are treated as equal.
#[test]
fn diff_date_normalisation_no_false_positive() {
    let (mut node_a, node_b) = make_identical_pair("org-dt", "LEI_DT", "DateOrg");
    // Set valid_from with a non-padded date variant in the CalendarDate
    // (CalendarDate enforces YYYY-MM-DD format, so we test via the extra field
    // which accepts raw JSON values).
    node_a.extra.insert(
        "x_test_date".to_owned(),
        DynValue::String("2026-2-9".to_owned()),
    );
    let mut node_b_mut = node_b;
    node_b_mut.extra.insert(
        "x_test_date".to_owned(),
        DynValue::String("2026-02-09".to_owned()),
    );

    let a = make_file(vec![node_a], vec![]);
    let b = make_file_b(vec![node_b_mut], vec![]);
    let result = diff(&a, &b);

    // x_test_date should be treated as equal after normalisation.
    let any_modified = result
        .nodes
        .modified
        .iter()
        .any(|nd| nd.property_changes.iter().any(|c| c.field == "x_test_date"));
    assert!(
        !any_modified,
        "normalised dates should not produce a change; got: {:?}",
        result
            .nodes
            .modified
            .iter()
            .flat_map(|nd| nd.property_changes.iter())
            .collect::<Vec<_>>()
    );
}

/// Adding an identifier to a node is detected.
#[test]
fn diff_identifier_added() {
    let (node_a, mut node_b) = make_identical_pair("org-id", "LEI_ID", "IdOrg");
    // Add a DUNS to B.
    let duns = Identifier {
        scheme: "duns".to_owned(),
        value: "123456789".to_owned(),
        authority: None,
        valid_from: None,
        valid_to: None,
        sensitivity: None,
        verification_status: None,
        verification_date: None,
        extra: BTreeMap::new(),
    };
    node_b.identifiers.get_or_insert_with(Vec::new).push(duns);

    let a = make_file(vec![node_a], vec![]);
    let b = make_file_b(vec![node_b], vec![]);
    let result = diff(&a, &b);

    assert_eq!(
        result.nodes.modified.len(),
        1,
        "identifier added → modified"
    );
    let nd = &result.nodes.modified[0];
    assert_eq!(nd.identifier_changes.added.len(), 1);
    assert!(nd.identifier_changes.removed.is_empty());
    assert_eq!(nd.identifier_changes.added[0].scheme, "duns");
}

/// Removing an identifier from a node is detected.
#[test]
fn diff_identifier_removed() {
    let (mut node_a, node_b) = make_identical_pair("org-idr", "LEI_IDR", "IdROrg");
    // Add a DUNS only to A.
    let duns = Identifier {
        scheme: "duns".to_owned(),
        value: "987654321".to_owned(),
        authority: None,
        valid_from: None,
        valid_to: None,
        sensitivity: None,
        verification_status: None,
        verification_date: None,
        extra: BTreeMap::new(),
    };
    node_a.identifiers.get_or_insert_with(Vec::new).push(duns);

    let a = make_file(vec![node_a], vec![]);
    let b = make_file_b(vec![node_b], vec![]);
    let result = diff(&a, &b);

    assert_eq!(
        result.nodes.modified.len(),
        1,
        "identifier removed → modified"
    );
    let nd = &result.nodes.modified[0];
    assert!(nd.identifier_changes.added.is_empty());
    assert_eq!(nd.identifier_changes.removed.len(), 1);
    assert_eq!(nd.identifier_changes.removed[0].scheme, "duns");
}

/// Adding a label to a node is detected.
#[test]
fn diff_label_added() {
    use crate::types::Label;
    let (node_a, mut node_b) = make_identical_pair("org-lb", "LEI_LB", "LabelOrg");
    node_b.labels = Some(vec![Label {
        key: "tier".to_owned(),
        value: Some("1".to_owned()),
        extra: BTreeMap::new(),
    }]);

    let a = make_file(vec![node_a], vec![]);
    let b = make_file_b(vec![node_b], vec![]);
    let result = diff(&a, &b);

    assert_eq!(result.nodes.modified.len(), 1, "label added → modified");
    let nd = &result.nodes.modified[0];
    assert_eq!(nd.label_changes.added.len(), 1);
    assert!(nd.label_changes.removed.is_empty());
    assert_eq!(nd.label_changes.added[0].key, "tier");
}

/// Removing a label from a node is detected.
#[test]
fn diff_label_removed() {
    use crate::types::Label;
    let (mut node_a, node_b) = make_identical_pair("org-lbr", "LEI_LBR", "LabelRmOrg");
    node_a.labels = Some(vec![Label {
        key: "risk-tier".to_owned(),
        value: Some("high".to_owned()),
        extra: BTreeMap::new(),
    }]);

    let a = make_file(vec![node_a], vec![]);
    let b = make_file_b(vec![node_b], vec![]);
    let result = diff(&a, &b);

    assert_eq!(result.nodes.modified.len(), 1, "label removed → modified");
    let nd = &result.nodes.modified[0];
    assert!(nd.label_changes.added.is_empty());
    assert_eq!(nd.label_changes.removed.len(), 1);
    assert_eq!(nd.label_changes.removed[0].key, "risk-tier");
}

/// A label value change appears as a removal of the old and addition of the new.
#[test]
fn diff_label_value_change_is_remove_plus_add() {
    use crate::types::Label;
    let (mut node_a, mut node_b) = make_identical_pair("org-lbv", "LEI_LBV", "LabelValOrg");
    node_a.labels = Some(vec![Label {
        key: "risk-tier".to_owned(),
        value: Some("low".to_owned()),
        extra: BTreeMap::new(),
    }]);
    node_b.labels = Some(vec![Label {
        key: "risk-tier".to_owned(),
        value: Some("medium".to_owned()),
        extra: BTreeMap::new(),
    }]);

    let a = make_file(vec![node_a], vec![]);
    let b = make_file_b(vec![node_b], vec![]);
    let result = diff(&a, &b);

    assert_eq!(
        result.nodes.modified.len(),
        1,
        "label value change → modified"
    );
    let nd = &result.nodes.modified[0];
    assert_eq!(nd.label_changes.added.len(), 1, "new value added");
    assert_eq!(nd.label_changes.removed.len(), 1, "old value removed");
    assert_eq!(nd.label_changes.added[0].value.as_deref(), Some("medium"));
    assert_eq!(nd.label_changes.removed[0].value.as_deref(), Some("low"));
}

/// DiffFilter ignore_fields excludes specified fields from comparison.
#[test]
fn diff_filter_ignore_fields() {
    let (mut node_a, node_b) = make_identical_pair("org-ign", "LEI_IGN", "IgnOrg");
    // Change address in A only.
    node_a.address = Some("Old Address".to_owned());

    let a = make_file(vec![node_a], vec![]);
    let b = make_file_b(vec![node_b], vec![]);

    // Without ignore: should be modified (address differs).
    let result_all = diff(&a, &b);
    assert_eq!(
        result_all.nodes.modified.len(),
        1,
        "without ignore: address change detected"
    );

    // With ignore on "address": should be unchanged.
    let mut filter = DiffFilter::default();
    filter.ignore_fields.insert("address".to_owned());
    let result_filtered = diff_filtered(&a, &b, Some(&filter));
    assert_eq!(
        result_filtered.nodes.unchanged.len(),
        1,
        "with address ignored: should be unchanged"
    );
    assert!(result_filtered.nodes.modified.is_empty());
}

/// DiffSummary.is_empty is false when there are modifications.
#[test]
fn diff_is_empty_false_when_modified() {
    let (mut node_a, node_b) = make_identical_pair("org-mod", "LEI_MOD", "ModOrg");
    node_a.address = Some("Different".to_owned());

    let a = make_file(vec![node_a], vec![]);
    let b = make_file_b(vec![node_b], vec![]);
    let result = diff(&a, &b);
    assert!(
        !result.is_empty(),
        "diff with modifications should not be empty"
    );
    let summary = result.summary();
    assert_eq!(summary.nodes_modified, 1);
}
