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

// ---------------------------------------------------------------------------
// diff: identical files (exit 0)
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// diff: differing files (exit 1)
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// diff: parse failure (exit 2)
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// diff: --ids-only flag
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// diff: --summary-only flag
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// diff: --format json
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// diff: writes to stdout
// ---------------------------------------------------------------------------

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
