//! Implementation of `omtsf convert <file>`.
//!
//! Parses an `.omts` file into the typed data model and re-serializes it to
//! stdout. Unknown fields captured via `serde(flatten)` are preserved.
//!
//! Flags:
//! - `--to json` (default): serialize as JSON.
//! - `--to cbor`: serialize as CBOR with self-describing tag 55799.
//! - `--pretty` (default when `--to json`): pretty-print JSON with 2-space indentation.
//! - `--compact`: emit minified JSON with no extra whitespace.
//! - `--compress`: wrap serialized output in a zstd frame.
//!
//! Exit codes: 0 = success, 2 = parse/serialization failure.
use std::io::Write as _;

use omtsf_core::OmtsFile;

use crate::TargetEncoding;
use crate::error::CliError;

/// Runs the `convert` command.
///
/// Re-serializes `file` to the requested target encoding and writes the output
/// to stdout. Unknown fields are preserved via `serde(flatten)`.
///
/// # Encoding and formatting rules
///
/// - `--to json --pretty` (default): pretty-print JSON with 2-space indentation,
///   followed by a trailing newline.
/// - `--to json --compact`: compact JSON with no extra whitespace, followed by
///   a trailing newline.
/// - `--to cbor`: CBOR binary output, no trailing newline.
/// - `--compress`: wraps serialized bytes in a zstd frame before writing.
///   Compatible with both `--to json` and `--to cbor`. No trailing newline when
///   compression is active (output is binary regardless of the base encoding).
/// - When `--to cbor`, the `pretty` and `compact` flags are silently ignored.
///
/// # Errors
///
/// Returns [`CliError`] with exit code 2 if serialization or stdout write fails.
pub fn run(
    file: &OmtsFile,
    to: &TargetEncoding,
    pretty: bool,
    compact: bool,
    compress: bool,
) -> Result<(), CliError> {
    let bytes = serialize(file, to, pretty, compact, compress)?;

    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    out.write_all(&bytes).map_err(|e| CliError::IoError {
        source: "stdout".to_owned(),
        detail: e.to_string(),
    })?;

    // Append a trailing newline for uncompressed JSON so the shell prompt
    // appears on a new line.  Binary outputs (CBOR, any compressed payload)
    // must not have an appended newline because that would corrupt the stream.
    let is_text_output = matches!(to, TargetEncoding::Json) && !compress;
    if is_text_output {
        out.write_all(b"\n").map_err(|e| CliError::IoError {
            source: "stdout".to_owned(),
            detail: e.to_string(),
        })?;
    }

    Ok(())
}

/// Serializes `file` to bytes according to the requested encoding and flags.
///
/// For `--to json --pretty` without compression, the CLI uses
/// `serde_json::to_vec_pretty` directly.  For all other cases the work is
/// delegated to [`omtsf_core::convert`] so that CBOR encoding and zstd
/// compression are handled in a single place.
///
/// When `--to cbor`, the `pretty` and `compact` flags are silently ignored:
/// CBOR has no formatting options.
fn serialize(
    file: &OmtsFile,
    to: &TargetEncoding,
    pretty: bool,
    compact: bool,
    compress: bool,
) -> Result<Vec<u8>, CliError> {
    match to {
        TargetEncoding::Cbor => {
            // CBOR has no formatting options; pretty/compact are silently ignored.
            omtsf_core::convert(file, omtsf_core::Encoding::Cbor, compress)
                .map_err(|e| convert_error_to_cli(&e))
        }
        TargetEncoding::Json => {
            // `compact` wins when explicitly set; otherwise the default is pretty.
            let use_compact = compact || !pretty;
            if use_compact {
                omtsf_core::convert(file, omtsf_core::Encoding::Json, compress)
                    .map_err(|e| convert_error_to_cli(&e))
            } else {
                pretty_json_bytes(file, compress)
            }
        }
    }
}

/// Produces pretty-printed JSON bytes, optionally compressed with zstd.
///
/// This is the CLI-layer path for `--to json --pretty [--compress]`.
/// `omtsf_core::convert` always produces compact JSON, so pretty-printing
/// must be handled here before handing the bytes to the compression layer.
fn pretty_json_bytes(file: &OmtsFile, compress: bool) -> Result<Vec<u8>, CliError> {
    let json_bytes = serde_json::to_vec_pretty(file).map_err(|e| CliError::InternalError {
        detail: format!("JSON pretty-print failed: {e}"),
    })?;

    if compress {
        omtsf_core::compress_zstd(&json_bytes).map_err(|e| CliError::InternalError {
            detail: format!("zstd compression failed: {e}"),
        })
    } else {
        Ok(json_bytes)
    }
}

/// Maps a [`omtsf_core::ConvertError`] to a [`CliError`].
fn convert_error_to_cli(e: &omtsf_core::ConvertError) -> CliError {
    CliError::InternalError {
        detail: e.to_string(),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    #![allow(clippy::panic)]

    use omtsf_core::{OmtsFile, decode_cbor, decompress_zstd};

    use super::*;

    const MINIMAL_JSON: &str = r#"{
        "omtsf_version": "1.0.0",
        "snapshot_date": "2026-02-19",
        "file_salt": "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef",
        "nodes": [],
        "edges": []
    }"#;

    fn parse_json(s: &str) -> OmtsFile {
        serde_json::from_str(s).expect("valid OMTS JSON")
    }

    fn parse_cbor(bytes: &[u8]) -> OmtsFile {
        decode_cbor(bytes).expect("valid CBOR")
    }

    // ---- JSON output tests ------------------------------------------------

    #[test]
    fn json_to_json_pretty_is_ok() {
        let file = parse_json(MINIMAL_JSON);
        let result = serialize(&file, &TargetEncoding::Json, true, false, false);
        assert!(result.is_ok(), "expected Ok: {result:?}");
        let bytes = result.expect("already checked Ok");
        let text = std::str::from_utf8(&bytes).expect("valid UTF-8");
        assert!(text.contains('\n'), "pretty output should contain newlines");
    }

    #[test]
    fn json_to_json_compact_is_ok() {
        let file = parse_json(MINIMAL_JSON);
        let result = serialize(&file, &TargetEncoding::Json, false, true, false);
        assert!(result.is_ok(), "expected Ok: {result:?}");
        let bytes = result.expect("already checked Ok");
        let text = std::str::from_utf8(&bytes).expect("valid UTF-8");
        assert!(
            !text.contains('\n'),
            "compact output must not have newlines"
        );
    }

    #[test]
    fn json_to_json_pretty_produces_valid_json() {
        let file = parse_json(MINIMAL_JSON);
        let bytes = serialize(&file, &TargetEncoding::Json, true, false, false).expect("serialize");
        let reparsed: OmtsFile = serde_json::from_slice(&bytes).expect("re-parse pretty JSON");
        assert_eq!(file, reparsed);
    }

    #[test]
    fn json_to_json_compact_produces_valid_json() {
        let file = parse_json(MINIMAL_JSON);
        let bytes = serialize(&file, &TargetEncoding::Json, false, true, false).expect("serialize");
        let reparsed: OmtsFile = serde_json::from_slice(&bytes).expect("re-parse compact JSON");
        assert_eq!(file, reparsed);
    }

    // ---- CBOR output tests ------------------------------------------------

    #[test]
    fn json_to_cbor_produces_cbor_tag() {
        let file = parse_json(MINIMAL_JSON);
        let bytes =
            serialize(&file, &TargetEncoding::Cbor, false, false, false).expect("serialize CBOR");
        assert_eq!(
            &bytes[..3],
            &[0xD9, 0xD9, 0xF7],
            "CBOR output must start with self-describing tag 55799"
        );
    }

    #[test]
    fn json_to_cbor_round_trips() {
        let file = parse_json(MINIMAL_JSON);
        let cbor_bytes =
            serialize(&file, &TargetEncoding::Cbor, false, false, false).expect("to CBOR");
        let reparsed = parse_cbor(&cbor_bytes);
        assert_eq!(file, reparsed);
    }

    #[test]
    fn cbor_to_json_round_trips() {
        let original = parse_json(MINIMAL_JSON);
        let cbor_bytes =
            serialize(&original, &TargetEncoding::Cbor, false, false, false).expect("to CBOR");
        let from_cbor = parse_cbor(&cbor_bytes);
        let json_bytes =
            serialize(&from_cbor, &TargetEncoding::Json, true, false, false).expect("to JSON");
        let reparsed: OmtsFile = serde_json::from_slice(&json_bytes).expect("parse JSON");
        assert_eq!(original, reparsed);
    }

    #[test]
    fn cbor_pretty_flag_ignored() {
        let file = parse_json(MINIMAL_JSON);
        let with_pretty =
            serialize(&file, &TargetEncoding::Cbor, true, false, false).expect("cbor+pretty");
        let with_compact =
            serialize(&file, &TargetEncoding::Cbor, false, true, false).expect("cbor+compact");
        assert_eq!(
            with_pretty, with_compact,
            "CBOR output must be identical regardless of pretty/compact flags"
        );
    }

    // ---- Compression tests ------------------------------------------------

    #[test]
    fn json_to_zstd_json_starts_with_magic() {
        let file = parse_json(MINIMAL_JSON);
        let bytes =
            serialize(&file, &TargetEncoding::Json, true, false, true).expect("compressed JSON");
        assert_eq!(
            &bytes[..4],
            &[0x28, 0xB5, 0x2F, 0xFD],
            "compressed output must start with zstd magic bytes"
        );
    }

    #[test]
    fn cbor_to_zstd_cbor_starts_with_magic() {
        let file = parse_json(MINIMAL_JSON);
        let bytes =
            serialize(&file, &TargetEncoding::Cbor, false, false, true).expect("compressed CBOR");
        assert_eq!(
            &bytes[..4],
            &[0x28, 0xB5, 0x2F, 0xFD],
            "compressed CBOR output must start with zstd magic bytes"
        );
    }

    #[test]
    fn json_to_zstd_json_round_trips() {
        let original = parse_json(MINIMAL_JSON);
        let compressed =
            serialize(&original, &TargetEncoding::Json, true, false, true).expect("compress");
        let decompressed = decompress_zstd(&compressed, 1024 * 1024).expect("decompress");
        let reparsed: OmtsFile = serde_json::from_slice(&decompressed).expect("re-parse");
        assert_eq!(original, reparsed);
    }

    #[test]
    fn cbor_to_zstd_cbor_round_trips() {
        let original = parse_json(MINIMAL_JSON);
        let compressed =
            serialize(&original, &TargetEncoding::Cbor, false, false, true).expect("compress");
        let decompressed = decompress_zstd(&compressed, 1024 * 1024).expect("decompress");
        let reparsed = parse_cbor(&decompressed);
        assert_eq!(original, reparsed);
    }

    // ---- run() smoke tests ------------------------------------------------

    #[test]
    fn run_json_pretty_is_ok() {
        let file = parse_json(MINIMAL_JSON);
        let result = run(&file, &TargetEncoding::Json, true, false, false);
        assert!(result.is_ok(), "expected Ok: {result:?}");
    }

    #[test]
    fn run_json_compact_is_ok() {
        let file = parse_json(MINIMAL_JSON);
        let result = run(&file, &TargetEncoding::Json, false, true, false);
        assert!(result.is_ok(), "expected Ok: {result:?}");
    }

    #[test]
    fn run_cbor_is_ok() {
        let file = parse_json(MINIMAL_JSON);
        let result = run(&file, &TargetEncoding::Cbor, false, false, false);
        assert!(result.is_ok(), "expected Ok: {result:?}");
    }

    #[test]
    fn run_json_compress_is_ok() {
        let file = parse_json(MINIMAL_JSON);
        let result = run(&file, &TargetEncoding::Json, true, false, true);
        assert!(result.is_ok(), "expected Ok: {result:?}");
    }

    #[test]
    fn run_cbor_compress_is_ok() {
        let file = parse_json(MINIMAL_JSON);
        let result = run(&file, &TargetEncoding::Cbor, false, false, true);
        assert!(result.is_ok(), "expected Ok: {result:?}");
    }
}
