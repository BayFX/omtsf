//! Integration tests for `omtsf subgraph`.
#![allow(clippy::expect_used)]

use std::path::PathBuf;
use std::process::Command;

/// Path to the compiled `omtsf` binary.
fn omtsf_bin() -> PathBuf {
    let mut path = std::env::current_exe().expect("current exe");
    path.pop();
    if path.ends_with("deps") {
        path.pop();
    }
    path.push("omtsf");
    path
}

/// Path to a shared fixture file.
fn fixture(name: &str) -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("../../tests/fixtures");
    path.push(name);
    path
}

// ---------------------------------------------------------------------------
// subgraph: basic extraction
// ---------------------------------------------------------------------------

#[test]
fn subgraph_single_node_exits_0() {
    let out = Command::new(omtsf_bin())
        .args([
            "subgraph",
            fixture("graph-query.omts").to_str().expect("path"),
            "org-a",
        ])
        .output()
        .expect("run omtsf subgraph");
    assert!(out.status.success(), "exit code: {:?}", out.status.code());
}

#[test]
fn subgraph_output_is_valid_json() {
    let out = Command::new(omtsf_bin())
        .args([
            "subgraph",
            fixture("graph-query.omts").to_str().expect("path"),
            "org-a",
        ])
        .output()
        .expect("run omtsf subgraph");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: Result<serde_json::Value, _> = serde_json::from_str(stdout.trim());
    assert!(parsed.is_ok(), "output is not valid JSON: {stdout}");
}

#[test]
fn subgraph_output_is_valid_omts_file() {
    let out = Command::new(omtsf_bin())
        .args([
            "subgraph",
            fixture("graph-query.omts").to_str().expect("path"),
            "org-a",
            "org-b",
        ])
        .output()
        .expect("run omtsf subgraph");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("valid JSON from subgraph");
    // A valid .omts file has omtsf_version, snapshot_date, file_salt, nodes, edges.
    assert!(
        value.get("omtsf_version").is_some(),
        "missing omtsf_version"
    );
    assert!(
        value.get("snapshot_date").is_some(),
        "missing snapshot_date"
    );
    assert!(value.get("file_salt").is_some(), "missing file_salt");
    assert!(value.get("nodes").is_some(), "missing nodes");
    assert!(value.get("edges").is_some(), "missing edges");
}

#[test]
fn subgraph_two_nodes_includes_edge_between_them() {
    // org-a and org-b have edge e-ab between them.
    let out = Command::new(omtsf_bin())
        .args([
            "subgraph",
            fixture("graph-query.omts").to_str().expect("path"),
            "org-a",
            "org-b",
        ])
        .output()
        .expect("run omtsf subgraph org-a org-b");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("valid JSON from subgraph");

    let nodes = value["nodes"].as_array().expect("nodes array");
    assert_eq!(nodes.len(), 2, "expected exactly 2 nodes");

    let edges = value["edges"].as_array().expect("edges array");
    assert_eq!(edges.len(), 1, "expected exactly 1 edge (e-ab)");

    let node_ids: Vec<&str> = nodes.iter().filter_map(|n| n["id"].as_str()).collect();
    assert!(node_ids.contains(&"org-a"), "org-a should be in subgraph");
    assert!(node_ids.contains(&"org-b"), "org-b should be in subgraph");
}

#[test]
fn subgraph_two_non_adjacent_nodes_has_no_edges() {
    // org-a and org-d are not directly connected (3 hops apart).
    let out = Command::new(omtsf_bin())
        .args([
            "subgraph",
            fixture("graph-query.omts").to_str().expect("path"),
            "org-a",
            "org-d",
        ])
        .output()
        .expect("run omtsf subgraph org-a org-d");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("valid JSON from subgraph");

    let edges = value["edges"].as_array().expect("edges array");
    assert!(
        edges.is_empty(),
        "no edge between org-a and org-d: {stdout}"
    );
}

// ---------------------------------------------------------------------------
// subgraph: --expand flag
// ---------------------------------------------------------------------------

#[test]
fn subgraph_expand_1_includes_neighbours() {
    // Starting from org-b with expand=1, we get org-b plus its 1-hop both-direction
    // neighbours: org-a (incoming) and org-c (outgoing).
    let out = Command::new(omtsf_bin())
        .args([
            "subgraph",
            "--expand",
            "1",
            fixture("graph-query.omts").to_str().expect("path"),
            "org-b",
        ])
        .output()
        .expect("run omtsf subgraph --expand 1 org-b");
    assert!(out.status.success(), "exit code: {:?}", out.status.code());
    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("valid JSON from subgraph");

    let nodes = value["nodes"].as_array().expect("nodes array");
    let node_ids: Vec<&str> = nodes.iter().filter_map(|n| n["id"].as_str()).collect();

    // org-b (center), org-a (incoming 1 hop), org-c (outgoing 1 hop).
    assert!(node_ids.contains(&"org-b"), "org-b must be present");
    assert!(
        node_ids.contains(&"org-a"),
        "org-a is 1 hop incoming from org-b"
    );
    assert!(
        node_ids.contains(&"org-c"),
        "org-c is 1 hop outgoing from org-b"
    );
}

#[test]
fn subgraph_expand_0_is_same_as_no_expand() {
    // --expand 0 is the default; should be identical to omitting the flag.
    let out_default = Command::new(omtsf_bin())
        .args([
            "subgraph",
            fixture("graph-query.omts").to_str().expect("path"),
            "org-a",
            "org-b",
        ])
        .output()
        .expect("run omtsf subgraph (no expand)");

    let out_explicit = Command::new(omtsf_bin())
        .args([
            "subgraph",
            "--expand",
            "0",
            fixture("graph-query.omts").to_str().expect("path"),
            "org-a",
            "org-b",
        ])
        .output()
        .expect("run omtsf subgraph --expand 0");

    assert_eq!(
        out_default.stdout, out_explicit.stdout,
        "expand 0 == default"
    );
}

// ---------------------------------------------------------------------------
// subgraph: error cases
// ---------------------------------------------------------------------------

#[test]
fn subgraph_unknown_node_exits_1() {
    let out = Command::new(omtsf_bin())
        .args([
            "subgraph",
            fixture("graph-query.omts").to_str().expect("path"),
            "no-such-node",
        ])
        .output()
        .expect("run omtsf subgraph with unknown node");
    assert_eq!(
        out.status.code(),
        Some(1),
        "expected exit 1 for unknown node"
    );
}

#[test]
fn subgraph_nonexistent_file_exits_2() {
    let out = Command::new(omtsf_bin())
        .args(["subgraph", "/no/such/file/ever.omts", "org-a"])
        .output()
        .expect("run omtsf subgraph nonexistent");
    assert_eq!(
        out.status.code(),
        Some(2),
        "expected exit 2 for nonexistent file"
    );
}

#[test]
fn subgraph_invalid_json_exits_2() {
    use std::io::Write as _;
    let mut tmp = tempfile::NamedTempFile::new().expect("temp file");
    tmp.write_all(b"not-json").expect("write");
    let out = Command::new(omtsf_bin())
        .args(["subgraph", tmp.path().to_str().expect("path"), "org-a"])
        .output()
        .expect("run omtsf subgraph bad-json");
    assert_eq!(
        out.status.code(),
        Some(2),
        "expected exit 2 for invalid JSON"
    );
}
