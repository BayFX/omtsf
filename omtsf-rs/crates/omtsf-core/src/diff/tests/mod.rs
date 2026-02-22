#![allow(clippy::expect_used)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::field_reassign_with_default)]

mod edge_tests;
mod node_tests;

use crate::enums::{EdgeType, EdgeTypeTag, NodeType, NodeTypeTag};
use crate::file::OmtsFile;
use crate::newtypes::{CalendarDate, EdgeId, FileSalt, NodeId, SemVer};
use crate::structures::{Edge, EdgeProperties, Node};
use crate::types::Identifier;
use std::collections::BTreeMap;

pub(crate) const SALT_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
pub(crate) const SALT_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

pub(crate) fn semver(s: &str) -> SemVer {
    SemVer::try_from(s).expect("semver")
}

pub(crate) fn date(s: &str) -> CalendarDate {
    CalendarDate::try_from(s).expect("date")
}

pub(crate) fn node_id(s: &str) -> NodeId {
    NodeId::try_from(s).expect("node id")
}

pub(crate) fn edge_id(s: &str) -> EdgeId {
    EdgeId::try_from(s).expect("edge id")
}

pub(crate) fn file_salt(s: &str) -> FileSalt {
    FileSalt::try_from(s).expect("salt")
}

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
        id: node_id(id),
        node_type: NodeTypeTag::Known(NodeType::Organization),
        identifiers: None,
        data_quality: None,
        labels: None,
        name: Some(id.to_owned()),
        jurisdiction: None,
        status: None,
        governance_structure: None,
        operator: None,
        address: None,
        geo: None,
        commodity_code: None,
        unit: None,
        role: None,
        attestation_type: None,
        standard: None,
        issuer: None,
        valid_from: None,
        valid_to: None,
        outcome: None,
        attestation_status: None,
        reference: None,
        risk_severity: None,
        risk_likelihood: None,
        lot_id: None,
        quantity: None,
        production_date: None,
        origin_country: None,
        direct_emissions_co2e: None,
        indirect_emissions_co2e: None,
        emission_factor_source: None,
        installation_id: None,
        extra: BTreeMap::new(),
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

pub(crate) fn supplies_edge(id: &str, src: &str, tgt: &str) -> Edge {
    Edge {
        id: edge_id(id),
        edge_type: EdgeTypeTag::Known(EdgeType::Supplies),
        source: node_id(src),
        target: node_id(tgt),
        identifiers: None,
        properties: EdgeProperties::default(),
        extra: BTreeMap::new(),
    }
}

pub(crate) fn ownership_edge(id: &str, src: &str, tgt: &str) -> Edge {
    Edge {
        id: edge_id(id),
        edge_type: EdgeTypeTag::Known(EdgeType::Ownership),
        source: node_id(src),
        target: node_id(tgt),
        identifiers: None,
        properties: EdgeProperties::default(),
        extra: BTreeMap::new(),
    }
}

/// Helper: two matched org nodes with same LEI and same name → identical pair.
pub(crate) fn make_identical_pair(id: &str, lei: &str, name: &str) -> (Node, Node) {
    let mut node_a = org_node(id);
    node_a.name = Some(name.to_owned());
    let node_a = with_lei(node_a, lei);
    let node_b = node_a.clone();
    (node_a, node_b)
}
