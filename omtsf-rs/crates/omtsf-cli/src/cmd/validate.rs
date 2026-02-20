//! Implementation of `omtsf validate <file>`.
//!
//! Parses an `.omts` file and runs the OMTSF validation engine at the
//! requested level, emitting diagnostics to stderr.
//!
//! Flags:
//! - `--level <n>` (default 2): 1 = L1 only, 2 = L1+L2, 3 = L1+L2+L3.
//!
//! Exit codes:
//! - 0 = valid (no L1 errors)
//! - 1 = validation errors (at least one L1 violation)
//! - 2 = parse failure (not valid JSON or missing required fields)
use omtsf_core::{OmtsFile, ValidationConfig, validate};

use crate::OutputFormat;
use crate::error::CliError;
use crate::format::{FormatMode, FormatterConfig, write_diagnostic, write_summary};

// ---------------------------------------------------------------------------
// run
// ---------------------------------------------------------------------------

/// Runs the `validate` command.
///
/// Parses `content` as an OMTSF file, runs the validation engine at the
/// requested `level`, and emits diagnostics to stderr. The summary line is
/// written to stderr in human mode (or as a final NDJSON object in JSON mode).
///
/// Returns `Ok(())` when the file is conformant (no L1 errors). Returns
/// [`CliError::ValidationErrors`] (exit code 1) when L1 errors are found, or
/// [`CliError::ParseFailed`] (exit code 2) when the content cannot be parsed.
///
/// # Errors
///
/// - [`CliError::ParseFailed`] — content is not a valid OMTSF file.
/// - [`CliError::ValidationErrors`] — one or more L1 errors were found.
pub fn run(
    content: &str,
    level: u8,
    format: &OutputFormat,
    quiet: bool,
    verbose: bool,
    no_color: bool,
) -> Result<(), CliError> {
    // --- Parse ---
    let file: OmtsFile = serde_json::from_str(content).map_err(|e| CliError::ParseFailed {
        detail: format!("line {}, column {}: {e}", e.line(), e.column()),
    })?;

    // --- Build ValidationConfig from --level ---
    let config = config_for_level(level);

    // --- Validate ---
    let result = validate(&file, &config, None);

    // --- Emit diagnostics to stderr ---
    let mode = match format {
        OutputFormat::Human => FormatMode::Human,
        OutputFormat::Json => FormatMode::Json,
    };
    let fmt_config = FormatterConfig::from_flags(no_color, quiet, verbose);

    let stderr = std::io::stderr();
    let mut err_out = stderr.lock();

    for diag in &result.diagnostics {
        write_diagnostic(&mut err_out, diag, mode, &fmt_config).map_err(|e| CliError::IoError {
            source: "stderr".to_owned(),
            detail: e.to_string(),
        })?;
    }

    // --- Summary line ---
    let error_count = result.errors().count();
    let warning_count = result.warnings().count();
    let info_count = result.infos().count();

    write_summary(
        &mut err_out,
        error_count,
        warning_count,
        info_count,
        mode,
        &fmt_config,
    )
    .map_err(|e| CliError::IoError {
        source: "stderr".to_owned(),
        detail: e.to_string(),
    })?;

    // --- Exit code ---
    if result.has_errors() {
        Err(CliError::ValidationErrors)
    } else {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Builds a [`ValidationConfig`] from a `--level` value (1, 2, or 3).
///
/// Level 1 runs L1 rules only; level 2 adds L2; level 3 adds L3.
/// Callers guarantee that `level` is in the range `1..=3`.
fn config_for_level(level: u8) -> ValidationConfig {
    match level {
        1 => ValidationConfig {
            run_l1: true,
            run_l2: false,
            run_l3: false,
        },
        2 => ValidationConfig {
            run_l1: true,
            run_l2: true,
            run_l3: false,
        },
        // level 3 (and any other value, though clap prevents values outside 1..=3)
        _ => ValidationConfig {
            run_l1: true,
            run_l2: true,
            run_l3: true,
        },
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

    // Minimal valid OMTS JSON — no nodes, no edges, clean file.
    const MINIMAL_VALID: &str = r#"{
        "omtsf_version": "1.0.0",
        "snapshot_date": "2026-02-19",
        "file_salt": "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef",
        "nodes": [],
        "edges": []
    }"#;

    // OMTS JSON with an edge referencing a non-existent target node (L1-GDM-03).
    const INVALID_EDGE_TARGET: &str = r#"{
        "omtsf_version": "1.0.0",
        "snapshot_date": "2026-02-19",
        "file_salt": "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef",
        "nodes": [
            { "id": "org-001", "type": "organization", "name": "Acme" }
        ],
        "edges": [
            {
                "id": "e-001",
                "type": "supplies",
                "source": "org-001",
                "target": "node-does-not-exist"
            }
        ]
    }"#;

    // Not valid JSON at all.
    const NOT_JSON: &str = "this is not json";

    // ── config_for_level ──────────────────────────────────────────────────────

    #[test]
    fn config_level_1_runs_only_l1() {
        let cfg = config_for_level(1);
        assert!(cfg.run_l1);
        assert!(!cfg.run_l2);
        assert!(!cfg.run_l3);
    }

    #[test]
    fn config_level_2_runs_l1_and_l2() {
        let cfg = config_for_level(2);
        assert!(cfg.run_l1);
        assert!(cfg.run_l2);
        assert!(!cfg.run_l3);
    }

    #[test]
    fn config_level_3_runs_all_levels() {
        let cfg = config_for_level(3);
        assert!(cfg.run_l1);
        assert!(cfg.run_l2);
        assert!(cfg.run_l3);
    }

    // ── run: happy path ───────────────────────────────────────────────────────

    #[test]
    fn run_valid_file_returns_ok() {
        let result = run(MINIMAL_VALID, 2, &OutputFormat::Human, false, false, true);
        assert!(result.is_ok(), "expected Ok for clean file: {result:?}");
    }

    // ── run: parse failure ────────────────────────────────────────────────────

    #[test]
    fn run_invalid_json_returns_parse_failed() {
        let result = run(NOT_JSON, 2, &OutputFormat::Human, false, false, true);
        match result {
            Err(CliError::ParseFailed { .. }) => {}
            other => panic!("expected ParseFailed, got {other:?}"),
        }
    }

    #[test]
    fn run_parse_failure_exit_code_is_2() {
        let result = run(NOT_JSON, 2, &OutputFormat::Human, false, false, true);
        let err = result.expect_err("should fail");
        assert_eq!(err.exit_code(), 2);
    }

    #[test]
    fn run_parse_error_message_includes_line_and_column() {
        // Multi-line JSON with a syntax error on a known line.
        let bad_json = "{\n  \"omtsf_version\": !!bad\n}";
        let err = run(bad_json, 2, &OutputFormat::Human, false, false, true)
            .expect_err("should fail to parse");
        let msg = err.message();
        assert!(msg.contains("line"), "message should contain 'line': {msg}");
        assert!(
            msg.contains("column"),
            "message should contain 'column': {msg}"
        );
    }

    // ── run: validation errors ────────────────────────────────────────────────

    #[test]
    fn run_invalid_edge_returns_validation_errors() {
        let result = run(
            INVALID_EDGE_TARGET,
            2,
            &OutputFormat::Human,
            false,
            false,
            true,
        );
        match result {
            Err(CliError::ValidationErrors) => {}
            other => panic!("expected ValidationErrors, got {other:?}"),
        }
    }

    #[test]
    fn run_validation_error_exit_code_is_1() {
        let result = run(
            INVALID_EDGE_TARGET,
            2,
            &OutputFormat::Human,
            false,
            false,
            true,
        );
        let err = result.expect_err("should fail");
        assert_eq!(err.exit_code(), 1);
    }

    // ── run: level flag interaction ───────────────────────────────────────────

    #[test]
    fn run_level_1_returns_ok_for_clean_file() {
        let result = run(MINIMAL_VALID, 1, &OutputFormat::Human, false, false, true);
        assert!(result.is_ok());
    }

    #[test]
    fn run_level_3_returns_ok_for_clean_file() {
        let result = run(MINIMAL_VALID, 3, &OutputFormat::Human, false, false, true);
        assert!(result.is_ok());
    }

    // ── run: JSON output format ───────────────────────────────────────────────

    #[test]
    fn run_json_format_valid_file_returns_ok() {
        let result = run(MINIMAL_VALID, 2, &OutputFormat::Json, false, false, true);
        assert!(result.is_ok());
    }

    #[test]
    fn run_json_format_invalid_edge_returns_validation_errors() {
        let result = run(
            INVALID_EDGE_TARGET,
            2,
            &OutputFormat::Json,
            false,
            false,
            true,
        );
        match result {
            Err(CliError::ValidationErrors) => {}
            other => panic!("expected ValidationErrors, got {other:?}"),
        }
    }
}
