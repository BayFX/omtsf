//! Shared test helper functions for constructing test fixtures.
//!
//! This module is compiled only in test builds and provides common constructors
//! for [`Node`], [`Edge`], and [`OmtsFile`] used across unit test modules
//! throughout `omtsf-core`.
//!
//! Integration tests in `crates/omtsf-core/tests/` define their own local
//! helpers because they link against the non-test library build where this
//! module is not available.
#![allow(clippy::expect_used)]

use std::collections::BTreeMap;

use crate::enums::{EdgeType, EdgeTypeTag, NodeType, NodeTypeTag};
use crate::file::OmtsFile;
use crate::newtypes::{CalendarDate, EdgeId, FileSalt, NodeId, SemVer};
use crate::structures::{Edge, EdgeProperties, Node};

/// A 64-hex-char salt used as the default in test fixtures.
pub const TEST_SALT: &str = "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef";

/// Parses a semver string, panicking on invalid input (test-only).
pub fn semver(s: &str) -> SemVer {
    SemVer::try_from(s).expect("valid SemVer")
}

/// Parses a calendar date string, panicking on invalid input (test-only).
pub fn date(s: &str) -> CalendarDate {
    CalendarDate::try_from(s).expect("valid CalendarDate")
}

/// Parses a file salt hex string, panicking on invalid input (test-only).
pub fn file_salt(s: &str) -> FileSalt {
    FileSalt::try_from(s).expect("valid FileSalt")
}

/// Creates a [`NodeId`] from a string slice, panicking on invalid input.
pub fn node_id(s: &str) -> NodeId {
    NodeId::try_from(s).expect("valid NodeId")
}

/// Creates an [`EdgeId`] from a string slice, panicking on invalid input.
pub fn edge_id(s: &str) -> EdgeId {
    EdgeId::try_from(s).expect("valid EdgeId")
}

/// Builds a minimal [`OmtsFile`] with the given nodes and edges.
///
/// Uses [`TEST_SALT`], version `1.0.0`, and date `2026-02-19`.
/// All optional header fields are `None`.
pub fn minimal_file(nodes: Vec<Node>, edges: Vec<Edge>) -> OmtsFile {
    OmtsFile {
        omtsf_version: semver("1.0.0"),
        snapshot_date: date("2026-02-19"),
        file_salt: file_salt(TEST_SALT),
        disclosure_scope: None,
        previous_snapshot_ref: None,
        snapshot_sequence: None,
        reporting_entity: None,
        nodes,
        edges,
        extra: BTreeMap::new(),
    }
}

/// Creates an organization [`Node`] with the given ID and all optional fields absent.
pub fn org_node(id: &str) -> Node {
    Node {
        id: node_id(id),
        node_type: NodeTypeTag::Known(NodeType::Organization),
        identifiers: None,
        data_quality: None,
        labels: None,
        name: None,
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

/// Creates a facility [`Node`] with the given ID and all optional fields absent.
pub fn facility_node(id: &str) -> Node {
    Node {
        node_type: NodeTypeTag::Known(NodeType::Facility),
        ..org_node(id)
    }
}

/// Creates a [`Node`] with an extension (non-built-in) type string.
pub fn extension_node(id: &str, type_str: &str) -> Node {
    Node {
        node_type: NodeTypeTag::Extension(type_str.to_owned()),
        ..org_node(id)
    }
}

/// Creates a [`Node`] with the given known node type.
pub fn typed_node(id: &str, node_type: NodeType) -> Node {
    Node {
        node_type: NodeTypeTag::Known(node_type),
        ..org_node(id)
    }
}

/// Creates a `supplies` [`Edge`] between two node IDs.
pub fn supplies_edge(id: &str, source: &str, target: &str) -> Edge {
    Edge {
        id: edge_id(id),
        edge_type: EdgeTypeTag::Known(EdgeType::Supplies),
        source: node_id(source),
        target: node_id(target),
        identifiers: None,
        properties: EdgeProperties::default(),
        extra: BTreeMap::new(),
    }
}

/// Creates an `ownership` [`Edge`] between two node IDs.
pub fn ownership_edge(id: &str, source: &str, target: &str) -> Edge {
    Edge {
        id: edge_id(id),
        edge_type: EdgeTypeTag::Known(EdgeType::Ownership),
        source: node_id(source),
        target: node_id(target),
        identifiers: None,
        properties: EdgeProperties::default(),
        extra: BTreeMap::new(),
    }
}

/// Creates a `legal_parentage` [`Edge`] between two node IDs.
pub fn legal_parentage_edge(id: &str, source: &str, target: &str) -> Edge {
    Edge {
        id: edge_id(id),
        edge_type: EdgeTypeTag::Known(EdgeType::LegalParentage),
        source: node_id(source),
        target: node_id(target),
        identifiers: None,
        properties: EdgeProperties::default(),
        extra: BTreeMap::new(),
    }
}

/// Creates an extension [`Edge`] with the given type string.
pub fn extension_edge(id: &str, source: &str, target: &str, type_str: &str) -> Edge {
    Edge {
        id: edge_id(id),
        edge_type: EdgeTypeTag::Extension(type_str.to_owned()),
        source: node_id(source),
        target: node_id(target),
        identifiers: None,
        properties: EdgeProperties::default(),
        extra: BTreeMap::new(),
    }
}

/// Creates an [`Edge`] with the given known edge type.
pub fn typed_edge(id: &str, edge_type: EdgeType, source: &str, target: &str) -> Edge {
    Edge {
        id: edge_id(id),
        edge_type: EdgeTypeTag::Known(edge_type),
        source: node_id(source),
        target: node_id(target),
        identifiers: None,
        properties: EdgeProperties::default(),
        extra: BTreeMap::new(),
    }
}
