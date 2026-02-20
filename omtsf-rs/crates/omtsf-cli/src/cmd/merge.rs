//! Implementation of `omtsf merge <file>...`.
//!
//! Reads two or more `.omts` files, runs the merge engine, and writes the
//! merged file to stdout. Diagnostics (warnings, conflict counts) go to
//! stderr.
//!
//! Exit codes:
//! - 0 = success
//! - 1 = merge conflict (unresolvable property collision or internal error)
//! - 2 = parse/validation failure on any input file
use std::io::Write as _;

use omtsf_core::{OmtsFile, merge};

use crate::PathOrStdin;
use crate::error::CliError;
use crate::io::read_input;

// ---------------------------------------------------------------------------
// run
// ---------------------------------------------------------------------------

/// Runs the `merge` command.
///
/// Reads each path in `files`, parses them as OMTSF files, runs the merge
/// engine, and writes the merged output to stdout as pretty-printed JSON.
/// Warnings and conflict statistics are written to stderr.
///
/// # Errors
///
/// - [`CliError::ParseFailed`] — any input file is not a valid OMTSF file.
/// - [`CliError::MergeConflict`] — the merge engine reports an internal error.
pub fn run(files: &[PathOrStdin], max_file_size: u64) -> Result<(), CliError> {
    // --- Read and parse each file ---
    let mut parsed: Vec<OmtsFile> = Vec::with_capacity(files.len());
    for source in files {
        let content = read_input(source, max_file_size)?;
        let file: OmtsFile = serde_json::from_str(&content).map_err(|e| CliError::ParseFailed {
            detail: e.to_string(),
        })?;
        parsed.push(file);
    }

    // --- Run merge engine ---
    let output = merge(&parsed).map_err(|e| CliError::MergeConflict {
        detail: e.to_string(),
    })?;

    // --- Emit warnings to stderr ---
    let stderr = std::io::stderr();
    let mut err_out = stderr.lock();

    for warning in &output.warnings {
        writeln!(err_out, "warning: {warning}").map_err(|e| CliError::IoError {
            source: "stderr".to_owned(),
            detail: e.to_string(),
        })?;
    }

    if output.conflict_count > 0 {
        writeln!(
            err_out,
            "merge complete: {} conflict(s) recorded",
            output.conflict_count
        )
        .map_err(|e| CliError::IoError {
            source: "stderr".to_owned(),
            detail: e.to_string(),
        })?;
    }

    // --- Write merged file to stdout ---
    let json = serde_json::to_string_pretty(&output.file).map_err(|e| CliError::MergeConflict {
        detail: format!("serialization failed: {e}"),
    })?;

    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    writeln!(out, "{json}").map_err(|_| {
        // Broken pipe is treated as silent exit 0 (standard Unix behavior).
        // We return IoError here but the SIGPIPE handler will have already
        // terminated the process in normal pipe-break scenarios.
        CliError::IoError {
            source: "stdout".to_owned(),
            detail: "write failed".to_owned(),
        }
    })?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    #![allow(clippy::panic)]

    use super::*;

    // Minimal valid OMTS JSON (no nodes, no edges).
    const MINIMAL_A: &str = r#"{
        "omtsf_version": "1.0.0",
        "snapshot_date": "2026-02-19",
        "file_salt": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "nodes": [],
        "edges": []
    }"#;

    const MINIMAL_B: &str = r#"{
        "omtsf_version": "1.0.0",
        "snapshot_date": "2026-02-19",
        "file_salt": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        "nodes": [],
        "edges": []
    }"#;

    // Invalid JSON for parse failure tests.
    const NOT_JSON: &str = "this is not json";

    // ── parse helpers ─────────────────────────────────────────────────────────

    fn parse_file(s: &str) -> OmtsFile {
        serde_json::from_str(s).expect("valid OMTS JSON")
    }

    // ── merge engine integration ──────────────────────────────────────────────

    /// Two empty files can be merged without error.
    #[test]
    fn merge_two_empty_files_succeeds() {
        let a = parse_file(MINIMAL_A);
        let b = parse_file(MINIMAL_B);
        let result = merge(&[a, b]);
        assert!(result.is_ok(), "expected merge success: {result:?}");
    }

    /// The merge engine requires at least one file.
    #[test]
    fn merge_no_files_returns_error() {
        let result = merge(&[]);
        assert!(result.is_err(), "expected error with no files");
    }

    /// parse failure produces `ParseFailed` with exit code 2.
    #[test]
    fn parse_failure_exit_code_2() {
        let err = serde_json::from_str::<OmtsFile>(NOT_JSON)
            .map_err(|e| CliError::ParseFailed {
                detail: e.to_string(),
            })
            .expect_err("should fail");
        assert_eq!(err.exit_code(), 2);
    }

    /// `MergeConflict` maps to exit code 1.
    #[test]
    fn merge_conflict_exit_code_1() {
        let err = CliError::MergeConflict {
            detail: "test conflict".to_owned(),
        };
        assert_eq!(err.exit_code(), 1);
    }

    /// Two files with overlapping LEI merge into one node.
    #[test]
    fn merge_shared_lei_produces_one_node() {
        let file_a: OmtsFile = serde_json::from_str(
            r#"{
            "omtsf_version": "1.0.0",
            "snapshot_date": "2026-02-19",
            "file_salt": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "nodes": [
                {
                    "id": "org-a",
                    "type": "organization",
                    "name": "Acme Corp",
                    "identifiers": [
                        { "scheme": "lei", "value": "TESTLEISHAREDTEST062" }
                    ]
                }
            ],
            "edges": []
        }"#,
        )
        .expect("parse file A");

        let file_b: OmtsFile = serde_json::from_str(
            r#"{
            "omtsf_version": "1.0.0",
            "snapshot_date": "2026-02-19",
            "file_salt": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            "nodes": [
                {
                    "id": "org-b",
                    "type": "organization",
                    "name": "Acme Corp",
                    "identifiers": [
                        { "scheme": "lei", "value": "TESTLEISHAREDTEST062" }
                    ]
                }
            ],
            "edges": []
        }"#,
        )
        .expect("parse file B");

        let output = merge(&[file_a, file_b]).expect("merge should succeed");
        assert_eq!(
            output.file.nodes.len(),
            1,
            "nodes with shared LEI must merge into one"
        );
    }
}
