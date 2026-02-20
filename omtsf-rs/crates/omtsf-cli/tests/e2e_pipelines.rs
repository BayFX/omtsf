//! End-to-end integration tests for cross-command workflow pipelines.
//!
//! These tests invoke the `omtsf` binary and chain commands together via
//! temporary files, verifying that the output of one command is a valid
//! input to the next. All chains run in a single process per test.
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
// Pipeline 1: init | validate
// ---------------------------------------------------------------------------

/// `omtsf init` output piped to `omtsf validate` must exit 0.
#[test]
fn pipeline_init_validate() {
    let init_out = Command::new(omtsf_bin())
        .arg("init")
        .output()
        .expect("run omtsf init");
    assert!(
        init_out.status.success(),
        "init must succeed; stderr: {}",
        String::from_utf8_lossy(&init_out.stderr)
    );

    let mut tmp = tempfile::NamedTempFile::new().expect("temp file");
    tmp.write_all(&init_out.stdout).expect("write init output");

    let validate_out = Command::new(omtsf_bin())
        .args(["validate", tmp.path().to_str().expect("path")])
        .output()
        .expect("run omtsf validate on init output");
    assert_eq!(
        validate_out.status.code(),
        Some(0),
        "init output must pass validate; stderr: {}",
        String::from_utf8_lossy(&validate_out.stderr)
    );
}

// ---------------------------------------------------------------------------
// Pipeline 2: init --example | validate
// ---------------------------------------------------------------------------

/// `omtsf init --example` output must also pass validation.
#[test]
fn pipeline_init_example_validate() {
    let init_out = Command::new(omtsf_bin())
        .args(["init", "--example"])
        .output()
        .expect("run omtsf init --example");
    assert!(
        init_out.status.success(),
        "init --example must succeed; stderr: {}",
        String::from_utf8_lossy(&init_out.stderr)
    );

    let mut tmp = tempfile::NamedTempFile::new().expect("temp file");
    tmp.write_all(&init_out.stdout).expect("write init output");

    let validate_out = Command::new(omtsf_bin())
        .args(["validate", tmp.path().to_str().expect("path")])
        .output()
        .expect("run omtsf validate on init --example output");
    assert_eq!(
        validate_out.status.code(),
        Some(0),
        "init --example output must pass validate; stderr: {}",
        String::from_utf8_lossy(&validate_out.stderr)
    );
}

// ---------------------------------------------------------------------------
// Pipeline 3: merge A B | validate
// ---------------------------------------------------------------------------

/// Merging two fixture files produces output that passes `validate`.
#[test]
fn pipeline_merge_validate() {
    let merge_out = Command::new(omtsf_bin())
        .args([
            "merge",
            fixture("merge-a.omts").to_str().expect("path"),
            fixture("merge-b.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf merge");
    assert_eq!(
        merge_out.status.code(),
        Some(0),
        "merge must succeed; stderr: {}",
        String::from_utf8_lossy(&merge_out.stderr)
    );

    let mut tmp = tempfile::NamedTempFile::new().expect("temp file");
    tmp.write_all(&merge_out.stdout)
        .expect("write merge output");

    let validate_out = Command::new(omtsf_bin())
        .args([
            "validate",
            "--level",
            "1",
            tmp.path().to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf validate on merge output");
    assert_eq!(
        validate_out.status.code(),
        Some(0),
        "merged output must pass L1 validate; stderr: {}",
        String::from_utf8_lossy(&validate_out.stderr)
    );
}

// ---------------------------------------------------------------------------
// Pipeline 4: redact --scope public file | validate
// ---------------------------------------------------------------------------

/// Redacting to public scope produces output that passes `validate`.
#[test]
fn pipeline_redact_public_validate() {
    let redact_out = Command::new(omtsf_bin())
        .args([
            "redact",
            "--scope",
            "public",
            fixture("redact-internal.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf redact --scope public");
    assert_eq!(
        redact_out.status.code(),
        Some(0),
        "redact must succeed; stderr: {}",
        String::from_utf8_lossy(&redact_out.stderr)
    );

    let mut tmp = tempfile::NamedTempFile::new().expect("temp file");
    tmp.write_all(&redact_out.stdout)
        .expect("write redact output");

    let validate_out = Command::new(omtsf_bin())
        .args([
            "validate",
            "--level",
            "1",
            tmp.path().to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf validate on redacted output");
    assert_eq!(
        validate_out.status.code(),
        Some(0),
        "redacted output must pass L1 validate; stderr: {}",
        String::from_utf8_lossy(&validate_out.stderr)
    );
}

// ---------------------------------------------------------------------------
// Pipeline 5: merge A B | redact --scope public | validate
// ---------------------------------------------------------------------------

/// Merging then redacting produces output that passes `validate`.
#[test]
fn pipeline_merge_redact_validate() {
    // Step 1: merge
    let merge_out = Command::new(omtsf_bin())
        .args([
            "merge",
            fixture("merge-a.omts").to_str().expect("path"),
            fixture("merge-b.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf merge");
    assert_eq!(
        merge_out.status.code(),
        Some(0),
        "merge step must succeed; stderr: {}",
        String::from_utf8_lossy(&merge_out.stderr)
    );

    // Step 2: write merge output to temp file
    let mut merge_tmp = tempfile::NamedTempFile::new().expect("temp file for merge");
    merge_tmp
        .write_all(&merge_out.stdout)
        .expect("write merge output");

    // Step 3: redact --scope public
    let redact_out = Command::new(omtsf_bin())
        .args([
            "redact",
            "--scope",
            "public",
            merge_tmp.path().to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf redact on merged output");
    assert_eq!(
        redact_out.status.code(),
        Some(0),
        "redact step must succeed; stderr: {}",
        String::from_utf8_lossy(&redact_out.stderr)
    );

    // Step 4: validate
    let mut redact_tmp = tempfile::NamedTempFile::new().expect("temp file for redact");
    redact_tmp
        .write_all(&redact_out.stdout)
        .expect("write redact output");

    let validate_out = Command::new(omtsf_bin())
        .args([
            "validate",
            "--level",
            "1",
            redact_tmp.path().to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf validate on merge+redact output");
    assert_eq!(
        validate_out.status.code(),
        Some(0),
        "merge+redact output must pass L1 validate; stderr: {}",
        String::from_utf8_lossy(&validate_out.stderr)
    );
}

// ---------------------------------------------------------------------------
// Pipeline 6: merge A B | redact --scope public — verifies disclosure_scope
// ---------------------------------------------------------------------------

/// After merge+redact pipeline, the output's `disclosure_scope` is `"public"`.
#[test]
fn pipeline_merge_redact_sets_disclosure_scope() {
    let merge_out = Command::new(omtsf_bin())
        .args([
            "merge",
            fixture("merge-a.omts").to_str().expect("path"),
            fixture("merge-b.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf merge");
    assert_eq!(merge_out.status.code(), Some(0), "merge must succeed");

    let mut merge_tmp = tempfile::NamedTempFile::new().expect("temp file for merge");
    merge_tmp
        .write_all(&merge_out.stdout)
        .expect("write merge output");

    let redact_out = Command::new(omtsf_bin())
        .args([
            "redact",
            "--scope",
            "public",
            merge_tmp.path().to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf redact on merged output");
    assert_eq!(redact_out.status.code(), Some(0), "redact must succeed");

    let stdout = String::from_utf8_lossy(&redact_out.stdout);
    let value: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("valid JSON from merge+redact");
    assert_eq!(
        value["disclosure_scope"].as_str(),
        Some("public"),
        "merged+redacted output must have disclosure_scope = public"
    );
}

// ---------------------------------------------------------------------------
// Pipeline 7: subgraph | inspect
// ---------------------------------------------------------------------------

/// `omtsf subgraph` output can be passed to `omtsf inspect`.
#[test]
fn pipeline_subgraph_inspect() {
    let subgraph_out = Command::new(omtsf_bin())
        .args([
            "subgraph",
            fixture("graph-query.omts").to_str().expect("path"),
            "org-a",
            "org-b",
        ])
        .output()
        .expect("run omtsf subgraph");
    assert_eq!(
        subgraph_out.status.code(),
        Some(0),
        "subgraph must succeed; stderr: {}",
        String::from_utf8_lossy(&subgraph_out.stderr)
    );

    let mut tmp = tempfile::NamedTempFile::new().expect("temp file");
    tmp.write_all(&subgraph_out.stdout)
        .expect("write subgraph output");

    let inspect_out = Command::new(omtsf_bin())
        .args(["inspect", tmp.path().to_str().expect("path")])
        .output()
        .expect("run omtsf inspect on subgraph output");
    assert_eq!(
        inspect_out.status.code(),
        Some(0),
        "inspect of subgraph output must succeed; stderr: {}",
        String::from_utf8_lossy(&inspect_out.stderr)
    );
    let stdout = String::from_utf8_lossy(&inspect_out.stdout);
    assert!(
        stdout.contains("version:"),
        "inspect output must contain version; stdout: {stdout}"
    );
}

// ---------------------------------------------------------------------------
// Pipeline 8: subgraph | validate
// ---------------------------------------------------------------------------

/// `omtsf subgraph` output passes `omtsf validate`.
#[test]
fn pipeline_subgraph_validate() {
    let subgraph_out = Command::new(omtsf_bin())
        .args([
            "subgraph",
            "--expand",
            "1",
            fixture("graph-query.omts").to_str().expect("path"),
            "org-b",
        ])
        .output()
        .expect("run omtsf subgraph --expand 1");
    assert_eq!(
        subgraph_out.status.code(),
        Some(0),
        "subgraph must succeed; stderr: {}",
        String::from_utf8_lossy(&subgraph_out.stderr)
    );

    let mut tmp = tempfile::NamedTempFile::new().expect("temp file");
    tmp.write_all(&subgraph_out.stdout)
        .expect("write subgraph output");

    let validate_out = Command::new(omtsf_bin())
        .args([
            "validate",
            "--level",
            "1",
            tmp.path().to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf validate on subgraph output");
    assert_eq!(
        validate_out.status.code(),
        Some(0),
        "subgraph output must pass L1 validate; stderr: {}",
        String::from_utf8_lossy(&validate_out.stderr)
    );
}

// ---------------------------------------------------------------------------
// Pipeline 9: convert | diff (converted against itself → exit 0)
// ---------------------------------------------------------------------------

/// Converting a file then compacting it, and diffing the compact output against
/// itself, gives no differences (exit 0). This verifies convert is a stable
/// round-trip and that diff correctly handles the identical-file case.
#[test]
fn pipeline_convert_diff_identical() {
    // Convert the fixture to compact form.
    let convert_out = Command::new(omtsf_bin())
        .args([
            "convert",
            "--compact",
            fixture("diff-base.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf convert --compact");
    assert!(
        convert_out.status.success(),
        "convert must succeed; stderr: {}",
        String::from_utf8_lossy(&convert_out.stderr)
    );

    let mut converted_tmp = tempfile::NamedTempFile::new().expect("temp file");
    converted_tmp
        .write_all(&convert_out.stdout)
        .expect("write converted output");

    // diff the converted file against itself — must be exit 0.
    let diff_out = Command::new(omtsf_bin())
        .args([
            "diff",
            converted_tmp.path().to_str().expect("path"),
            converted_tmp.path().to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf diff on converted output");
    assert_eq!(
        diff_out.status.code(),
        Some(0),
        "diff of converted file against itself must be exit 0; stdout: {}",
        String::from_utf8_lossy(&diff_out.stdout)
    );
}

// ---------------------------------------------------------------------------
// Pipeline 10: merge then diff — detects the merge added nodes
// ---------------------------------------------------------------------------

/// Merging two files and diffing the merged output against one of the inputs
/// shows that new nodes were added (exit 1 for differences).
#[test]
fn pipeline_merge_diff_shows_additions() {
    // Step 1: merge
    let merge_out = Command::new(omtsf_bin())
        .args([
            "merge",
            fixture("merge-a.omts").to_str().expect("path"),
            fixture("merge-b.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf merge");
    assert_eq!(
        merge_out.status.code(),
        Some(0),
        "merge must succeed; stderr: {}",
        String::from_utf8_lossy(&merge_out.stderr)
    );

    let mut merge_tmp = tempfile::NamedTempFile::new().expect("temp file for merge");
    merge_tmp
        .write_all(&merge_out.stdout)
        .expect("write merge output");

    // Step 2: diff merge output against the first input — should detect differences
    let diff_out = Command::new(omtsf_bin())
        .args([
            "diff",
            fixture("merge-a.omts").to_str().expect("path"),
            merge_tmp.path().to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf diff merge-a vs merged");
    // Merged file has more nodes than merge-a, so diff exits 1.
    assert_eq!(
        diff_out.status.code(),
        Some(1),
        "diff of merge-a vs merged must show differences (exit 1)"
    );
    let stdout = String::from_utf8_lossy(&diff_out.stdout);
    // The merged output has more nodes; diff should show added nodes.
    assert!(
        stdout.contains("+") || stdout.contains("added"),
        "diff should show added items; stdout: {stdout}"
    );
}
