//! Implementation of `omtsf redact <file> --scope <scope>`.
//!
//! Reads a single `.omts` file, applies redaction rules for the target
//! disclosure scope, and writes the redacted output to stdout. Redaction
//! statistics (nodes redacted, boundary refs generated) are written to stderr.
//!
//! Exit codes:
//! - 0 = success
//! - 1 = redaction error (scope less restrictive than existing `disclosure_scope`,
//!        or the engine produces an invalid output)
//! - 2 = parse/validation failure
use std::collections::HashSet;
use std::io::Write as _;

use omtsf_core::redact;
use omtsf_core::{DisclosureScope as CoreScope, OmtsFile, enums::NodeType, enums::NodeTypeTag};

use crate::DisclosureScope as CliScope;
use crate::error::CliError;

// ---------------------------------------------------------------------------
// run
// ---------------------------------------------------------------------------

/// Runs the `redact` command.
///
/// Parses `content` as an OMTSF file, checks that the target scope is at
/// least as restrictive as the file's existing `disclosure_scope`, then
/// applies the redaction engine. The redacted file is written to stdout;
/// statistics go to stderr.
///
/// # Errors
///
/// - [`CliError::ParseFailed`] — content is not a valid OMTSF file.
/// - [`CliError::RedactionError`] — target scope is less restrictive than
///   the existing scope, or the engine produces an invalid output.
pub fn run(content: &str, scope: &CliScope) -> Result<(), CliError> {
    // --- Parse ---
    let file: OmtsFile = serde_json::from_str(content).map_err(|e| CliError::ParseFailed {
        detail: format!("line {}, column {}: {e}", e.line(), e.column()),
    })?;

    // --- Scope compatibility check ---
    // Target scope must be at least as restrictive as the existing disclosure_scope.
    let target_core = cli_scope_to_core(scope);
    if let Some(existing) = &file.disclosure_scope {
        if scope_is_less_restrictive(&target_core, existing) {
            return Err(CliError::RedactionError {
                detail: format!(
                    "target scope '{target_core:?}' is less restrictive than \
                     existing disclosure_scope '{existing:?}'"
                ),
            });
        }
    }

    // --- Gather statistics before redaction ---
    let total_nodes_before = file.nodes.len();
    let person_nodes_before = file
        .nodes
        .iter()
        .filter(|n| matches!(&n.node_type, NodeTypeTag::Known(NodeType::Person)))
        .count();

    // --- Apply redaction (retain all nodes, producer decides replacement) ---
    // We pass an empty retain_ids set so that all non-omitted nodes are
    // replaced with boundary_ref stubs, except for the subset the caller
    // explicitly retains. The CLI's `redact` command uses the "retain none"
    // policy (all nodes replaced) which is the minimal, safe default.
    let retain_ids = HashSet::new();
    let redacted =
        redact(&file, target_core, &retain_ids).map_err(|e| CliError::RedactionError {
            detail: e.to_string(),
        })?;

    // --- Emit statistics to stderr ---
    let stderr = std::io::stderr();
    let mut err_out = stderr.lock();

    let nodes_after = redacted.nodes.len();
    let boundary_refs = redacted
        .nodes
        .iter()
        .filter(|n| matches!(&n.node_type, NodeTypeTag::Known(NodeType::BoundaryRef)))
        .count();
    let nodes_redacted = total_nodes_before.saturating_sub(nodes_after - boundary_refs);

    writeln!(
        err_out,
        "redaction complete: scope={scope:?}, nodes_before={total_nodes_before}, \
         nodes_redacted={nodes_redacted}, person_nodes_dropped={person_nodes_before}, \
         boundary_refs={boundary_refs}"
    )
    .map_err(|e| CliError::IoError {
        source: "stderr".to_owned(),
        detail: e.to_string(),
    })?;

    // --- Write redacted file to stdout ---
    let json = serde_json::to_string_pretty(&redacted).map_err(|e| CliError::InternalError {
        detail: format!("JSON serialization of redacted output failed: {e}"),
    })?;

    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    writeln!(out, "{json}").map_err(|_| CliError::IoError {
        source: "stdout".to_owned(),
        detail: "write failed".to_owned(),
    })?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Converts a CLI [`crate::DisclosureScope`] to the core library [`CoreScope`].
fn cli_scope_to_core(scope: &CliScope) -> CoreScope {
    match scope {
        CliScope::Public => CoreScope::Public,
        CliScope::Partner => CoreScope::Partner,
        CliScope::Internal => CoreScope::Internal,
    }
}

/// Returns `true` if `target` is less restrictive than `existing`.
///
/// Restrictiveness order (most to least): `Public` > `Partner` > `Internal`.
/// For example, redacting an `internal` file to `partner` is valid (more
/// restrictive), but redacting a `public` file to `partner` is invalid (less
/// restrictive).
fn scope_is_less_restrictive(target: &CoreScope, existing: &CoreScope) -> bool {
    scope_level(target) < scope_level(existing)
}

/// Maps a scope to a numeric restrictiveness level for comparison.
///
/// Higher value = more restrictive.
fn scope_level(scope: &CoreScope) -> u8 {
    match scope {
        CoreScope::Internal => 0,
        CoreScope::Partner => 1,
        CoreScope::Public => 2,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    #![allow(clippy::panic)]

    use super::*;
    use crate::DisclosureScope as CliScope;

    // Minimal valid OMTS JSON without a disclosure_scope.
    const MINIMAL: &str = r#"{
        "omtsf_version": "1.0.0",
        "snapshot_date": "2026-02-19",
        "file_salt": "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef",
        "nodes": [],
        "edges": []
    }"#;

    // File that already declares disclosure_scope = "public".
    const ALREADY_PUBLIC: &str = r#"{
        "omtsf_version": "1.0.0",
        "snapshot_date": "2026-02-19",
        "file_salt": "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef",
        "disclosure_scope": "public",
        "nodes": [],
        "edges": []
    }"#;

    // File that already declares disclosure_scope = "partner".
    const ALREADY_PARTNER: &str = r#"{
        "omtsf_version": "1.0.0",
        "snapshot_date": "2026-02-19",
        "file_salt": "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef",
        "disclosure_scope": "partner",
        "nodes": [],
        "edges": []
    }"#;

    // Not valid JSON.
    const NOT_JSON: &str = "this is not json";

    // ── scope_level ───────────────────────────────────────────────────────────

    #[test]
    fn scope_level_ordering() {
        assert!(scope_level(&CoreScope::Internal) < scope_level(&CoreScope::Partner));
        assert!(scope_level(&CoreScope::Partner) < scope_level(&CoreScope::Public));
    }

    // ── scope_is_less_restrictive ────────────────────────────────────────────

    #[test]
    fn internal_is_less_restrictive_than_partner() {
        assert!(scope_is_less_restrictive(
            &CoreScope::Internal,
            &CoreScope::Partner
        ));
    }

    #[test]
    fn internal_is_less_restrictive_than_public() {
        assert!(scope_is_less_restrictive(
            &CoreScope::Internal,
            &CoreScope::Public
        ));
    }

    #[test]
    fn partner_is_less_restrictive_than_public() {
        assert!(scope_is_less_restrictive(
            &CoreScope::Partner,
            &CoreScope::Public
        ));
    }

    #[test]
    fn public_is_not_less_restrictive_than_partner() {
        assert!(!scope_is_less_restrictive(
            &CoreScope::Public,
            &CoreScope::Partner
        ));
    }

    #[test]
    fn public_is_not_less_restrictive_than_internal() {
        assert!(!scope_is_less_restrictive(
            &CoreScope::Public,
            &CoreScope::Internal
        ));
    }

    #[test]
    fn same_scope_is_not_less_restrictive() {
        assert!(!scope_is_less_restrictive(
            &CoreScope::Public,
            &CoreScope::Public
        ));
        assert!(!scope_is_less_restrictive(
            &CoreScope::Partner,
            &CoreScope::Partner
        ));
        assert!(!scope_is_less_restrictive(
            &CoreScope::Internal,
            &CoreScope::Internal
        ));
    }

    // ── cli_scope_to_core ────────────────────────────────────────────────────

    #[test]
    fn cli_scope_maps_to_core() {
        assert_eq!(cli_scope_to_core(&CliScope::Public), CoreScope::Public);
        assert_eq!(cli_scope_to_core(&CliScope::Partner), CoreScope::Partner);
        assert_eq!(cli_scope_to_core(&CliScope::Internal), CoreScope::Internal);
    }

    // ── run: parse failure ────────────────────────────────────────────────────

    #[test]
    fn run_invalid_json_returns_parse_failed() {
        let result = run(NOT_JSON, &CliScope::Public);
        match result {
            Err(CliError::ParseFailed { .. }) => {}
            other => panic!("expected ParseFailed, got {other:?}"),
        }
    }

    #[test]
    fn run_parse_failure_exit_code_is_2() {
        let result = run(NOT_JSON, &CliScope::Public);
        let err = result.expect_err("should fail");
        assert_eq!(err.exit_code(), 2);
    }

    // ── run: scope compatibility ──────────────────────────────────────────────

    /// Redacting a `public` file to `partner` scope (less restrictive) → error.
    #[test]
    fn run_less_restrictive_scope_returns_error() {
        let result = run(ALREADY_PUBLIC, &CliScope::Partner);
        match result {
            Err(CliError::RedactionError { .. }) => {}
            other => panic!("expected RedactionError, got {other:?}"),
        }
    }

    #[test]
    fn run_less_restrictive_scope_exit_code_is_1() {
        let result = run(ALREADY_PUBLIC, &CliScope::Partner);
        let err = result.expect_err("should fail");
        assert_eq!(err.exit_code(), 1);
    }

    /// Redacting a `partner` file to `internal` scope (less restrictive) → error.
    #[test]
    fn run_partner_to_internal_returns_error() {
        let result = run(ALREADY_PARTNER, &CliScope::Internal);
        match result {
            Err(CliError::RedactionError { .. }) => {}
            other => panic!("expected RedactionError, got {other:?}"),
        }
    }

    /// Same scope is allowed (idempotent re-redaction).
    #[test]
    fn run_same_scope_is_ok() {
        let result = run(ALREADY_PUBLIC, &CliScope::Public);
        assert!(result.is_ok(), "same scope should succeed: {result:?}");
    }

    // ── run: happy path ───────────────────────────────────────────────────────

    /// Redacting a minimal file to public scope succeeds.
    #[test]
    fn run_minimal_to_public_succeeds() {
        let result = run(MINIMAL, &CliScope::Public);
        assert!(result.is_ok(), "expected Ok for minimal file: {result:?}");
    }

    /// Redacting a minimal file to partner scope succeeds.
    #[test]
    fn run_minimal_to_partner_succeeds() {
        let result = run(MINIMAL, &CliScope::Partner);
        assert!(result.is_ok(), "expected Ok for minimal file: {result:?}");
    }

    /// Redacting a minimal file to internal scope succeeds (no-op path).
    #[test]
    fn run_minimal_to_internal_succeeds() {
        let result = run(MINIMAL, &CliScope::Internal);
        assert!(result.is_ok(), "expected Ok for minimal file: {result:?}");
    }

    /// A file with a person node: redacting to public should succeed
    /// (the person node is omitted by the engine).
    #[test]
    fn run_file_with_person_node_to_public_succeeds() {
        let content = r#"{
            "omtsf_version": "1.0.0",
            "snapshot_date": "2026-02-19",
            "file_salt": "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef",
            "nodes": [
                { "id": "org-001", "type": "organization", "name": "Acme Corp" },
                { "id": "person-001", "type": "person", "name": "Jane Doe" }
            ],
            "edges": []
        }"#;
        let result = run(content, &CliScope::Public);
        assert!(result.is_ok(), "expected Ok: {result:?}");
    }

    // ── RedactionError exit code ──────────────────────────────────────────────

    #[test]
    fn redaction_error_exit_code_is_1() {
        let err = CliError::RedactionError {
            detail: "test error".to_owned(),
        };
        assert_eq!(err.exit_code(), 1);
    }

    #[test]
    fn redaction_error_message_contains_detail() {
        let err = CliError::RedactionError {
            detail: "scope conflict".to_owned(),
        };
        let msg = err.message();
        assert!(msg.contains("scope conflict"), "message: {msg}");
        assert!(msg.contains("redaction"), "message: {msg}");
    }
}
