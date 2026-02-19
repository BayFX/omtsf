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

// ---------------------------------------------------------------------------
// run
// ---------------------------------------------------------------------------

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
    let file: OmtsFile = serde_json::from_str(content).map_err(|e| CliError::IoError {
        source: "<input>".to_owned(),
        detail: format!("JSON parse error: {e}"),
    })?;

    let output = if compact {
        serde_json::to_string(&file).map_err(|e| CliError::IoError {
            source: "<input>".to_owned(),
            detail: format!("JSON serialize error: {e}"),
        })?
    } else {
        serde_json::to_string_pretty(&file).map_err(|e| CliError::IoError {
            source: "<input>".to_owned(),
            detail: format!("JSON serialize error: {e}"),
        })?
    };

    println!("{output}");
    Ok(())
}
