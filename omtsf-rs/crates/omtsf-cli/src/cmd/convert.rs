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
/// Re-serializes the pre-parsed `file` and writes the output to stdout.
/// Unknown fields are preserved via `serde(flatten)`.
///
/// `compact` controls whether output is minified (`true`) or pretty-printed
/// (`false`, default).
///
/// # Errors
///
/// Returns [`CliError`] with exit code 2 if serialization or writing fails.
pub fn run(file: &OmtsFile, compact: bool) -> Result<(), CliError> {
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

    fn parse(s: &str) -> OmtsFile {
        serde_json::from_str(s).expect("valid OMTS JSON")
    }

    #[test]
    fn run_valid_pretty_returns_ok() {
        let file = parse(MINIMAL);
        let result = run(&file, false);
        assert!(result.is_ok(), "expected Ok: {result:?}");
    }

    #[test]
    fn run_valid_compact_returns_ok() {
        let file = parse(MINIMAL);
        let result = run(&file, true);
        assert!(result.is_ok(), "expected Ok: {result:?}");
    }
}
