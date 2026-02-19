/// Shared structural types used across nodes and edges.
///
/// This module defines [`Identifier`], [`DataQuality`], [`Label`], and [`Geo`]
/// as specified in the OMTSF data model (data-model.md Sections 7.1–7.4).
///
/// All structs carry a `#[serde(flatten)]` `extra` field to preserve unknown
/// JSON fields across round trips. See Section 8.2 for rationale.
use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::enums::{Confidence, Sensitivity, VerificationStatus};
use crate::newtypes::CalendarDate;

// ---------------------------------------------------------------------------
// GeoParseError
// ---------------------------------------------------------------------------

/// Errors produced when parsing a raw `serde_json::Value` into a [`Geo`].
#[derive(Debug, Clone, PartialEq)]
pub enum GeoParseError {
    /// The value was `null` or not a JSON object or known geometry type.
    NotAnObject,
    /// A `{lat, lon}` point object was present but `lat` was missing or not a number.
    MissingOrInvalidLat,
    /// A `{lat, lon}` point object was present but `lon` was missing or not a number.
    MissingOrInvalidLon,
}

impl fmt::Display for GeoParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAnObject => write!(f, "geo value is not a JSON object"),
            Self::MissingOrInvalidLat => {
                write!(f, "point geo object is missing a numeric `lat` field")
            }
            Self::MissingOrInvalidLon => {
                write!(f, "point geo object is missing a numeric `lon` field")
            }
        }
    }
}

impl std::error::Error for GeoParseError {}

// ---------------------------------------------------------------------------
// Geo
// ---------------------------------------------------------------------------

/// A parsed geographical value from a facility node's `geo` field.
///
/// The raw `geo` field on [`crate::Node`] (when defined in T-005) is stored as
/// `serde_json::Value` to avoid forcing a schema during deserialization.  Use
/// [`parse_geo`] to convert that raw value into a typed `Geo`.
///
/// SPEC-001 Section 4.2 defines two accepted shapes:
/// - A simple `{lat, lon}` point object.
/// - Any valid `GeoJSON` geometry object (passed through as [`Geo::GeoJson`]).
#[derive(Debug, Clone, PartialEq)]
pub enum Geo {
    /// A simple WGS-84 point with latitude and longitude.
    Point {
        /// Latitude in decimal degrees (WGS-84).
        lat: f64,
        /// Longitude in decimal degrees (WGS-84).
        lon: f64,
    },
    /// An arbitrary `GeoJSON` geometry value (RFC 7946).
    GeoJson(Value),
}

/// Parses a raw `serde_json::Value` into a typed [`Geo`].
///
/// # Heuristic
///
/// 1. If the value is a JSON object with both `lat` and `lon` keys that are
///    JSON numbers, it is interpreted as a `Point`.
/// 2. Otherwise, if the value is any JSON object, it is passed through as
///    `GeoJson` (representing an arbitrary `GeoJSON` geometry).
/// 3. If the value is not a JSON object, [`GeoParseError::NotAnObject`] is
///    returned.
///
/// This function is intended to be called from `Node::geo_parsed()` (T-005).
///
/// # Errors
///
/// Returns [`GeoParseError`] if the value cannot be interpreted as any known
/// geo shape.
pub fn parse_geo(value: &Value) -> Result<Geo, GeoParseError> {
    let Some(obj) = value.as_object() else {
        return Err(GeoParseError::NotAnObject);
    };

    // Detect a simple {lat, lon} point: both keys must be present and numeric.
    let has_lat = obj.contains_key("lat");
    let has_lon = obj.contains_key("lon");

    if has_lat || has_lon {
        // At least one of lat/lon is present — treat as an attempted Point.
        let lat = obj
            .get("lat")
            .and_then(Value::as_f64)
            .ok_or(GeoParseError::MissingOrInvalidLat)?;
        let lon = obj
            .get("lon")
            .and_then(Value::as_f64)
            .ok_or(GeoParseError::MissingOrInvalidLon)?;
        return Ok(Geo::Point { lat, lon });
    }

    // No lat/lon keys — treat the entire object as a GeoJSON geometry.
    Ok(Geo::GeoJson(value.clone()))
}

// ---------------------------------------------------------------------------
// Identifier
// ---------------------------------------------------------------------------

/// An external or internal identifier attached to a node or edge.
///
/// Corresponds to the identifier record defined in SPEC-002 Section 3.
/// The `scheme` field accepts any string; core schemes (`lei`, `duns`, `gln`,
/// `nat-reg`, `vat`, `internal`) and arbitrary extension schemes are all valid.
/// Scheme-specific format validation belongs in the validation engine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Identifier {
    /// Identifier scheme (e.g. `"lei"`, `"duns"`, `"com.example.internal"`).
    pub scheme: String,

    /// Identifier value within the given scheme.
    pub value: String,

    /// Issuing authority for the identifier, if known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authority: Option<String>,

    /// Date from which this identifier is valid.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub valid_from: Option<CalendarDate>,

    /// Date on which this identifier ceases to be valid.
    ///
    /// `None` = field absent (not provided).
    /// `Some(None)` = explicit `null` in JSON (identifier has no expiry).
    /// `Some(Some(date))` = expires on the given date.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::serde_helpers::deserialize_optional_nullable"
    )]
    pub valid_to: Option<Option<CalendarDate>>,

    /// Sensitivity classification for this identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sensitivity: Option<Sensitivity>,

    /// Verification status of this identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verification_status: Option<VerificationStatus>,

    /// Date on which this identifier was last verified.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verification_date: Option<CalendarDate>,

    /// Unknown fields preserved for round-trip fidelity (SPEC-001 Section 2.2).
    #[serde(flatten)]
    pub extra: serde_json::Map<String, Value>,
}

// ---------------------------------------------------------------------------
// DataQuality
// ---------------------------------------------------------------------------

/// Data quality metadata attached to a node or edge.
///
/// Corresponds to the `data_quality` property defined in SPEC-001 Section 8.3.
/// All fields are optional; an empty `DataQuality` object is valid.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct DataQuality {
    /// Confidence level of the data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<Confidence>,

    /// Human-readable description of the data source.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,

    /// Date on which the data was last independently verified.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_verified: Option<CalendarDate>,

    /// Unknown fields preserved for round-trip fidelity.
    #[serde(flatten)]
    pub extra: serde_json::Map<String, Value>,
}

// ---------------------------------------------------------------------------
// Label
// ---------------------------------------------------------------------------

/// A key/value label attached to a node or edge for tagging and filtering.
///
/// Corresponds to the label record defined in SPEC-001 Section 8.4.
/// Labels are free-form; the `key` is required, `value` is optional.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Label {
    /// Label key (required, non-empty by convention).
    pub key: String,

    /// Label value, or `None` if the label is a boolean flag.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,

    /// Unknown fields preserved for round-trip fidelity.
    #[serde(flatten)]
    pub extra: serde_json::Map<String, Value>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use serde_json::json;

    use super::*;

    // --- helpers ------------------------------------------------------------

    fn to_json<T: Serialize>(v: &T) -> String {
        serde_json::to_string(v).expect("serialize")
    }

    fn from_json<T: for<'de> Deserialize<'de>>(s: &str) -> T {
        serde_json::from_str(s).expect("deserialize")
    }

    fn round_trip<T>(v: &T) -> T
    where
        T: Serialize + for<'de> Deserialize<'de> + fmt::Debug + PartialEq,
    {
        let json = to_json(v);
        let back: T = from_json(&json);
        assert_eq!(*v, back, "round-trip mismatch for {json}");
        back
    }

    // --- Identifier ---------------------------------------------------------

    #[test]
    fn identifier_minimal_round_trip() {
        let id = Identifier {
            scheme: "lei".to_owned(),
            value: "529900T8BM49AURSDO55".to_owned(),
            authority: None,
            valid_from: None,
            valid_to: None,
            sensitivity: None,
            verification_status: None,
            verification_date: None,
            extra: serde_json::Map::new(),
        };
        round_trip(&id);
    }

    #[test]
    fn identifier_serializes_required_fields() {
        let id = Identifier {
            scheme: "duns".to_owned(),
            value: "123456789".to_owned(),
            authority: None,
            valid_from: None,
            valid_to: None,
            sensitivity: None,
            verification_status: None,
            verification_date: None,
            extra: serde_json::Map::new(),
        };
        let json = to_json(&id);
        assert!(json.contains(r#""scheme":"duns""#));
        assert!(json.contains(r#""value":"123456789""#));
    }

    #[test]
    fn identifier_optional_fields_omitted_when_none() {
        let id = Identifier {
            scheme: "gln".to_owned(),
            value: "1234567890123".to_owned(),
            authority: None,
            valid_from: None,
            valid_to: None,
            sensitivity: None,
            verification_status: None,
            verification_date: None,
            extra: serde_json::Map::new(),
        };
        let json = to_json(&id);
        assert!(!json.contains("authority"));
        assert!(!json.contains("valid_from"));
        assert!(!json.contains("valid_to"));
        assert!(!json.contains("sensitivity"));
    }

    #[test]
    fn identifier_full_round_trip() {
        let raw = r#"{
            "scheme": "lei",
            "value": "529900T8BM49AURSDO55",
            "authority": "GLEIF",
            "valid_from": "2020-01-01",
            "sensitivity": "public",
            "verification_status": "verified",
            "verification_date": "2026-01-15"
        }"#;
        let id: Identifier = serde_json::from_str(raw).expect("deserialize");
        assert_eq!(id.scheme, "lei");
        assert_eq!(id.authority.as_deref(), Some("GLEIF"));
        assert_eq!(id.sensitivity, Some(Sensitivity::Public));
        round_trip(&id);
    }

    #[test]
    fn identifier_valid_to_null_vs_absent() {
        // valid_to absent → outer Option is None
        let raw_absent = r#"{"scheme":"lei","value":"abc"}"#;
        let id_absent: Identifier = serde_json::from_str(raw_absent).expect("deserialize absent");
        assert_eq!(id_absent.valid_to, None);

        // valid_to: null → outer Option is Some(None)
        let raw_null = r#"{"scheme":"lei","value":"abc","valid_to":null}"#;
        let id_null: Identifier = serde_json::from_str(raw_null).expect("deserialize null");
        assert_eq!(id_null.valid_to, Some(None));

        // valid_to: "2030-01-01" → Some(Some(date))
        let raw_date = r#"{"scheme":"lei","value":"abc","valid_to":"2030-01-01"}"#;
        let id_date: Identifier = serde_json::from_str(raw_date).expect("deserialize date");
        assert!(id_date.valid_to.is_some());
        assert!(
            id_date
                .valid_to
                .as_ref()
                .expect("valid_to should be Some")
                .is_some()
        );
    }

    #[test]
    fn identifier_preserves_extra_fields() {
        let raw = r#"{"scheme":"internal","value":"x1","x_custom_field":"hello"}"#;
        let id: Identifier = serde_json::from_str(raw).expect("deserialize");
        assert_eq!(
            id.extra.get("x_custom_field").and_then(|v| v.as_str()),
            Some("hello")
        );
        let re_serialized = to_json(&id);
        assert!(re_serialized.contains("x_custom_field"));
    }

    // --- DataQuality --------------------------------------------------------

    #[test]
    fn data_quality_empty_round_trip() {
        let dq = DataQuality {
            confidence: None,
            source: None,
            last_verified: None,
            extra: serde_json::Map::new(),
        };
        round_trip(&dq);
    }

    #[test]
    fn data_quality_all_fields_round_trip() {
        let raw = r#"{
            "confidence": "verified",
            "source": "Annual report 2025",
            "last_verified": "2025-12-31"
        }"#;
        let dq: DataQuality = serde_json::from_str(raw).expect("deserialize");
        assert_eq!(dq.confidence, Some(Confidence::Verified));
        assert_eq!(dq.source.as_deref(), Some("Annual report 2025"));
        round_trip(&dq);
    }

    #[test]
    fn data_quality_optional_fields_omitted_when_none() {
        let dq = DataQuality {
            confidence: None,
            source: None,
            last_verified: None,
            extra: serde_json::Map::new(),
        };
        let json = to_json(&dq);
        assert!(!json.contains("confidence"));
        assert!(!json.contains("source"));
        assert!(!json.contains("last_verified"));
    }

    #[test]
    fn data_quality_preserves_extra_fields() {
        let raw = r#"{"confidence":"reported","x_reviewer":"alice"}"#;
        let dq: DataQuality = serde_json::from_str(raw).expect("deserialize");
        assert_eq!(
            dq.extra.get("x_reviewer").and_then(|v| v.as_str()),
            Some("alice")
        );
        let re_serialized = to_json(&dq);
        assert!(re_serialized.contains("x_reviewer"));
    }

    // --- Label --------------------------------------------------------------

    #[test]
    fn label_key_only_round_trip() {
        let label = Label {
            key: "environment".to_owned(),
            value: None,
            extra: serde_json::Map::new(),
        };
        round_trip(&label);
    }

    #[test]
    fn label_key_value_round_trip() {
        let label = Label {
            key: "tier".to_owned(),
            value: Some("1".to_owned()),
            extra: serde_json::Map::new(),
        };
        let json = to_json(&label);
        assert!(json.contains(r#""key":"tier""#));
        assert!(json.contains(r#""value":"1""#));
        round_trip(&label);
    }

    #[test]
    fn label_value_omitted_when_none() {
        let label = Label {
            key: "flag".to_owned(),
            value: None,
            extra: serde_json::Map::new(),
        };
        let json = to_json(&label);
        assert!(!json.contains("value"));
    }

    #[test]
    fn label_preserves_extra_fields() {
        let raw = r#"{"key":"env","value":"prod","x_source":"manual"}"#;
        let label: Label = serde_json::from_str(raw).expect("deserialize");
        assert_eq!(
            label.extra.get("x_source").and_then(|v| v.as_str()),
            Some("manual")
        );
        let re_serialized = to_json(&label);
        assert!(re_serialized.contains("x_source"));
    }

    // --- Geo / parse_geo ----------------------------------------------------

    #[test]
    fn parse_geo_point_valid() {
        let val = json!({"lat": 51.5074, "lon": -0.1278});
        let geo = parse_geo(&val).expect("valid point");
        assert_eq!(
            geo,
            Geo::Point {
                lat: 51.5074,
                lon: -0.1278
            }
        );
    }

    #[test]
    fn parse_geo_point_zero_zero() {
        let val = json!({"lat": 0.0, "lon": 0.0});
        let geo = parse_geo(&val).expect("origin point");
        assert_eq!(geo, Geo::Point { lat: 0.0, lon: 0.0 });
    }

    #[test]
    fn parse_geo_point_negative_coords() {
        let val = json!({"lat": -33.8688, "lon": 151.2093});
        let geo = parse_geo(&val).expect("Sydney");
        assert_eq!(
            geo,
            Geo::Point {
                lat: -33.8688,
                lon: 151.2093
            }
        );
    }

    #[test]
    fn parse_geo_point_missing_lon() {
        let val = json!({"lat": 51.5074});
        let err = parse_geo(&val).expect_err("lon missing");
        assert_eq!(err, GeoParseError::MissingOrInvalidLon);
    }

    #[test]
    fn parse_geo_point_missing_lat() {
        let val = json!({"lon": -0.1278});
        let err = parse_geo(&val).expect_err("lat missing");
        assert_eq!(err, GeoParseError::MissingOrInvalidLat);
    }

    #[test]
    fn parse_geo_point_lat_not_number() {
        let val = json!({"lat": "51.5074", "lon": -0.1278});
        let err = parse_geo(&val).expect_err("lat is string");
        assert_eq!(err, GeoParseError::MissingOrInvalidLat);
    }

    #[test]
    fn parse_geo_point_lon_not_number() {
        let val = json!({"lat": 51.5074, "lon": "west"});
        let err = parse_geo(&val).expect_err("lon is string");
        assert_eq!(err, GeoParseError::MissingOrInvalidLon);
    }

    #[test]
    fn parse_geo_geojson_polygon() {
        let val = json!({
            "type": "Polygon",
            "coordinates": [[[100.0, 0.0],[101.0, 0.0],[101.0, 1.0],[100.0, 1.0],[100.0, 0.0]]]
        });
        let geo = parse_geo(&val).expect("GeoJSON polygon");
        assert!(matches!(geo, Geo::GeoJson(_)));
    }

    #[test]
    fn parse_geo_geojson_point_geojson_format() {
        // A GeoJSON Point (with "type": "Point") — no lat/lon keys, treated as GeoJson.
        let val = json!({"type": "Point", "coordinates": [125.6, 10.1]});
        let geo = parse_geo(&val).expect("GeoJSON Point geometry");
        assert!(matches!(geo, Geo::GeoJson(_)));
    }

    #[test]
    fn parse_geo_not_object_null() {
        let err = parse_geo(&Value::Null).expect_err("null");
        assert_eq!(err, GeoParseError::NotAnObject);
    }

    #[test]
    fn parse_geo_not_object_string() {
        let err = parse_geo(&json!("not an object")).expect_err("string");
        assert_eq!(err, GeoParseError::NotAnObject);
    }

    #[test]
    fn parse_geo_not_object_array() {
        let err = parse_geo(&json!([1.0, 2.0])).expect_err("array");
        assert_eq!(err, GeoParseError::NotAnObject);
    }

    #[test]
    fn geo_parse_error_display() {
        assert!(!GeoParseError::NotAnObject.to_string().is_empty());
        assert!(!GeoParseError::MissingOrInvalidLat.to_string().is_empty());
        assert!(!GeoParseError::MissingOrInvalidLon.to_string().is_empty());
    }

    #[test]
    fn geo_parse_error_is_std_error() {
        let err: Box<dyn std::error::Error> = Box::new(GeoParseError::NotAnObject);
        assert!(!err.to_string().is_empty());
    }
}
