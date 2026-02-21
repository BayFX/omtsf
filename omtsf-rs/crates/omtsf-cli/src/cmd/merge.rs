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

use omtsf_core::validation::{ValidationConfig, validate};
use omtsf_core::{OmtsFile, merge};

use crate::MergeStrategy;
use crate::PathOrStdin;
use crate::TargetEncoding;
use crate::error::CliError;
use crate::io::read_and_parse;

/// Runs the `merge` command.
///
/// Reads each path in `files` using the multi-encoding pipeline, runs L1
/// validation on each, runs the merge engine, and writes the merged output to
/// stdout in the requested encoding. Warnings and conflict statistics are
/// written to stderr.
///
/// # Errors
///
/// - [`CliError::ParseFailed`] — any input file is not a valid OMTSF file.
/// - [`CliError::ValidationErrors`] — any input file fails L1 validation.
/// - [`CliError::MergeConflict`] — the merge engine reports an internal error.
pub fn run(
    files: &[PathOrStdin],
    strategy: &MergeStrategy,
    to: &TargetEncoding,
    compress: bool,
    max_file_size: u64,
    verbose: bool,
) -> Result<(), CliError> {
    if matches!(strategy, MergeStrategy::Intersect) {
        eprintln!("error: intersect strategy is not yet implemented");
        return Err(CliError::MergeConflict {
            detail: "intersect strategy is not yet implemented".to_owned(),
        });
    }

    let l1_config = ValidationConfig {
        run_l1: true,
        run_l2: false,
        run_l3: false,
    };
    let stderr = std::io::stderr();
    let mut err_out = stderr.lock();

    let mut parsed: Vec<OmtsFile> = Vec::with_capacity(files.len());
    for source in files {
        let (file, _encoding) = read_and_parse(source, max_file_size, verbose)?;

        let validation_result = validate(&file, &l1_config, None);
        if validation_result.has_errors() {
            for diag in validation_result.errors() {
                writeln!(
                    err_out,
                    "error: {} {}: {}",
                    diag.rule_id, diag.location, diag.message
                )
                .map_err(|e| CliError::IoError {
                    source: "stderr".to_owned(),
                    detail: e.to_string(),
                })?;
            }
            return Err(CliError::ValidationErrors);
        }

        parsed.push(file);
    }

    let output = merge(&parsed).map_err(|e| CliError::MergeConflict {
        detail: e.to_string(),
    })?;

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

    let bytes = encode_output(&output.file, to, compress)?;

    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    out.write_all(&bytes).map_err(|e| CliError::IoError {
        source: "stdout".to_owned(),
        detail: e.to_string(),
    })?;

    // Append a trailing newline for uncompressed JSON so the shell prompt
    // appears on a new line. Binary outputs must not have an appended newline.
    if matches!(to, TargetEncoding::Json) && !compress {
        out.write_all(b"\n").map_err(|e| CliError::IoError {
            source: "stdout".to_owned(),
            detail: e.to_string(),
        })?;
    }

    Ok(())
}

/// Serializes `file` to the requested encoding, optionally compressing with zstd.
///
/// Pretty-printed JSON is the default for `--to json`. CBOR uses the
/// self-describing tag 55799 prepended per SPEC-007 Section 4.1.
fn encode_output(
    file: &OmtsFile,
    to: &TargetEncoding,
    compress: bool,
) -> Result<Vec<u8>, CliError> {
    match to {
        TargetEncoding::Cbor => omtsf_core::convert(file, omtsf_core::Encoding::Cbor, compress)
            .map_err(|e| CliError::InternalError {
                detail: e.to_string(),
            }),
        TargetEncoding::Json => {
            let json_bytes =
                serde_json::to_vec_pretty(file).map_err(|e| CliError::InternalError {
                    detail: format!("JSON serialization of merged output failed: {e}"),
                })?;
            if compress {
                omtsf_core::compress_zstd(&json_bytes).map_err(|e| CliError::InternalError {
                    detail: format!("zstd compression failed: {e}"),
                })
            } else {
                Ok(json_bytes)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    #![allow(clippy::panic)]

    use super::*;
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

    const NOT_JSON: &str = "this is not json";

    fn parse_file(s: &str) -> OmtsFile {
        serde_json::from_str(s).expect("valid OMTS JSON")
    }

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
