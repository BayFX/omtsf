#![allow(clippy::expect_used)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::field_reassign_with_default)]

mod edge_tests;
mod node_tests;

use crate::file::OmtsFile;
use crate::structures::{Edge, Node};
use crate::types::Identifier;
use std::collections::BTreeMap;

pub(crate) use crate::test_helpers::{
    date, file_salt, node_id, ownership_edge, semver, supplies_edge,
};

pub(crate) const SALT_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
pub(crate) const SALT_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

pub(crate) fn make_file(nodes: Vec<Node>, edges: Vec<Edge>) -> OmtsFile {
    OmtsFile {
        omtsf_version: semver("1.0.0"),
        snapshot_date: date("2026-02-20"),
        file_salt: file_salt(SALT_A),
        disclosure_scope: None,
        previous_snapshot_ref: None,
        snapshot_sequence: None,
        reporting_entity: None,
        nodes,
        edges,
        extra: BTreeMap::new(),
    }
}

pub(crate) fn make_file_b(nodes: Vec<Node>, edges: Vec<Edge>) -> OmtsFile {
    OmtsFile {
        file_salt: file_salt(SALT_B),
        ..make_file(nodes, edges)
    }
}

pub(crate) fn org_node(id: &str) -> Node {
    Node {
        name: Some(id.to_owned()),
        ..crate::test_helpers::org_node(id)
    }
}

pub(crate) fn with_lei(mut node: Node, lei: &str) -> Node {
    let id = Identifier {
        scheme: "lei".to_owned(),
        value: lei.to_owned(),
        authority: None,
        valid_from: None,
        valid_to: None,
        sensitivity: None,
        verification_status: None,
        verification_date: None,
        extra: BTreeMap::new(),
    };
    node.identifiers = Some(vec![id]);
    node
}

pub(crate) fn with_duns(mut node: Node, duns: &str) -> Node {
    let id = Identifier {
        scheme: "duns".to_owned(),
        value: duns.to_owned(),
        authority: None,
        valid_from: None,
        valid_to: None,
        sensitivity: None,
        verification_status: None,
        verification_date: None,
        extra: BTreeMap::new(),
    };
    let ids = node.identifiers.get_or_insert_with(Vec::new);
    ids.push(id);
    node
}

/// Helper: two matched org nodes with same LEI and same name → identical pair.
pub(crate) fn make_identical_pair(id: &str, lei: &str, name: &str) -> (Node, Node) {
    let mut node_a = org_node(id);
    node_a.name = Some(name.to_owned());
    let node_a = with_lei(node_a, lei);
    let node_b = node_a.clone();
    (node_a, node_b)
}
