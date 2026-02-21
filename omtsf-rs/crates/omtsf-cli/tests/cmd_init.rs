//! Integration tests for `omtsf init`.
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

#[test]
fn init_exits_0() {
    let out = Command::new(omtsf_bin())
        .arg("init")
        .output()
        .expect("run omtsf init");
    assert!(out.status.success(), "exit code: {:?}", out.status.code());
}

#[test]
fn init_outputs_valid_json() {
    let out = Command::new(omtsf_bin())
        .arg("init")
        .output()
        .expect("run omtsf init");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: Result<serde_json::Value, _> = serde_json::from_str(stdout.trim());
    assert!(parsed.is_ok(), "init output is not valid JSON: {stdout}");
}

#[test]
fn init_has_required_fields() {
    let out = Command::new(omtsf_bin())
        .arg("init")
        .output()
        .expect("run omtsf init");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("valid JSON from init");
    assert!(
        value.get("omtsf_version").is_some(),
        "missing omtsf_version"
    );
    assert!(
        value.get("snapshot_date").is_some(),
        "missing snapshot_date"
    );
    assert!(value.get("file_salt").is_some(), "missing file_salt");
    assert!(value.get("nodes").is_some(), "missing nodes");
    assert!(value.get("edges").is_some(), "missing edges");
}

#[test]
fn init_minimal_empty_nodes_and_edges() {
    let out = Command::new(omtsf_bin())
        .arg("init")
        .output()
        .expect("run omtsf init");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("valid JSON from init");
    let nodes = value["nodes"].as_array().expect("nodes is array");
    let edges = value["edges"].as_array().expect("edges is array");
    assert!(nodes.is_empty(), "minimal init should have no nodes");
    assert!(edges.is_empty(), "minimal init should have no edges");
}

#[test]
fn init_file_salt_is_64_hex_chars() {
    let out = Command::new(omtsf_bin())
        .arg("init")
        .output()
        .expect("run omtsf init");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("valid JSON from init");
    let salt = value["file_salt"].as_str().expect("file_salt is string");
    assert_eq!(salt.len(), 64, "file_salt must be 64 characters");
    assert!(
        salt.chars()
            .all(|c| c.is_ascii_digit() || matches!(c, 'a'..='f')),
        "file_salt must be lowercase hex: {salt}"
    );
}

#[test]
fn init_snapshot_date_is_today() {
    let out = Command::new(omtsf_bin())
        .arg("init")
        .output()
        .expect("run omtsf init");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("valid JSON from init");
    let date = value["snapshot_date"]
        .as_str()
        .expect("snapshot_date is string");
    let today = today_string();
    assert_eq!(date, today, "snapshot_date should be today: {date}");
}

#[test]
fn init_two_calls_produce_different_salts() {
    let out1 = Command::new(omtsf_bin())
        .arg("init")
        .output()
        .expect("run omtsf init first");
    let out2 = Command::new(omtsf_bin())
        .arg("init")
        .output()
        .expect("run omtsf init second");
    let v1: serde_json::Value =
        serde_json::from_str(String::from_utf8_lossy(&out1.stdout).trim()).expect("v1");
    let v2: serde_json::Value =
        serde_json::from_str(String::from_utf8_lossy(&out2.stdout).trim()).expect("v2");
    let salt1 = v1["file_salt"].as_str().expect("salt1");
    let salt2 = v2["file_salt"].as_str().expect("salt2");
    assert_ne!(
        salt1, salt2,
        "two init calls should produce different salts"
    );
}

#[test]
fn init_version_is_semver() {
    let out = Command::new(omtsf_bin())
        .arg("init")
        .output()
        .expect("run omtsf init");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("valid JSON from init");
    let version = value["omtsf_version"]
        .as_str()
        .expect("omtsf_version is string");
    let parts: Vec<&str> = version.split('.').collect();
    assert_eq!(
        parts.len(),
        3,
        "version should be MAJOR.MINOR.PATCH: {version}"
    );
    for part in parts {
        part.parse::<u32>()
            .expect("each version component should be numeric");
    }
}

#[test]
fn init_example_exits_0() {
    let out = Command::new(omtsf_bin())
        .args(["init", "--example"])
        .output()
        .expect("run omtsf init --example");
    assert!(out.status.success(), "exit code: {:?}", out.status.code());
}

#[test]
fn init_example_outputs_valid_json() {
    let out = Command::new(omtsf_bin())
        .args(["init", "--example"])
        .output()
        .expect("run omtsf init --example");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: Result<serde_json::Value, _> = serde_json::from_str(stdout.trim());
    assert!(
        parsed.is_ok(),
        "init --example output is not valid JSON: {stdout}"
    );
}

#[test]
fn init_example_has_nodes_and_edges() {
    let out = Command::new(omtsf_bin())
        .args(["init", "--example"])
        .output()
        .expect("run omtsf init --example");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("valid JSON from init --example");
    let nodes = value["nodes"].as_array().expect("nodes is array");
    let edges = value["edges"].as_array().expect("edges is array");
    assert!(!nodes.is_empty(), "example should have at least one node");
    assert!(!edges.is_empty(), "example should have at least one edge");
}

#[test]
fn init_example_includes_org_facility_good() {
    let out = Command::new(omtsf_bin())
        .args(["init", "--example"])
        .output()
        .expect("run omtsf init --example");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("valid JSON from init --example");
    let nodes = value["nodes"].as_array().expect("nodes is array");

    let has_org = nodes
        .iter()
        .any(|n| n["type"].as_str() == Some("organization"));
    let has_facility = nodes.iter().any(|n| n["type"].as_str() == Some("facility"));
    let has_good = nodes.iter().any(|n| n["type"].as_str() == Some("good"));

    assert!(has_org, "example should include an organization node");
    assert!(has_facility, "example should include a facility node");
    assert!(has_good, "example should include a good node");
}

#[test]
fn init_example_edge_references_valid_nodes() {
    let out = Command::new(omtsf_bin())
        .args(["init", "--example"])
        .output()
        .expect("run omtsf init --example");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("valid JSON from init --example");

    let node_ids: std::collections::HashSet<&str> = value["nodes"]
        .as_array()
        .expect("nodes")
        .iter()
        .filter_map(|n| n["id"].as_str())
        .collect();

    for edge in value["edges"].as_array().expect("edges") {
        let source = edge["source"].as_str().expect("edge has source");
        let target = edge["target"].as_str().expect("edge has target");
        assert!(
            node_ids.contains(source),
            "edge source '{source}' not in node list"
        );
        assert!(
            node_ids.contains(target),
            "edge target '{target}' not in node list"
        );
    }
}

#[test]
fn init_output_parseable_by_inspect() {
    use std::io::Write as _;

    let init_out = Command::new(omtsf_bin())
        .arg("init")
        .output()
        .expect("run omtsf init");
    assert!(
        init_out.status.success(),
        "init failed: {:?}",
        init_out.status.code()
    );

    let mut tmp = tempfile::NamedTempFile::new().expect("temp file");
    tmp.write_all(&init_out.stdout).expect("write tmp");

    let inspect_out = Command::new(omtsf_bin())
        .args(["inspect", tmp.path().to_str().expect("path")])
        .output()
        .expect("run omtsf inspect on init output");
    assert!(
        inspect_out.status.success(),
        "inspect of init output failed: {:?}",
        inspect_out.status.code()
    );
}

#[test]
fn init_example_output_parseable_by_inspect() {
    use std::io::Write as _;

    let init_out = Command::new(omtsf_bin())
        .args(["init", "--example"])
        .output()
        .expect("run omtsf init --example");
    assert!(
        init_out.status.success(),
        "init --example failed: {:?}",
        init_out.status.code()
    );

    let mut tmp = tempfile::NamedTempFile::new().expect("temp file");
    tmp.write_all(&init_out.stdout).expect("write tmp");

    let inspect_out = Command::new(omtsf_bin())
        .args(["inspect", tmp.path().to_str().expect("path")])
        .output()
        .expect("run omtsf inspect on init --example output");
    assert!(
        inspect_out.status.success(),
        "inspect of init --example output failed: {:?}",
        inspect_out.status.code()
    );
}

/// Returns today's date as `YYYY-MM-DD`, mirroring the `init` command's logic.
fn today_string() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock after epoch")
        .as_secs();
    let (y, m, d) = epoch_secs_to_ymd(secs);
    format!("{y:04}-{m:02}-{d:02}")
}

fn epoch_secs_to_ymd(secs: u64) -> (u32, u32, u32) {
    let days = (secs / 86_400) as u32;
    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}
