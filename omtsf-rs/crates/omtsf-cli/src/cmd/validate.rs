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
//! - 2 = parse/encoding failure (handled by the dispatch layer)
use omtsf_core::{OmtsFile, ValidationConfig, validate};

use crate::OutputFormat;
use crate::error::CliError;
use crate::format::{FormatMode, FormatterConfig, write_diagnostic, write_summary, write_timing};

/// Runs the `validate` command.
///
/// Runs the validation engine at the requested `level` on the pre-parsed
/// `file`, and emits diagnostics to stderr. The summary line is written to
/// stderr in human mode (or as a final NDJSON object in JSON mode).
///
/// Returns `Ok(())` when the file is conformant (no L1 errors). Returns
/// [`CliError::ValidationErrors`] (exit code 1) when L1 errors are found.
///
/// # Errors
///
/// - [`CliError::ValidationErrors`] — one or more L1 errors were found.
pub fn run(
    file: &OmtsFile,
    level: u8,
    format: &OutputFormat,
    quiet: bool,
    verbose: bool,
    no_color: bool,
) -> Result<(), CliError> {
    let config = config_for_level(level);

    let validate_start = std::time::Instant::now();
    let result = validate(file, &config, None);
    let validate_elapsed = validate_start.elapsed();

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

    write_timing(&mut err_out, "validated", validate_elapsed, &fmt_config).map_err(|e| {
        CliError::IoError {
            source: "stderr".to_owned(),
            detail: e.to_string(),
        }
    })?;

    if result.has_errors() {
        Err(CliError::ValidationErrors)
    } else {
        Ok(())
    }
}

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
        _ => ValidationConfig {
            run_l1: true,
            run_l2: true,
            run_l3: true,
        },
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    #![allow(clippy::panic)]

    use super::*;
    const MINIMAL_VALID: &str = r#"{
        "omtsf_version": "1.0.0",
        "snapshot_date": "2026-02-19",
        "file_salt": "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef",
        "nodes": [],
        "edges": []
    }"#;

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

    fn parse(s: &str) -> OmtsFile {
        serde_json::from_str(s).expect("valid OMTS JSON")
    }

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

    #[test]
    fn run_valid_file_returns_ok() {
        let file = parse(MINIMAL_VALID);
        let result = run(&file, 2, &OutputFormat::Human, false, false, true);
        assert!(result.is_ok(), "expected Ok for clean file: {result:?}");
    }

    #[test]
    fn run_invalid_edge_returns_validation_errors() {
        let file = parse(INVALID_EDGE_TARGET);
        let result = run(&file, 2, &OutputFormat::Human, false, false, true);
        match result {
            Err(CliError::ValidationErrors) => {}
            other => panic!("expected ValidationErrors, got {other:?}"),
        }
    }

    #[test]
    fn run_validation_error_exit_code_is_1() {
        let file = parse(INVALID_EDGE_TARGET);
        let result = run(&file, 2, &OutputFormat::Human, false, false, true);
        let err = result.expect_err("should fail");
        assert_eq!(err.exit_code(), 1);
    }

    #[test]
    fn run_level_1_returns_ok_for_clean_file() {
        let file = parse(MINIMAL_VALID);
        let result = run(&file, 1, &OutputFormat::Human, false, false, true);
        assert!(result.is_ok());
    }

    #[test]
    fn run_level_3_returns_ok_for_clean_file() {
        let file = parse(MINIMAL_VALID);
        let result = run(&file, 3, &OutputFormat::Human, false, false, true);
        assert!(result.is_ok());
    }

    #[test]
    fn run_json_format_valid_file_returns_ok() {
        let file = parse(MINIMAL_VALID);
        let result = run(&file, 2, &OutputFormat::Json, false, false, true);
        assert!(result.is_ok());
    }

    #[test]
    fn run_json_format_invalid_edge_returns_validation_errors() {
        let file = parse(INVALID_EDGE_TARGET);
        let result = run(&file, 2, &OutputFormat::Json, false, false, true);
        match result {
            Err(CliError::ValidationErrors) => {}
            other => panic!("expected ValidationErrors, got {other:?}"),
        }
    }
}
