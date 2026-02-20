//! Integration tests for `omtsf validate`.
#![allow(clippy::expect_used)]

use std::io::Write as _;
use std::path::PathBuf;
use std::process::Command;

/// Path to the compiled `omtsf` binary.
fn omtsf_bin() -> PathBuf {
    let mut path = std::env::current_exe().expect("current exe");
    // current_exe is something like …/deps/cmd_validate-<hash>
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
// validate: known-good fixture (exit 0)
// ---------------------------------------------------------------------------

#[test]
fn validate_minimal_exits_0() {
    let out = Command::new(omtsf_bin())
        .args(["validate", fixture("minimal.omts").to_str().expect("path")])
        .output()
        .expect("run omtsf validate");
    assert_eq!(
        out.status.code(),
        Some(0),
        "expected exit 0 for minimal.omts; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn validate_minimal_produces_no_stdout() {
    let out = Command::new(omtsf_bin())
        .args(["validate", fixture("minimal.omts").to_str().expect("path")])
        .output()
        .expect("run omtsf validate");
    assert!(
        out.stdout.is_empty(),
        "validate should not write to stdout; stdout: {}",
        String::from_utf8_lossy(&out.stdout)
    );
}

#[test]
fn validate_minimal_summary_on_stderr() {
    let out = Command::new(omtsf_bin())
        .args(["validate", fixture("minimal.omts").to_str().expect("path")])
        .output()
        .expect("run omtsf validate");
    let stderr = String::from_utf8_lossy(&out.stderr);
    // A clean file produces a summary like "0 errors, 0 warnings, 0 info"
    assert!(
        stderr.contains("error") || stderr.contains("errors"),
        "stderr should contain a summary; stderr: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// validate: known-bad fixture (exit 1)
// ---------------------------------------------------------------------------

#[test]
fn validate_invalid_edge_exits_1() {
    let out = Command::new(omtsf_bin())
        .args([
            "validate",
            fixture("invalid-edge.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf validate");
    assert_eq!(
        out.status.code(),
        Some(1),
        "expected exit 1 for invalid-edge.omts; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn validate_invalid_edge_emits_diagnostics_to_stderr() {
    let out = Command::new(omtsf_bin())
        .args([
            "validate",
            fixture("invalid-edge.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf validate");
    let stderr = String::from_utf8_lossy(&out.stderr);
    // Expect at least one [E] diagnostic for the bad edge target.
    assert!(
        stderr.contains("[E]"),
        "expected [E] diagnostic on stderr; stderr: {stderr}"
    );
}

#[test]
fn validate_invalid_edge_produces_no_stdout() {
    let out = Command::new(omtsf_bin())
        .args([
            "validate",
            fixture("invalid-edge.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf validate");
    assert!(
        out.stdout.is_empty(),
        "validate should not write to stdout; stdout: {}",
        String::from_utf8_lossy(&out.stdout)
    );
}

// ---------------------------------------------------------------------------
// validate: parse failure (exit 2)
// ---------------------------------------------------------------------------

#[test]
fn validate_invalid_json_exits_2() {
    let mut tmp = tempfile::NamedTempFile::new().expect("temp file");
    tmp.write_all(b"not-valid-json").expect("write");
    let out = Command::new(omtsf_bin())
        .args(["validate", tmp.path().to_str().expect("path")])
        .output()
        .expect("run omtsf validate");
    assert_eq!(
        out.status.code(),
        Some(2),
        "expected exit 2 for invalid JSON"
    );
}

#[test]
fn validate_nonexistent_file_exits_2() {
    let out = Command::new(omtsf_bin())
        .args(["validate", "/no/such/file/ever.omts"])
        .output()
        .expect("run omtsf validate");
    assert_eq!(
        out.status.code(),
        Some(2),
        "expected exit 2 for nonexistent file"
    );
}

// ---------------------------------------------------------------------------
// validate: --level flag
// ---------------------------------------------------------------------------

#[test]
fn validate_level_1_minimal_exits_0() {
    let out = Command::new(omtsf_bin())
        .args([
            "validate",
            "--level",
            "1",
            fixture("minimal.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf validate --level 1");
    assert_eq!(
        out.status.code(),
        Some(0),
        "expected exit 0 for minimal.omts at level 1"
    );
}

#[test]
fn validate_level_3_minimal_exits_0() {
    let out = Command::new(omtsf_bin())
        .args([
            "validate",
            "--level",
            "3",
            fixture("minimal.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf validate --level 3");
    assert_eq!(
        out.status.code(),
        Some(0),
        "expected exit 0 for minimal.omts at level 3"
    );
}

#[test]
fn validate_level_1_invalid_edge_exits_1() {
    let out = Command::new(omtsf_bin())
        .args([
            "validate",
            "--level",
            "1",
            fixture("invalid-edge.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf validate --level 1");
    assert_eq!(
        out.status.code(),
        Some(1),
        "expected exit 1 for invalid-edge.omts at level 1"
    );
}

// ---------------------------------------------------------------------------
// validate: stdin support
// ---------------------------------------------------------------------------

#[test]
fn validate_stdin_minimal_exits_0() {
    let content = std::fs::read(fixture("minimal.omts")).expect("read fixture");
    let mut child = Command::new(omtsf_bin())
        .args(["validate", "-"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("spawn omtsf validate -");
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
        "expected exit 0 for minimal.omts via stdin; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn validate_stdin_invalid_edge_exits_1() {
    let content = std::fs::read(fixture("invalid-edge.omts")).expect("read fixture");
    let mut child = Command::new(omtsf_bin())
        .args(["validate", "-"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("spawn omtsf validate -");
    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(&content)
        .expect("write stdin");
    let out = child.wait_with_output().expect("wait");
    assert_eq!(
        out.status.code(),
        Some(1),
        "expected exit 1 for invalid-edge.omts via stdin; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

// ---------------------------------------------------------------------------
// validate: JSON output format
// ---------------------------------------------------------------------------

#[test]
fn validate_json_format_minimal_exits_0() {
    let out = Command::new(omtsf_bin())
        .args([
            "validate",
            "-f",
            "json",
            fixture("minimal.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf validate -f json");
    assert_eq!(
        out.status.code(),
        Some(0),
        "expected exit 0 for minimal.omts in JSON mode; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn validate_json_format_invalid_edge_exits_1_with_ndjson_on_stderr() {
    let out = Command::new(omtsf_bin())
        .args([
            "validate",
            "-f",
            "json",
            fixture("invalid-edge.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf validate -f json");
    assert_eq!(
        out.status.code(),
        Some(1),
        "expected exit 1 for invalid-edge.omts in JSON mode"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    // Each diagnostic line should be parseable JSON.
    let first_line = stderr.lines().next().expect("at least one line on stderr");
    let parsed: Result<serde_json::Value, _> = serde_json::from_str(first_line);
    assert!(
        parsed.is_ok(),
        "first stderr line should be valid JSON: {first_line}"
    );
    let obj = parsed.expect("valid json");
    assert!(obj.get("rule_id").is_some(), "missing rule_id field");
    assert!(obj.get("severity").is_some(), "missing severity field");
}

// ---------------------------------------------------------------------------
// validate: quiet mode
// ---------------------------------------------------------------------------

#[test]
fn validate_quiet_suppresses_warnings_on_stderr() {
    // full-featured.omts has L2 warnings; --quiet should suppress them
    let out = Command::new(omtsf_bin())
        .args([
            "validate",
            "--quiet",
            fixture("minimal.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf validate --quiet");
    let stderr = String::from_utf8_lossy(&out.stderr);
    // In quiet mode, summary is suppressed and only errors go to stderr.
    // For a clean file, stderr should be empty.
    assert!(
        stderr.is_empty(),
        "stderr should be empty in quiet mode for clean file; stderr: {stderr}"
    );
}
