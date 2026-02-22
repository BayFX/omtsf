//! Lossless cross-encoding conversion between JSON and CBOR.
//!
//! Implements the conversion rules defined in SPEC-007 Section 5.
//! Field names, values, null vs. absent, and array element order are
//! preserved through every conversion.  JSON whitespace and key ordering
//! are not preserved.

use crate::cbor::{CborError, encode_cbor};
#[cfg(feature = "compression")]
use crate::compression::{CompressionError, compress_zstd};
use crate::encoding::Encoding;
use crate::file::OmtsFile;

/// Error produced by [`convert`].
#[derive(Debug)]
pub enum ConvertError {
    /// Serializing the file to JSON failed.
    Json(serde_json::Error),
    /// Encoding the file to CBOR failed.
    Cbor(CborError),
    /// [`Encoding::Zstd`] was passed as the target encoding.
    ///
    /// zstd is the compression layer (SPEC-007 Section 6), not an encoding.
    /// Use [`Encoding::Json`] or [`Encoding::Cbor`] as the target and set
    /// `compress = true` to produce a zstd-wrapped payload.
    ZstdIsNotAnEncoding,
    /// `compress = true` was requested but the `compression` feature is
    /// disabled.
    ///
    /// Build `omtsf-core` with the `compression` feature (enabled by default)
    /// to produce compressed output.
    #[cfg(not(feature = "compression"))]
    CompressionNotAvailable,
    /// The zstd compression step failed after serialization.
    #[cfg(feature = "compression")]
    Compression(CompressionError),
}

impl std::fmt::Display for ConvertError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConvertError::Json(e) => write!(f, "JSON serialization error: {e}"),
            ConvertError::Cbor(e) => write!(f, "CBOR error: {e}"),
            ConvertError::ZstdIsNotAnEncoding => write!(
                f,
                "zstd is the compression layer, not a target encoding; \
                 use Encoding::Json or Encoding::Cbor and set compress=true"
            ),
            #[cfg(not(feature = "compression"))]
            ConvertError::CompressionNotAvailable => write!(
                f,
                "compression is not available; build with the `compression` feature"
            ),
            #[cfg(feature = "compression")]
            ConvertError::Compression(e) => write!(f, "compression error: {e}"),
        }
    }
}

impl std::error::Error for ConvertError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ConvertError::Json(e) => Some(e),
            ConvertError::Cbor(e) => Some(e),
            ConvertError::ZstdIsNotAnEncoding => None,
            #[cfg(not(feature = "compression"))]
            ConvertError::CompressionNotAvailable => None,
            #[cfg(feature = "compression")]
            ConvertError::Compression(e) => Some(e),
        }
    }
}

/// Converts an [`OmtsFile`] to the specified target encoding, optionally
/// compressing the result with zstd.
///
/// # Lossless Conversion (SPEC-007 Section 5)
///
/// Both JSON and CBOR encodings carry the same abstract model.  This function
/// re-serializes the in-memory representation into the requested format:
///
/// - All field names (including unknown extension fields) are preserved.
/// - All values and types are preserved per the mapping in SPEC-007 Section 4.2.
/// - The null-vs.-absent distinction is preserved.
/// - Array element order (`nodes`, `edges`, `identifiers`, `labels`) is preserved.
///
/// # Compression
///
/// When `compress = true` the serialized bytes are wrapped in a zstd frame
/// (SPEC-007 Section 6).  This requires the `compression` feature (enabled by
/// default).  If `compress = true` and the feature is disabled,
/// [`ConvertError::CompressionNotAvailable`] is returned.
///
/// # Errors
///
/// - [`ConvertError::ZstdIsNotAnEncoding`] — `target` is [`Encoding::Zstd`].
/// - [`ConvertError::Json`] — JSON serialization failed.
/// - [`ConvertError::Cbor`] — CBOR encoding failed.
/// - [`ConvertError::CompressionNotAvailable`] — `compress = true` but the
///   `compression` feature is disabled.
/// - [`ConvertError::Compression`] — zstd compression failed (`compression`
///   feature only).
pub fn convert(file: &OmtsFile, target: Encoding, compress: bool) -> Result<Vec<u8>, ConvertError> {
    let bytes = match target {
        Encoding::Json => serde_json::to_vec(file).map_err(ConvertError::Json)?,
        Encoding::Cbor => encode_cbor(file).map_err(ConvertError::Cbor)?,
        Encoding::Zstd => return Err(ConvertError::ZstdIsNotAnEncoding),
    };

    if compress {
        #[cfg(feature = "compression")]
        {
            return compress_zstd(&bytes).map_err(ConvertError::Compression);
        }
        #[cfg(not(feature = "compression"))]
        {
            return Err(ConvertError::CompressionNotAvailable);
        }
    }

    Ok(bytes)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use crate::cbor::decode_cbor;
    use crate::dynvalue::DynValue;

    const SALT: &str = "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef";

    fn minimal_file() -> OmtsFile {
        let json = format!(
            r#"{{"omtsf_version":"1.0.0","snapshot_date":"2026-02-19","file_salt":"{SALT}","nodes":[],"edges":[]}}"#
        );
        serde_json::from_str(&json).expect("parse minimal file")
    }

    fn file_with_nodes_and_unknown_fields() -> OmtsFile {
        let json = format!(
            r#"{{
                "omtsf_version": "1.0.0",
                "snapshot_date": "2026-02-19",
                "file_salt": "{SALT}",
                "nodes": [
                    {{"id": "org-1", "type": "organization", "name": "Acme Corp", "x_node_ext": "node-val"}},
                    {{"id": "org-2", "type": "organization", "name": "Beta Ltd"}}
                ],
                "edges": [
                    {{
                        "id": "e-1",
                        "type": "supplies",
                        "source": "org-2",
                        "target": "org-1",
                        "properties": {{"tier": 1}}
                    }}
                ],
                "x_top_string": "preserved",
                "x_top_number": 99,
                "x_top_null": null
            }}"#
        );
        serde_json::from_str(&json).expect("parse file with nodes and unknown fields")
    }

    /// JSON → CBOR → JSON round-trip produces logically equivalent output.
    #[test]
    fn json_to_cbor_to_json_round_trip() {
        let original = file_with_nodes_and_unknown_fields();

        let cbor_bytes = convert(&original, Encoding::Cbor, false).expect("convert to CBOR");
        let from_cbor = decode_cbor(&cbor_bytes).expect("decode CBOR");
        let json_bytes = convert(&from_cbor, Encoding::Json, false).expect("convert to JSON");
        let result: OmtsFile = serde_json::from_slice(&json_bytes).expect("parse JSON");

        assert_eq!(
            original, result,
            "JSON→CBOR→JSON must produce logically equivalent output"
        );
    }

    /// CBOR → JSON → CBOR round-trip produces logically equivalent output.
    #[test]
    fn cbor_to_json_to_cbor_round_trip() {
        let original = file_with_nodes_and_unknown_fields();
        let cbor_start = convert(&original, Encoding::Cbor, false).expect("initial CBOR encode");

        let decoded = decode_cbor(&cbor_start).expect("decode initial CBOR");
        let json_bytes = convert(&decoded, Encoding::Json, false).expect("convert to JSON");
        let from_json: OmtsFile = serde_json::from_slice(&json_bytes).expect("parse JSON");
        let cbor_end = convert(&from_json, Encoding::Cbor, false).expect("convert back to CBOR");
        let result = decode_cbor(&cbor_end).expect("decode final CBOR");

        assert_eq!(
            original, result,
            "CBOR→JSON→CBOR must produce logically equivalent output"
        );
    }

    /// Unknown top-level and nested fields are preserved through JSON→CBOR→JSON conversion.
    #[test]
    fn unknown_fields_preserved_through_conversion() {
        let original = file_with_nodes_and_unknown_fields();

        let cbor_bytes = convert(&original, Encoding::Cbor, false).expect("convert to CBOR");
        let decoded = decode_cbor(&cbor_bytes).expect("decode CBOR");
        let json_bytes = convert(&decoded, Encoding::Json, false).expect("convert to JSON");
        let result: OmtsFile = serde_json::from_slice(&json_bytes).expect("parse JSON");

        assert_eq!(
            result.extra.get("x_top_string").and_then(|v| v.as_str()),
            Some("preserved"),
            "top-level string unknown field must survive conversion"
        );
        assert_eq!(
            result.extra.get("x_top_number").and_then(DynValue::as_u64),
            Some(99),
            "top-level number unknown field must survive conversion"
        );
        assert!(
            result
                .extra
                .get("x_top_null")
                .is_some_and(DynValue::is_null),
            "top-level null unknown field must survive conversion"
        );
        assert_eq!(
            result.nodes[0]
                .extra
                .get("x_node_ext")
                .and_then(|v| v.as_str()),
            Some("node-val"),
            "nested unknown field in node must survive conversion"
        );
    }

    /// Array element order for nodes and edges is preserved through conversion.
    #[test]
    fn array_element_order_preserved() {
        let original = file_with_nodes_and_unknown_fields();

        let cbor_bytes = convert(&original, Encoding::Cbor, false).expect("convert to CBOR");
        let decoded = decode_cbor(&cbor_bytes).expect("decode CBOR");
        let json_bytes = convert(&decoded, Encoding::Json, false).expect("convert to JSON");
        let result: OmtsFile = serde_json::from_slice(&json_bytes).expect("parse JSON");

        let orig_node_ids: Vec<_> = original.nodes.iter().map(|n| &n.id).collect();
        let result_node_ids: Vec<_> = result.nodes.iter().map(|n| &n.id).collect();
        assert_eq!(
            orig_node_ids, result_node_ids,
            "node order must be preserved through conversion"
        );
    }

    /// `Encoding::Zstd` as a target encoding returns `ZstdIsNotAnEncoding`.
    #[test]
    fn zstd_target_encoding_returns_error() {
        let file = minimal_file();
        let result = convert(&file, Encoding::Zstd, false);
        assert!(
            matches!(result, Err(ConvertError::ZstdIsNotAnEncoding)),
            "expected ZstdIsNotAnEncoding error"
        );
    }

    /// `Encoding::Zstd` with `compress=true` still returns `ZstdIsNotAnEncoding`
    /// because target encoding is validated before the compress flag.
    #[test]
    fn zstd_target_with_compress_returns_not_an_encoding() {
        let file = minimal_file();
        let result = convert(&file, Encoding::Zstd, true);
        assert!(
            matches!(result, Err(ConvertError::ZstdIsNotAnEncoding)),
            "ZstdIsNotAnEncoding must be returned even when compress=true"
        );
    }

    /// JSON output is valid UTF-8 that starts with `{`.
    #[test]
    fn json_output_starts_with_open_brace() {
        let file = minimal_file();
        let bytes = convert(&file, Encoding::Json, false).expect("convert to JSON");
        let first_non_ws = bytes
            .iter()
            .copied()
            .find(|&b| !matches!(b, b' ' | b'\t' | b'\n' | b'\r'));
        assert_eq!(first_non_ws, Some(b'{'), "JSON output must start with {{");
    }

    /// CBOR output starts with the self-describing tag `0xD9 0xD9 0xF7`.
    #[test]
    fn cbor_output_starts_with_self_describing_tag() {
        let file = minimal_file();
        let bytes = convert(&file, Encoding::Cbor, false).expect("convert to CBOR");
        assert_eq!(
            &bytes[..3],
            &[0xD9, 0xD9, 0xF7],
            "CBOR output must start with self-describing tag 55799"
        );
    }

    /// Full-featured fixture survives JSON→CBOR→JSON conversion.
    #[test]
    fn full_fixture_json_to_cbor_to_json() {
        let fixture_json = include_str!("../../../tests/fixtures/full-featured.omts");
        let original: OmtsFile =
            serde_json::from_str(fixture_json).expect("parse full-featured fixture");

        let cbor = convert(&original, Encoding::Cbor, false).expect("convert to CBOR");
        let decoded = decode_cbor(&cbor).expect("decode CBOR");
        let json_bytes = convert(&decoded, Encoding::Json, false).expect("convert back to JSON");
        let result: OmtsFile = serde_json::from_slice(&json_bytes).expect("parse result JSON");

        assert_eq!(
            original, result,
            "full-featured fixture must survive JSON→CBOR→JSON"
        );
        assert_eq!(original.nodes.len(), result.nodes.len());
        assert_eq!(original.edges.len(), result.edges.len());
    }

    /// `ZstdIsNotAnEncoding` error display mentions zstd.
    #[test]
    fn error_display_zstd_not_an_encoding() {
        let msg = ConvertError::ZstdIsNotAnEncoding.to_string();
        assert!(
            msg.contains("zstd"),
            "error message should mention zstd: {msg}"
        );
    }

    /// Compressed JSON output starts with zstd magic bytes.
    #[cfg(feature = "compression")]
    #[test]
    fn compressed_output_starts_with_zstd_magic() {
        let file = minimal_file();
        let bytes = convert(&file, Encoding::Json, true).expect("convert to compressed JSON");
        assert_eq!(
            &bytes[..4],
            &[0x28, 0xB5, 0x2F, 0xFD],
            "compressed output must start with zstd magic"
        );
    }

    /// Compressed JSON round-trip: convert→compress→decompress→parse produces
    /// the original file.
    #[cfg(feature = "compression")]
    #[test]
    fn compressed_json_round_trip() {
        use crate::compression::decompress_zstd;

        let original = file_with_nodes_and_unknown_fields();
        let compressed =
            convert(&original, Encoding::Json, true).expect("convert to compressed JSON");
        let decompressed = decompress_zstd(&compressed, 1024 * 1024).expect("decompress");
        let result: OmtsFile =
            serde_json::from_slice(&decompressed).expect("parse decompressed JSON");

        assert_eq!(
            original, result,
            "compressed JSON round-trip must be lossless"
        );
    }

    /// Compressed CBOR round-trip: convert→compress→decompress→decode produces
    /// the original file.
    #[cfg(feature = "compression")]
    #[test]
    fn compressed_cbor_round_trip() {
        use crate::compression::decompress_zstd;

        let original = file_with_nodes_and_unknown_fields();
        let compressed =
            convert(&original, Encoding::Cbor, true).expect("convert to compressed CBOR");
        let decompressed = decompress_zstd(&compressed, 1024 * 1024).expect("decompress");
        let result = decode_cbor(&decompressed).expect("decode CBOR");

        assert_eq!(
            original, result,
            "compressed CBOR round-trip must be lossless"
        );
    }

    /// compress=true without the compression feature returns CompressionNotAvailable.
    #[cfg(not(feature = "compression"))]
    #[test]
    fn compress_without_feature_returns_error() {
        let file = minimal_file();
        let result = convert(&file, Encoding::Json, true);
        assert!(
            matches!(result, Err(ConvertError::CompressionNotAvailable)),
            "expected CompressionNotAvailable without compression feature"
        );
    }
}
