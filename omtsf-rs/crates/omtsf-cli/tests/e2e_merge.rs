//! E2E test: Import two Excel files and merge them.
//!
//! Verifies that the merge command combines two independently imported graphs
//! into a valid, unified graph with correct deduplication.
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

/// Path to an Excel fixture file relative to the repo root.
fn excel_fixture(name: &str) -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("../../../templates/excel");
    path.push(name);
    path
}

/// Import both Excel examples, merge them, and verify the merged graph.
///
/// The full template (5 nodes, 5 edges) and supplier list (7 nodes, 7 edges)
/// share at least one organization (Bolt Supplies Ltd, DUNS 234567890), so the
/// merged graph should have fewer nodes than the sum of both inputs.
#[test]
fn merge_two_excel_imports() {
    // Import the full-template example (5 nodes, 5 edges).
    let import_full = Command::new(omtsf_bin())
        .args([
            "import",
            excel_fixture("omts-import-example.xlsx")
                .to_str()
                .expect("path"),
        ])
        .output()
        .expect("import full example");
    assert_eq!(
        import_full.status.code(),
        Some(0),
        "import full must succeed; stderr: {}",
        String::from_utf8_lossy(&import_full.stderr)
    );

    let full_stdout = String::from_utf8(import_full.stdout).expect("UTF-8");
    let full_json: serde_json::Value = serde_json::from_str(&full_stdout).expect("JSON");
    let full_nodes = full_json["nodes"].as_array().expect("nodes").len();
    let full_edges = full_json["edges"].as_array().expect("edges").len();

    let mut full_tmp = tempfile::NamedTempFile::new().expect("temp file");
    full_tmp
        .write_all(full_stdout.as_bytes())
        .expect("write full");
    let full_path = full_tmp.path().to_str().expect("path").to_owned();

    // Import the supplier-list example (7 nodes, 7 edges).
    let import_sl = Command::new(omtsf_bin())
        .args([
            "import",
            excel_fixture("omts-supplier-list-example.xlsx")
                .to_str()
                .expect("path"),
        ])
        .output()
        .expect("import supplier list");
    assert_eq!(
        import_sl.status.code(),
        Some(0),
        "import supplier list must succeed; stderr: {}",
        String::from_utf8_lossy(&import_sl.stderr)
    );

    let sl_stdout = String::from_utf8(import_sl.stdout).expect("UTF-8");
    let sl_json: serde_json::Value = serde_json::from_str(&sl_stdout).expect("JSON");
    let sl_nodes = sl_json["nodes"].as_array().expect("nodes").len();
    let sl_edges = sl_json["edges"].as_array().expect("edges").len();

    let mut sl_tmp = tempfile::NamedTempFile::new().expect("temp file");
    sl_tmp
        .write_all(sl_stdout.as_bytes())
        .expect("write supplier list");
    let sl_path = sl_tmp.path().to_str().expect("path").to_owned();

    // -- Merge both imported files --
    let merge_out = Command::new(omtsf_bin())
        .args(["merge", &full_path, &sl_path])
        .output()
        .expect("run omtsf merge");
    assert_eq!(
        merge_out.status.code(),
        Some(0),
        "merge must succeed; stderr: {}",
        String::from_utf8_lossy(&merge_out.stderr)
    );

    let merge_stdout = String::from_utf8(merge_out.stdout).expect("UTF-8");
    let merge_json: serde_json::Value =
        serde_json::from_str(&merge_stdout).expect("merge output JSON");
    let merge_nodes = merge_json["nodes"].as_array().expect("merged nodes");
    let merge_edges = merge_json["edges"].as_array().expect("merged edges");

    // Merged graph must have at least as many nodes as the larger input.
    assert!(
        merge_nodes.len() >= std::cmp::max(full_nodes, sl_nodes),
        "merged node count ({}) must be >= max input ({})",
        merge_nodes.len(),
        std::cmp::max(full_nodes, sl_nodes)
    );
    // Merged graph must not exceed the sum of both inputs (deduplication only removes).
    assert!(
        merge_nodes.len() <= full_nodes + sl_nodes,
        "merged node count ({}) must be <= sum of inputs ({})",
        merge_nodes.len(),
        full_nodes + sl_nodes
    );
    assert!(
        merge_edges.len() >= std::cmp::max(full_edges, sl_edges),
        "merged edge count ({}) must be >= max input ({})",
        merge_edges.len(),
        std::cmp::max(full_edges, sl_edges)
    );

    // -- Validate merged output --
    let mut merged_tmp = tempfile::NamedTempFile::new().expect("temp file");
    merged_tmp
        .write_all(merge_stdout.as_bytes())
        .expect("write merged");
    let merged_path = merged_tmp.path().to_str().expect("path").to_owned();

    let validate_out = Command::new(omtsf_bin())
        .args(["validate", "--level", "1", &merged_path])
        .output()
        .expect("validate merged output");
    assert_eq!(
        validate_out.status.code(),
        Some(0),
        "merged output must pass L1 validation; stderr: {}",
        String::from_utf8_lossy(&validate_out.stderr)
    );

    // -- Inspect merged output --
    let inspect_out = Command::new(omtsf_bin())
        .args(["inspect", "-f", "json", &merged_path])
        .output()
        .expect("inspect merged output");
    assert_eq!(
        inspect_out.status.code(),
        Some(0),
        "inspect on merged output must succeed"
    );
    let inspect_json: serde_json::Value =
        serde_json::from_str(String::from_utf8_lossy(&inspect_out.stdout).trim())
            .expect("inspect JSON");
    assert_eq!(
        inspect_json["node_count"].as_u64().expect("node_count") as usize,
        merge_nodes.len(),
        "inspect node_count must match merged nodes"
    );
    assert_eq!(
        inspect_json["edge_count"].as_u64().expect("edge_count") as usize,
        merge_edges.len(),
        "inspect edge_count must match merged edges"
    );

    // Verify merged graph contains node types from both inputs.
    // The full template contributes facility, good, and attestation nodes
    // that don't exist in the supplier list.
    let has_facility = merge_nodes
        .iter()
        .any(|n| n["type"].as_str() == Some("facility"));
    let has_good = merge_nodes
        .iter()
        .any(|n| n["type"].as_str() == Some("good"));
    let has_attestation = merge_nodes
        .iter()
        .any(|n| n["type"].as_str() == Some("attestation"));
    assert!(
        has_facility,
        "merged graph must contain facility from full template"
    );
    assert!(
        has_good,
        "merged graph must contain good from full template"
    );
    assert!(
        has_attestation,
        "merged graph must contain attestation from full template"
    );

    // All supplier-list nodes are organizations; the merged graph must have
    // at least as many organizations as the supplier list.
    let org_count = merge_nodes
        .iter()
        .filter(|n| n["type"].as_str() == Some("organization"))
        .count();
    assert!(
        org_count >= sl_nodes,
        "merged org count ({org_count}) must be >= supplier-list org count ({sl_nodes})"
    );
}
