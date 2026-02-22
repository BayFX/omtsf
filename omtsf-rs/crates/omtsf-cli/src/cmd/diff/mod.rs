//! Implementation of `omtsf diff <a> <b>`.
//!
//! Parses two `.omts` files, runs the structural diff engine, and writes the
//! result to stdout.
//!
//! Flags:
//! - `--ids-only`: Only show IDs of changed elements, no property detail.
//! - `--summary-only`: Only show the summary statistics line.
//! - `--node-type <TYPE>` (repeatable): Restrict diff to nodes of this type.
//! - `--edge-type <TYPE>` (repeatable): Restrict diff to edges of this type.
//! - `--ignore-field <FIELD>` (repeatable): Exclude this property from comparison.
//!
//! Exit codes:
//! - 0 = files are identical
//! - 1 = differences found
//! - 2 = parse failure on either file
mod human;
mod json;

use std::collections::HashSet;

use omtsf_core::{DiffFilter, DiffResult, OmtsFile, diff_filtered};

use crate::OutputFormat;
use crate::error::CliError;
use crate::format::{FormatterConfig, write_timing};

/// Runs the `diff` command.
///
/// Constructs the [`DiffFilter`] from CLI flags, calls [`diff_filtered`] on
/// the pre-parsed files, and writes the result to stdout in the requested
/// format.
///
/// Returns `Ok(())` when the files are identical (exit 0).
/// Returns [`CliError::DiffHasDifferences`] (exit 1) when differences are found.
///
/// # Errors
///
/// - [`CliError::DiffHasDifferences`] — the diff is non-empty.
/// - [`CliError::IoError`] — stdout write failed.
#[allow(clippy::too_many_arguments)]
pub fn run(
    file_a: &OmtsFile,
    file_b: &OmtsFile,
    ids_only: bool,
    summary_only: bool,
    node_types: &[String],
    edge_types: &[String],
    ignore_fields: &[String],
    format: &OutputFormat,
    verbose: bool,
    no_color: bool,
) -> Result<(), CliError> {
    let filter = DiffFilter {
        node_types: if node_types.is_empty() {
            None
        } else {
            Some(node_types.iter().cloned().collect::<HashSet<_>>())
        },
        edge_types: if edge_types.is_empty() {
            None
        } else {
            Some(edge_types.iter().cloned().collect::<HashSet<_>>())
        },
        ignore_fields: ignore_fields.iter().cloned().collect::<HashSet<_>>(),
    };

    let diff_start = std::time::Instant::now();
    let result = diff_filtered(file_a, file_b, Some(&filter));
    let diff_elapsed = diff_start.elapsed();

    write_result(&result, ids_only, summary_only, format)?;

    let fmt_config = FormatterConfig::from_flags(no_color, false, verbose);
    let stderr = std::io::stderr();
    let mut err_out = stderr.lock();
    write_timing(&mut err_out, "diffed", diff_elapsed, &fmt_config).map_err(|e| {
        CliError::IoError {
            source: "stderr".to_owned(),
            detail: e.to_string(),
        }
    })?;

    if result.is_empty() {
        Ok(())
    } else {
        Err(CliError::DiffHasDifferences)
    }
}

fn write_result(
    result: &DiffResult,
    ids_only: bool,
    summary_only: bool,
    format: &OutputFormat,
) -> Result<(), CliError> {
    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    match format {
        OutputFormat::Human => human::write_human(&mut out, result, ids_only, summary_only),
        OutputFormat::Json => json::write_json(&mut out, result),
    }
    .map_err(|e| CliError::IoError {
        source: "stdout".to_owned(),
        detail: e.to_string(),
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    #![allow(clippy::panic)]

    use omtsf_core::{DiffFilter, OmtsFile, diff_filtered};

    use super::*;

    const EMPTY: &str = r#"{
        "omtsf_version": "1.0.0",
        "snapshot_date": "2026-02-19",
        "file_salt": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "nodes": [],
        "edges": []
    }"#;

    const WITH_ORG: &str = r#"{
        "omtsf_version": "1.0.0",
        "snapshot_date": "2026-02-19",
        "file_salt": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        "nodes": [
            {"id": "org-001", "type": "organization", "name": "Acme Corp"}
        ],
        "edges": []
    }"#;

    const WITH_ORG_MODIFIED: &str = r#"{
        "omtsf_version": "1.0.0",
        "snapshot_date": "2026-02-19",
        "file_salt": "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
        "nodes": [
            {
                "id": "org-001",
                "type": "organization",
                "name": "Acme Corporation",
                "identifiers": [{"scheme": "duns", "value": "123456789"}]
            }
        ],
        "edges": []
    }"#;

    const WITH_ORG_SAME_EXT_ID: &str = r#"{
        "omtsf_version": "1.0.0",
        "snapshot_date": "2026-02-19",
        "file_salt": "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
        "nodes": [
            {
                "id": "org-001",
                "type": "organization",
                "name": "Acme Corp",
                "identifiers": [{"scheme": "duns", "value": "123456789"}]
            }
        ],
        "edges": []
    }"#;

    fn parse(s: &str) -> OmtsFile {
        serde_json::from_str(s).expect("valid OMTS JSON")
    }

    #[test]
    fn run_identical_empty_files_returns_ok() {
        let a = parse(EMPTY);
        let b = parse(EMPTY);
        let result = run(
            &a,
            &b,
            false,
            false,
            &[],
            &[],
            &[],
            &OutputFormat::Human,
            false,
            true,
        );
        assert!(
            result.is_ok(),
            "expected Ok for identical files: {result:?}"
        );
    }

    #[test]
    fn run_identical_files_exit_code_is_0() {
        let a = parse(WITH_ORG_SAME_EXT_ID);
        let b = parse(WITH_ORG_SAME_EXT_ID);
        let result = run(
            &a,
            &b,
            false,
            false,
            &[],
            &[],
            &[],
            &OutputFormat::Human,
            false,
            true,
        );
        assert!(result.is_ok(), "identical files should exit 0: {result:?}");
    }

    #[test]
    fn run_different_files_returns_diff_has_differences() {
        let a = parse(EMPTY);
        let b = parse(WITH_ORG);
        let result = run(
            &a,
            &b,
            false,
            false,
            &[],
            &[],
            &[],
            &OutputFormat::Human,
            false,
            true,
        );
        match result {
            Err(CliError::DiffHasDifferences) => {}
            other => panic!("expected DiffHasDifferences, got {other:?}"),
        }
    }

    #[test]
    fn run_different_files_exit_code_is_1() {
        let a = parse(EMPTY);
        let b = parse(WITH_ORG);
        let err = run(
            &a,
            &b,
            false,
            false,
            &[],
            &[],
            &[],
            &OutputFormat::Human,
            false,
            true,
        )
        .expect_err("should fail with differences");
        assert_eq!(err.exit_code(), 1);
    }

    #[test]
    fn run_ids_only_still_exits_1_for_differences() {
        let a = parse(EMPTY);
        let b = parse(WITH_ORG);
        let err = run(
            &a,
            &b,
            true,
            false,
            &[],
            &[],
            &[],
            &OutputFormat::Human,
            false,
            true,
        )
        .expect_err("should still exit 1");
        assert_eq!(err.exit_code(), 1);
    }

    #[test]
    fn run_summary_only_exits_1_for_differences() {
        let a = parse(EMPTY);
        let b = parse(WITH_ORG);
        let err = run(
            &a,
            &b,
            false,
            true,
            &[],
            &[],
            &[],
            &OutputFormat::Human,
            false,
            true,
        )
        .expect_err("should exit 1");
        assert_eq!(err.exit_code(), 1);
    }

    #[test]
    fn run_json_identical_files_returns_ok() {
        let a = parse(EMPTY);
        let b = parse(EMPTY);
        let result = run(
            &a,
            &b,
            false,
            false,
            &[],
            &[],
            &[],
            &OutputFormat::Json,
            false,
            true,
        );
        assert!(result.is_ok(), "expected Ok: {result:?}");
    }

    #[test]
    fn run_json_different_files_returns_diff_has_differences() {
        let a = parse(EMPTY);
        let b = parse(WITH_ORG);
        let result = run(
            &a,
            &b,
            false,
            false,
            &[],
            &[],
            &[],
            &OutputFormat::Json,
            false,
            true,
        );
        assert!(
            matches!(result, Err(CliError::DiffHasDifferences)),
            "expected DiffHasDifferences: {result:?}"
        );
    }

    fn capture_human(
        content_a: &str,
        content_b: &str,
        ids_only: bool,
        summary_only: bool,
    ) -> String {
        let file_a: OmtsFile = serde_json::from_str(content_a).expect("parse A");
        let file_b: OmtsFile = serde_json::from_str(content_b).expect("parse B");
        let filter = DiffFilter::default();
        let result = diff_filtered(&file_a, &file_b, Some(&filter));
        let mut buf: Vec<u8> = Vec::new();
        human::write_human(&mut buf, &result, ids_only, summary_only).expect("write");
        String::from_utf8(buf).expect("utf8")
    }

    #[test]
    fn human_output_contains_summary_line() {
        let s = capture_human(EMPTY, WITH_ORG, false, false);
        assert!(s.contains("Summary:"), "output: {s}");
    }

    #[test]
    fn human_output_summary_only_contains_summary() {
        let s = capture_human(EMPTY, WITH_ORG, false, true);
        assert!(s.contains("Summary:"), "output: {s}");
    }

    #[test]
    fn human_output_added_node_has_plus_prefix() {
        let s = capture_human(EMPTY, WITH_ORG, false, false);
        assert!(s.contains("  + org-001"), "output: {s}");
    }

    #[test]
    fn human_output_removed_node_has_minus_prefix() {
        let s = capture_human(WITH_ORG, EMPTY, false, false);
        assert!(s.contains("  - org-001"), "output: {s}");
    }

    #[test]
    fn human_output_modified_node_has_tilde_prefix() {
        let s = capture_human(WITH_ORG_SAME_EXT_ID, WITH_ORG_MODIFIED, false, false);
        assert!(s.contains("  ~ org-001"), "output: {s}");
    }

    #[test]
    fn human_output_modified_node_shows_property_change() {
        let s = capture_human(WITH_ORG_SAME_EXT_ID, WITH_ORG_MODIFIED, false, false);
        assert!(s.contains("name"), "output should mention 'name': {s}");
    }

    #[test]
    fn human_output_ids_only_omits_property_details() {
        let s = capture_human(WITH_ORG_SAME_EXT_ID, WITH_ORG_MODIFIED, true, false);
        assert!(s.contains("  ~ org-001"), "output: {s}");
        assert!(
            !s.contains("    ~"),
            "ids_only should omit property detail: {s}"
        );
    }

    fn capture_json(content_a: &str, content_b: &str) -> serde_json::Value {
        let file_a: OmtsFile = serde_json::from_str(content_a).expect("parse A");
        let file_b: OmtsFile = serde_json::from_str(content_b).expect("parse B");
        let filter = DiffFilter::default();
        let result = diff_filtered(&file_a, &file_b, Some(&filter));
        let mut buf: Vec<u8> = Vec::new();
        json::write_json(&mut buf, &result).expect("write");
        serde_json::from_str(&String::from_utf8(buf).expect("utf8")).expect("json")
    }

    #[test]
    fn json_output_has_summary_field() {
        let v = capture_json(EMPTY, WITH_ORG);
        assert!(v.get("summary").is_some(), "missing 'summary': {v}");
    }

    #[test]
    fn json_output_has_nodes_field() {
        let v = capture_json(EMPTY, WITH_ORG);
        assert!(v.get("nodes").is_some(), "missing 'nodes': {v}");
    }

    #[test]
    fn json_output_has_edges_field() {
        let v = capture_json(EMPTY, WITH_ORG);
        assert!(v.get("edges").is_some(), "missing 'edges': {v}");
    }

    #[test]
    fn json_output_summary_counts_added_node() {
        let v = capture_json(EMPTY, WITH_ORG);
        let added = v["summary"]["nodes_added"].as_u64().expect("nodes_added");
        assert_eq!(added, 1, "expected 1 added node");
    }

    #[test]
    fn json_output_nodes_added_contains_id() {
        let v = capture_json(EMPTY, WITH_ORG);
        let added = v["nodes"]["added"].as_array().expect("nodes.added array");
        let ids: Vec<&str> = added.iter().filter_map(|n| n["id"].as_str()).collect();
        assert!(ids.contains(&"org-001"), "expected org-001 in added: {v}");
    }
}
