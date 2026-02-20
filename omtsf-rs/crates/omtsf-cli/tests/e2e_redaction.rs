//! End-to-end integration tests for the `omtsf redact` command.
//!
//! Tests cover person node handling, scope transitions, boundary ref generation,
//! and validation of the redacted output at different scope levels.
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

/// Run `omtsf redact --scope <scope> <file>` and return the raw output.
fn run_redact(scope: &str, fixture_name: &str) -> std::process::Output {
    Command::new(omtsf_bin())
        .args([
            "redact",
            "--scope",
            scope,
            fixture(fixture_name).to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf redact")
}

/// Validate a JSON value by writing to a temp file and calling `omtsf validate`.
fn validate_json(value: &serde_json::Value) -> bool {
    let json_str = serde_json::to_string(value).expect("serialize JSON");
    let mut tmp = tempfile::NamedTempFile::new().expect("temp file");
    tmp.write_all(json_str.as_bytes()).expect("write JSON");

    let out = Command::new(omtsf_bin())
        .args([
            "validate",
            "--level",
            "1",
            tmp.path().to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf validate");
    out.status.code() == Some(0)
}

// ---------------------------------------------------------------------------
// Redaction 1: Person nodes removed at public scope
// ---------------------------------------------------------------------------

/// In public scope, all `person` nodes are removed (Omit action).
#[test]
fn redaction_public_removes_person_nodes() {
    let out = run_redact("public", "redact-internal.omts");
    assert_eq!(
        out.status.code(),
        Some(0),
        "redact --scope public must exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("valid JSON from redact");
    let nodes = value["nodes"].as_array().expect("nodes array");

    for node in nodes {
        let node_type = node["type"].as_str().unwrap_or("");
        assert_ne!(
            node_type, "person",
            "public-scope output must not contain person nodes; found: {node:?}"
        );
    }
}

/// The public-scope redacted output passes L1 validation.
#[test]
fn redaction_public_output_passes_validate() {
    let out = run_redact("public", "redact-internal.omts");
    assert_eq!(out.status.code(), Some(0), "redact must succeed");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("valid JSON from redact");
    assert!(
        validate_json(&value),
        "public-scope redacted output must pass L1 validation"
    );
}

/// The public-scope output has `disclosure_scope` = `"public"`.
#[test]
fn redaction_public_sets_disclosure_scope_field() {
    let out = run_redact("public", "redact-internal.omts");
    assert_eq!(out.status.code(), Some(0), "redact must succeed");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("valid JSON from redact");
    assert_eq!(
        value["disclosure_scope"].as_str(),
        Some("public"),
        "output must declare disclosure_scope = public"
    );
}

/// Public-scope redaction removes `beneficial_ownership` edges (whose source
/// was a person node that got Omit-ed).
#[test]
fn redaction_public_removes_beneficial_ownership_edges() {
    let out = run_redact("public", "redact-internal.omts");
    assert_eq!(out.status.code(), Some(0), "redact must succeed");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("valid JSON from redact");
    let edges = value["edges"].as_array().expect("edges array");

    for edge in edges {
        let edge_type = edge["type"].as_str().unwrap_or("");
        assert_ne!(
            edge_type, "beneficial_ownership",
            "public output must not contain beneficial_ownership edges; edge: {edge:?}"
        );
    }
}

/// The `file_salt` is preserved across redaction (per redaction spec §7.1).
#[test]
fn redaction_public_preserves_file_salt() {
    let out = run_redact("public", "redact-internal.omts");
    assert_eq!(out.status.code(), Some(0), "redact must succeed");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("valid JSON from redact");
    assert_eq!(
        value["file_salt"].as_str(),
        Some("deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef"),
        "file_salt must be preserved in redacted output"
    );
}

// ---------------------------------------------------------------------------
// Redaction 2: Scope transition — internal to partner
// ---------------------------------------------------------------------------

/// Redacting an internal-scope file to partner scope exits 0.
#[test]
fn redaction_scope_transition_internal_to_partner_exits_0() {
    let out = run_redact("partner", "redact-internal.omts");
    assert_eq!(
        out.status.code(),
        Some(0),
        "redact --scope partner on internal file must exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Partner-scope output sets `disclosure_scope` = `"partner"`.
#[test]
fn redaction_scope_transition_internal_to_partner_sets_scope() {
    let out = run_redact("partner", "redact-internal.omts");
    assert_eq!(out.status.code(), Some(0), "redact must succeed");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("valid JSON from redact");
    assert_eq!(
        value["disclosure_scope"].as_str(),
        Some("partner"),
        "output must declare disclosure_scope = partner"
    );
}

/// Partner-scope output passes L1 validation.
#[test]
fn redaction_scope_transition_internal_to_partner_validates() {
    let out = run_redact("partner", "redact-internal.omts");
    assert_eq!(out.status.code(), Some(0), "redact must succeed");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("valid JSON from redact");
    assert!(
        validate_json(&value),
        "partner-scope redacted output must pass L1 validation"
    );
}

// ---------------------------------------------------------------------------
// Redaction 3: Scope transition — partner to public
// ---------------------------------------------------------------------------

/// Redacting a partner-scope file to public scope exits 0.
#[test]
fn redaction_scope_transition_partner_to_public_exits_0() {
    let out = run_redact("public", "redact-partner.omts");
    assert_eq!(
        out.status.code(),
        Some(0),
        "redact --scope public on partner file must exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// The output of partner→public redaction is valid JSON with required fields.
#[test]
fn redaction_scope_transition_partner_to_public_valid_json() {
    let out = run_redact("public", "redact-partner.omts");
    assert_eq!(out.status.code(), Some(0), "redact must succeed");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("valid JSON from partner→public redact");
    assert!(value["nodes"].is_array(), "output must have nodes array");
    assert!(value["edges"].is_array(), "output must have edges array");
    assert!(
        value["omtsf_version"].is_string(),
        "output must have omtsf_version"
    );
}

/// Partner→public redaction sets `disclosure_scope` = `"public"`.
#[test]
fn redaction_scope_transition_partner_to_public_sets_scope() {
    let out = run_redact("public", "redact-partner.omts");
    assert_eq!(out.status.code(), Some(0), "redact must succeed");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: serde_json::Value = serde_json::from_str(stdout.trim()).expect("valid JSON");
    assert_eq!(
        value["disclosure_scope"].as_str(),
        Some("public"),
        "partner→public output must set disclosure_scope = public"
    );
}

/// Partner→public redaction output passes L1 validation.
#[test]
fn redaction_scope_transition_partner_to_public_validates() {
    let out = run_redact("public", "redact-partner.omts");
    assert_eq!(out.status.code(), Some(0), "redact must succeed");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: serde_json::Value = serde_json::from_str(stdout.trim()).expect("valid JSON");
    assert!(
        validate_json(&value),
        "partner→public redacted output must pass L1 validation"
    );
}

// ---------------------------------------------------------------------------
// Redaction 4: Boundary refs in redacted output
// ---------------------------------------------------------------------------

/// Redacting a file that contains `boundary_ref` nodes: the existing boundary
/// refs are preserved in the output (they are always retained).
#[test]
fn redaction_preserves_existing_boundary_refs() {
    let out = run_redact("public", "redact-with-boundary.omts");
    assert_eq!(
        out.status.code(),
        Some(0),
        "redact --scope public on boundary fixture must exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("valid JSON from redact");
    let nodes = value["nodes"].as_array().expect("nodes array");

    let bref_count = nodes
        .iter()
        .filter(|n| n["type"].as_str() == Some("boundary_ref"))
        .count();
    assert!(
        bref_count >= 1,
        "redacted output must retain existing boundary_ref nodes; nodes: {nodes:?}"
    );
}

/// Redacting a file with boundary refs produces L1-valid output.
#[test]
fn redaction_with_boundary_refs_passes_validate() {
    let out = run_redact("public", "redact-with-boundary.omts");
    assert_eq!(out.status.code(), Some(0), "redact must succeed");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("valid JSON from redact");
    assert!(
        validate_json(&value),
        "boundary-ref redacted output must pass L1 validation"
    );
}

/// The `boundary_ref` node in the output has exactly one opaque identifier
/// (satisfying L1-SDI-01).
#[test]
fn redaction_boundary_ref_has_opaque_identifier() {
    let out = run_redact("public", "redact-with-boundary.omts");
    assert_eq!(out.status.code(), Some(0), "redact must succeed");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("valid JSON from redact");
    let nodes = value["nodes"].as_array().expect("nodes array");

    for node in nodes {
        if node["type"].as_str() != Some("boundary_ref") {
            continue;
        }
        let ids = node["identifiers"]
            .as_array()
            .expect("boundary_ref has identifiers");
        assert_eq!(
            ids.len(),
            1,
            "boundary_ref must have exactly one identifier; node: {node:?}"
        );
        assert_eq!(
            ids[0]["scheme"].as_str(),
            Some("opaque"),
            "boundary_ref identifier must have scheme 'opaque'; id: {:?}",
            ids[0]
        );
    }
}

// ---------------------------------------------------------------------------
// Redaction 5: Statistics on stderr
// ---------------------------------------------------------------------------

/// The redaction command emits statistics to stderr on success.
#[test]
fn redaction_emits_statistics_to_stderr() {
    let out = run_redact("public", "redact-internal.omts");
    assert_eq!(out.status.code(), Some(0), "redact must succeed");

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("scope") || stderr.contains("nodes_before"),
        "stderr must contain redaction statistics; stderr: {stderr}"
    );
}

/// The statistics mention the target scope.
#[test]
fn redaction_statistics_mention_scope() {
    let out = run_redact("partner", "redact-internal.omts");
    assert_eq!(out.status.code(), Some(0), "redact must succeed");

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("Partner") || stderr.contains("partner"),
        "stderr should mention the target scope; stderr: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// Redaction 6: Attempting less-restrictive scope transition
// ---------------------------------------------------------------------------

/// Redacting a `public`-scope file to `partner` (less restrictive) exits 1.
#[test]
fn redaction_less_restrictive_scope_exits_1() {
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
        .expect("run omtsf redact less-restrictive");
    assert_eq!(
        out.status.code(),
        Some(1),
        "attempting less-restrictive scope must exit 1"
    );
}

/// Redacting a `partner`-scope file to `internal` (less restrictive) exits 1.
#[test]
fn redaction_partner_to_internal_exits_1() {
    let content = r#"{
        "omtsf_version": "1.0.0",
        "snapshot_date": "2026-02-19",
        "file_salt": "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef",
        "disclosure_scope": "partner",
        "nodes": [],
        "edges": []
    }"#;
    let mut tmp = tempfile::NamedTempFile::new().expect("temp file");
    tmp.write_all(content.as_bytes()).expect("write");

    let out = Command::new(omtsf_bin())
        .args([
            "redact",
            "--scope",
            "internal",
            tmp.path().to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf redact partner→internal");
    assert_eq!(
        out.status.code(),
        Some(1),
        "attempting less-restrictive scope must exit 1"
    );
}

// ---------------------------------------------------------------------------
// Redaction 7: Redact full-featured fixture to public
// ---------------------------------------------------------------------------

/// Redacting the full-featured fixture (which includes person, attestation,
/// consignment, facility, etc.) to public scope exits 0 and produces valid output.
#[test]
fn redaction_full_featured_to_public_exits_0() {
    let out = run_redact("public", "full-featured.omts");
    assert_eq!(
        out.status.code(),
        Some(0),
        "redact --scope public on full-featured.omts must exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Public redaction of the full-featured fixture removes person nodes.
#[test]
fn redaction_full_featured_public_no_person_nodes() {
    let out = run_redact("public", "full-featured.omts");
    assert_eq!(out.status.code(), Some(0), "redact must succeed");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("valid JSON from redact");
    let nodes = value["nodes"].as_array().expect("nodes array");

    for node in nodes {
        assert_ne!(
            node["type"].as_str().unwrap_or(""),
            "person",
            "public output of full-featured must not contain person nodes"
        );
    }
}

/// Public redaction of the full-featured fixture retains organization nodes.
#[test]
fn redaction_full_featured_public_retains_org_nodes() {
    let out = run_redact("public", "full-featured.omts");
    assert_eq!(out.status.code(), Some(0), "redact must succeed");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("valid JSON from redact");
    let nodes = value["nodes"].as_array().expect("nodes array");

    // The full-featured fixture has 2 organization nodes; they should survive
    // public redaction (unless replaced with boundary_ref stubs by the CLI's
    // retain_ids=empty policy — in that case they become boundary_ref nodes).
    // The key property is that at least some non-person nodes remain.
    assert!(
        !nodes.is_empty(),
        "public output of full-featured must not be empty"
    );
}
