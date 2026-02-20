//! Integration tests for `omtsf merge`.
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
// merge: two fixture files (exit 0)
// ---------------------------------------------------------------------------

#[test]
fn merge_two_fixtures_exits_0() {
    let out = Command::new(omtsf_bin())
        .args([
            "merge",
            fixture("merge-a.omts").to_str().expect("path"),
            fixture("merge-b.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf merge");
    assert_eq!(
        out.status.code(),
        Some(0),
        "expected exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn merge_two_fixtures_writes_valid_json_to_stdout() {
    let out = Command::new(omtsf_bin())
        .args([
            "merge",
            fixture("merge-a.omts").to_str().expect("path"),
            fixture("merge-b.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf merge");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: Result<serde_json::Value, _> = serde_json::from_str(stdout.trim());
    assert!(
        parsed.is_ok(),
        "stdout should be valid JSON; stdout: {stdout}"
    );
}

#[test]
fn merge_two_fixtures_output_has_nodes_and_edges() {
    let out = Command::new(omtsf_bin())
        .args([
            "merge",
            fixture("merge-a.omts").to_str().expect("path"),
            fixture("merge-b.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf merge");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("valid JSON from merge");
    assert!(value["nodes"].is_array(), "output must have nodes array");
    assert!(value["edges"].is_array(), "output must have edges array");
}

/// Two files sharing the same LEI must produce a merged node (fewer total
/// nodes than the sum of inputs).
#[test]
fn merge_shared_lei_deduplicates_nodes() {
    let out = Command::new(omtsf_bin())
        .args([
            "merge",
            fixture("merge-a.omts").to_str().expect("path"),
            fixture("merge-b.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf merge");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("valid JSON from merge");

    let nodes = value["nodes"].as_array().expect("nodes array");
    // merge-a has 2 nodes; merge-b has 2 nodes; they share one LEI so
    // the merged output has 3 unique entity groups.
    assert_eq!(
        nodes.len(),
        3,
        "three distinct entities expected after merge; nodes: {nodes:?}"
    );
}

#[test]
fn merge_output_is_valid_omts_file() {
    let out = Command::new(omtsf_bin())
        .args([
            "merge",
            fixture("merge-a.omts").to_str().expect("path"),
            fixture("merge-b.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf merge");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("valid JSON from merge");

    // Required fields in the output.
    assert!(
        value["omtsf_version"].is_string(),
        "omtsf_version must be present"
    );
    assert!(
        value["snapshot_date"].is_string(),
        "snapshot_date must be present"
    );
    assert!(value["file_salt"].is_string(), "file_salt must be present");
    // Merge metadata must be present.
    assert!(
        value["merge_metadata"].is_object(),
        "merge_metadata must be present in merged output"
    );
}

// ---------------------------------------------------------------------------
// merge: parse failure (exit 2)
// ---------------------------------------------------------------------------

#[test]
fn merge_invalid_json_file_exits_2() {
    let mut tmp = tempfile::NamedTempFile::new().expect("temp file");
    tmp.write_all(b"not-valid-json").expect("write");

    let out = Command::new(omtsf_bin())
        .args([
            "merge",
            fixture("merge-a.omts").to_str().expect("path"),
            tmp.path().to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf merge bad json");
    assert_eq!(
        out.status.code(),
        Some(2),
        "expected exit 2 for invalid JSON input"
    );
}

#[test]
fn merge_nonexistent_file_exits_2() {
    let out = Command::new(omtsf_bin())
        .args([
            "merge",
            fixture("merge-a.omts").to_str().expect("path"),
            "/no/such/file.omts",
        ])
        .output()
        .expect("run omtsf merge nonexistent");
    assert_eq!(
        out.status.code(),
        Some(2),
        "expected exit 2 for nonexistent file"
    );
}

// ---------------------------------------------------------------------------
// merge: requires at least two files
// ---------------------------------------------------------------------------

#[test]
fn merge_one_file_is_clap_error() {
    // clap enforces num_args = 2.. so one file fails at argument parsing.
    let out = Command::new(omtsf_bin())
        .args(["merge", fixture("merge-a.omts").to_str().expect("path")])
        .output()
        .expect("run omtsf merge one file");
    // clap exits 2 for argument errors.
    assert_eq!(
        out.status.code(),
        Some(2),
        "merge with one file should be rejected by clap"
    );
}

// ---------------------------------------------------------------------------
// merge: stdin support
// ---------------------------------------------------------------------------

#[test]
fn merge_stdin_and_file_exits_0() {
    let content = std::fs::read(fixture("merge-b.omts")).expect("read fixture");

    let mut child = Command::new(omtsf_bin())
        .args([
            "merge",
            "-",
            fixture("merge-a.omts").to_str().expect("path"),
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("spawn omtsf merge -");

    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(&content)
        .expect("write stdin");

    let out = child.wait_with_output().expect("wait");
    assert_eq!(
        out.status.code(),
        Some(0),
        "expected exit 0 for stdin merge; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

// ---------------------------------------------------------------------------
// merge: validate the merged output with `omtsf validate`
// ---------------------------------------------------------------------------

#[test]
fn merge_output_passes_validate() {
    // Merge → pipe to validate via a temp file.
    let merge_out = Command::new(omtsf_bin())
        .args([
            "merge",
            fixture("merge-a.omts").to_str().expect("path"),
            fixture("merge-b.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf merge");
    assert_eq!(merge_out.status.code(), Some(0), "merge must succeed first");

    let mut tmp = tempfile::NamedTempFile::new().expect("temp file");
    tmp.write_all(&merge_out.stdout)
        .expect("write merged output");

    let validate_out = Command::new(omtsf_bin())
        .args([
            "validate",
            "--level",
            "1",
            tmp.path().to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf validate on merged output");
    assert_eq!(
        validate_out.status.code(),
        Some(0),
        "merged output must pass L1 validation; stderr: {}",
        String::from_utf8_lossy(&validate_out.stderr)
    );
}
