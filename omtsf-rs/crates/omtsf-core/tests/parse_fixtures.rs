#![allow(clippy::expect_used)]

use std::path::PathBuf;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .canonicalize()
        .expect("fixtures directory should exist")
}

#[test]
fn parse_minimal_fixture() {
    let path = fixtures_dir().join("minimal.omts");
    let content = std::fs::read_to_string(&path).expect("should read minimal.omts");
    let value: serde_json::Value = serde_json::from_str(&content).expect("should parse as JSON");

    let obj = value.as_object().expect("root should be an object");
    assert!(obj.contains_key("omtsf_version"), "missing omtsf_version");
    assert!(obj.contains_key("nodes"), "missing nodes");
    assert!(obj.contains_key("edges"), "missing edges");
}

#[test]
fn parse_full_featured_fixture() {
    let path = fixtures_dir().join("full-featured.omts");
    let content = std::fs::read_to_string(&path).expect("should read full-featured.omts");
    let value: serde_json::Value = serde_json::from_str(&content).expect("should parse as JSON");

    let obj = value.as_object().expect("root should be an object");
    assert!(obj.contains_key("omtsf_version"), "missing omtsf_version");

    let nodes = obj["nodes"].as_array().expect("nodes should be an array");
    assert!(
        nodes.len() > 1,
        "full-featured fixture should have multiple nodes"
    );

    let edges = obj["edges"].as_array().expect("edges should be an array");
    assert!(!edges.is_empty(), "full-featured fixture should have edges");
}
