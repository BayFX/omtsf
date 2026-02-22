//! Integration tests for `omtsf import`.
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

/// Path to an Excel fixture file relative to the repo root.
fn excel_fixture(name: &str) -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("../../../tests/fixtures/excel");
    path.push(name);
    path
}

#[test]
fn import_example_xlsx_exits_0() {
    let out = Command::new(omtsf_bin())
        .args([
            "import",
            excel_fixture("omts-import-example.xlsx")
                .to_str()
                .expect("path"),
        ])
        .output()
        .expect("run omtsf import");
    assert_eq!(
        out.status.code(),
        Some(0),
        "expected exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn import_example_xlsx_stdout_is_valid_json() {
    let out = Command::new(omtsf_bin())
        .args([
            "import",
            excel_fixture("omts-import-example.xlsx")
                .to_str()
                .expect("path"),
        ])
        .output()
        .expect("run omtsf import");
    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8(out.stdout).expect("UTF-8 stdout");
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("output must be valid JSON");
    assert!(
        parsed.is_object(),
        "expected a JSON object at top level, got: {parsed}"
    );
}

#[test]
fn import_example_xlsx_has_omtsf_version_field() {
    let out = Command::new(omtsf_bin())
        .args([
            "import",
            excel_fixture("omts-import-example.xlsx")
                .to_str()
                .expect("path"),
        ])
        .output()
        .expect("run omtsf import");
    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8(out.stdout).expect("UTF-8 stdout");
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert!(
        parsed.get("omtsf_version").is_some(),
        "output must contain 'omtsf_version' field; output: {parsed}"
    );
}

#[test]
fn import_example_xlsx_has_nodes_array() {
    let out = Command::new(omtsf_bin())
        .args([
            "import",
            excel_fixture("omts-import-example.xlsx")
                .to_str()
                .expect("path"),
        ])
        .output()
        .expect("run omtsf import");
    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8(out.stdout).expect("UTF-8 stdout");
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    let nodes = parsed.get("nodes").expect("must have 'nodes' field");
    assert!(nodes.is_array(), "'nodes' must be an array");
    assert!(
        !nodes.as_array().expect("array").is_empty(),
        "example workbook must import at least one node"
    );
}

#[test]
fn import_nonexistent_file_exits_2() {
    let out = Command::new(omtsf_bin())
        .args(["import", "/tmp/does-not-exist-xyz.xlsx"])
        .output()
        .expect("run omtsf import (missing file)");
    assert_eq!(
        out.status.code(),
        Some(2),
        "expected exit 2 for missing file; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn import_nonexistent_file_prints_error_on_stderr() {
    let out = Command::new(omtsf_bin())
        .args(["import", "/tmp/does-not-exist-xyz.xlsx"])
        .output()
        .expect("run omtsf import (missing file)");
    assert!(!out.stderr.is_empty(), "expected error message on stderr");
}

#[test]
fn import_output_flag_writes_file() {
    let tmp = tempfile::NamedTempFile::new().expect("temp file");
    let out_path = tmp.path().to_path_buf();

    let out = Command::new(omtsf_bin())
        .args([
            "import",
            excel_fixture("omts-import-example.xlsx")
                .to_str()
                .expect("path"),
            "-o",
            out_path.to_str().expect("output path"),
        ])
        .output()
        .expect("run omtsf import -o");
    assert_eq!(
        out.status.code(),
        Some(0),
        "expected exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let content = std::fs::read_to_string(&out_path).expect("read output file");
    let parsed: serde_json::Value = serde_json::from_str(&content).expect("valid JSON in file");
    assert!(
        parsed.get("omtsf_version").is_some(),
        "written file must contain 'omtsf_version'"
    );
}

#[test]
fn import_output_flag_stdout_is_empty() {
    let tmp = tempfile::NamedTempFile::new().expect("temp file");
    let out_path = tmp.path().to_path_buf();

    let out = Command::new(omtsf_bin())
        .args([
            "import",
            excel_fixture("omts-import-example.xlsx")
                .to_str()
                .expect("path"),
            "-o",
            out_path.to_str().expect("output path"),
        ])
        .output()
        .expect("run omtsf import -o");
    assert_eq!(out.status.code(), Some(0));
    assert!(
        out.stdout.is_empty(),
        "stdout must be empty when -o is used; stdout: {}",
        String::from_utf8_lossy(&out.stdout)
    );
}

#[test]
fn import_default_format_is_excel() {
    // Omitting --format should default to excel and succeed.
    let out = Command::new(omtsf_bin())
        .args([
            "import",
            excel_fixture("omts-import-example.xlsx")
                .to_str()
                .expect("path"),
        ])
        .output()
        .expect("run omtsf import (no --format)");
    assert_eq!(
        out.status.code(),
        Some(0),
        "default --format excel should succeed; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn import_explicit_format_excel_exits_0() {
    let out = Command::new(omtsf_bin())
        .args([
            "import",
            "--input-format",
            "excel",
            excel_fixture("omts-import-example.xlsx")
                .to_str()
                .expect("path"),
        ])
        .output()
        .expect("run omtsf import --format excel");
    assert_eq!(
        out.status.code(),
        Some(0),
        "expected exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn import_unknown_format_exits_2() {
    let out = Command::new(omtsf_bin())
        .args([
            "import",
            "--input-format",
            "csv",
            excel_fixture("omts-import-example.xlsx")
                .to_str()
                .expect("path"),
        ])
        .output()
        .expect("run omtsf import --input-format csv");
    assert_eq!(
        out.status.code(),
        Some(2),
        "unknown --input-format should produce exit 2; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn import_pipe_to_validate_exits_0() {
    // Run import, write output to a tempfile, then validate that file.
    let tmp = tempfile::NamedTempFile::new().expect("temp file");
    let tmp_path = tmp.path().to_path_buf();

    let import_out = Command::new(omtsf_bin())
        .args([
            "import",
            excel_fixture("omts-import-example.xlsx")
                .to_str()
                .expect("path"),
            "-o",
            tmp_path.to_str().expect("output path"),
        ])
        .output()
        .expect("run omtsf import -o");
    assert_eq!(
        import_out.status.code(),
        Some(0),
        "import must succeed; stderr: {}",
        String::from_utf8_lossy(&import_out.stderr)
    );

    let validate_out = Command::new(omtsf_bin())
        .args(["validate", tmp_path.to_str().expect("path")])
        .output()
        .expect("run omtsf validate");
    assert_eq!(
        validate_out.status.code(),
        Some(0),
        "validate must exit 0 on imported file; stderr: {}",
        String::from_utf8_lossy(&validate_out.stderr)
    );
}

#[test]
fn import_example_xlsx_node_and_edge_counts() {
    // The omts-import-example.xlsx workbook has exactly 5 nodes and 5 edges.
    // This test pins those counts to catch regressions in sheet parsing.
    let out = Command::new(omtsf_bin())
        .args([
            "import",
            excel_fixture("omts-import-example.xlsx")
                .to_str()
                .expect("path"),
        ])
        .output()
        .expect("run omtsf import");
    assert_eq!(out.status.code(), Some(0));

    let stdout = String::from_utf8(out.stdout).expect("UTF-8 stdout");
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");

    let nodes = parsed["nodes"].as_array().expect("nodes array");
    let edges = parsed["edges"].as_array().expect("edges array");

    assert_eq!(
        nodes.len(),
        5,
        "expected 5 nodes from example workbook, got {}",
        nodes.len()
    );
    assert_eq!(
        edges.len(),
        5,
        "expected 5 edges from example workbook, got {}",
        edges.len()
    );
}
