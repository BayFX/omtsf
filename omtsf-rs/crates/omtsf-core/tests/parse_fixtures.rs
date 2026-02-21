//! Integration tests: load fixture files and parse them as typed [`OmtsFile`] values.
//!
//! Each test reads a `.omts` file from `tests/fixtures/`, deserialises it with
//! [`serde_json::from_str::<OmtsFile>()`], asserts structural invariants, and
//! performs a full round-trip (serialise → re-parse → equality) to confirm that
//! no data is dropped.
#![allow(clippy::expect_used)]

use std::path::PathBuf;

use omtsf_core::OmtsFile;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .canonicalize()
        .expect("fixtures directory should exist")
}

fn read_fixture(name: &str) -> String {
    let path = fixtures_dir().join(name);
    std::fs::read_to_string(&path).expect("fixture file should be readable")
}

/// Deserialise JSON text into an [`OmtsFile`], asserting success.
fn parse(json: &str, _label: &str) -> OmtsFile {
    serde_json::from_str::<OmtsFile>(json).expect("fixture should parse as OmtsFile")
}

/// Serialise, re-parse, and assert structural equality.
fn round_trip(f: &OmtsFile, label: &str) {
    let serialised = serde_json::to_string(f).expect("OmtsFile should serialise to JSON");
    let back: OmtsFile =
        serde_json::from_str(&serialised).expect("re-serialised OmtsFile should re-parse");
    assert_eq!(*f, back, "{label}: round-trip produced a different value");
}

/// Parse `minimal.omts` as a typed [`OmtsFile`] and verify required fields.
#[test]
fn parse_minimal_fixture() {
    let content = read_fixture("minimal.omts");
    let f = parse(&content, "minimal.omts");

    assert!(
        !f.omtsf_version.is_empty(),
        "omtsf_version must be non-empty"
    );
    assert!(
        !f.snapshot_date.is_empty(),
        "snapshot_date must be non-empty"
    );
    assert!(!f.file_salt.is_empty(), "file_salt must be non-empty");

    assert_eq!(
        f.nodes.len(),
        1,
        "minimal fixture should have exactly one node"
    );
    assert!(f.edges.is_empty(), "minimal fixture should have no edges");

    let node = &f.nodes[0];
    assert_eq!(&*node.id, "org-acme");

    round_trip(&f, "minimal.omts");
}

/// Parse `full-featured.omts` as a typed [`OmtsFile`] and verify a rich graph.
#[test]
fn parse_full_featured_fixture() {
    let content = read_fixture("full-featured.omts");
    let f = parse(&content, "full-featured.omts");

    assert!(
        f.disclosure_scope.is_some(),
        "disclosure_scope should be set"
    );
    assert!(
        f.previous_snapshot_ref.is_some(),
        "previous_snapshot_ref should be set"
    );
    assert!(
        f.snapshot_sequence.is_some(),
        "snapshot_sequence should be set"
    );

    assert!(
        f.nodes.len() > 1,
        "full-featured fixture must have multiple nodes"
    );
    assert!(!f.edges.is_empty(), "full-featured fixture must have edges");

    use omtsf_core::enums::{NodeType, NodeTypeTag};
    let types_present: Vec<&NodeTypeTag> = f.nodes.iter().map(|n| &n.node_type).collect();
    assert!(
        types_present.contains(&&NodeTypeTag::Known(NodeType::Organization)),
        "must include an organization node"
    );
    assert!(
        types_present.contains(&&NodeTypeTag::Known(NodeType::Facility)),
        "must include a facility node"
    );
    assert!(
        types_present.contains(&&NodeTypeTag::Known(NodeType::Good)),
        "must include a good node"
    );

    use omtsf_core::enums::{EdgeType, EdgeTypeTag};
    let edge_types: Vec<&EdgeTypeTag> = f.edges.iter().map(|e| &e.edge_type).collect();
    assert!(
        edge_types.contains(&&EdgeTypeTag::Known(EdgeType::Ownership)),
        "must include an ownership edge"
    );
    assert!(
        edge_types.contains(&&EdgeTypeTag::Known(EdgeType::Supplies)),
        "must include a supplies edge"
    );

    let org_alpha = f
        .nodes
        .iter()
        .find(|n| &*n.id == "org-alpha")
        .expect("org-alpha node must be present");
    let ids = org_alpha
        .identifiers
        .as_ref()
        .expect("org-alpha should have identifiers");
    assert!(ids.len() >= 2, "org-alpha should have multiple identifiers");

    let dq = org_alpha
        .data_quality
        .as_ref()
        .expect("org-alpha should have data_quality");
    assert!(
        dq.confidence.is_some(),
        "data_quality.confidence should be set"
    );

    round_trip(&f, "full-featured.omts");
}

/// Parse `extension-types.omts` as a typed [`OmtsFile`].
///
/// This fixture exercises:
/// - [`NodeTypeTag::Extension`] for non-built-in node type strings
/// - [`EdgeTypeTag::Extension`] for non-built-in edge type strings
/// - Unknown top-level fields captured in [`OmtsFile::extra`]
/// - Unknown node-level fields captured in [`omtsf_core::Node::extra`]
/// - Unknown edge-level fields captured in [`omtsf_core::Edge::extra`]
#[test]
fn parse_extension_types_fixture() {
    let content = read_fixture("extension-types.omts");
    let f = parse(&content, "extension-types.omts");

    assert!(
        f.extra.contains_key("x_producer"),
        "x_producer should be captured in OmtsFile::extra"
    );
    assert!(
        f.extra.contains_key("x_schema_hint"),
        "x_schema_hint should be captured in OmtsFile::extra"
    );

    use omtsf_core::enums::NodeTypeTag;
    let ext_nodes: Vec<_> = f
        .nodes
        .iter()
        .filter(|n| matches!(&n.node_type, NodeTypeTag::Extension(_)))
        .collect();
    assert!(
        ext_nodes.len() >= 2,
        "extension-types fixture should have at least two extension-typed nodes, got {}",
        ext_nodes.len()
    );

    let custom_site = f
        .nodes
        .iter()
        .find(|n| &*n.id == "ext-node-custom-site")
        .expect("ext-node-custom-site must be present");
    assert!(
        matches!(&custom_site.node_type, NodeTypeTag::Extension(s) if s == "com.example.custom-site"),
        "node type should be Extension(com.example.custom-site)"
    );
    assert!(
        custom_site.extra.contains_key("x_site_code"),
        "x_site_code should be in node extra"
    );
    assert!(
        custom_site.extra.contains_key("x_site_metadata"),
        "x_site_metadata should be in node extra"
    );

    use omtsf_core::enums::EdgeTypeTag;
    let ext_edges: Vec<_> = f
        .edges
        .iter()
        .filter(|e| matches!(&e.edge_type, EdgeTypeTag::Extension(_)))
        .collect();
    assert!(
        ext_edges.len() >= 2,
        "extension-types fixture should have at least two extension-typed edges, got {}",
        ext_edges.len()
    );

    let carbon_edge = f
        .edges
        .iter()
        .find(|e| &*e.id == "e-ext-carbon-link")
        .expect("e-ext-carbon-link edge must be present");
    assert!(
        matches!(&carbon_edge.edge_type, EdgeTypeTag::Extension(s) if s == "com.example.carbon-accounting-link"),
        "edge type should be Extension(com.example.carbon-accounting-link)"
    );
    assert!(
        carbon_edge.extra.contains_key("x_link_basis"),
        "x_link_basis should be in edge extra"
    );

    assert!(
        carbon_edge.properties.extra.contains_key("x_methodology"),
        "x_methodology should be in EdgeProperties::extra"
    );

    round_trip(&f, "extension-types.omts");
}
