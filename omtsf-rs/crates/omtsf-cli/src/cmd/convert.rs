//! Implementation of `omtsf convert <file>`.
//!
//! Parses an `.omts` file into the typed data model and re-serializes it to
//! stdout. Unknown fields captured via `serde(flatten)` are preserved.
//!
//! Flags:
//! - `--pretty` (default): pretty-print JSON with 2-space indentation.
//! - `--compact`: emit minified JSON with no extra whitespace.
//!
//! Exit codes: 0 = success, 2 = parse failure.
use omtsf_core::OmtsFile;

use crate::error::CliError;

/// Runs the `convert` command.
///
/// Parses `content` as an OMTSF file, re-serializes it, and writes the output
/// to stdout. Unknown fields are preserved via `serde(flatten)`.
///
/// `compact` controls whether output is minified (`true`) or pretty-printed
/// (`false`, default).
///
/// # Errors
///
/// Returns [`CliError`] with exit code 2 if the content cannot be parsed or
/// serialized.
pub fn run(content: &str, compact: bool) -> Result<(), CliError> {
    let file: OmtsFile = serde_json::from_str(content).map_err(|e| CliError::ParseFailed {
        detail: format!("line {}, column {}: {e}", e.line(), e.column()),
    })?;

    let output = if compact {
        serde_json::to_string(&file).map_err(|e| CliError::InternalError {
            detail: format!("JSON serialization failed: {e}"),
        })?
    } else {
        serde_json::to_string_pretty(&file).map_err(|e| CliError::InternalError {
            detail: format!("JSON serialization failed: {e}"),
        })?
    };

    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    std::io::Write::write_fmt(&mut out, format_args!("{output}\n")).map_err(|e| CliError::IoError {
        source: "stdout".to_owned(),
        detail: e.to_string(),
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    #![allow(clippy::panic)]

    use super::*;

    const MINIMAL: &str = r#"{
        "omtsf_version": "1.0.0",
        "snapshot_date": "2026-02-19",
        "file_salt": "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef",
        "nodes": [],
        "edges": []
    }"#;

    const NOT_JSON: &str = "not valid json {{ here";

    #[test]
    fn run_valid_pretty_returns_ok() {
        let result = run(MINIMAL, false);
        assert!(result.is_ok(), "expected Ok: {result:?}");
    }

    #[test]
    fn run_valid_compact_returns_ok() {
        let result = run(MINIMAL, true);
        assert!(result.is_ok(), "expected Ok: {result:?}");
    }

    #[test]
    fn run_invalid_json_returns_parse_failed() {
        let result = run(NOT_JSON, false);
        match result {
            Err(CliError::ParseFailed { .. }) => {}
            other => panic!("expected ParseFailed, got {other:?}"),
        }
    }

    #[test]
    fn run_parse_failure_exit_code_is_2() {
        let err = run(NOT_JSON, false).expect_err("should fail");
        assert_eq!(err.exit_code(), 2);
    }

    #[test]
    fn run_parse_error_detail_includes_line_and_column() {
        let bad_json = "{\n  \"omtsf_version\": !!bad\n}";
        let err = run(bad_json, false).expect_err("should fail");
        let msg = err.message();
        assert!(msg.contains("line"), "message should include line: {msg}");
        assert!(
            msg.contains("column"),
            "message should include column: {msg}"
        );
    }
}
