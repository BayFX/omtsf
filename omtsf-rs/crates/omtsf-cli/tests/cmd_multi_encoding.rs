//! Integration tests for multi-encoding CLI input (CBOR and zstd).
//!
//! Verifies that `validate`, `inspect`, and `diff` transparently accept JSON,
//! CBOR, and zstd-compressed inputs per the T-054 acceptance criteria.
#![allow(clippy::expect_used)]

use std::io::Write as _;
use std::path::PathBuf;
use std::process::Command;

use omtsf_core::{OmtsFile, cbor::encode_cbor, compression::compress_zstd};

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

/// Minimal valid OMTS JSON bytes.
fn minimal_json_bytes() -> Vec<u8> {
    let json = r#"{
        "omtsf_version": "1.0.0",
        "snapshot_date": "2026-02-19",
        "file_salt": "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef",
        "nodes": [],
        "edges": []
    }"#;
    json.as_bytes().to_vec()
}

/// Parse the minimal JSON into an `OmtsFile`.
fn minimal_omts_file() -> OmtsFile {
    serde_json::from_slice(&minimal_json_bytes()).expect("parse minimal JSON")
}

/// Write bytes to a named temporary file and return it.
fn temp_file_with(contents: &[u8], suffix: &str) -> tempfile::NamedTempFile {
    let file = tempfile::Builder::new()
        .suffix(suffix)
        .tempfile()
        .expect("create temp file");
    let mut f = file.reopen().expect("reopen");
    f.write_all(contents).expect("write");
    drop(f);
    file
}

/// Validates that `omtsf validate` accepts a JSON file (baseline).
#[test]
fn validate_accepts_json() {
    let tmp = temp_file_with(&minimal_json_bytes(), ".omts");
    let out = Command::new(omtsf_bin())
        .args(["validate", tmp.path().to_str().expect("path")])
        .output()
        .expect("run omtsf validate");
    assert_eq!(
        out.status.code(),
        Some(0),
        "JSON validate should exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Validates that `omtsf validate` accepts a CBOR-encoded file.
#[test]
fn validate_accepts_cbor() {
    let file = minimal_omts_file();
    let cbor_bytes = encode_cbor(&file).expect("encode CBOR");
    let tmp = temp_file_with(&cbor_bytes, ".omts");
    let out = Command::new(omtsf_bin())
        .args(["validate", tmp.path().to_str().expect("path")])
        .output()
        .expect("run omtsf validate");
    assert_eq!(
        out.status.code(),
        Some(0),
        "CBOR validate should exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Validates that `omtsf validate` accepts a zstd-compressed JSON file.
#[test]
fn validate_accepts_zstd_json() {
    let compressed = compress_zstd(&minimal_json_bytes()).expect("compress JSON");
    let tmp = temp_file_with(&compressed, ".omts.zst");
    let out = Command::new(omtsf_bin())
        .args(["validate", tmp.path().to_str().expect("path")])
        .output()
        .expect("run omtsf validate");
    assert_eq!(
        out.status.code(),
        Some(0),
        "zstd+JSON validate should exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Validates that `omtsf validate` accepts a zstd-compressed CBOR file.
#[test]
fn validate_accepts_zstd_cbor() {
    let file = minimal_omts_file();
    let cbor_bytes = encode_cbor(&file).expect("encode CBOR");
    let compressed = compress_zstd(&cbor_bytes).expect("compress CBOR");
    let tmp = temp_file_with(&compressed, ".omts.zst");
    let out = Command::new(omtsf_bin())
        .args(["validate", tmp.path().to_str().expect("path")])
        .output()
        .expect("run omtsf validate");
    assert_eq!(
        out.status.code(),
        Some(0),
        "zstd+CBOR validate should exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Validates that `omtsf inspect` accepts a CBOR-encoded file.
#[test]
fn inspect_accepts_cbor() {
    let file = minimal_omts_file();
    let cbor_bytes = encode_cbor(&file).expect("encode CBOR");
    let tmp = temp_file_with(&cbor_bytes, ".omts");
    let out = Command::new(omtsf_bin())
        .args(["inspect", tmp.path().to_str().expect("path")])
        .output()
        .expect("run omtsf inspect");
    assert_eq!(
        out.status.code(),
        Some(0),
        "CBOR inspect should exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("nodes"),
        "inspect output should mention nodes; stdout: {stdout}"
    );
}

/// Validates that `omtsf inspect` accepts a zstd-compressed file.
#[test]
fn inspect_accepts_zstd() {
    let compressed = compress_zstd(&minimal_json_bytes()).expect("compress JSON");
    let tmp = temp_file_with(&compressed, ".omts.zst");
    let out = Command::new(omtsf_bin())
        .args(["inspect", tmp.path().to_str().expect("path")])
        .output()
        .expect("run omtsf inspect");
    assert_eq!(
        out.status.code(),
        Some(0),
        "zstd inspect should exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Validates that `omtsf diff` accepts files with different encodings.
#[test]
fn diff_accepts_mixed_encodings() {
    let json_tmp = temp_file_with(&minimal_json_bytes(), ".omts");
    let file = minimal_omts_file();
    let cbor_bytes = encode_cbor(&file).expect("encode CBOR");
    let cbor_tmp = temp_file_with(&cbor_bytes, ".omts");

    let out = Command::new(omtsf_bin())
        .args([
            "diff",
            json_tmp.path().to_str().expect("path a"),
            cbor_tmp.path().to_str().expect("path b"),
        ])
        .output()
        .expect("run omtsf diff");
    assert_eq!(
        out.status.code(),
        Some(0),
        "diff of identical files in different encodings should exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Validates that `--verbose` reports the detected encoding on stderr.
#[test]
fn verbose_reports_encoding_json() {
    let tmp = temp_file_with(&minimal_json_bytes(), ".omts");
    let out = Command::new(omtsf_bin())
        .args(["--verbose", "inspect", tmp.path().to_str().expect("path")])
        .output()
        .expect("run omtsf inspect --verbose");
    assert_eq!(
        out.status.code(),
        Some(0),
        "verbose inspect should exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("encoding:"),
        "verbose mode should report encoding; stderr: {stderr}"
    );
    assert!(
        stderr.contains("json"),
        "verbose mode should report 'json'; stderr: {stderr}"
    );
}

/// Validates that `--verbose` reports the CBOR encoding on stderr.
#[test]
fn verbose_reports_encoding_cbor() {
    let file = minimal_omts_file();
    let cbor_bytes = encode_cbor(&file).expect("encode CBOR");
    let tmp = temp_file_with(&cbor_bytes, ".omts");
    let out = Command::new(omtsf_bin())
        .args(["--verbose", "inspect", tmp.path().to_str().expect("path")])
        .output()
        .expect("run omtsf inspect --verbose cbor");
    assert_eq!(
        out.status.code(),
        Some(0),
        "verbose CBOR inspect should exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("encoding:"),
        "verbose mode should report encoding; stderr: {stderr}"
    );
    assert!(
        stderr.contains("cbor"),
        "verbose mode should report 'cbor'; stderr: {stderr}"
    );
}

/// Validates that unrecognized bytes produce an encoding detection error.
#[test]
fn validate_rejects_unrecognized_encoding() {
    let garbage = vec![0xFFu8, 0x00, 0xAB, 0xCD, 0xEF];
    let tmp = temp_file_with(&garbage, ".omts");
    let out = Command::new(omtsf_bin())
        .args(["validate", tmp.path().to_str().expect("path")])
        .output()
        .expect("run omtsf validate");
    assert_eq!(
        out.status.code(),
        Some(2),
        "unrecognized encoding should exit 2; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("unrecognized") || stderr.contains("encoding"),
        "error message should mention encoding; stderr: {stderr}"
    );
}
