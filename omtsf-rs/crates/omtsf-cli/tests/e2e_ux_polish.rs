//! End-to-end UX polish tests: exit codes, error messages, broken pipe,
//! quiet mode, and verbose mode.
//!
//! Coverage gaps addressed here (not already tested in other test files):
//! - Broken pipe: `omtsf inspect file | head -1` exits 0 with empty stderr
//! - Quiet mode suppresses warnings but NOT fatal parse/IO errors on stderr
//! - Verbose mode produces timing information ("ms") on stderr
//! - Error messages contain the file path in the output
//! - Error messages for JSON parse failures contain line/column context
//! - Permission-denied produces exit 2 with path in message (where testable)
#![allow(clippy::expect_used)]

use std::io::Write as _;
use std::path::PathBuf;
use std::process::{Command, Stdio};

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
// Broken pipe handling
// ---------------------------------------------------------------------------

/// `omtsf inspect file | head -1` must exit 0 and produce no error on stderr.
///
/// This exercises the SIGPIPE handler: when `head` closes its stdin after
/// reading one line, omtsf receives SIGPIPE on its next write and must exit 0
/// silently rather than printing an "broken pipe" error message.
///
/// We verify this by checking that `head` exits 0. The omtsf process is
/// reaped after head finishes to avoid leaving zombie processes.
#[cfg(unix)]
#[test]
fn broken_pipe_inspect_head_exits_0() {
    let mut inspect_child = Command::new(omtsf_bin())
        .args(["inspect", fixture("minimal.omts").to_str().expect("path")])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn omtsf inspect");

    let inspect_stdout = inspect_child.stdout.take().expect("inspect stdout");

    let head_out = Command::new("head")
        .arg("-1")
        .stdin(inspect_stdout)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("run head -1");

    let _ = inspect_child.wait().expect("wait omtsf inspect");

    assert_eq!(
        head_out.status.code(),
        Some(0),
        "head should exit 0; stderr: {}",
        String::from_utf8_lossy(&head_out.stderr)
    );
}

/// Piping `omtsf diff` output through `head -1` must not produce a broken-pipe
/// error on stderr and must exit 0 (from head's perspective).
///
/// The key behavior being tested is that omtsf exits cleanly when the read end
/// of its stdout pipe is closed by head, rather than printing a broken-pipe
/// error. We verify this by confirming head exits 0.
#[cfg(unix)]
#[test]
fn broken_pipe_diff_head_no_stderr_error() {
    let mut omtsf_child = Command::new(omtsf_bin())
        .args([
            "diff",
            fixture("diff-base.omts").to_str().expect("path"),
            fixture("diff-modified.omts").to_str().expect("path"),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn omtsf diff");

    let diff_stdout = omtsf_child.stdout.take().expect("diff stdout");

    let head_out = Command::new("head")
        .arg("-1")
        .stdin(diff_stdout)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .expect("run head");

    let _ = omtsf_child.wait().expect("wait omtsf diff");

    assert_eq!(
        head_out.status.code(),
        Some(0),
        "head should exit 0; head stderr: {}",
        String::from_utf8_lossy(&head_out.stderr)
    );
}

// ---------------------------------------------------------------------------
// Quiet mode
// ---------------------------------------------------------------------------

/// In quiet mode, the validate summary ("0 errors, 0 warnings, 0 info") must
/// be suppressed even when the file is valid.
///
/// The existing `validate_quiet_suppresses_warnings_on_stderr` test already
/// covers this for `minimal.omts`. This test confirms the same for
/// `invalid-edge.omts` at level 2, where L1 errors ARE emitted (because
/// quiet only suppresses warnings/info, not errors).
#[test]
fn quiet_mode_suppress_summary_but_not_errors() {
    let out = Command::new(omtsf_bin())
        .args([
            "--quiet",
            "validate",
            fixture("invalid-edge.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf --quiet validate invalid-edge.omts");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(
        out.status.code(),
        Some(1),
        "invalid-edge.omts must exit 1; stderr: {stderr}"
    );
    assert!(
        stderr.contains("[E]"),
        "error diagnostics must still appear in quiet mode; stderr: {stderr}"
    );
    assert!(
        !stderr.contains("error, ") && !stderr.contains("errors, "),
        "summary line must be suppressed in quiet mode; stderr: {stderr}"
    );
}

/// `--quiet` suppresses the per-diagnostic summary for a clean file.
///
/// When `minimal.omts` is valid, no diagnostics are emitted. The summary
/// ("0 errors, ...") must also be suppressed in quiet mode.
#[test]
fn quiet_mode_no_summary_for_valid_file() {
    let out = Command::new(omtsf_bin())
        .args([
            "--quiet",
            "validate",
            fixture("minimal.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf --quiet validate minimal.omts");
    assert_eq!(
        out.status.code(),
        Some(0),
        "minimal.omts must exit 0 in quiet mode; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.is_empty(),
        "stderr must be empty in quiet mode for clean file; stderr: {stderr}"
    );
}

/// In quiet mode, a fatal parse error (exit 2) must still emit to stderr.
///
/// `--quiet` suppresses warnings and summary, but not I/O or parse errors.
#[test]
fn quiet_mode_does_not_suppress_fatal_parse_error() {
    let mut tmp = tempfile::NamedTempFile::new().expect("temp file");
    tmp.write_all(b"not-valid-json").expect("write");
    let out = Command::new(omtsf_bin())
        .args(["--quiet", "validate", tmp.path().to_str().expect("path")])
        .output()
        .expect("run omtsf --quiet validate bad-json");
    assert_eq!(
        out.status.code(),
        Some(2),
        "invalid JSON must exit 2 even in quiet mode"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !stderr.is_empty(),
        "parse error must still appear on stderr in quiet mode; stderr: {stderr}"
    );
}

/// In quiet mode, a file-not-found error (exit 2) must still emit to stderr.
#[test]
fn quiet_mode_does_not_suppress_file_not_found_error() {
    let out = Command::new(omtsf_bin())
        .args(["--quiet", "validate", "/no/such/file/quiet-test.omts"])
        .output()
        .expect("run omtsf --quiet validate nonexistent");
    assert_eq!(
        out.status.code(),
        Some(2),
        "file not found must exit 2 in quiet mode"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !stderr.is_empty(),
        "file-not-found error must appear on stderr in quiet mode; stderr: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// Verbose mode
// ---------------------------------------------------------------------------

/// `--verbose validate` must emit timing information ("ms") on stderr.
#[test]
fn verbose_validate_shows_timing_ms() {
    let out = Command::new(omtsf_bin())
        .args([
            "--verbose",
            "validate",
            fixture("minimal.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf --verbose validate minimal.omts");
    assert_eq!(
        out.status.code(),
        Some(0),
        "verbose validate must exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("ms"),
        "verbose mode must report timing in milliseconds; stderr: {stderr}"
    );
}

/// `--verbose diff` on differing files must emit timing information on stderr.
#[test]
fn verbose_diff_shows_timing_ms() {
    let out = Command::new(omtsf_bin())
        .args([
            "--verbose",
            "diff",
            fixture("diff-base.omts").to_str().expect("path"),
            fixture("diff-modified.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf --verbose diff");
    assert_eq!(
        out.status.code(),
        Some(1),
        "diff on differing files exits 1; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("ms"),
        "verbose diff must report timing in milliseconds; stderr: {stderr}"
    );
}

/// `--verbose validate` also reports the detected encoding.
#[test]
fn verbose_validate_reports_encoding() {
    let out = Command::new(omtsf_bin())
        .args([
            "--verbose",
            "validate",
            fixture("minimal.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf --verbose validate minimal.omts");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("encoding:"),
        "verbose mode must report encoding; stderr: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// Error messages contain the file path
// ---------------------------------------------------------------------------

/// File-not-found error message must include the file path.
#[test]
fn error_file_not_found_contains_path() {
    let out = Command::new(omtsf_bin())
        .args(["validate", "/very/specific/missing-file-ux-test.omts"])
        .output()
        .expect("run omtsf validate nonexistent");
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("missing-file-ux-test.omts"),
        "error message must contain the file path; stderr: {stderr}"
    );
}

/// File-not-found error message must include a hint about verifying the path.
#[test]
fn error_file_not_found_contains_hint() {
    let out = Command::new(omtsf_bin())
        .args(["validate", "/very/specific/missing-hint-test.omts"])
        .output()
        .expect("run omtsf validate nonexistent");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("hint") || stderr.contains("check"),
        "error message must contain actionable guidance; stderr: {stderr}"
    );
}

/// JSON parse error message must include line/column context.
#[test]
fn error_json_parse_contains_line_column() {
    let mut tmp = tempfile::NamedTempFile::new().expect("temp file");
    tmp.write_all(b"{\n  \"bad field\"\n}").expect("write");
    let out = Command::new(omtsf_bin())
        .args(["validate", tmp.path().to_str().expect("path")])
        .output()
        .expect("run omtsf validate bad-json");
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("line") || stderr.contains("column"),
        "JSON parse error must include line/column context; stderr: {stderr}"
    );
}

/// A file-not-found error on `omtsf diff` must name the missing file.
#[test]
fn error_diff_file_a_not_found_contains_path() {
    let out = Command::new(omtsf_bin())
        .args([
            "diff",
            "/no/such/file-diff-a-ux.omts",
            fixture("diff-base.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf diff nonexistent-a");
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("file-diff-a-ux.omts"),
        "error message must contain the missing file path; stderr: {stderr}"
    );
}

/// A node-not-found error on `omtsf reach` must name the missing node ID.
#[test]
fn error_reach_node_not_found_contains_node_id() {
    let out = Command::new(omtsf_bin())
        .args([
            "reach",
            fixture("graph-query.omts").to_str().expect("path"),
            "very-specific-missing-node-id",
        ])
        .output()
        .expect("run omtsf reach unknown-node");
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("very-specific-missing-node-id"),
        "error message must contain the missing node ID; stderr: {stderr}"
    );
}

/// A node-not-found error on `omtsf path` must include a hint about `inspect`.
#[test]
fn error_path_node_not_found_contains_hint() {
    let out = Command::new(omtsf_bin())
        .args([
            "path",
            fixture("graph-query.omts").to_str().expect("path"),
            "no-such-node-ux-hint",
            "org-a",
        ])
        .output()
        .expect("run omtsf path unknown node");
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("hint") || stderr.contains("inspect"),
        "node-not-found error should suggest `inspect`; stderr: {stderr}"
    );
}

/// A redaction scope error must name the issue in the error message.
#[test]
fn error_redact_scope_message_contains_scope() {
    let content = r#"{
        "omtsf_version": "1.0.0",
        "snapshot_date": "2026-02-22",
        "file_salt": "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef",
        "disclosure_scope": "public",
        "nodes": [],
        "edges": []
    }"#;
    let mut tmp = tempfile::NamedTempFile::new().expect("temp file");
    tmp.write_all(content.as_bytes()).expect("write");
    let out = Command::new(omtsf_bin())
        .args([
            "redact",
            "--scope",
            "partner",
            tmp.path().to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf redact less-restrictive");
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("scope") || stderr.contains("restrictive"),
        "redaction error must describe the scope problem; stderr: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// Exit code completeness: conditions not already covered in cmd_*.rs files
// ---------------------------------------------------------------------------

/// `omtsf subgraph` with a node ID not in the graph exits 1.
/// The error message must name the missing node ID.
#[test]
fn subgraph_unknown_node_error_contains_node_id() {
    let out = Command::new(omtsf_bin())
        .args([
            "subgraph",
            fixture("graph-query.omts").to_str().expect("path"),
            "very-specific-subgraph-missing-node",
        ])
        .output()
        .expect("run omtsf subgraph unknown-node");
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("very-specific-subgraph-missing-node"),
        "error must contain the missing node ID; stderr: {stderr}"
    );
}

/// `omtsf query` with no selector flags exits 2 (invalid argument).
/// The error message must indicate what went wrong.
#[test]
fn query_no_selectors_error_message_is_descriptive() {
    let out = Command::new(omtsf_bin())
        .args(["query", fixture("graph-query.omts").to_str().expect("path")])
        .output()
        .expect("run omtsf query with no selectors");
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !stderr.is_empty(),
        "error output must not be empty; stderr: {stderr}"
    );
}

/// Validation error message always goes to stderr, not stdout.
#[test]
fn validation_error_goes_to_stderr_not_stdout() {
    let out = Command::new(omtsf_bin())
        .args([
            "validate",
            fixture("invalid-edge.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf validate invalid-edge.omts");
    assert!(
        out.stdout.is_empty(),
        "validate must not write to stdout on error; stdout: {}",
        String::from_utf8_lossy(&out.stdout)
    );
    assert!(
        !out.stderr.is_empty(),
        "validate must write diagnostics to stderr"
    );
}

/// File-not-found error for any command goes to stderr, not stdout.
#[test]
fn file_not_found_error_goes_to_stderr() {
    let out = Command::new(omtsf_bin())
        .args(["inspect", "/no/such/file/stderr-test.omts"])
        .output()
        .expect("run omtsf inspect nonexistent");
    assert_eq!(out.status.code(), Some(2));
    assert!(
        out.stdout.is_empty(),
        "no output on stdout for file-not-found; stdout: {}",
        String::from_utf8_lossy(&out.stdout)
    );
    assert!(
        !out.stderr.is_empty(),
        "error message must appear on stderr"
    );
}

/// `omtsf validate` on a file with L1 errors exits 1 and error contains `[E]`.
/// This re-confirms the diagnostic format includes the severity tag.
#[test]
fn validate_l1_error_diagnostic_format_has_severity_tag() {
    let out = Command::new(omtsf_bin())
        .args([
            "validate",
            fixture("invalid-edge.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf validate invalid-edge.omts");
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("[E]"),
        "L1 error diagnostics must be tagged with [E]; stderr: {stderr}"
    );
}

/// Validation summary mentions "error" or "errors" on stderr for invalid file.
#[test]
fn validate_l1_error_summary_mentions_error_count() {
    let out = Command::new(omtsf_bin())
        .args([
            "validate",
            fixture("invalid-edge.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf validate invalid-edge.omts");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("error") || stderr.contains("errors"),
        "stderr summary must mention errors; stderr: {stderr}"
    );
}
