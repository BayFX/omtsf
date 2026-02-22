//! Integration tests for `omtsf query`.
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

// ---- exit code: no selectors → 2 -------------------------------------------

#[test]
fn query_no_selectors_exits_2() {
    let out = Command::new(omtsf_bin())
        .args(["query", fixture("graph-query.omts").to_str().expect("path")])
        .output()
        .expect("run omtsf query (no selectors)");
    assert_eq!(
        out.status.code(),
        Some(2),
        "expected exit 2 when no selector flags given"
    );
}

// ---- exit code: nonexistent file → 2 ----------------------------------------

#[test]
fn query_nonexistent_file_exits_2() {
    let out = Command::new(omtsf_bin())
        .args([
            "query",
            "/no/such/file/ever.omts",
            "--node-type",
            "organization",
        ])
        .output()
        .expect("run omtsf query on nonexistent file");
    assert_eq!(
        out.status.code(),
        Some(2),
        "expected exit 2 for nonexistent file"
    );
}

// ---- exit code: empty result → 1 --------------------------------------------

#[test]
fn query_no_match_exits_1() {
    // graph-query.omts has only organizations, no facilities
    let out = Command::new(omtsf_bin())
        .args([
            "query",
            fixture("graph-query.omts").to_str().expect("path"),
            "--node-type",
            "facility",
        ])
        .output()
        .expect("run omtsf query --node-type facility on graph-query.omts");
    assert_eq!(
        out.status.code(),
        Some(1),
        "expected exit 1 when no elements match"
    );
}

// ---- --node-type filtering --------------------------------------------------

#[test]
fn query_node_type_organization_exits_0() {
    let out = Command::new(omtsf_bin())
        .args([
            "query",
            fixture("graph-query.omts").to_str().expect("path"),
            "--node-type",
            "organization",
        ])
        .output()
        .expect("run omtsf query --node-type organization");
    assert!(
        out.status.success(),
        "expected exit 0 for matching node type; exit: {:?}",
        out.status.code()
    );
}

#[test]
fn query_node_type_organization_produces_output() {
    let out = Command::new(omtsf_bin())
        .args([
            "query",
            fixture("graph-query.omts").to_str().expect("path"),
            "--node-type",
            "organization",
        ])
        .output()
        .expect("run omtsf query --node-type organization");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("organization"),
        "stdout should mention organization type; stdout: {stdout}"
    );
}

#[test]
fn query_node_type_facility_in_full_featured_exits_0() {
    let out = Command::new(omtsf_bin())
        .args([
            "query",
            fixture("full-featured.omts").to_str().expect("path"),
            "--node-type",
            "facility",
        ])
        .output()
        .expect("run omtsf query --node-type facility on full-featured.omts");
    assert!(
        out.status.success(),
        "expected exit 0; exit: {:?}",
        out.status.code()
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("fac-sheffield"),
        "stdout should contain fac-sheffield; stdout: {stdout}"
    );
}

// ---- --identifier scheme and scheme:value matching --------------------------

#[test]
fn query_identifier_scheme_lei_exits_0() {
    // full-featured.omts has a node with scheme=lei
    let out = Command::new(omtsf_bin())
        .args([
            "query",
            fixture("full-featured.omts").to_str().expect("path"),
            "--identifier",
            "lei",
        ])
        .output()
        .expect("run omtsf query --identifier lei");
    assert!(
        out.status.success(),
        "expected exit 0 for --identifier lei; exit: {:?}",
        out.status.code()
    );
}

#[test]
fn query_identifier_scheme_value_exits_0() {
    // org-alpha has lei 5493006MHB84DD0ZWV18
    let out = Command::new(omtsf_bin())
        .args([
            "query",
            fixture("full-featured.omts").to_str().expect("path"),
            "--identifier",
            "lei:5493006MHB84DD0ZWV18",
        ])
        .output()
        .expect("run omtsf query --identifier lei:VALUE");
    assert!(
        out.status.success(),
        "expected exit 0 for --identifier lei:VALUE; exit: {:?}",
        out.status.code()
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("org-alpha"),
        "stdout should contain org-alpha; stdout: {stdout}"
    );
}

#[test]
fn query_identifier_scheme_value_no_match_exits_1() {
    let out = Command::new(omtsf_bin())
        .args([
            "query",
            fixture("full-featured.omts").to_str().expect("path"),
            "--identifier",
            "lei:NONEXISTENT000000000",
        ])
        .output()
        .expect("run omtsf query --identifier lei:NONEXISTENT");
    assert_eq!(
        out.status.code(),
        Some(1),
        "expected exit 1 for nonexistent identifier value"
    );
}

// ---- --jurisdiction filtering -----------------------------------------------

#[test]
fn query_jurisdiction_de_exits_0() {
    // full-featured.omts has nodes with jurisdiction=DE
    let out = Command::new(omtsf_bin())
        .args([
            "query",
            fixture("full-featured.omts").to_str().expect("path"),
            "--jurisdiction",
            "DE",
        ])
        .output()
        .expect("run omtsf query --jurisdiction DE");
    assert!(
        out.status.success(),
        "expected exit 0 for --jurisdiction DE; exit: {:?}",
        out.status.code()
    );
}

#[test]
fn query_jurisdiction_de_output_contains_org_alpha() {
    let out = Command::new(omtsf_bin())
        .args([
            "query",
            fixture("full-featured.omts").to_str().expect("path"),
            "--jurisdiction",
            "DE",
        ])
        .output()
        .expect("run omtsf query --jurisdiction DE");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("org-alpha"),
        "stdout should contain org-alpha (jurisdiction=DE); stdout: {stdout}"
    );
}

#[test]
fn query_jurisdiction_us_no_match_exits_1() {
    // full-featured.omts has no nodes with jurisdiction=US
    let out = Command::new(omtsf_bin())
        .args([
            "query",
            fixture("full-featured.omts").to_str().expect("path"),
            "--jurisdiction",
            "US",
        ])
        .output()
        .expect("run omtsf query --jurisdiction US (no match)");
    assert_eq!(
        out.status.code(),
        Some(1),
        "expected exit 1 when no node has jurisdiction=US"
    );
}

// ---- --name substring matching ----------------------------------------------

#[test]
fn query_name_substring_exits_0() {
    // full-featured.omts has "Alpha Manufacturing GmbH"
    let out = Command::new(omtsf_bin())
        .args([
            "query",
            fixture("full-featured.omts").to_str().expect("path"),
            "--name",
            "Alpha",
        ])
        .output()
        .expect("run omtsf query --name Alpha");
    assert!(
        out.status.success(),
        "expected exit 0 for --name Alpha; exit: {:?}",
        out.status.code()
    );
}

#[test]
fn query_name_case_insensitive_exits_0() {
    // Name matching is case-insensitive per CLI doc
    let out = Command::new(omtsf_bin())
        .args([
            "query",
            fixture("full-featured.omts").to_str().expect("path"),
            "--name",
            "alpha",
        ])
        .output()
        .expect("run omtsf query --name alpha (lowercase)");
    assert!(
        out.status.success(),
        "expected exit 0 for case-insensitive --name alpha; exit: {:?}",
        out.status.code()
    );
}

#[test]
fn query_name_no_match_exits_1() {
    let out = Command::new(omtsf_bin())
        .args([
            "query",
            fixture("full-featured.omts").to_str().expect("path"),
            "--name",
            "ZZZNonexistentNameZZZ",
        ])
        .output()
        .expect("run omtsf query --name ZZZ (no match)");
    assert_eq!(
        out.status.code(),
        Some(1),
        "expected exit 1 for --name with no match"
    );
}

// ---- --count output format --------------------------------------------------

#[test]
fn query_count_exits_0() {
    let out = Command::new(omtsf_bin())
        .args([
            "query",
            fixture("graph-query.omts").to_str().expect("path"),
            "--node-type",
            "organization",
            "--count",
        ])
        .output()
        .expect("run omtsf query --node-type organization --count");
    assert!(
        out.status.success(),
        "expected exit 0 for --count with matches; exit: {:?}",
        out.status.code()
    );
}

#[test]
fn query_count_output_format_human() {
    let out = Command::new(omtsf_bin())
        .args([
            "query",
            fixture("graph-query.omts").to_str().expect("path"),
            "--node-type",
            "organization",
            "--count",
        ])
        .output()
        .expect("run omtsf query --count human");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("nodes:"),
        "count output should contain 'nodes:'; stdout: {stdout}"
    );
    assert!(
        stdout.contains("edges:"),
        "count output should contain 'edges:'; stdout: {stdout}"
    );
}

#[test]
fn query_count_output_format_json() {
    // --count with --format json still emits the count lines on stdout (not JSON)
    let out = Command::new(omtsf_bin())
        .args([
            "query",
            "--format",
            "json",
            fixture("graph-query.omts").to_str().expect("path"),
            "--node-type",
            "organization",
            "--count",
        ])
        .output()
        .expect("run omtsf query --format json --count");
    assert!(
        out.status.success(),
        "expected exit 0 for --format json --count; exit: {:?}",
        out.status.code()
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("nodes:"),
        "count output should contain 'nodes:'; stdout: {stdout}"
    );
}

// ---- --format json output structure -----------------------------------------

#[test]
fn query_format_json_exits_0() {
    let out = Command::new(omtsf_bin())
        .args([
            "query",
            "--format",
            "json",
            fixture("graph-query.omts").to_str().expect("path"),
            "--node-type",
            "organization",
        ])
        .output()
        .expect("run omtsf query --format json");
    assert!(
        out.status.success(),
        "expected exit 0 for --format json; exit: {:?}",
        out.status.code()
    );
}

#[test]
fn query_format_json_output_is_valid_json() {
    let out = Command::new(omtsf_bin())
        .args([
            "query",
            "--format",
            "json",
            fixture("graph-query.omts").to_str().expect("path"),
            "--node-type",
            "organization",
        ])
        .output()
        .expect("run omtsf query --format json");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: Result<serde_json::Value, _> = serde_json::from_str(stdout.trim());
    assert!(
        parsed.is_ok(),
        "output should be valid JSON; stdout: {stdout}"
    );
}

#[test]
fn query_format_json_has_nodes_and_edges_keys() {
    let out = Command::new(omtsf_bin())
        .args([
            "query",
            "--format",
            "json",
            fixture("graph-query.omts").to_str().expect("path"),
            "--node-type",
            "organization",
        ])
        .output()
        .expect("run omtsf query --format json");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("valid JSON from query");
    assert!(
        value.get("nodes").is_some(),
        "JSON output should have 'nodes' key"
    );
    assert!(
        value.get("edges").is_some(),
        "JSON output should have 'edges' key"
    );
}

#[test]
fn query_format_json_nodes_array_is_non_empty_for_organization() {
    let out = Command::new(omtsf_bin())
        .args([
            "query",
            "--format",
            "json",
            fixture("graph-query.omts").to_str().expect("path"),
            "--node-type",
            "organization",
        ])
        .output()
        .expect("run omtsf query --format json --node-type organization");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("valid JSON from query");
    let nodes = value["nodes"].as_array().expect("nodes is an array");
    assert!(!nodes.is_empty(), "nodes array should be non-empty");
}

// ---- human output format ----------------------------------------------------

#[test]
fn query_human_output_contains_header() {
    let out = Command::new(omtsf_bin())
        .args([
            "query",
            fixture("graph-query.omts").to_str().expect("path"),
            "--node-type",
            "organization",
        ])
        .output()
        .expect("run omtsf query human output");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("KIND"),
        "human output should contain KIND column header; stdout: {stdout}"
    );
    assert!(
        stdout.contains("ID"),
        "human output should contain ID column header; stdout: {stdout}"
    );
    assert!(
        stdout.contains("TYPE"),
        "human output should contain TYPE column header; stdout: {stdout}"
    );
}

// ---- mixed selectors (AND/OR composition) -----------------------------------

#[test]
fn query_node_type_and_jurisdiction_intersection() {
    // Both --node-type organization AND --jurisdiction DE must be satisfied.
    // full-featured.omts: org-alpha is organization+DE, org-beta is organization+GB
    let out = Command::new(omtsf_bin())
        .args([
            "query",
            fixture("full-featured.omts").to_str().expect("path"),
            "--node-type",
            "organization",
            "--jurisdiction",
            "DE",
        ])
        .output()
        .expect("run omtsf query --node-type organization --jurisdiction DE");
    assert!(
        out.status.success(),
        "expected exit 0; exit: {:?}",
        out.status.code()
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("org-alpha"),
        "org-alpha should match (organization + DE); stdout: {stdout}"
    );
}

#[test]
fn query_node_type_and_name_combined() {
    let out = Command::new(omtsf_bin())
        .args([
            "query",
            fixture("full-featured.omts").to_str().expect("path"),
            "--node-type",
            "organization",
            "--name",
            "Beta",
        ])
        .output()
        .expect("run omtsf query --node-type organization --name Beta");
    assert!(
        out.status.success(),
        "expected exit 0; exit: {:?}",
        out.status.code()
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("org-beta"),
        "org-beta should appear in output; stdout: {stdout}"
    );
}

#[test]
fn query_multiple_node_types_or_composed() {
    // Two --node-type flags: organization OR facility
    let out = Command::new(omtsf_bin())
        .args([
            "query",
            fixture("full-featured.omts").to_str().expect("path"),
            "--node-type",
            "organization",
            "--node-type",
            "facility",
        ])
        .output()
        .expect("run omtsf query --node-type organization --node-type facility");
    assert!(
        out.status.success(),
        "expected exit 0; exit: {:?}",
        out.status.code()
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("org-alpha") || stdout.contains("fac-sheffield"),
        "output should contain organization or facility nodes; stdout: {stdout}"
    );
}

#[test]
fn query_identifier_and_jurisdiction_combined() {
    // --identifier lei (scheme) AND --jurisdiction DE
    let out = Command::new(omtsf_bin())
        .args([
            "query",
            fixture("full-featured.omts").to_str().expect("path"),
            "--identifier",
            "lei",
            "--jurisdiction",
            "DE",
        ])
        .output()
        .expect("run omtsf query --identifier lei --jurisdiction DE");
    assert!(
        out.status.success(),
        "expected exit 0 for combined selectors; exit: {:?}",
        out.status.code()
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("org-alpha"),
        "org-alpha matches both lei and DE; stdout: {stdout}"
    );
}

// ---- edge type matching -----------------------------------------------------

#[test]
fn query_edge_type_supplies_exits_0() {
    let out = Command::new(omtsf_bin())
        .args([
            "query",
            fixture("graph-query.omts").to_str().expect("path"),
            "--edge-type",
            "supplies",
        ])
        .output()
        .expect("run omtsf query --edge-type supplies");
    assert!(
        out.status.success(),
        "expected exit 0 for --edge-type supplies; exit: {:?}",
        out.status.code()
    );
}

#[test]
fn query_edge_type_nonexistent_exits_1() {
    let out = Command::new(omtsf_bin())
        .args([
            "query",
            fixture("graph-query.omts").to_str().expect("path"),
            "--edge-type",
            "ownership",
        ])
        .output()
        .expect("run omtsf query --edge-type ownership (not in graph-query)");
    assert_eq!(
        out.status.code(),
        Some(1),
        "expected exit 1 when no edges of that type exist"
    );
}

// ---- stderr: matched counts always emitted ----------------------------------

#[test]
fn query_matched_counts_on_stderr() {
    let out = Command::new(omtsf_bin())
        .args([
            "query",
            fixture("graph-query.omts").to_str().expect("path"),
            "--node-type",
            "organization",
        ])
        .output()
        .expect("run omtsf query (check stderr)");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("matched"),
        "stderr should contain 'matched' count summary; stderr: {stderr}"
    );
}
