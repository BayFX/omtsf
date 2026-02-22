use std::collections::BTreeMap;
use std::collections::HashSet;

use crate::enums::{EdgeType, EdgeTypeTag};
use crate::newtypes::EdgeId;
use crate::structures::{Edge, EdgeProperties};

use super::super::engine::{diff, diff_filtered};
use super::super::types::DiffFilter;
use super::{make_file, make_file_b, node_id, org_node, ownership_edge, supplies_edge, with_lei};

/// Edges are matched when both endpoints match and type is the same.
#[test]
fn diff_edges_matched_exact() {
    let node_a1 = with_lei(org_node("org-a1"), "LEI_0001");
    let node_a2 = with_lei(org_node("org-a2"), "LEI_0002");
    let node_b1 = with_lei(org_node("org-b1"), "LEI_0001");
    let node_b2 = with_lei(org_node("org-b2"), "LEI_0002");

    let edge_a = supplies_edge("e-a", "org-a1", "org-a2");
    let edge_b = supplies_edge("e-b", "org-b1", "org-b2");

    let a = make_file(vec![node_a1, node_a2], vec![edge_a]);
    let b = make_file_b(vec![node_b1, node_b2], vec![edge_b]);

    let result = diff(&a, &b);
    assert!(result.edges.added.is_empty(), "no additions");
    assert!(result.edges.removed.is_empty(), "no deletions");
    assert_eq!(result.edges.unchanged.len(), 1, "one matched edge pair");
    assert_eq!(result.edges.unchanged[0].id_a, "e-a");
    assert_eq!(result.edges.unchanged[0].id_b, "e-b");
}

/// Edges in A with no match in B are deletions.
#[test]
fn diff_edge_deletion() {
    let node_a1 = with_lei(org_node("org-a1"), "LEI_0001");
    let node_a2 = with_lei(org_node("org-a2"), "LEI_0002");
    let node_b1 = with_lei(org_node("org-b1"), "LEI_0001");
    let node_b2 = with_lei(org_node("org-b2"), "LEI_0002");

    let edge_a = supplies_edge("e-a", "org-a1", "org-a2");
    // B has no edges.

    let a = make_file(vec![node_a1, node_a2], vec![edge_a]);
    let b = make_file_b(vec![node_b1, node_b2], vec![]);

    let result = diff(&a, &b);
    assert_eq!(result.edges.removed.len(), 1, "e-a is a deletion");
    assert!(result.edges.added.is_empty());
    assert!(result.edges.unchanged.is_empty());
}

/// Edges in B with no match in A are additions.
#[test]
fn diff_edge_addition() {
    let node_a1 = with_lei(org_node("org-a1"), "LEI_0001");
    let node_a2 = with_lei(org_node("org-a2"), "LEI_0002");
    let node_b1 = with_lei(org_node("org-b1"), "LEI_0001");
    let node_b2 = with_lei(org_node("org-b2"), "LEI_0002");

    let edge_b = supplies_edge("e-b", "org-b1", "org-b2");
    // A has no edges.

    let a = make_file(vec![node_a1, node_a2], vec![]);
    let b = make_file_b(vec![node_b1, node_b2], vec![edge_b]);

    let result = diff(&a, &b);
    assert_eq!(result.edges.added.len(), 1, "e-b is an addition");
    assert!(result.edges.removed.is_empty());
    assert!(result.edges.unchanged.is_empty());
}

/// Edges with different types are not matched.
#[test]
fn diff_edges_different_type_not_matched() {
    let node_a1 = with_lei(org_node("org-a1"), "LEI_0001");
    let node_a2 = with_lei(org_node("org-a2"), "LEI_0002");
    let node_b1 = with_lei(org_node("org-b1"), "LEI_0001");
    let node_b2 = with_lei(org_node("org-b2"), "LEI_0002");

    let edge_a = supplies_edge("e-a", "org-a1", "org-a2");
    let edge_b = ownership_edge("e-b", "org-b1", "org-b2");

    let a = make_file(vec![node_a1, node_a2], vec![edge_a]);
    let b = make_file_b(vec![node_b1, node_b2], vec![edge_b]);

    let result = diff(&a, &b);
    assert_eq!(
        result.edges.removed.len(),
        1,
        "e-a is a deletion (type mismatch)"
    );
    assert_eq!(
        result.edges.added.len(),
        1,
        "e-b is an addition (type mismatch)"
    );
    assert!(result.edges.unchanged.is_empty());
}

/// Edges whose nodes are unmatched are reported as additions/deletions.
#[test]
fn diff_edges_with_unmatched_nodes() {
    // Node in A has no counterpart in B. The edge is therefore a deletion.
    let node_a1 = org_node("org-a1"); // no identifiers → no match
    let node_a2 = org_node("org-a2");
    let node_b1 = org_node("org-b1");
    let node_b2 = org_node("org-b2");

    let edge_a = supplies_edge("e-a", "org-a1", "org-a2");
    let edge_b = supplies_edge("e-b", "org-b1", "org-b2");

    let a = make_file(vec![node_a1, node_a2], vec![edge_a]);
    let b = make_file_b(vec![node_b1, node_b2], vec![edge_b]);

    let result = diff(&a, &b);
    // Nodes don't match, so edges can't match.
    assert_eq!(result.edges.removed.len(), 1);
    assert_eq!(result.edges.added.len(), 1);
    assert!(result.edges.unchanged.is_empty());
}

/// same_as edges are never matched; they appear as deletions/additions.
#[test]
fn diff_same_as_edges_not_matched() {
    let node_a1 = with_lei(org_node("org-a1"), "LEI_X");
    let node_a2 = with_lei(org_node("org-a2"), "LEI_Y");
    let node_b1 = with_lei(org_node("org-b1"), "LEI_X");
    let node_b2 = with_lei(org_node("org-b2"), "LEI_Y");

    let same_as_a = Edge {
        id: EdgeId::try_from("same-a").expect("edge id"),
        edge_type: EdgeTypeTag::Known(EdgeType::SameAs),
        source: node_id("org-a1"),
        target: node_id("org-a2"),
        identifiers: None,
        properties: EdgeProperties::default(),
        extra: BTreeMap::new(),
    };
    let same_as_b = Edge {
        id: EdgeId::try_from("same-b").expect("edge id"),
        edge_type: EdgeTypeTag::Known(EdgeType::SameAs),
        source: node_id("org-b1"),
        target: node_id("org-b2"),
        identifiers: None,
        properties: EdgeProperties::default(),
        extra: BTreeMap::new(),
    };

    let a = make_file(vec![node_a1, node_a2], vec![same_as_a]);
    let b = make_file_b(vec![node_b1, node_b2], vec![same_as_b]);

    let result = diff(&a, &b);
    // same_as edges are never matched — both appear as deletion and addition.
    assert_eq!(result.edges.removed.len(), 1, "same_as in A is a deletion");
    assert_eq!(result.edges.added.len(), 1, "same_as in B is an addition");
    assert!(result.edges.unchanged.is_empty());
}

/// Supplies edges matched by identity properties (no external identifiers).
#[test]
fn diff_edges_matched_by_identity_properties() {
    let node_a1 = with_lei(org_node("org-a1"), "LEI_P");
    let node_a2 = with_lei(org_node("org-a2"), "LEI_Q");
    let node_b1 = with_lei(org_node("org-b1"), "LEI_P");
    let node_b2 = with_lei(org_node("org-b2"), "LEI_Q");

    // Both edges: supplies with commodity "steel", no external identifier.
    let mut props = EdgeProperties::default();
    props.commodity = Some("steel".to_owned());

    let mut edge_a = supplies_edge("e-a", "org-a1", "org-a2");
    edge_a.properties = props.clone();
    let mut edge_b = supplies_edge("e-b", "org-b1", "org-b2");
    edge_b.properties = props;

    let a = make_file(vec![node_a1, node_a2], vec![edge_a]);
    let b = make_file_b(vec![node_b1, node_b2], vec![edge_b]);

    let result = diff(&a, &b);
    assert!(result.edges.added.is_empty());
    assert!(result.edges.removed.is_empty());
    assert_eq!(result.edges.unchanged.len(), 1);
}

/// Two edges with different identity properties are not matched.
#[test]
fn diff_edges_not_matched_different_identity_properties() {
    let node_a1 = with_lei(org_node("org-a1"), "LEI_P");
    let node_a2 = with_lei(org_node("org-a2"), "LEI_Q");
    let node_b1 = with_lei(org_node("org-b1"), "LEI_P");
    let node_b2 = with_lei(org_node("org-b2"), "LEI_Q");

    let mut props_a = EdgeProperties::default();
    props_a.commodity = Some("steel".to_owned());

    let mut props_b = EdgeProperties::default();
    props_b.commodity = Some("aluminum".to_owned());

    let mut edge_a = supplies_edge("e-a", "org-a1", "org-a2");
    edge_a.properties = props_a;
    let mut edge_b = supplies_edge("e-b", "org-b1", "org-b2");
    edge_b.properties = props_b;

    let a = make_file(vec![node_a1, node_a2], vec![edge_a]);
    let b = make_file_b(vec![node_b1, node_b2], vec![edge_b]);

    let result = diff(&a, &b);
    assert_eq!(result.edges.removed.len(), 1);
    assert_eq!(result.edges.added.len(), 1);
    assert!(result.edges.unchanged.is_empty());
}

/// Edge property change (volume) is detected.
#[test]
fn diff_edge_property_change() {
    let na = with_lei(org_node("org-ep-a"), "LEI_EPA");
    let nb = na.clone();
    let nc = with_lei(org_node("org-ep-b"), "LEI_EPB");
    let nd = nc.clone();

    let mut edge_a = supplies_edge("e-vol", "org-ep-a", "org-ep-b");
    edge_a.properties.commodity = Some("coal".to_owned());
    edge_a.properties.volume = Some(1000.0_f64);

    let mut edge_b = edge_a.clone();
    edge_b.properties.volume = Some(1500.0_f64);

    let a = make_file(vec![na, nc], vec![edge_a]);
    let b = make_file_b(vec![nb, nd], vec![edge_b]);
    let result = diff(&a, &b);

    assert_eq!(
        result.edges.modified.len(),
        1,
        "volume change → modified edge"
    );
    let ed = &result.edges.modified[0];
    let vol_change = ed.property_changes.iter().find(|c| c.field == "volume");
    assert!(vol_change.is_some(), "should find a 'volume' change");
}

/// DiffFilter with edge_type restricts edge diffing.
#[test]
fn diff_filter_edge_type() {
    let na = with_lei(org_node("org-fet-a"), "LEI_FET_A");
    let nb = na.clone();
    let nc = with_lei(org_node("org-fet-b"), "LEI_FET_B");
    let nd = nc.clone();

    // One supplies edge (identity prop: commodity) and one ownership edge.
    let mut sup = supplies_edge("e-sup", "org-fet-a", "org-fet-b");
    sup.properties.commodity = Some("iron".to_owned());
    let own = ownership_edge("e-own", "org-fet-a", "org-fet-b");

    let a = make_file(vec![na, nc], vec![sup.clone(), own.clone()]);
    let b = make_file_b(vec![nb, nd], vec![sup, own]);

    let mut filter = DiffFilter::default();
    filter.edge_types = Some(HashSet::from(["supplies".to_owned()]));

    let result = diff_filtered(&a, &b, Some(&filter));
    // Only supplies edge should be diffed; ownership excluded.
    let total_edges = result.edges.unchanged.len()
        + result.edges.modified.len()
        + result.edges.added.len()
        + result.edges.removed.len();
    assert_eq!(total_edges, 1, "only one edge type considered");
}
