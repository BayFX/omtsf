//! Integration tests for `omtsf diff`.
#![allow(clippy::expect_used)]

use std::io::Write as _;
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

/// Diffing a file against itself must exit 0 (no differences).
#[test]
fn diff_identical_files_exits_0() {
    let out = Command::new(omtsf_bin())
        .args([
            "diff",
            fixture("diff-base.omts").to_str().expect("path"),
            fixture("diff-base.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf diff");
    assert_eq!(
        out.status.code(),
        Some(0),
        "expected exit 0 for identical files; stdout: {}; stderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Diffing a file against itself must produce a summary showing zero changes.
#[test]
fn diff_identical_files_summary_shows_zero_changes() {
    let out = Command::new(omtsf_bin())
        .args([
            "diff",
            fixture("diff-base.omts").to_str().expect("path"),
            fixture("diff-base.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf diff");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("0 added"),
        "summary should show 0 added; stdout: {stdout}"
    );
    assert!(
        stdout.contains("0 removed"),
        "summary should show 0 removed; stdout: {stdout}"
    );
    assert!(
        stdout.contains("0 modified"),
        "summary should show 0 modified; stdout: {stdout}"
    );
}

/// Diffing two files with different node names must exit 1 (differences found).
#[test]
fn diff_modified_files_exits_1() {
    let out = Command::new(omtsf_bin())
        .args([
            "diff",
            fixture("diff-base.omts").to_str().expect("path"),
            fixture("diff-modified.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf diff");
    assert_eq!(
        out.status.code(),
        Some(1),
        "expected exit 1 for files with differences; stdout: {}",
        String::from_utf8_lossy(&out.stdout)
    );
}

/// The stdout output for modified files must contain a `~` prefix.
#[test]
fn diff_modified_files_output_contains_tilde_prefix() {
    let out = Command::new(omtsf_bin())
        .args([
            "diff",
            fixture("diff-base.omts").to_str().expect("path"),
            fixture("diff-modified.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf diff");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("~"),
        "output should contain ~ for modified element; stdout: {stdout}"
    );
}

/// The human output for modified files must include the summary line.
#[test]
fn diff_modified_files_output_contains_summary() {
    let out = Command::new(omtsf_bin())
        .args([
            "diff",
            fixture("diff-base.omts").to_str().expect("path"),
            fixture("diff-modified.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf diff");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("Summary:"),
        "output should contain Summary line; stdout: {stdout}"
    );
}

/// Diffing a non-JSON file must exit 2.
#[test]
fn diff_invalid_file_a_exits_2() {
    let mut tmp = tempfile::NamedTempFile::new().expect("temp file");
    tmp.write_all(b"not-valid-json").expect("write");
    let out = Command::new(omtsf_bin())
        .args([
            "diff",
            tmp.path().to_str().expect("path"),
            fixture("diff-base.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf diff");
    assert_eq!(
        out.status.code(),
        Some(2),
        "expected exit 2 for invalid file A"
    );
}

/// Diffing against a non-JSON file B must exit 2.
#[test]
fn diff_invalid_file_b_exits_2() {
    let mut tmp = tempfile::NamedTempFile::new().expect("temp file");
    tmp.write_all(b"not-valid-json").expect("write");
    let out = Command::new(omtsf_bin())
        .args([
            "diff",
            fixture("diff-base.omts").to_str().expect("path"),
            tmp.path().to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf diff");
    assert_eq!(
        out.status.code(),
        Some(2),
        "expected exit 2 for invalid file B"
    );
}

/// Diffing a non-existent file must exit 2.
#[test]
fn diff_nonexistent_file_exits_2() {
    let out = Command::new(omtsf_bin())
        .args([
            "diff",
            "/no/such/file/ever.omts",
            fixture("diff-base.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf diff");
    assert_eq!(
        out.status.code(),
        Some(2),
        "expected exit 2 for nonexistent file"
    );
}

/// `--ids-only` must still exit 1 when there are differences.
#[test]
fn diff_ids_only_exits_1_for_differences() {
    let out = Command::new(omtsf_bin())
        .args([
            "diff",
            "--ids-only",
            fixture("diff-base.omts").to_str().expect("path"),
            fixture("diff-modified.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf diff --ids-only");
    assert_eq!(
        out.status.code(),
        Some(1),
        "expected exit 1 with --ids-only; stdout: {}",
        String::from_utf8_lossy(&out.stdout)
    );
}

/// `--ids-only` output should contain the modified node ID.
#[test]
fn diff_ids_only_output_contains_node_id() {
    let out = Command::new(omtsf_bin())
        .args([
            "diff",
            "--ids-only",
            fixture("diff-base.omts").to_str().expect("path"),
            fixture("diff-modified.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf diff --ids-only");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("org-001"),
        "output should contain node ID; stdout: {stdout}"
    );
}

/// `--summary-only` must still exit 1 when there are differences.
#[test]
fn diff_summary_only_exits_1_for_differences() {
    let out = Command::new(omtsf_bin())
        .args([
            "diff",
            "--summary-only",
            fixture("diff-base.omts").to_str().expect("path"),
            fixture("diff-modified.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf diff --summary-only");
    assert_eq!(
        out.status.code(),
        Some(1),
        "expected exit 1 with --summary-only"
    );
}

/// `--summary-only` output must contain the Summary line.
#[test]
fn diff_summary_only_output_contains_summary_line() {
    let out = Command::new(omtsf_bin())
        .args([
            "diff",
            "--summary-only",
            fixture("diff-base.omts").to_str().expect("path"),
            fixture("diff-modified.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf diff --summary-only");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("Summary:"),
        "output should contain Summary line; stdout: {stdout}"
    );
}

/// JSON output for identical files must be valid JSON and contain a summary.
#[test]
fn diff_json_identical_files_exits_0_and_is_valid_json() {
    let out = Command::new(omtsf_bin())
        .args([
            "diff",
            "-f",
            "json",
            fixture("diff-base.omts").to_str().expect("path"),
            fixture("diff-base.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf diff -f json");
    assert_eq!(
        out.status.code(),
        Some(0),
        "expected exit 0 for identical files in JSON mode"
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: Result<serde_json::Value, _> = serde_json::from_str(&stdout);
    assert!(
        parsed.is_ok(),
        "stdout should be valid JSON; stdout: {stdout}"
    );
    let obj = parsed.expect("valid json");
    assert!(obj.get("summary").is_some(), "missing summary field");
    assert!(obj.get("nodes").is_some(), "missing nodes field");
    assert!(obj.get("edges").is_some(), "missing edges field");
}

/// JSON output for modified files must exit 1 and contain the modified summary counts.
#[test]
fn diff_json_modified_files_exits_1_and_is_valid_json() {
    let out = Command::new(omtsf_bin())
        .args([
            "diff",
            "-f",
            "json",
            fixture("diff-base.omts").to_str().expect("path"),
            fixture("diff-modified.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf diff -f json");
    assert_eq!(
        out.status.code(),
        Some(1),
        "expected exit 1 for modified files in JSON mode"
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let obj: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout should be valid JSON");
    let modified = obj["summary"]["nodes_modified"]
        .as_u64()
        .expect("nodes_modified");
    assert!(
        modified >= 1,
        "expected at least 1 modified node; obj: {obj}"
    );
}

/// Human mode output must go to stdout, not stderr.
#[test]
fn diff_output_goes_to_stdout() {
    let out = Command::new(omtsf_bin())
        .args([
            "diff",
            fixture("diff-base.omts").to_str().expect("path"),
            fixture("diff-modified.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf diff");
    assert!(!out.stdout.is_empty(), "diff output should go to stdout");
}

/// Identical files with `--ids-only` must exit 0 and show zero-change summary.
#[test]
fn diff_ids_only_identical_files_exits_0() {
    let out = Command::new(omtsf_bin())
        .args([
            "diff",
            "--ids-only",
            fixture("diff-base.omts").to_str().expect("path"),
            fixture("diff-base.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf diff --ids-only identical");
    assert_eq!(
        out.status.code(),
        Some(0),
        "expected exit 0 for identical files with --ids-only; stdout: {}",
        String::from_utf8_lossy(&out.stdout)
    );
}

/// `--ids-only` output must not contain property-level detail lines (indented `~`).
#[test]
fn diff_ids_only_suppresses_property_detail() {
    let out = Command::new(omtsf_bin())
        .args([
            "diff",
            "--ids-only",
            fixture("diff-base.omts").to_str().expect("path"),
            fixture("diff-modified.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf diff --ids-only");
    let stdout = String::from_utf8_lossy(&out.stdout);
    // The `~` for the changed node ID is expected, but property-level detail
    // lines are indented with four spaces and another `~`. Check those are absent.
    assert!(
        !stdout.contains("    ~"),
        "--ids-only should suppress property-level detail lines; stdout: {stdout}"
    );
}

/// `--summary-only` with identical files must exit 0 and show zero-change summary.
#[test]
fn diff_summary_only_identical_files_exits_0() {
    let out = Command::new(omtsf_bin())
        .args([
            "diff",
            "--summary-only",
            fixture("diff-base.omts").to_str().expect("path"),
            fixture("diff-base.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf diff --summary-only identical");
    assert_eq!(
        out.status.code(),
        Some(0),
        "expected exit 0 for identical files with --summary-only; stdout: {}",
        String::from_utf8_lossy(&out.stdout)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("0 added"),
        "summary should show 0 added; stdout: {stdout}"
    );
}

/// `--summary-only` output must not contain per-element Nodes/Edges section headers.
#[test]
fn diff_summary_only_omits_element_sections() {
    let out = Command::new(omtsf_bin())
        .args([
            "diff",
            "--summary-only",
            fixture("diff-base.omts").to_str().expect("path"),
            fixture("diff-modified.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf diff --summary-only");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        !stdout.contains("Nodes:"),
        "--summary-only should omit the Nodes: section; stdout: {stdout}"
    );
    assert!(
        !stdout.contains("Edges:"),
        "--summary-only should omit the Edges: section; stdout: {stdout}"
    );
}

/// Diffing a file against a minimal empty-graph file must show added node with `+`.
#[test]
fn diff_added_node_shows_plus_prefix() {
    let mut tmp = tempfile::NamedTempFile::new().expect("temp file");
    tmp.write_all(
        br#"{
  "omtsf_version": "0.1.0",
  "snapshot_date": "2026-02-18",
  "file_salt": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  "nodes": [],
  "edges": []
}"#,
    )
    .expect("write");
    let out = Command::new(omtsf_bin())
        .args([
            "diff",
            tmp.path().to_str().expect("path"),
            fixture("diff-base.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf diff (added node)");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("  + "),
        "output should contain `  + ` for added node; stdout: {stdout}"
    );
    assert_eq!(
        out.status.code(),
        Some(1),
        "should exit 1 when node is added; stdout: {stdout}"
    );
}

/// Diffing with `--node-type` that does not match any changed node must exit 0.
#[test]
fn diff_node_type_filter_unchanged_type_exits_0() {
    // diff-base and diff-modified both have only an "organization" node whose
    // name changed. Restricting the diff to "facility" nodes means no diff is
    // found, so exit code must be 0.
    let out = Command::new(omtsf_bin())
        .args([
            "diff",
            "--node-type",
            "facility",
            fixture("diff-base.omts").to_str().expect("path"),
            fixture("diff-modified.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf diff --node-type facility");
    assert_eq!(
        out.status.code(),
        Some(0),
        "expected exit 0 when --node-type filter matches no changed nodes; stdout: {}",
        String::from_utf8_lossy(&out.stdout)
    );
}

/// `--ignore-field name` on the name-only diff must exit 0 (difference filtered out).
#[test]
fn diff_ignore_field_name_exits_0() {
    // diff-base -> diff-modified only differs in the `name` field.
    // With --ignore-field name the diff should be empty and exit 0.
    let out = Command::new(omtsf_bin())
        .args([
            "diff",
            "--ignore-field",
            "name",
            fixture("diff-base.omts").to_str().expect("path"),
            fixture("diff-modified.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf diff --ignore-field name");
    assert_eq!(
        out.status.code(),
        Some(0),
        "expected exit 0 when the only differing field is ignored; stdout: {}; stderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

/// JSON output must be a single JSON object (not NDJSON or an array).
#[test]
fn diff_json_output_is_single_object() {
    let out = Command::new(omtsf_bin())
        .args([
            "diff",
            "-f",
            "json",
            fixture("diff-base.omts").to_str().expect("path"),
            fixture("diff-modified.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf diff -f json");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let trimmed = stdout.trim();
    // Must parse as a JSON object, not an array or primitive.
    let val: serde_json::Value =
        serde_json::from_str(trimmed).expect("stdout should be valid JSON");
    assert!(
        val.is_object(),
        "JSON output must be a single object, not an array or primitive; stdout: {stdout}"
    );
}

/// JSON output must contain `nodes_modified >= 1` in summary for the name-change diff.
#[test]
fn diff_json_summary_nodes_modified_count() {
    let out = Command::new(omtsf_bin())
        .args([
            "diff",
            "-f",
            "json",
            fixture("diff-base.omts").to_str().expect("path"),
            fixture("diff-modified.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf diff -f json");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let obj: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    let modified = obj["summary"]["nodes_modified"]
        .as_u64()
        .expect("nodes_modified should be a number");
    assert_eq!(
        modified, 1,
        "exactly one node was modified between the fixtures; obj: {obj}"
    );
}

/// JSON output `nodes.modified` array must contain an entry with the changed node ID.
#[test]
fn diff_json_nodes_modified_contains_id() {
    let out = Command::new(omtsf_bin())
        .args([
            "diff",
            "-f",
            "json",
            fixture("diff-base.omts").to_str().expect("path"),
            fixture("diff-modified.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf diff -f json");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let obj: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    let modified = obj["nodes"]["modified"]
        .as_array()
        .expect("nodes.modified should be an array");
    let ids: Vec<&str> = modified
        .iter()
        .filter_map(|e| e["id_a"].as_str().or_else(|| e["id_b"].as_str()))
        .collect();
    assert!(
        ids.contains(&"org-001"),
        "nodes.modified should contain org-001; ids: {ids:?}; obj: {obj}"
    );
}

/// JSON output `nodes.modified[0].property_changes` must contain a `name` field change.
#[test]
fn diff_json_property_changes_contain_name_field() {
    let out = Command::new(omtsf_bin())
        .args([
            "diff",
            "-f",
            "json",
            fixture("diff-base.omts").to_str().expect("path"),
            fixture("diff-modified.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf diff -f json");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let obj: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    let modified = obj["nodes"]["modified"]
        .as_array()
        .expect("nodes.modified should be an array");
    let changes = modified[0]["property_changes"]
        .as_array()
        .expect("property_changes should be an array");
    let fields: Vec<&str> = changes.iter().filter_map(|c| c["field"].as_str()).collect();
    assert!(
        fields.contains(&"name"),
        "property_changes should include the 'name' field; fields: {fields:?}; obj: {obj}"
    );
}
