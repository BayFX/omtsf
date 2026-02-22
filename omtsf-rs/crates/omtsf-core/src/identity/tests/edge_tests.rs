#![allow(clippy::expect_used)]

use crate::identity::{build_edge_candidate_index, edge_composite_key};
use std::collections::BTreeMap;

pub(super) fn make_edge(
    id: &str,
    edge_type: crate::enums::EdgeTypeTag,
    source: &str,
    target: &str,
) -> crate::structures::Edge {
    use crate::newtypes::{EdgeId, NodeId};
    use crate::structures::EdgeProperties;
    crate::structures::Edge {
        id: EdgeId::try_from(id).expect("valid EdgeId"),
        edge_type,
        source: NodeId::try_from(source).expect("valid NodeId"),
        target: NodeId::try_from(target).expect("valid NodeId"),
        identifiers: None,
        properties: EdgeProperties::default(),
        extra: BTreeMap::new(),
    }
}

pub(super) fn with_edge_identifiers(
    mut edge: crate::structures::Edge,
    ids: Vec<crate::types::Identifier>,
) -> crate::structures::Edge {
    edge.identifiers = Some(ids);
    edge
}

pub(super) fn with_edge_properties(
    mut edge: crate::structures::Edge,
    props: crate::structures::EdgeProperties,
) -> crate::structures::Edge {
    edge.properties = props;
    edge
}

#[test]
fn composite_key_same_as_excluded() {
    use crate::enums::{EdgeType, EdgeTypeTag};
    let edge = make_edge("e1", EdgeTypeTag::Known(EdgeType::SameAs), "org-1", "org-2");
    assert!(
        edge_composite_key(0, 1, &edge).is_none(),
        "same_as edges must return None"
    );
}

#[test]
fn composite_key_supplies_included() {
    use crate::enums::{EdgeType, EdgeTypeTag};
    let edge = make_edge(
        "e1",
        EdgeTypeTag::Known(EdgeType::Supplies),
        "org-1",
        "org-2",
    );
    let key = edge_composite_key(0, 1, &edge);
    assert!(key.is_some());
    let key = key.expect("should be Some");
    assert_eq!(key.source_rep, 0);
    assert_eq!(key.target_rep, 1);
    assert_eq!(key.edge_type, EdgeTypeTag::Known(EdgeType::Supplies));
}

#[test]
fn composite_key_extension_type_included() {
    let edge = make_edge(
        "e1",
        crate::enums::EdgeTypeTag::Extension("com.acme.custom".to_owned()),
        "n-1",
        "n-2",
    );
    let key = edge_composite_key(5, 7, &edge);
    assert!(key.is_some());
    let key = key.expect("Some");
    assert_eq!(key.source_rep, 5);
    assert_eq!(key.target_rep, 7);
    assert_eq!(
        key.edge_type,
        crate::enums::EdgeTypeTag::Extension("com.acme.custom".to_owned())
    );
}

#[test]
fn composite_key_different_reps_produce_different_keys() {
    use crate::enums::{EdgeType, EdgeTypeTag};
    let edge = make_edge("e1", EdgeTypeTag::Known(EdgeType::Ownership), "s", "t");
    let key_01 = edge_composite_key(0, 1, &edge).expect("Some");
    let key_02 = edge_composite_key(0, 2, &edge).expect("Some");
    assert_ne!(key_01, key_02, "different target_rep must differ");
}

#[test]
fn candidate_index_empty_edges() {
    let edges: Vec<crate::structures::Edge> = vec![];
    let index = build_edge_candidate_index(&edges, |_| None, |x| x);
    assert!(index.is_empty());
}

#[test]
fn candidate_index_same_as_edges_excluded() {
    use crate::enums::{EdgeType, EdgeTypeTag};
    // All edges are same_as — index should be empty.
    let edges = vec![make_edge(
        "e1",
        EdgeTypeTag::Known(EdgeType::SameAs),
        "org-1",
        "org-2",
    )];
    // node_ordinal resolves every node id to ordinal 0 or 1
    let index = build_edge_candidate_index(
        &edges,
        |id| if id == "org-1" { Some(0) } else { Some(1) },
        |x| x,
    );
    assert!(index.is_empty());
}

#[test]
fn candidate_index_dangling_source_skipped() {
    use crate::enums::{EdgeType, EdgeTypeTag};
    let edges = vec![make_edge(
        "e1",
        EdgeTypeTag::Known(EdgeType::Supplies),
        "missing-node",
        "org-2",
    )];
    let index = build_edge_candidate_index(
        &edges,
        |id| if id == "org-2" { Some(1) } else { None },
        |x| x,
    );
    assert!(index.is_empty(), "dangling source should skip the edge");
}

#[test]
fn candidate_index_dangling_target_skipped() {
    use crate::enums::{EdgeType, EdgeTypeTag};
    let edges = vec![make_edge(
        "e1",
        EdgeTypeTag::Known(EdgeType::Supplies),
        "org-1",
        "missing-node",
    )];
    let index = build_edge_candidate_index(
        &edges,
        |id| if id == "org-1" { Some(0) } else { None },
        |x| x,
    );
    assert!(index.is_empty(), "dangling target should skip the edge");
}

#[test]
fn candidate_index_two_edges_same_bucket() {
    use crate::enums::{EdgeType, EdgeTypeTag};
    // Two supplies edges from resolved endpoints (0→1) go in the same bucket.
    let edges = vec![
        make_edge(
            "e1",
            EdgeTypeTag::Known(EdgeType::Supplies),
            "org-1",
            "org-2",
        ),
        make_edge(
            "e2",
            EdgeTypeTag::Known(EdgeType::Supplies),
            "org-1",
            "org-2",
        ),
    ];
    let index = build_edge_candidate_index(
        &edges,
        |id| match id {
            "org-1" => Some(0),
            "org-2" => Some(1),
            _ => None,
        },
        |x| x,
    );
    assert_eq!(index.len(), 1, "both edges share one composite key");
    let bucket = index.values().next().expect("one bucket");
    let mut bucket = bucket.clone();
    bucket.sort_unstable();
    assert_eq!(bucket, vec![0usize, 1]);
}

#[test]
fn candidate_index_different_type_different_bucket() {
    use crate::enums::{EdgeType, EdgeTypeTag};
    let edges = vec![
        make_edge(
            "e1",
            EdgeTypeTag::Known(EdgeType::Supplies),
            "org-1",
            "org-2",
        ),
        make_edge(
            "e2",
            EdgeTypeTag::Known(EdgeType::Ownership),
            "org-1",
            "org-2",
        ),
    ];
    let index = build_edge_candidate_index(
        &edges,
        |id| match id {
            "org-1" => Some(0),
            "org-2" => Some(1),
            _ => None,
        },
        |x| x,
    );
    assert_eq!(index.len(), 2, "different types → different buckets");
}

#[test]
fn candidate_index_union_find_rep_used() {
    use crate::enums::{EdgeType, EdgeTypeTag};
    // org-1 and org-3 are unioned → both should resolve to rep 0.
    // An edge from org-1 and an edge from org-3 to org-2 share a bucket.
    let edges = vec![
        make_edge(
            "e1",
            EdgeTypeTag::Known(EdgeType::Supplies),
            "org-1",
            "org-2",
        ),
        make_edge(
            "e2",
            EdgeTypeTag::Known(EdgeType::Supplies),
            "org-3",
            "org-2",
        ),
    ];
    let index = build_edge_candidate_index(
        &edges,
        |id| match id {
            "org-1" => Some(0),
            "org-2" => Some(1),
            "org-3" => Some(2),
            _ => None,
        },
        // org-3 (ordinal 2) is unioned with org-1 (ordinal 0) → rep is 0.
        |x| if x == 2 { 0 } else { x },
    );
    assert_eq!(index.len(), 1, "merged source nodes → same bucket");
    let bucket = index.values().next().expect("one bucket");
    let mut bucket = bucket.clone();
    bucket.sort_unstable();
    assert_eq!(bucket, vec![0usize, 1]);
}
