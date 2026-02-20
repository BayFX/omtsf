//! Integration tests for `omtsf reach`.
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
// reach: human mode — basic reachability
// ---------------------------------------------------------------------------

#[test]
fn reach_forward_from_root_exits_0() {
    let out = Command::new(omtsf_bin())
        .args([
            "reach",
            fixture("graph-query.omts").to_str().expect("path"),
            "org-a",
        ])
        .output()
        .expect("run omtsf reach");
    assert!(out.status.success(), "exit code: {:?}", out.status.code());
}

#[test]
fn reach_forward_from_root_lists_all_reachable_nodes() {
    let out = Command::new(omtsf_bin())
        .args([
            "reach",
            fixture("graph-query.omts").to_str().expect("path"),
            "org-a",
        ])
        .output()
        .expect("run omtsf reach");
    let stdout = String::from_utf8_lossy(&out.stdout);
    // org-a → org-b → org-c → org-d  and  org-a → org-e
    assert!(
        stdout.contains("org-b"),
        "org-b should be reachable: {stdout}"
    );
    assert!(
        stdout.contains("org-c"),
        "org-c should be reachable: {stdout}"
    );
    assert!(
        stdout.contains("org-d"),
        "org-d should be reachable: {stdout}"
    );
    assert!(
        stdout.contains("org-e"),
        "org-e should be reachable: {stdout}"
    );
    // The start node itself is excluded.
    assert!(
        !stdout.contains("org-a"),
        "org-a (start) must not appear: {stdout}"
    );
}

#[test]
fn reach_forward_leaf_node_is_empty() {
    let out = Command::new(omtsf_bin())
        .args([
            "reach",
            fixture("graph-query.omts").to_str().expect("path"),
            "org-d",
        ])
        .output()
        .expect("run omtsf reach");
    assert!(out.status.success(), "exit code: {:?}", out.status.code());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.trim().is_empty(),
        "no nodes reachable from leaf org-d: {stdout}"
    );
}

// ---------------------------------------------------------------------------
// reach: --depth flag
// ---------------------------------------------------------------------------

#[test]
fn reach_with_depth_1_returns_direct_neighbours_only() {
    let out = Command::new(omtsf_bin())
        .args([
            "reach",
            "--depth",
            "1",
            fixture("graph-query.omts").to_str().expect("path"),
            "org-a",
        ])
        .output()
        .expect("run omtsf reach --depth 1");
    assert!(out.status.success(), "exit code: {:?}", out.status.code());
    let stdout = String::from_utf8_lossy(&out.stdout);
    // Direct neighbours of org-a are org-b and org-e.
    assert!(
        stdout.contains("org-b"),
        "org-b is a direct neighbour: {stdout}"
    );
    assert!(
        stdout.contains("org-e"),
        "org-e is a direct neighbour: {stdout}"
    );
    // org-c and org-d require more than 1 hop.
    assert!(
        !stdout.contains("org-c"),
        "org-c is 2 hops away, must not appear: {stdout}"
    );
    assert!(
        !stdout.contains("org-d"),
        "org-d is 3 hops away, must not appear: {stdout}"
    );
}

// ---------------------------------------------------------------------------
// reach: --direction flag
// ---------------------------------------------------------------------------

#[test]
fn reach_incoming_direction_traverses_upstream() {
    let out = Command::new(omtsf_bin())
        .args([
            "reach",
            "--direction",
            "incoming",
            fixture("graph-query.omts").to_str().expect("path"),
            "org-d",
        ])
        .output()
        .expect("run omtsf reach --direction incoming");
    assert!(out.status.success(), "exit code: {:?}", out.status.code());
    let stdout = String::from_utf8_lossy(&out.stdout);
    // org-d is reachable backwards from org-a, org-b, org-c.
    assert!(
        stdout.contains("org-c"),
        "org-c is direct upstream of org-d: {stdout}"
    );
    assert!(
        stdout.contains("org-b"),
        "org-b is 2 hops upstream: {stdout}"
    );
    assert!(
        stdout.contains("org-a"),
        "org-a is 3 hops upstream: {stdout}"
    );
}

#[test]
fn reach_both_direction_finds_more_nodes() {
    let out = Command::new(omtsf_bin())
        .args([
            "reach",
            "--direction",
            "both",
            fixture("graph-query.omts").to_str().expect("path"),
            "org-b",
        ])
        .output()
        .expect("run omtsf reach --direction both");
    assert!(out.status.success(), "exit code: {:?}", out.status.code());
    let stdout = String::from_utf8_lossy(&out.stdout);
    // org-b can reach org-a backwards and org-c, org-d, org-e is reachable via org-a.
    assert!(
        stdout.contains("org-a"),
        "org-a is upstream (backward): {stdout}"
    );
    assert!(
        stdout.contains("org-c"),
        "org-c is downstream (forward): {stdout}"
    );
}

// ---------------------------------------------------------------------------
// reach: JSON mode
// ---------------------------------------------------------------------------

#[test]
fn reach_json_mode_exits_0() {
    let out = Command::new(omtsf_bin())
        .args([
            "reach",
            "-f",
            "json",
            fixture("graph-query.omts").to_str().expect("path"),
            "org-a",
        ])
        .output()
        .expect("run omtsf reach -f json");
    assert!(out.status.success(), "exit code: {:?}", out.status.code());
}

#[test]
fn reach_json_mode_output_is_valid_json() {
    let out = Command::new(omtsf_bin())
        .args([
            "reach",
            "-f",
            "json",
            fixture("graph-query.omts").to_str().expect("path"),
            "org-a",
        ])
        .output()
        .expect("run omtsf reach -f json");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: Result<serde_json::Value, _> = serde_json::from_str(stdout.trim());
    assert!(parsed.is_ok(), "output is not valid JSON: {stdout}");
}

#[test]
fn reach_json_mode_contains_node_ids_and_count() {
    let out = Command::new(omtsf_bin())
        .args([
            "reach",
            "-f",
            "json",
            fixture("graph-query.omts").to_str().expect("path"),
            "org-a",
        ])
        .output()
        .expect("run omtsf reach -f json");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("valid JSON from reach");
    assert!(value.get("node_ids").is_some(), "missing node_ids field");
    assert!(value.get("count").is_some(), "missing count field");
    // 4 reachable nodes: org-b, org-c, org-d, org-e
    assert_eq!(value["count"], 4, "expected count 4");
}

// ---------------------------------------------------------------------------
// reach: error cases
// ---------------------------------------------------------------------------

#[test]
fn reach_unknown_node_exits_1() {
    let out = Command::new(omtsf_bin())
        .args([
            "reach",
            fixture("graph-query.omts").to_str().expect("path"),
            "no-such-node",
        ])
        .output()
        .expect("run omtsf reach with unknown node");
    assert_eq!(
        out.status.code(),
        Some(1),
        "expected exit 1 for unknown node"
    );
}

#[test]
fn reach_nonexistent_file_exits_2() {
    let out = Command::new(omtsf_bin())
        .args(["reach", "/no/such/file/ever.omts", "org-a"])
        .output()
        .expect("run omtsf reach nonexistent");
    assert_eq!(
        out.status.code(),
        Some(2),
        "expected exit 2 for nonexistent file"
    );
}

#[test]
fn reach_invalid_json_exits_2() {
    use std::io::Write as _;
    let mut tmp = tempfile::NamedTempFile::new().expect("temp file");
    tmp.write_all(b"not-json").expect("write");
    let out = Command::new(omtsf_bin())
        .args(["reach", tmp.path().to_str().expect("path"), "org-a"])
        .output()
        .expect("run omtsf reach bad-json");
    assert_eq!(
        out.status.code(),
        Some(2),
        "expected exit 2 for invalid JSON"
    );
}
