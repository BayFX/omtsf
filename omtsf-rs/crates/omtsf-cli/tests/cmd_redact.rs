//! Integration tests for `omtsf redact`.
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

#[test]
fn redact_to_public_exits_0() {
    let out = Command::new(omtsf_bin())
        .args([
            "redact",
            "--scope",
            "public",
            fixture("redact-internal.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf redact --scope public");
    assert_eq!(
        out.status.code(),
        Some(0),
        "expected exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn redact_to_public_writes_valid_json_to_stdout() {
    let out = Command::new(omtsf_bin())
        .args([
            "redact",
            "--scope",
            "public",
            fixture("redact-internal.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf redact --scope public");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: Result<serde_json::Value, _> = serde_json::from_str(stdout.trim());
    assert!(
        parsed.is_ok(),
        "stdout should be valid JSON; stdout: {stdout}"
    );
}

/// In public scope, person nodes must be omitted entirely.
#[test]
fn redact_to_public_removes_person_nodes() {
    let out = Command::new(omtsf_bin())
        .args([
            "redact",
            "--scope",
            "public",
            fixture("redact-internal.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf redact --scope public");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("valid JSON from redact");

    let nodes = value["nodes"].as_array().expect("nodes array");
    for node in nodes {
        let node_type = node["type"].as_str().unwrap_or("");
        assert_ne!(
            node_type, "person",
            "person nodes must be absent in public output"
        );
    }
}

/// In public scope, `beneficial_ownership` edges must be omitted.
#[test]
fn redact_to_public_removes_beneficial_ownership_edges() {
    let out = Command::new(omtsf_bin())
        .args([
            "redact",
            "--scope",
            "public",
            fixture("redact-internal.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf redact --scope public");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("valid JSON from redact");

    let edges = value["edges"].as_array().expect("edges array");
    for edge in edges {
        let edge_type = edge["type"].as_str().unwrap_or("");
        assert_ne!(
            edge_type, "beneficial_ownership",
            "beneficial_ownership edges must be absent in public output"
        );
    }
}

/// The output must declare `disclosure_scope` = "public".
#[test]
fn redact_to_public_sets_disclosure_scope() {
    let out = Command::new(omtsf_bin())
        .args([
            "redact",
            "--scope",
            "public",
            fixture("redact-internal.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf redact --scope public");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("valid JSON from redact");

    assert_eq!(
        value["disclosure_scope"].as_str(),
        Some("public"),
        "output must set disclosure_scope to public"
    );
}

/// Statistics go to stderr, not stdout.
#[test]
fn redact_to_public_emits_statistics_to_stderr() {
    let out = Command::new(omtsf_bin())
        .args([
            "redact",
            "--scope",
            "public",
            fixture("redact-internal.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf redact --scope public");

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("Public") || stderr.contains("public") || stderr.contains("scope"),
        "stderr should contain redaction statistics; stderr: {stderr}"
    );
}

#[test]
fn redact_to_partner_exits_0() {
    let out = Command::new(omtsf_bin())
        .args([
            "redact",
            "--scope",
            "partner",
            fixture("redact-internal.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf redact --scope partner");
    assert_eq!(
        out.status.code(),
        Some(0),
        "expected exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// In partner scope, person nodes are retained (identifiers filtered).
#[test]
fn redact_to_partner_does_not_remove_person_nodes() {
    let out = Command::new(omtsf_bin())
        .args([
            "redact",
            "--scope",
            "partner",
            fixture("redact-internal.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf redact --scope partner");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("valid JSON from redact partner");

    assert!(value["nodes"].is_array(), "nodes must be an array");
    assert!(value["edges"].is_array(), "edges must be an array");
}

/// Trying to redact a "public" file to "partner" scope (less restrictive) exits 1.
#[test]
fn redact_less_restrictive_scope_exits_1() {
    let content = r#"{
        "omtsf_version": "1.0.0",
        "snapshot_date": "2026-02-19",
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
        .expect("run omtsf redact less-restrictive scope");
    assert_eq!(
        out.status.code(),
        Some(1),
        "expected exit 1 for less-restrictive scope"
    );
}

#[test]
fn redact_invalid_json_exits_2() {
    let mut tmp = tempfile::NamedTempFile::new().expect("temp file");
    tmp.write_all(b"not-valid-json").expect("write");

    let out = Command::new(omtsf_bin())
        .args([
            "redact",
            "--scope",
            "public",
            tmp.path().to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf redact bad-json");
    assert_eq!(
        out.status.code(),
        Some(2),
        "expected exit 2 for invalid JSON"
    );
}

#[test]
fn redact_nonexistent_file_exits_2() {
    let out = Command::new(omtsf_bin())
        .args(["redact", "--scope", "public", "/no/such/file.omts"])
        .output()
        .expect("run omtsf redact nonexistent");
    assert_eq!(
        out.status.code(),
        Some(2),
        "expected exit 2 for nonexistent file"
    );
}

#[test]
fn redact_to_public_output_passes_validate() {
    let redact_out = Command::new(omtsf_bin())
        .args([
            "redact",
            "--scope",
            "public",
            fixture("redact-internal.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf redact");
    assert_eq!(
        redact_out.status.code(),
        Some(0),
        "redact must succeed first"
    );

    let mut tmp = tempfile::NamedTempFile::new().expect("temp file");
    tmp.write_all(&redact_out.stdout)
        .expect("write redacted output");

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
        "redacted output must pass L1 validation; stderr: {}",
        String::from_utf8_lossy(&validate_out.stderr)
    );
}

/// The redacted file preserves the original `file_salt` (per redaction.md §7.1).
#[test]
fn redact_to_public_preserves_file_salt() {
    let out = Command::new(omtsf_bin())
        .args([
            "redact",
            "--scope",
            "public",
            fixture("redact-internal.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf redact --scope public");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("valid JSON from redact");

    assert_eq!(
        value["file_salt"].as_str(),
        Some("deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef"),
        "file_salt must be preserved in redacted output"
    );
}

#[test]
fn redact_stdin_exits_0() {
    let content = std::fs::read(fixture("redact-internal.omts")).expect("read fixture");

    let mut child = Command::new(omtsf_bin())
        .args(["redact", "--scope", "public", "-"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("spawn omtsf redact -");

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
        "expected exit 0 for stdin redact; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// `--to cbor` outputs bytes starting with the CBOR self-describing tag 55799.
#[test]
fn redact_to_cbor_starts_with_cbor_tag() {
    let out = Command::new(omtsf_bin())
        .args([
            "redact",
            "--scope",
            "public",
            "--to",
            "cbor",
            fixture("redact-internal.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf redact --to cbor");
    assert_eq!(
        out.status.code(),
        Some(0),
        "expected exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        out.stdout.starts_with(&[0xD9, 0xD9, 0xF7]),
        "CBOR output must begin with self-describing tag 55799"
    );
}

/// `--to cbor` redacted output passes L1 validation and has no person nodes.
#[test]
fn redact_to_cbor_passes_validate_and_removes_person_nodes() {
    let redact_out = Command::new(omtsf_bin())
        .args([
            "redact",
            "--scope",
            "public",
            "--to",
            "cbor",
            fixture("redact-internal.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf redact --to cbor");
    assert_eq!(
        redact_out.status.code(),
        Some(0),
        "redact must succeed first"
    );

    let mut tmp = tempfile::NamedTempFile::new().expect("temp file");
    tmp.write_all(&redact_out.stdout)
        .expect("write redacted CBOR output");

    let validate_out = Command::new(omtsf_bin())
        .args([
            "validate",
            "--level",
            "1",
            tmp.path().to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf validate on redacted CBOR output");
    assert_eq!(
        validate_out.status.code(),
        Some(0),
        "redacted CBOR output must pass L1 validation; stderr: {}",
        String::from_utf8_lossy(&validate_out.stderr)
    );

    // Convert CBOR back to JSON and check no person nodes remain.
    let convert_out = Command::new(omtsf_bin())
        .args([
            "convert",
            "--to",
            "json",
            tmp.path().to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf convert cbor to json");
    let stdout = String::from_utf8_lossy(&convert_out.stdout);
    let value: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("valid JSON after CBOR round-trip");
    let nodes = value["nodes"].as_array().expect("nodes array");
    for node in nodes {
        let node_type = node["type"].as_str().unwrap_or("");
        assert_ne!(
            node_type, "person",
            "CBOR public-scope output must not contain person nodes; found: {node:?}"
        );
    }
}

/// `--compress` produces output starting with the zstd magic bytes.
#[test]
fn redact_compress_starts_with_zstd_magic() {
    let out = Command::new(omtsf_bin())
        .args([
            "redact",
            "--scope",
            "public",
            "--compress",
            fixture("redact-internal.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf redact --compress");
    assert_eq!(
        out.status.code(),
        Some(0),
        "expected exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        out.stdout.starts_with(&[0x28, 0xB5, 0x2F, 0xFD]),
        "compressed output must begin with zstd magic bytes"
    );
}

/// `--to cbor --compress` produces zstd-compressed CBOR that passes validate.
#[test]
fn redact_cbor_compress_passes_validate() {
    let redact_out = Command::new(omtsf_bin())
        .args([
            "redact",
            "--scope",
            "public",
            "--to",
            "cbor",
            "--compress",
            fixture("redact-internal.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf redact --to cbor --compress");
    assert_eq!(
        redact_out.status.code(),
        Some(0),
        "expected exit 0; stderr: {}",
        String::from_utf8_lossy(&redact_out.stderr)
    );
    assert!(
        redact_out.stdout.starts_with(&[0x28, 0xB5, 0x2F, 0xFD]),
        "compressed CBOR must start with zstd magic"
    );

    let mut tmp = tempfile::NamedTempFile::new().expect("temp file");
    tmp.write_all(&redact_out.stdout)
        .expect("write redacted output");

    let validate_out = Command::new(omtsf_bin())
        .args([
            "validate",
            "--level",
            "1",
            tmp.path().to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf validate on redacted cbor+zstd output");
    assert_eq!(
        validate_out.status.code(),
        Some(0),
        "redacted cbor+zstd output must pass L1 validation; stderr: {}",
        String::from_utf8_lossy(&validate_out.stderr)
    );
}
