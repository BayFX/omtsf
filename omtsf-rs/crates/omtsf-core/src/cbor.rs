//! CBOR serialization and deserialization for [`OmtsFile`].
//!
//! Implements the CBOR binding defined in SPEC-007 Section 4:
//! - Encodes with self-describing tag 55799 (`0xD9 0xD9 0xF7`) prepended.
//! - All map keys are CBOR text strings (major type 3).
//! - Date fields are text strings in `YYYY-MM-DD` form, not CBOR date tags.
//!
//! Because [`OmtsFile`] now uses [`crate::DynValue`] for its `extra` fields
//! instead of `serde_json::Value`, the data model can be serialized directly
//! with ciborium's serde backend without an intermediate JSON representation.

use crate::OmtsFile;

/// Self-describing CBOR tag 55799 bytes (RFC 8949 Section 3.4.6).
const SELF_DESCRIBING_TAG_BYTES: [u8; 3] = [0xD9, 0xD9, 0xF7];

/// Error produced by CBOR encoding and decoding operations.
#[derive(Debug)]
pub enum CborError {
    /// Encoding the value to CBOR bytes failed.
    Encode(String),
    /// Decoding the CBOR bytes failed.
    Decode(String),
}

impl std::fmt::Display for CborError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CborError::Encode(msg) => write!(f, "CBOR encode error: {msg}"),
            CborError::Decode(msg) => write!(f, "CBOR decode error: {msg}"),
        }
    }
}

impl std::error::Error for CborError {}

/// Encodes an [`OmtsFile`] to CBOR bytes.
///
/// Prepends the self-describing CBOR tag 55799 (`0xD9 0xD9 0xF7`) so that
/// format detection per SPEC-007 Section 2 works without extra context.
/// All map keys are emitted as CBOR text strings; date fields are text strings
/// in `YYYY-MM-DD` form per SPEC-007 Section 4.2.
pub fn encode_cbor(file: &OmtsFile) -> Result<Vec<u8>, CborError> {
    let mut buf = Vec::from(SELF_DESCRIBING_TAG_BYTES);
    ciborium::into_writer(file, &mut buf).map_err(|e| CborError::Encode(e.to_string()))?;
    Ok(buf)
}

/// Decodes CBOR bytes into an [`OmtsFile`].
///
/// Accepts bytes with or without the self-describing tag 55799 per SPEC-007
/// Section 4.1.
pub fn decode_cbor(bytes: &[u8]) -> Result<OmtsFile, CborError> {
    let payload = if bytes.starts_with(&SELF_DESCRIBING_TAG_BYTES) {
        &bytes[3..]
    } else {
        bytes
    };
    ciborium::from_reader(payload).map_err(|e| CborError::Decode(e.to_string()))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    const SALT: &str = "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef";

    fn minimal_file() -> OmtsFile {
        let json = format!(
            r#"{{"omtsf_version":"1.0.0","snapshot_date":"2026-02-19","file_salt":"{SALT}","nodes":[],"edges":[]}}"#
        );
        serde_json::from_str(&json).expect("parse minimal file")
    }

    /// Encoded output starts with the three self-describing tag bytes.
    #[test]
    fn encode_starts_with_self_describing_tag() {
        let cbor = encode_cbor(&minimal_file()).expect("encode");
        assert_eq!(
            &cbor[..3],
            &[0xD9, 0xD9, 0xF7],
            "first three bytes must be the CBOR self-describing tag 55799"
        );
    }

    /// Minimal file round-trips through CBOR without data loss.
    #[test]
    fn round_trip_minimal_file() {
        let original = minimal_file();
        let cbor = encode_cbor(&original).expect("encode");
        let decoded = decode_cbor(&cbor).expect("decode");
        assert_eq!(original, decoded);
    }

    /// Decoder accepts CBOR bytes that do not carry the self-describing tag.
    #[test]
    fn decode_without_tag_accepted() {
        let file = minimal_file();
        let cbor = encode_cbor(&file).expect("encode");
        let without_tag = &cbor[3..];
        let decoded = decode_cbor(without_tag).expect("decode without tag");
        assert_eq!(file, decoded);
    }

    /// Unknown top-level fields are preserved through a CBOR round-trip.
    #[test]
    fn round_trip_unknown_fields_preserved() {
        use crate::DynValue;
        let json = format!(
            r#"{{
                "omtsf_version": "1.0.0",
                "snapshot_date": "2026-02-19",
                "file_salt": "{SALT}",
                "nodes": [],
                "edges": [],
                "x_string": "hello",
                "x_number": 42,
                "x_bool": true,
                "x_null": null,
                "x_array": [1, 2, 3],
                "x_object": {{"nested": "value"}}
            }}"#
        );
        let original: OmtsFile = serde_json::from_str(&json).expect("parse");
        let cbor = encode_cbor(&original).expect("encode");
        let decoded = decode_cbor(&cbor).expect("decode");

        assert_eq!(
            original, decoded,
            "OmtsFile structs must match after CBOR round-trip"
        );
        assert_eq!(
            decoded.extra.get("x_string").and_then(|v| v.as_str()),
            Some("hello")
        );
        assert_eq!(
            decoded.extra.get("x_number").and_then(DynValue::as_u64),
            Some(42)
        );
        assert_eq!(
            decoded.extra.get("x_bool").and_then(DynValue::as_bool),
            Some(true)
        );
        assert!(decoded.extra.get("x_null").is_some_and(DynValue::is_null));
        assert_eq!(
            decoded
                .extra
                .get("x_object")
                .and_then(|v| v.get("nested"))
                .and_then(|v| v.as_str()),
            Some("value")
        );
    }

    /// Full-featured JSON fixture round-trips through CBOR: parse → encode
    /// CBOR → decode CBOR → re-encode JSON → compare with original.
    #[test]
    fn round_trip_full_featured_fixture() {
        let fixture_json = include_str!("../../../tests/fixtures/full-featured.omts");
        let original: OmtsFile =
            serde_json::from_str(fixture_json).expect("parse full-featured fixture");

        let cbor = encode_cbor(&original).expect("encode CBOR");
        let decoded = decode_cbor(&cbor).expect("decode CBOR");

        assert_eq!(
            original, decoded,
            "OmtsFile must be identical after CBOR round-trip"
        );

        let original_json = serde_json::to_value(&original).expect("re-encode original");
        let decoded_json = serde_json::to_value(&decoded).expect("re-encode decoded");
        assert_eq!(original_json, decoded_json);
    }

    /// File with nodes and edges (including floats) round-trips correctly.
    #[test]
    fn round_trip_with_nodes_and_edges() {
        let json = format!(
            r#"{{
                "omtsf_version": "1.0.0",
                "snapshot_date": "2026-02-19",
                "file_salt": "{SALT}",
                "disclosure_scope": "internal",
                "snapshot_sequence": 1,
                "nodes": [
                    {{"id": "org-1", "type": "organization", "name": "Acme Corp"}},
                    {{"id": "org-2", "type": "organization", "name": "Beta Ltd"}}
                ],
                "edges": [
                    {{
                        "id": "e-1",
                        "type": "ownership",
                        "source": "org-2",
                        "target": "org-1",
                        "properties": {{"percentage": 51.0, "direct": true}}
                    }}
                ]
            }}"#
        );
        let original: OmtsFile = serde_json::from_str(&json).expect("parse");
        let cbor = encode_cbor(&original).expect("encode");
        let decoded = decode_cbor(&cbor).expect("decode");
        assert_eq!(original, decoded);
        assert_eq!(decoded.nodes.len(), 2);
        assert_eq!(decoded.edges.len(), 1);
    }

    /// `omtsf_version` key is present in the decoded file (SPEC-007 Section 4.6).
    #[test]
    fn decoded_file_has_omtsf_version() {
        let file = minimal_file();
        let cbor = encode_cbor(&file).expect("encode");
        let decoded = decode_cbor(&cbor).expect("decode");
        assert_eq!(decoded.omtsf_version, file.omtsf_version);
    }

    /// Invalid CBOR bytes return a decode error.
    #[test]
    fn decode_invalid_bytes_returns_error() {
        let result = decode_cbor(&[0xFF, 0x00, 0x01]);
        assert!(result.is_err(), "invalid CBOR should return an error");
    }
}
