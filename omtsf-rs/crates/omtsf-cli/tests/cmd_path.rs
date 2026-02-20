//! Integration tests for `omtsf path`.
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
// path: human mode — basic path finding
// ---------------------------------------------------------------------------

#[test]
fn path_exists_exits_0() {
    let out = Command::new(omtsf_bin())
        .args([
            "path",
            fixture("graph-query.omts").to_str().expect("path"),
            "org-a",
            "org-d",
        ])
        .output()
        .expect("run omtsf path");
    assert!(out.status.success(), "exit code: {:?}", out.status.code());
}

#[test]
fn path_human_output_uses_arrow_separator() {
    let out = Command::new(omtsf_bin())
        .args([
            "path",
            fixture("graph-query.omts").to_str().expect("path"),
            "org-a",
            "org-d",
        ])
        .output()
        .expect("run omtsf path");
    let stdout = String::from_utf8_lossy(&out.stdout);
    // At least one path must be present and use " -> " as separator.
    assert!(
        stdout.contains(" -> "),
        "human output should use ' -> ' separator: {stdout}"
    );
    assert!(
        stdout.contains("org-a"),
        "path should start with org-a: {stdout}"
    );
    assert!(
        stdout.contains("org-d"),
        "path should end with org-d: {stdout}"
    );
}

#[test]
fn path_includes_intermediate_nodes() {
    let out = Command::new(omtsf_bin())
        .args([
            "path",
            fixture("graph-query.omts").to_str().expect("path"),
            "org-a",
            "org-d",
        ])
        .output()
        .expect("run omtsf path");
    let stdout = String::from_utf8_lossy(&out.stdout);
    // The only path from org-a to org-d is org-a -> org-b -> org-c -> org-d.
    assert!(
        stdout.contains("org-b"),
        "path should include org-b: {stdout}"
    );
    assert!(
        stdout.contains("org-c"),
        "path should include org-c: {stdout}"
    );
}

// ---------------------------------------------------------------------------
// path: --max-paths flag
// ---------------------------------------------------------------------------

#[test]
fn path_max_paths_limits_output() {
    // From org-a to org-d there is only one path (linear chain); max-paths=1
    // should return exactly one path.
    let out = Command::new(omtsf_bin())
        .args([
            "path",
            "--max-paths",
            "1",
            fixture("graph-query.omts").to_str().expect("path"),
            "org-a",
            "org-d",
        ])
        .output()
        .expect("run omtsf path --max-paths 1");
    assert!(out.status.success(), "exit code: {:?}", out.status.code());
    let stdout = String::from_utf8_lossy(&out.stdout);
    let line_count = stdout.trim().lines().count();
    assert_eq!(line_count, 1, "expected exactly 1 path: {stdout}");
}

// ---------------------------------------------------------------------------
// path: --max-depth flag
// ---------------------------------------------------------------------------

#[test]
fn path_max_depth_too_short_exits_1() {
    // org-a to org-d requires 3 edges; max-depth=2 should find no path.
    let out = Command::new(omtsf_bin())
        .args([
            "path",
            "--max-depth",
            "2",
            fixture("graph-query.omts").to_str().expect("path"),
            "org-a",
            "org-d",
        ])
        .output()
        .expect("run omtsf path --max-depth 2");
    assert_eq!(
        out.status.code(),
        Some(1),
        "expected exit 1 when no path within depth: {:?}",
        out.status.code()
    );
}

// ---------------------------------------------------------------------------
// path: JSON mode
// ---------------------------------------------------------------------------

#[test]
fn path_json_mode_exits_0() {
    let out = Command::new(omtsf_bin())
        .args([
            "path",
            "-f",
            "json",
            fixture("graph-query.omts").to_str().expect("path"),
            "org-a",
            "org-d",
        ])
        .output()
        .expect("run omtsf path -f json");
    assert!(out.status.success(), "exit code: {:?}", out.status.code());
}

#[test]
fn path_json_output_is_valid_json() {
    let out = Command::new(omtsf_bin())
        .args([
            "path",
            "-f",
            "json",
            fixture("graph-query.omts").to_str().expect("path"),
            "org-a",
            "org-d",
        ])
        .output()
        .expect("run omtsf path -f json");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: Result<serde_json::Value, _> = serde_json::from_str(stdout.trim());
    assert!(parsed.is_ok(), "output is not valid JSON: {stdout}");
}

#[test]
fn path_json_contains_paths_and_count() {
    let out = Command::new(omtsf_bin())
        .args([
            "path",
            "-f",
            "json",
            fixture("graph-query.omts").to_str().expect("path"),
            "org-a",
            "org-d",
        ])
        .output()
        .expect("run omtsf path -f json");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("valid JSON from path");
    assert!(value.get("paths").is_some(), "missing paths field");
    assert!(value.get("count").is_some(), "missing count field");
    // paths should be an array of arrays.
    let paths = value["paths"].as_array().expect("paths should be an array");
    assert!(!paths.is_empty(), "paths array should not be empty");
    let first_path = paths[0].as_array().expect("first path should be an array");
    assert_eq!(
        first_path[0].as_str(),
        Some("org-a"),
        "first node should be org-a"
    );
    assert_eq!(
        first_path.last().and_then(|v| v.as_str()),
        Some("org-d"),
        "last node should be org-d"
    );
}

// ---------------------------------------------------------------------------
// path: error cases
// ---------------------------------------------------------------------------

#[test]
fn path_no_path_exits_1() {
    // org-d → org-a is impossible in the forward direction.
    let out = Command::new(omtsf_bin())
        .args([
            "path",
            fixture("graph-query.omts").to_str().expect("path"),
            "org-d",
            "org-a",
        ])
        .output()
        .expect("run omtsf path with no route");
    assert_eq!(
        out.status.code(),
        Some(1),
        "expected exit 1 when no path exists"
    );
}

#[test]
fn path_unknown_from_node_exits_1() {
    let out = Command::new(omtsf_bin())
        .args([
            "path",
            fixture("graph-query.omts").to_str().expect("path"),
            "no-such-node",
            "org-a",
        ])
        .output()
        .expect("run omtsf path with unknown from node");
    assert_eq!(
        out.status.code(),
        Some(1),
        "expected exit 1 for unknown from node"
    );
}

#[test]
fn path_unknown_to_node_exits_1() {
    let out = Command::new(omtsf_bin())
        .args([
            "path",
            fixture("graph-query.omts").to_str().expect("path"),
            "org-a",
            "no-such-node",
        ])
        .output()
        .expect("run omtsf path with unknown to node");
    assert_eq!(
        out.status.code(),
        Some(1),
        "expected exit 1 for unknown to node"
    );
}

#[test]
fn path_nonexistent_file_exits_2() {
    let out = Command::new(omtsf_bin())
        .args(["path", "/no/such/file/ever.omts", "org-a", "org-b"])
        .output()
        .expect("run omtsf path nonexistent");
    assert_eq!(
        out.status.code(),
        Some(2),
        "expected exit 2 for nonexistent file"
    );
}

#[test]
fn path_invalid_json_exits_2() {
    use std::io::Write as _;
    let mut tmp = tempfile::NamedTempFile::new().expect("temp file");
    tmp.write_all(b"not-json").expect("write");
    let out = Command::new(omtsf_bin())
        .args(["path", tmp.path().to_str().expect("path"), "org-a", "org-b"])
        .output()
        .expect("run omtsf path bad-json");
    assert_eq!(
        out.status.code(),
        Some(2),
        "expected exit 2 for invalid JSON"
    );
}
