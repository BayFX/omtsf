//! Integration tests for `omtsf inspect`.
#![allow(clippy::expect_used)]

use std::path::PathBuf;
use std::process::Command;

/// Path to the compiled `omtsf` binary.
fn omtsf_bin() -> PathBuf {
    let mut path = std::env::current_exe().expect("current exe");
    // current_exe is something like …/deps/cmd_inspect-<hash>
    // The binary lives in the parent directory.
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
    // CARGO_MANIFEST_DIR is .../crates/omtsf-cli; fixtures are in tests/fixtures
    // relative to the workspace root.
    path.push("../../tests/fixtures");
    path.push(name);
    path
}

// ---------------------------------------------------------------------------
// inspect: human mode
// ---------------------------------------------------------------------------

#[test]
fn inspect_minimal_human_exit_0() {
    let out = Command::new(omtsf_bin())
        .args(["inspect", fixture("minimal.omts").to_str().expect("path")])
        .output()
        .expect("run omtsf inspect");
    assert!(out.status.success(), "exit code: {:?}", out.status.code());
}

#[test]
fn inspect_minimal_human_shows_version() {
    let out = Command::new(omtsf_bin())
        .args(["inspect", fixture("minimal.omts").to_str().expect("path")])
        .output()
        .expect("run omtsf inspect");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("version:"), "stdout: {stdout}");
    assert!(stdout.contains("0.1.0"), "stdout: {stdout}");
}

#[test]
fn inspect_minimal_human_shows_snapshot_date() {
    let out = Command::new(omtsf_bin())
        .args(["inspect", fixture("minimal.omts").to_str().expect("path")])
        .output()
        .expect("run omtsf inspect");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("snapshot_date:"), "stdout: {stdout}");
    assert!(stdout.contains("2026-02-18"), "stdout: {stdout}");
}

#[test]
fn inspect_minimal_human_shows_node_count() {
    let out = Command::new(omtsf_bin())
        .args(["inspect", fixture("minimal.omts").to_str().expect("path")])
        .output()
        .expect("run omtsf inspect");
    let stdout = String::from_utf8_lossy(&out.stdout);
    // minimal.omts has 1 organization node and 0 edges
    assert!(stdout.contains("nodes:"), "stdout: {stdout}");
    assert!(stdout.contains("organization:"), "stdout: {stdout}");
}

#[test]
fn inspect_full_featured_human_counts() {
    let out = Command::new(omtsf_bin())
        .args([
            "inspect",
            fixture("full-featured.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf inspect");
    assert!(out.status.success(), "exit code: {:?}", out.status.code());
    let stdout = String::from_utf8_lossy(&out.stdout);
    // 8 nodes, 8 edges, disclosure scope present
    assert!(stdout.contains("disclosure:"), "stdout: {stdout}");
    assert!(stdout.contains("partner"), "stdout: {stdout}");
    assert!(stdout.contains("identifiers:"), "stdout: {stdout}");
}

// ---------------------------------------------------------------------------
// inspect: JSON mode
// ---------------------------------------------------------------------------

#[test]
fn inspect_minimal_json_exit_0() {
    let out = Command::new(omtsf_bin())
        .args([
            "inspect",
            "-f",
            "json",
            fixture("minimal.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf inspect -f json");
    assert!(out.status.success(), "exit code: {:?}", out.status.code());
}

#[test]
fn inspect_minimal_json_is_valid_json() {
    let out = Command::new(omtsf_bin())
        .args([
            "inspect",
            "-f",
            "json",
            fixture("minimal.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf inspect -f json");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: Result<serde_json::Value, _> = serde_json::from_str(stdout.trim());
    assert!(parsed.is_ok(), "output is not valid JSON: {stdout}");
}

#[test]
fn inspect_minimal_json_contains_required_fields() {
    let out = Command::new(omtsf_bin())
        .args([
            "inspect",
            "-f",
            "json",
            fixture("minimal.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf inspect -f json");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("valid JSON from inspect");
    assert!(value.get("version").is_some(), "missing version field");
    assert!(
        value.get("snapshot_date").is_some(),
        "missing snapshot_date"
    );
    assert!(value.get("node_count").is_some(), "missing node_count");
    assert!(value.get("edge_count").is_some(), "missing edge_count");
    assert!(value.get("node_counts").is_some(), "missing node_counts");
    assert!(value.get("edge_counts").is_some(), "missing edge_counts");
    assert!(
        value.get("identifier_count").is_some(),
        "missing identifier_count"
    );
    assert!(
        value.get("identifier_counts").is_some(),
        "missing identifier_counts"
    );
}

#[test]
fn inspect_full_json_has_correct_counts() {
    let out = Command::new(omtsf_bin())
        .args([
            "inspect",
            "-f",
            "json",
            fixture("full-featured.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf inspect -f json");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("valid JSON from inspect");

    assert_eq!(value["node_count"], 8, "expected 8 nodes");
    assert_eq!(value["edge_count"], 8, "expected 8 edges");
    assert_eq!(value["identifier_count"], 13, "expected 13 identifiers");
    assert_eq!(value["node_counts"]["organization"], 2, "expected 2 orgs");
    assert_eq!(value["disclosure_scope"], "partner");
}

// ---------------------------------------------------------------------------
// inspect: stdin
// ---------------------------------------------------------------------------

#[test]
fn inspect_stdin_minimal() {
    use std::io::Write as _;
    let content = std::fs::read(fixture("minimal.omts")).expect("read fixture");
    let mut child = Command::new(omtsf_bin())
        .args(["inspect", "-"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("spawn omtsf inspect -");
    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(&content)
        .expect("write stdin");
    let out = child.wait_with_output().expect("wait");
    assert!(out.status.success(), "exit code: {:?}", out.status.code());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("version:"), "stdout: {stdout}");
}

// ---------------------------------------------------------------------------
// inspect: error cases
// ---------------------------------------------------------------------------

#[test]
fn inspect_nonexistent_file_exits_2() {
    let out = Command::new(omtsf_bin())
        .args(["inspect", "/no/such/file/ever.omts"])
        .output()
        .expect("run omtsf inspect nonexistent");
    assert_eq!(
        out.status.code(),
        Some(2),
        "expected exit 2 for nonexistent file"
    );
}

#[test]
fn inspect_invalid_json_exits_2() {
    use std::io::Write as _;
    let mut tmp = tempfile::NamedTempFile::new().expect("temp file");
    tmp.write_all(b"not-json").expect("write");
    let out = Command::new(omtsf_bin())
        .args(["inspect", tmp.path().to_str().expect("path")])
        .output()
        .expect("run omtsf inspect bad-json");
    assert_eq!(
        out.status.code(),
        Some(2),
        "expected exit 2 for invalid JSON"
    );
}
