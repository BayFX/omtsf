#![allow(clippy::expect_used)]

use super::*;
use crate::enums::{EdgeType, EdgeTypeTag, NodeType, NodeTypeTag};
use crate::file::OmtsFile;
use crate::newtypes::{CalendarDate, FileSalt, NodeId, SemVer};
use crate::structures::{Edge, EdgeProperties, Node};
use crate::types::Identifier;
use crate::validation::{ValidationConfig, validate};
use std::collections::BTreeMap;

const SALT_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const SALT_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const SALT_C: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

fn semver(s: &str) -> SemVer {
    SemVer::try_from(s).expect("valid SemVer")
}

fn date(s: &str) -> CalendarDate {
    CalendarDate::try_from(s).expect("valid CalendarDate")
}

fn file_salt(s: &str) -> FileSalt {
    FileSalt::try_from(s).expect("valid FileSalt")
}

fn node_id(s: &str) -> NodeId {
    NodeId::try_from(s).expect("valid NodeId")
}

fn make_org_node(id: &str, name: Option<&str>, identifiers: Option<Vec<Identifier>>) -> Node {
    Node {
        id: node_id(id),
        node_type: NodeTypeTag::Known(NodeType::Organization),
        identifiers,
        name: name.map(str::to_owned),
        ..Node::default()
    }
}

fn make_identifier(scheme: &str, value: &str) -> Identifier {
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

fn make_supplies_edge(id: &str, src: &str, tgt: &str) -> Edge {
    Edge {
        id: node_id(id),
        edge_type: EdgeTypeTag::Known(EdgeType::Supplies),
        source: node_id(src),
        target: node_id(tgt),
        identifiers: None,
        properties: EdgeProperties::default(),
        extra: BTreeMap::new(),
    }
}

fn minimal_file(salt: &str, nodes: Vec<Node>, edges: Vec<Edge>) -> OmtsFile {
    OmtsFile {
        omtsf_version: semver("1.0.0"),
        snapshot_date: date("2026-02-20"),
        file_salt: file_salt(salt),
        disclosure_scope: None,
        previous_snapshot_ref: None,
        snapshot_sequence: None,
        reporting_entity: None,
        nodes,
        edges,
        extra: BTreeMap::new(),
    }
}

#[test]
fn merge_empty_input_returns_error() {
    let result = merge(&[]);
    assert!(matches!(result, Err(MergeError::NoInputFiles)));
}

#[test]
fn merge_single_file_passthrough() {
    let nodes = vec![make_org_node("org-1", Some("Acme"), None)];
    let file = minimal_file(SALT_A, nodes, vec![]);
    let output = merge(&[file]).expect("merge should succeed");
    assert_eq!(output.file.nodes.len(), 1);
    assert_eq!(output.file.edges.len(), 0);
    assert_eq!(output.warnings.len(), 0);
}

#[test]
fn merge_disjoint_graphs() {
    let node_a = make_org_node(
        "org-1",
        Some("Alpha Corp"),
        Some(vec![make_identifier("lei", "TESTLEIALPHATEST0091")]),
    );
    let node_b = make_org_node(
        "org-2",
        Some("Beta Ltd"),
        Some(vec![make_identifier("duns", "012345678")]),
    );

    let file_a = minimal_file(SALT_A, vec![node_a], vec![]);
    let file_b = minimal_file(SALT_B, vec![node_b], vec![]);

    let output = merge(&[file_a, file_b]).expect("disjoint merge should succeed");

    // Two distinct nodes — no merge happened.
    assert_eq!(
        output.file.nodes.len(),
        2,
        "disjoint nodes should both appear"
    );
    assert_eq!(output.conflict_count, 0);
    assert_eq!(output.warnings.len(), 0);
}

#[test]
fn merge_full_overlap_identical_files() {
    let lei = make_identifier("lei", "TESTLEISHAREDTEST062");
    let node = make_org_node("org-1", Some("SharedCorp"), Some(vec![lei]));
    let file_a = minimal_file(SALT_A, vec![node.clone()], vec![]);
    let file_b = minimal_file(SALT_B, vec![node], vec![]);

    let output = merge(&[file_a, file_b]).expect("full overlap merge should succeed");

    // Two identical nodes → merged into one.
    assert_eq!(
        output.file.nodes.len(),
        1,
        "identical nodes should merge into one"
    );
    assert_eq!(
        output.conflict_count, 0,
        "identical files produce no conflicts"
    );
    assert_eq!(output.warnings.len(), 0);
}

#[test]
fn merge_partial_overlap_with_conflict() {
    let lei = make_identifier("lei", "TESTLEICONFLICT00069");
    let node_a = make_org_node("org-a", Some("Acme Corp"), Some(vec![lei.clone()]));
    let node_b = make_org_node("org-b", Some("ACME Corporation"), Some(vec![lei]));

    let file_a = minimal_file(SALT_A, vec![node_a], vec![]);
    let file_b = minimal_file(SALT_B, vec![node_b], vec![]);

    let output = merge(&[file_a, file_b]).expect("conflict merge should succeed");

    // They share a LEI → merged into one node.
    assert_eq!(
        output.file.nodes.len(),
        1,
        "nodes with shared LEI must merge"
    );
    // Name conflict recorded.
    assert!(
        output.conflict_count > 0,
        "conflicting names must generate a conflict"
    );
    // The merged node's name should be absent (conflict).
    assert!(
        output.file.nodes[0].name.is_none(),
        "conflicting name field should be absent from output"
    );
    // _conflicts entry present.
    assert!(
        output.file.nodes[0].extra.contains_key("_conflicts"),
        "_conflicts should be present on merged node"
    );
}

#[test]
fn merge_three_files() {
    let lei = make_identifier("lei", "TESTLEITHREETEST0059");
    let duns = make_identifier("duns", "987654321");

    let node_a = make_org_node("n1", Some("Corp A"), Some(vec![lei.clone()]));
    let node_b = make_org_node("n2", Some("Corp A"), Some(vec![lei, duns.clone()]));
    let node_c = make_org_node("n3", Some("Corp A"), Some(vec![duns]));

    let file_a = minimal_file(SALT_A, vec![node_a], vec![]);
    let file_b = minimal_file(SALT_B, vec![node_b], vec![]);
    let file_c = minimal_file(SALT_C, vec![node_c], vec![]);

    let output = merge(&[file_a, file_b, file_c]).expect("three-file merge should succeed");

    // Transitive: A-B via LEI, B-C via DUNS → all three in one group.
    assert_eq!(
        output.file.nodes.len(),
        1,
        "transitive chain must merge all three nodes into one"
    );
    // Name agreed ("Corp A") across all three.
    assert_eq!(
        output.file.nodes[0].name.as_deref(),
        Some("Corp A"),
        "agreed name must be present"
    );
    assert_eq!(output.conflict_count, 0, "no conflicts when names agree");
}

#[test]
fn merge_rewrites_edge_endpoints() {
    let lei = make_identifier("lei", "TESTLEIEDGETEST00051");
    let node_a = make_org_node("supplier", Some("Supplier"), Some(vec![lei.clone()]));
    let node_b = make_org_node("buyer", Some("Buyer"), None);
    let edge = make_supplies_edge("e1", "supplier", "buyer");

    // Same node in file_b under different local ID.
    let node_a2 = make_org_node("supplier2", Some("Supplier"), Some(vec![lei]));
    let node_b2 = make_org_node("buyer2", Some("Buyer"), None);
    let edge2 = make_supplies_edge("e2", "supplier2", "buyer2");

    let file_a = minimal_file(SALT_A, vec![node_a, node_b], vec![edge]);
    let file_b = minimal_file(SALT_B, vec![node_a2, node_b2], vec![edge2]);

    let output = merge(&[file_a, file_b]).expect("edge rewrite merge should succeed");

    // supplier merges (shared LEI); buyer stays separate (2 buyers,
    // different files, no shared identifiers).
    // We expect: 1 supplier group + 2 buyer groups = 3 nodes.
    assert_eq!(output.file.nodes.len(), 3);

    // All edges must reference existing node IDs.
    let node_ids: std::collections::HashSet<&str> =
        output.file.nodes.iter().map(|n| &n.id as &str).collect();
    for edge in &output.file.edges {
        assert!(
            node_ids.contains(&edge.source as &str),
            "edge source {} must reference existing node",
            &edge.source as &str,
        );
        assert!(
            node_ids.contains(&edge.target as &str),
            "edge target {} must reference existing node",
            &edge.target as &str,
        );
    }
}

#[test]
fn merge_oversized_group_emits_warning() {
    // Create many nodes sharing the same LEI to trigger group size warning.
    let lei_val = "TESTLEIOVERSIZED0089";
    let nodes: Vec<Node> = (0..5)
        .map(|i| {
            make_org_node(
                &format!("org-{i}"),
                Some("OverCorp"),
                Some(vec![make_identifier("lei", lei_val)]),
            )
        })
        .collect();

    let file = minimal_file(SALT_A, nodes, vec![]);

    let config = MergeConfig {
        group_size_limit: 3, // trigger warning at > 3 nodes
        ..MergeConfig::default()
    };
    let output = merge_with_config(&[file], &config).expect("oversized merge should succeed");

    // 5 nodes with same LEI → 1 group of size 5 > limit 3.
    assert!(
        !output.warnings.is_empty(),
        "oversized group should emit warning"
    );
    let warning = &output.warnings[0];
    assert!(matches!(
        warning,
        MergeWarning::OversizedMergeGroup {
            group_size: 5,
            limit: 3,
            ..
        }
    ));
}

#[test]
fn merge_output_passes_l1_validation() {
    let node_a = make_org_node(
        "org-1",
        Some("Alpha"),
        Some(vec![make_identifier("lei", "TESTLEIL1TEST0000069")]),
    );
    let node_b = make_org_node("org-2", Some("Beta"), None);
    let edge = make_supplies_edge("e1", "org-1", "org-2");

    let file = minimal_file(SALT_A, vec![node_a, node_b], vec![edge]);
    let output = merge(&[file]).expect("single file merge should succeed");

    let cfg = ValidationConfig {
        run_l1: true,
        run_l2: false,
        run_l3: false,
    };
    let result = validate(&output.file, &cfg, None);
    assert!(
        result.is_conformant(),
        "merged output must pass L1 validation; errors: {:?}",
        result.errors().collect::<Vec<_>>()
    );
}

#[test]
fn merge_metadata_in_output() {
    let file_a = minimal_file(SALT_A, vec![], vec![]);
    let file_b = minimal_file(SALT_B, vec![], vec![]);

    let output = merge(&[file_a, file_b]).expect("merge should succeed");

    assert!(
        output.file.extra.contains_key("merge_metadata"),
        "merge_metadata must be present in output file extra"
    );
    assert_eq!(output.metadata.source_files.len(), 2);
}

/// Two files each have a node named "org-1", but they refer to different
/// real-world entities (different names, no shared external identifiers).
/// Each file also has an edge from "org-1" to its own distinct buyer node.
///
/// After merging, the pipeline must produce two separate supplier nodes (one
/// per file) and two separate edges, not mistakenly bucket both files'
/// edges as candidates for the same merge group because they share the
/// local string "org-1".
#[test]
fn merge_colliding_node_ids_across_files_are_distinct() {
    // File A: org-1 (Alpha Supplier) → buyer-1 (Alpha Buyer)
    let supplier_a = make_org_node(
        "org-1",
        Some("Alpha Supplier"),
        Some(vec![make_identifier("duns", "111111111")]),
    );
    let buyer_a = make_org_node("buyer-1", Some("Alpha Buyer"), None);
    let edge_a = make_supplies_edge("e-1", "org-1", "buyer-1");

    // File B: org-1 (Beta Supplier) → buyer-1 (Beta Buyer)
    // Same local node ID strings, entirely different entities.
    let supplier_b = make_org_node(
        "org-1",
        Some("Beta Supplier"),
        Some(vec![make_identifier("duns", "222222222")]),
    );
    let buyer_b = make_org_node("buyer-1", Some("Beta Buyer"), None);
    let edge_b = make_supplies_edge("e-1", "org-1", "buyer-1");

    let file_a = minimal_file(SALT_A, vec![supplier_a, buyer_a], vec![edge_a]);
    let file_b = minimal_file(SALT_B, vec![supplier_b, buyer_b], vec![edge_b]);

    let output = merge(&[file_a, file_b]).expect("colliding-id merge should succeed");

    // No shared external identifiers → all four nodes stay distinct.
    assert_eq!(
        output.file.nodes.len(),
        4,
        "four distinct nodes expected (2 suppliers + 2 buyers)"
    );

    // Each file contributes one edge; they connect different node pairs, so
    // they must NOT be merged together.
    assert_eq!(
        output.file.edges.len(),
        2,
        "two distinct edges expected — one per file"
    );

    // Every edge must reference nodes that exist in the output.
    let node_ids: std::collections::HashSet<&str> =
        output.file.nodes.iter().map(|n| &n.id as &str).collect();
    for edge in &output.file.edges {
        assert!(
            node_ids.contains(&edge.source as &str),
            "edge source {} must reference an existing merged node",
            &edge.source as &str,
        );
        assert!(
            node_ids.contains(&edge.target as &str),
            "edge target {} must reference an existing merged node",
            &edge.target as &str,
        );
    }

    // The two output edges must connect different source/target pairs,
    // confirming that file A's "org-1" and file B's "org-1" were resolved
    // to different merged node IDs.
    let edge_pairs: Vec<(&str, &str)> = output
        .file
        .edges
        .iter()
        .map(|e| (&e.source as &str, &e.target as &str))
        .collect();
    assert_eq!(edge_pairs.len(), 2, "there must be exactly two edge pairs");
    assert_ne!(
        edge_pairs[0], edge_pairs[1],
        "the two edges must connect different (source, target) pairs"
    );
}
