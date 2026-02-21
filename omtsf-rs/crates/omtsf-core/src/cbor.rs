//! CBOR serialization and deserialization for [`OmtsFile`].
//!
//! Implements the CBOR binding defined in SPEC-007 Section 4:
//! - Encodes with self-describing tag 55799 (`0xD9 0xD9 0xF7`) prepended.
//! - All map keys are CBOR text strings (major type 3).
//! - Date fields are text strings in `YYYY-MM-DD` form, not CBOR date tags.
//! - CBOR byte strings (major type 2) are rejected on decode.
//!
//! The implementation converts through [`serde_json::Value`] as an intermediate
//! representation to avoid compatibility issues between ciborium's serde backend
//! and the `#[serde(flatten)]` annotation on [`OmtsFile::extra`].

use ciborium::value::Value as CborValue;
use serde_json::{Map, Number as JsonNumber, Value as JsonValue};

use crate::OmtsFile;

/// CBOR self-describing tag number (RFC 8949 Section 3.4.6).
const SELF_DESCRIBING_TAG: u64 = 55799;

/// Error produced by CBOR encoding and decoding operations.
#[derive(Debug)]
pub enum CborError {
    /// Encoding the value to CBOR bytes failed.
    Encode(String),
    /// Decoding the CBOR bytes failed.
    Decode(String),
    /// A CBOR map contains a key that is not a text string (SPEC-007 Section 4.1).
    NonTextKey,
    /// A CBOR value type has no JSON equivalent and cannot be carried in an
    /// [`OmtsFile`] (e.g. a CBOR byte string; SPEC-007 Section 4.1).
    UnsupportedValue(String),
    /// The JSON value produced during decoding did not satisfy the [`OmtsFile`]
    /// schema.
    JsonConversion(serde_json::Error),
}

impl std::fmt::Display for CborError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CborError::Encode(msg) => write!(f, "CBOR encode error: {msg}"),
            CborError::Decode(msg) => write!(f, "CBOR decode error: {msg}"),
            CborError::NonTextKey => write!(f, "CBOR map key is not a text string"),
            CborError::UnsupportedValue(desc) => write!(f, "unsupported CBOR value: {desc}"),
            CborError::JsonConversion(e) => write!(f, "JSON conversion error: {e}"),
        }
    }
}

impl std::error::Error for CborError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            CborError::JsonConversion(e) => Some(e),
            CborError::Encode(_)
            | CborError::Decode(_)
            | CborError::NonTextKey
            | CborError::UnsupportedValue(_) => None,
        }
    }
}

/// Encodes an [`OmtsFile`] to CBOR bytes.
///
/// Prepends the self-describing CBOR tag 55799 (`0xD9 0xD9 0xF7`) so that
/// format detection per SPEC-007 Section 2 works without extra context.
/// All map keys are emitted as CBOR text strings; date fields are text strings
/// in `YYYY-MM-DD` form per SPEC-007 Section 4.2.
pub fn encode_cbor(file: &OmtsFile) -> Result<Vec<u8>, CborError> {
    let json_value = serde_json::to_value(file).map_err(CborError::JsonConversion)?;
    let cbor_value = json_to_cbor(json_value)?;
    let tagged = CborValue::Tag(SELF_DESCRIBING_TAG, Box::new(cbor_value));
    let mut buf = Vec::new();
    ciborium::into_writer(&tagged, &mut buf).map_err(|e| CborError::Encode(e.to_string()))?;
    Ok(buf)
}

/// Decodes CBOR bytes into an [`OmtsFile`].
///
/// Accepts bytes with or without the self-describing tag 55799 per SPEC-007
/// Section 4.1.  All map keys must be CBOR text strings; CBOR byte strings are
/// rejected because they have no JSON equivalent (Section 4.1).
pub fn decode_cbor(bytes: &[u8]) -> Result<OmtsFile, CborError> {
    let cbor_value: CborValue =
        ciborium::from_reader(bytes).map_err(|e| CborError::Decode(e.to_string()))?;

    // Strip the self-describing tag if the encoder added it.
    let inner = if let CborValue::Tag(SELF_DESCRIBING_TAG, inner) = cbor_value {
        *inner
    } else {
        cbor_value
    };

    let json_value = cbor_to_json(inner)?;
    serde_json::from_value(json_value).map_err(CborError::JsonConversion)
}

/// Converts a [`JsonValue`] to a [`CborValue`], mapping all JSON types per
/// SPEC-007 Section 4.2.  Object keys become CBOR text strings.
fn json_to_cbor(value: JsonValue) -> Result<CborValue, CborError> {
    match value {
        JsonValue::Null => Ok(CborValue::Null),
        JsonValue::Bool(b) => Ok(CborValue::Bool(b)),
        JsonValue::Number(n) => number_to_cbor(&n),
        JsonValue::String(s) => Ok(CborValue::Text(s)),
        JsonValue::Array(arr) => {
            let items: Result<Vec<_>, _> = arr.into_iter().map(json_to_cbor).collect();
            Ok(CborValue::Array(items?))
        }
        JsonValue::Object(map) => {
            let pairs: Result<Vec<_>, _> = map
                .into_iter()
                .map(|(k, v)| json_to_cbor(v).map(|cv| (CborValue::Text(k), cv)))
                .collect();
            Ok(CborValue::Map(pairs?))
        }
    }
}

/// Converts a JSON number to a CBOR integer or float.
///
/// Integer-valued JSON numbers are preferred as CBOR integers to preserve
/// round-trip fidelity.  Float-valued JSON numbers become CBOR floats.
fn number_to_cbor(n: &JsonNumber) -> Result<CborValue, CborError> {
    if let Some(i) = n.as_i64() {
        Ok(CborValue::Integer(i.into()))
    } else if let Some(u) = n.as_u64() {
        Ok(CborValue::Integer(u.into()))
    } else if let Some(f) = n.as_f64() {
        Ok(CborValue::Float(f))
    } else {
        Err(CborError::UnsupportedValue(format!(
            "JSON number {n} cannot be represented in CBOR"
        )))
    }
}

/// Converts a [`CborValue`] to a [`JsonValue`].
///
/// CBOR tags are unwrapped recursively (the self-describing tag 55799 and any
/// others).  CBOR byte strings are rejected per SPEC-007 Section 4.1.
fn cbor_to_json(value: CborValue) -> Result<JsonValue, CborError> {
    match value {
        CborValue::Null => Ok(JsonValue::Null),
        CborValue::Bool(b) => Ok(JsonValue::Bool(b)),
        CborValue::Integer(i) => integer_to_json(i),
        CborValue::Float(f) => float_to_json(f),
        CborValue::Text(s) => Ok(JsonValue::String(s)),
        CborValue::Bytes(b) => Err(CborError::UnsupportedValue(format!(
            "CBOR byte string of length {} has no JSON equivalent",
            b.len()
        ))),
        CborValue::Array(arr) => {
            let items: Result<Vec<_>, _> = arr.into_iter().map(cbor_to_json).collect();
            Ok(JsonValue::Array(items?))
        }
        CborValue::Map(map) => map_to_json(map),
        // Unwrap any CBOR tag and recurse; this handles the self-describing
        // tag 55799 when it appears on nested values, and gracefully ignores
        // other application-specific tags.
        CborValue::Tag(_, inner) => cbor_to_json(*inner),
        // ciborium::Value is #[non_exhaustive]; reject any future variants.
        _ => Err(CborError::UnsupportedValue(
            "unknown CBOR value type".to_owned(),
        )),
    }
}

/// Converts a CBOR integer to a JSON number, preferring `u64` for non-negative
/// values and falling back to `i64` for negatives.
fn integer_to_json(i: ciborium::value::Integer) -> Result<JsonValue, CborError> {
    let n: i128 = i.into();
    if let Ok(u) = u64::try_from(n) {
        Ok(JsonValue::Number(u.into()))
    } else if let Ok(signed) = i64::try_from(n) {
        Ok(JsonValue::Number(signed.into()))
    } else {
        Err(CborError::UnsupportedValue(format!(
            "CBOR integer {n} is out of i64/u64 range"
        )))
    }
}

/// Converts a CBOR float to a JSON number, rejecting non-finite values that
/// JSON cannot represent.
fn float_to_json(f: f64) -> Result<JsonValue, CborError> {
    JsonNumber::from_f64(f)
        .map(JsonValue::Number)
        .ok_or_else(|| {
            CborError::UnsupportedValue(format!("non-finite float {f} has no JSON representation"))
        })
}

/// Converts a CBOR map to a JSON object, requiring all keys to be text strings.
fn map_to_json(map: Vec<(CborValue, CborValue)>) -> Result<JsonValue, CborError> {
    let mut obj = Map::new();
    for (key, val) in map {
        let CborValue::Text(key_str) = key else {
            return Err(CborError::NonTextKey);
        };
        obj.insert(key_str, cbor_to_json(val)?);
    }
    Ok(JsonValue::Object(obj))
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
        // Strip the three self-describing tag bytes.
        let without_tag = &cbor[3..];
        let decoded = decode_cbor(without_tag).expect("decode without tag");
        assert_eq!(file, decoded);
    }

    /// Unknown top-level fields are preserved through a CBOR round-trip.
    #[test]
    fn round_trip_unknown_fields_preserved() {
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
            decoded
                .extra
                .get("x_number")
                .and_then(serde_json::Value::as_u64),
            Some(42)
        );
        assert_eq!(
            decoded
                .extra
                .get("x_bool")
                .and_then(serde_json::Value::as_bool),
            Some(true)
        );
        assert!(
            decoded
                .extra
                .get("x_null")
                .is_some_and(serde_json::Value::is_null)
        );
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

        // Also verify via JSON value comparison to catch any subtle serde differences.
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

    /// CBOR with a non-text map key is rejected.
    #[test]
    fn decode_non_text_key_returns_error() {
        // Construct a CBOR map with an integer key: {1: "value"}
        let cbor_map = CborValue::Map(vec![(
            CborValue::Integer(1i64.into()),
            CborValue::Text("value".to_owned()),
        )]);
        let mut buf = Vec::new();
        ciborium::into_writer(&cbor_map, &mut buf).expect("encode");
        let err = decode_cbor(&buf).expect_err("should fail on integer key");
        assert!(
            matches!(err, CborError::NonTextKey | CborError::JsonConversion(_)),
            "unexpected error variant: {err}"
        );
    }
}
