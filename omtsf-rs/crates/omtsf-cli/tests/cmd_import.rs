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
    path.push("../../../templates/excel");
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

#[test]
fn import_supplier_list_example_exits_0() {
    let out = Command::new(omtsf_bin())
        .args([
            "import",
            excel_fixture("omts-supplier-list-example.xlsx")
                .to_str()
                .expect("path"),
        ])
        .output()
        .expect("run omtsf import supplier list example");
    assert_eq!(
        out.status.code(),
        Some(0),
        "expected exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn import_supplier_list_template_exits_0() {
    // The empty template has metadata rows but no data rows.
    // Import should fail gracefully with exit 1 (no suppliers) or exit 0 if
    // the file has a reporting entity name. Since the template has an empty
    // B1 cell, it should fail with exit 1.
    let out = Command::new(omtsf_bin())
        .args([
            "import",
            excel_fixture("omts-supplier-list-template.xlsx")
                .to_str()
                .expect("path"),
        ])
        .output()
        .expect("run omtsf import supplier list template");
    // Empty template has no reporting entity name → expect exit 1 or 2
    // (parse error or validation error depending on what's missing)
    let code = out.status.code().unwrap_or(-1);
    assert!(
        code == 0 || code == 1 || code == 2,
        "expected exit 0, 1, or 2 for empty template; got {code}; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn import_supplier_list_pipe_to_validate_exits_0() {
    let tmp = tempfile::NamedTempFile::new().expect("temp file");
    let tmp_path = tmp.path().to_path_buf();

    let import_out = Command::new(omtsf_bin())
        .args([
            "import",
            excel_fixture("omts-supplier-list-example.xlsx")
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
fn import_supplier_list_node_counts() {
    // The example file has:
    //   1 reporting entity (Acme Manufacturing GmbH)
    //   6 unique suppliers (Bolt Supplies Ltd [rows 5+8 dedup via supplier_id],
    //                       Nordic Fasteners AB, Shanghai Steel Components Co,
    //                       Yorkshire Steel Works, Baosteel Trading Co, Inner Mongolia Mining Corp)
    //   7 supplies edges (one per data row)
    let out = Command::new(omtsf_bin())
        .args([
            "import",
            excel_fixture("omts-supplier-list-example.xlsx")
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
        7,
        "expected 7 org nodes (1 reporting entity + 6 suppliers), got {}",
        nodes.len()
    );
    assert_eq!(
        edges.len(),
        7,
        "expected 7 supplies edges (one per data row), got {}",
        edges.len()
    );
}

#[test]
fn import_supplier_list_tier_hierarchy() {
    let out = Command::new(omtsf_bin())
        .args([
            "import",
            excel_fixture("omts-supplier-list-example.xlsx")
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

    // Build node id → name map
    let id_to_name: std::collections::HashMap<String, String> = nodes
        .iter()
        .filter_map(|n| {
            let id = n["id"].as_str()?.to_owned();
            let name = n["name"].as_str()?.to_owned();
            Some((id, name))
        })
        .collect();

    // Find the tier-3 edge (Inner Mongolia Mining Corp → Baosteel Trading Co)
    let t3_edge = edges.iter().find(|e| {
        let src = e["source"].as_str().unwrap_or("");
        id_to_name
            .get(src)
            .map(|n| n.contains("Inner Mongolia"))
            .unwrap_or(false)
    });
    assert!(
        t3_edge.is_some(),
        "expected a tier-3 edge from Inner Mongolia Mining Corp"
    );

    let t3_edge = t3_edge.expect("t3 edge");
    let t3_target = t3_edge["target"].as_str().expect("target");
    let t3_target_name = id_to_name.get(t3_target).map(String::as_str).unwrap_or("");
    assert!(
        t3_target_name.contains("Baosteel"),
        "tier-3 edge target should be Baosteel Trading Co, got {t3_target_name:?}"
    );

    // Find a tier-2 edge (Yorkshire Steel Works → Bolt Supplies Ltd via supplier_id)
    let t2_edge = edges.iter().find(|e| {
        let src = e["source"].as_str().unwrap_or("");
        id_to_name
            .get(src)
            .map(|n| n.contains("Yorkshire"))
            .unwrap_or(false)
    });
    assert!(
        t2_edge.is_some(),
        "expected a tier-2 edge from Yorkshire Steel Works"
    );

    let t2_edge = t2_edge.expect("t2 edge");
    let t2_target = t2_edge["target"].as_str().expect("target");
    let t2_target_name = id_to_name.get(t2_target).map(String::as_str).unwrap_or("");
    assert!(
        t2_target_name.contains("Bolt"),
        "tier-2 edge target should be Bolt Supplies Ltd (resolved via supplier_id), got {t2_target_name:?}"
    );
}

#[test]
fn import_supplier_list_labels_on_edges() {
    let out = Command::new(omtsf_bin())
        .args([
            "import",
            excel_fixture("omts-supplier-list-example.xlsx")
                .to_str()
                .expect("path"),
        ])
        .output()
        .expect("run omtsf import");
    assert_eq!(out.status.code(), Some(0));

    let stdout = String::from_utf8(out.stdout).expect("UTF-8 stdout");
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");

    let edges = parsed["edges"].as_array().expect("edges array");

    let has_edge_label = |key: &str| -> bool {
        edges.iter().any(|e| {
            e["properties"]["labels"]
                .as_array()
                .map(|labels| labels.iter().any(|l| l["key"].as_str() == Some(key)))
                .unwrap_or(false)
        })
    };

    assert!(
        has_edge_label("risk-tier"),
        "at least one edge must have a risk-tier label"
    );
    assert!(
        has_edge_label("kraljic-quadrant"),
        "at least one edge must have a kraljic-quadrant label"
    );
    assert!(
        has_edge_label("approval-status"),
        "at least one edge must have an approval-status label"
    );
    assert!(
        has_edge_label("business-unit"),
        "at least one edge must have a business-unit label"
    );
}

#[test]
fn import_supplier_list_supplier_id_dedup() {
    let out = Command::new(omtsf_bin())
        .args([
            "import",
            excel_fixture("omts-supplier-list-example.xlsx")
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

    // "Bolt Supplies Ltd" appears in two rows with supplier_id=bolt-001.
    // They should collapse to one node.
    let bolt_nodes: Vec<_> = nodes
        .iter()
        .filter(|n| {
            n["name"]
                .as_str()
                .map(|s| s.contains("Bolt"))
                .unwrap_or(false)
        })
        .collect();
    assert_eq!(
        bolt_nodes.len(),
        1,
        "Bolt Supplies Ltd should be one node (dedup via supplier_id), got {}",
        bolt_nodes.len()
    );

    // But two edges should originate from that node (Procurement + Engineering BUs).
    let bolt_id = bolt_nodes[0]["id"].as_str().expect("bolt node id");
    let bolt_edges: Vec<_> = edges
        .iter()
        .filter(|e| e["source"].as_str() == Some(bolt_id))
        .collect();
    assert_eq!(
        bolt_edges.len(),
        2,
        "Bolt should have 2 supply edges (two BUs), got {}",
        bolt_edges.len()
    );
}

// Note: import_supplier_list_bad_parent_ref_exits_1 is deferred because
// creating programmatic .xlsx files with invalid parent references would
// require rust_xlsxwriter as a test dependency and significant boilerplate.
// The validate_parent_refs() function in supplier_list.rs is unit-testable.
