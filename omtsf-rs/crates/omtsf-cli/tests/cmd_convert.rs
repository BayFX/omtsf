//! Integration tests for `omtsf convert`.
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

#[test]
fn convert_minimal_pretty_exit_0() {
    let out = Command::new(omtsf_bin())
        .args(["convert", fixture("minimal.omts").to_str().expect("path")])
        .output()
        .expect("run omtsf convert");
    assert!(out.status.success(), "exit code: {:?}", out.status.code());
}

#[test]
fn convert_minimal_pretty_is_valid_json() {
    let out = Command::new(omtsf_bin())
        .args(["convert", fixture("minimal.omts").to_str().expect("path")])
        .output()
        .expect("run omtsf convert");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: Result<serde_json::Value, _> = serde_json::from_str(stdout.trim());
    assert!(parsed.is_ok(), "output is not valid JSON: {stdout}");
}

#[test]
fn convert_minimal_pretty_preserves_fields() {
    let out = Command::new(omtsf_bin())
        .args(["convert", fixture("minimal.omts").to_str().expect("path")])
        .output()
        .expect("run omtsf convert");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("valid JSON from convert");
    assert_eq!(value["omtsf_version"], "0.1.0");
    assert_eq!(value["snapshot_date"], "2026-02-18");
    assert!(value["nodes"].is_array());
    assert!(value["edges"].is_array());
}

#[test]
fn convert_minimal_pretty_has_indentation() {
    let out = Command::new(omtsf_bin())
        .args(["convert", fixture("minimal.omts").to_str().expect("path")])
        .output()
        .expect("run omtsf convert");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains('\n'), "expected newlines in pretty output");
}

#[test]
fn convert_compact_exit_0() {
    let out = Command::new(omtsf_bin())
        .args([
            "convert",
            "--compact",
            fixture("minimal.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf convert --compact");
    assert!(out.status.success(), "exit code: {:?}", out.status.code());
}

#[test]
fn convert_compact_is_single_line() {
    let out = Command::new(omtsf_bin())
        .args([
            "convert",
            "--compact",
            fixture("minimal.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf convert --compact");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let trimmed = stdout.trim_end_matches('\n');
    assert!(
        !trimmed.contains('\n'),
        "compact output should be single line: {stdout}"
    );
}

#[test]
fn convert_compact_is_valid_json() {
    let out = Command::new(omtsf_bin())
        .args([
            "convert",
            "--compact",
            fixture("minimal.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf convert --compact");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: Result<serde_json::Value, _> = serde_json::from_str(stdout.trim());
    assert!(parsed.is_ok(), "compact output is not valid JSON: {stdout}");
}

#[test]
fn convert_compact_is_smaller_than_pretty() {
    let pretty_out = Command::new(omtsf_bin())
        .args([
            "convert",
            "--pretty",
            fixture("full-featured.omts").to_str().expect("path"),
        ])
        .output()
        .expect("pretty convert");
    let compact_out = Command::new(omtsf_bin())
        .args([
            "convert",
            "--compact",
            fixture("full-featured.omts").to_str().expect("path"),
        ])
        .output()
        .expect("compact convert");
    assert!(
        compact_out.stdout.len() < pretty_out.stdout.len(),
        "compact ({}) should be smaller than pretty ({})",
        compact_out.stdout.len(),
        pretty_out.stdout.len()
    );
}

#[test]
fn convert_round_trips_full_featured() {
    let out = Command::new(omtsf_bin())
        .args([
            "convert",
            fixture("full-featured.omts").to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf convert full-featured");
    assert!(out.status.success(), "exit code: {:?}", out.status.code());
    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("valid JSON after convert");

    assert_eq!(value["omtsf_version"], "0.1.0");
    assert_eq!(value["disclosure_scope"], "partner");
    let nodes = value["nodes"].as_array().expect("nodes array");
    assert_eq!(nodes.len(), 8, "8 nodes expected after round-trip");
    let edges = value["edges"].as_array().expect("edges array");
    assert_eq!(edges.len(), 8, "8 edges expected after round-trip");
}

#[test]
fn convert_preserves_unknown_fields() {
    use std::io::Write as _;
    let content = r#"{
        "omtsf_version": "1.0.0",
        "snapshot_date": "2026-02-19",
        "file_salt": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "nodes": [],
        "edges": [],
        "x_custom_extension": "preserved"
    }"#;
    let mut tmp = tempfile::NamedTempFile::new().expect("temp file");
    tmp.write_all(content.as_bytes()).expect("write");
    let out = Command::new(omtsf_bin())
        .args(["convert", tmp.path().to_str().expect("path")])
        .output()
        .expect("run omtsf convert with unknown fields");
    assert!(out.status.success(), "exit code: {:?}", out.status.code());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("x_custom_extension"),
        "unknown field should be preserved: {stdout}"
    );
    assert!(
        stdout.contains("preserved"),
        "unknown field value should be preserved: {stdout}"
    );
}

#[test]
fn convert_stdin_exit_0() {
    use std::io::Write as _;
    let content = std::fs::read(fixture("minimal.omts")).expect("read fixture");
    let mut child = Command::new(omtsf_bin())
        .args(["convert", "-"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("spawn omtsf convert -");
    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(&content)
        .expect("write stdin");
    let out = child.wait_with_output().expect("wait");
    assert!(out.status.success(), "exit code: {:?}", out.status.code());
    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("valid JSON from convert -");
    assert_eq!(value["omtsf_version"], "0.1.0");
}

#[test]
fn convert_nonexistent_file_exits_2() {
    let out = Command::new(omtsf_bin())
        .args(["convert", "/no/such/file/ever.omts"])
        .output()
        .expect("run omtsf convert nonexistent");
    assert_eq!(
        out.status.code(),
        Some(2),
        "expected exit 2 for nonexistent file"
    );
}

#[test]
fn convert_invalid_json_exits_2() {
    use std::io::Write as _;
    let mut tmp = tempfile::NamedTempFile::new().expect("temp file");
    tmp.write_all(b"this is not json").expect("write");
    let out = Command::new(omtsf_bin())
        .args(["convert", tmp.path().to_str().expect("path")])
        .output()
        .expect("run omtsf convert bad-json");
    assert_eq!(
        out.status.code(),
        Some(2),
        "expected exit 2 for invalid JSON"
    );
}
