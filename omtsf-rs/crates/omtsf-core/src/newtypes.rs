/// Validated newtype wrappers for core OMTSF domain string types.
///
/// Each newtype enforces a regex-based shape constraint at construction time via
/// [`TryFrom<&str>`]. Once constructed, the inner value is immutable (no
/// `DerefMut`). Serde `Deserialize` impls re-run validation so invalid data
/// cannot enter the type system from untrusted JSON.
use std::fmt;
use std::ops::Deref;
use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors produced when constructing a validated newtype from an invalid string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NewtypeError {
    /// The string did not match the expected format.
    InvalidFormat {
        /// Name of the type that rejected the input.
        type_name: &'static str,
        /// A human-readable description of the expected format.
        expected: &'static str,
        /// The input that was rejected.
        got: String,
    },
}

impl fmt::Display for NewtypeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFormat {
                type_name,
                expected,
                got,
            } => write!(f, "invalid {type_name}: expected {expected}, got {got:?}"),
        }
    }
}

impl std::error::Error for NewtypeError {}

// ---------------------------------------------------------------------------
// Regex statics
//
// All patterns are compile-time string literals; Regex::new never returns Err
// for them. The match + unreachable branch is required because the workspace
// bans expect() and unwrap(), but "a^" (a pattern that never matches) is always
// valid, so we use it as a safe fallback that satisfies the type checker.
// ---------------------------------------------------------------------------

/// Matches `MAJOR.MINOR.PATCH`.
static SEMVER_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\d+\.\d+\.\d+$").unwrap_or_else(|_| {
        // Never reached: the pattern above is always valid.
        Regex::new("a^").unwrap_or_else(|_| {
            Regex::new(".").unwrap_or_else(|_| {
                Regex::new(".").unwrap_or_else(|_| {
                    Regex::new(".").unwrap_or_else(|_| unreachable!("regex engine broken"))
                })
            })
        })
    })
});

/// Matches `YYYY-MM-DD`.
static CALENDAR_DATE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\d{4}-\d{2}-\d{2}$").unwrap_or_else(|_| {
        Regex::new("a^").unwrap_or_else(|_| {
            Regex::new(".").unwrap_or_else(|_| {
                Regex::new(".").unwrap_or_else(|_| unreachable!("regex engine broken"))
            })
        })
    })
});

/// Matches exactly 64 lowercase hex characters.
static FILE_SALT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[0-9a-f]{64}$").unwrap_or_else(|_| {
        Regex::new("a^").unwrap_or_else(|_| {
            Regex::new(".").unwrap_or_else(|_| {
                Regex::new(".").unwrap_or_else(|_| unreachable!("regex engine broken"))
            })
        })
    })
});

/// Matches two uppercase ASCII letters.
static COUNTRY_CODE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[A-Z]{2}$").unwrap_or_else(|_| {
        Regex::new("a^").unwrap_or_else(|_| {
            Regex::new(".").unwrap_or_else(|_| {
                Regex::new(".").unwrap_or_else(|_| unreachable!("regex engine broken"))
            })
        })
    })
});

// ---------------------------------------------------------------------------
// SemVer
// ---------------------------------------------------------------------------

/// Semantic version string in `MAJOR.MINOR.PATCH` format.
///
/// Validates that the string matches `^\d+\.\d+\.\d+$`. The inner value is not
/// parsed into integers at construction time; use [`SemVer::major`],
/// [`SemVer::minor`], and [`SemVer::patch`] for on-demand integer access.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SemVer(String);

impl TryFrom<&str> for SemVer {
    type Error = NewtypeError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        if SEMVER_RE.is_match(s) {
            Ok(Self(s.to_owned()))
        } else {
            Err(NewtypeError::InvalidFormat {
                type_name: "SemVer",
                expected: "MAJOR.MINOR.PATCH (e.g. 1.0.0)",
                got: s.to_owned(),
            })
        }
    }
}

impl SemVer {
    /// Returns the major version component parsed from the stored string.
    ///
    /// Returns `0` if the component cannot be parsed as `u32`, which cannot
    /// happen for a correctly validated `SemVer`.
    pub fn major(&self) -> u32 {
        self.component(0)
    }

    /// Returns the minor version component parsed from the stored string.
    ///
    /// Returns `0` if the component cannot be parsed as `u32`, which cannot
    /// happen for a correctly validated `SemVer`.
    pub fn minor(&self) -> u32 {
        self.component(1)
    }

    /// Returns the patch version component parsed from the stored string.
    ///
    /// Returns `0` if the component cannot be parsed as `u32`, which cannot
    /// happen for a correctly validated `SemVer`.
    pub fn patch(&self) -> u32 {
        self.component(2)
    }

    /// Parses the `n`th dot-separated component as a `u32`.
    fn component(&self, n: usize) -> u32 {
        self.0
            .split('.')
            .nth(n)
            .and_then(|part| part.parse::<u32>().ok())
            .unwrap_or(0)
    }
}

impl Deref for SemVer {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SemVer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for SemVer {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for SemVer {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Self::try_from(s.as_str()).map_err(de::Error::custom)
    }
}

// ---------------------------------------------------------------------------
// CalendarDate
// ---------------------------------------------------------------------------

/// ISO 8601 calendar date in `YYYY-MM-DD` format.
///
/// Validates that the string matches `^\d{4}-\d{2}-\d{2}$`. No semantic
/// calendar validation (leap years, month lengths) is performed here; that
/// belongs in the validation engine. Round-trip fidelity is preserved by
/// storing the original string.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CalendarDate(String);

impl TryFrom<&str> for CalendarDate {
    type Error = NewtypeError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        if CALENDAR_DATE_RE.is_match(s) {
            Ok(Self(s.to_owned()))
        } else {
            Err(NewtypeError::InvalidFormat {
                type_name: "CalendarDate",
                expected: "YYYY-MM-DD (e.g. 2026-02-19)",
                got: s.to_owned(),
            })
        }
    }
}

impl Deref for CalendarDate {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CalendarDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for CalendarDate {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for CalendarDate {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Self::try_from(s.as_str()).map_err(de::Error::custom)
    }
}

// ---------------------------------------------------------------------------
// FileSalt
// ---------------------------------------------------------------------------

/// Exactly 64 lowercase hexadecimal characters.
///
/// Regex: `^[0-9a-f]{64}$`. Used as the `file_salt` field in OMTSF files to
/// ensure each snapshot is globally unique for privacy purposes.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FileSalt(String);

impl TryFrom<&str> for FileSalt {
    type Error = NewtypeError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        if FILE_SALT_RE.is_match(s) {
            Ok(Self(s.to_owned()))
        } else {
            Err(NewtypeError::InvalidFormat {
                type_name: "FileSalt",
                expected: "64 lowercase hex characters [0-9a-f]{64}",
                got: s.to_owned(),
            })
        }
    }
}

impl Deref for FileSalt {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for FileSalt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for FileSalt {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for FileSalt {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Self::try_from(s.as_str()).map_err(de::Error::custom)
    }
}

// ---------------------------------------------------------------------------
// NodeId
// ---------------------------------------------------------------------------

/// Non-empty, file-unique string identifier for nodes and edges.
///
/// Accepts any non-empty string; no further shape constraint is imposed by the
/// spec. Use [`EdgeId`] as a type alias when the identifier refers to an edge
/// for documentation clarity.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NodeId(String);

impl TryFrom<&str> for NodeId {
    type Error = NewtypeError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        if s.is_empty() {
            Err(NewtypeError::InvalidFormat {
                type_name: "NodeId",
                expected: "non-empty string",
                got: s.to_owned(),
            })
        } else {
            Ok(Self(s.to_owned()))
        }
    }
}

impl Deref for NodeId {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for NodeId {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for NodeId {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Self::try_from(s.as_str()).map_err(de::Error::custom)
    }
}

/// Alias for [`NodeId`] used when an identifier refers to an edge.
///
/// Semantically distinct in documentation; the same validation rules apply.
pub type EdgeId = NodeId;

// ---------------------------------------------------------------------------
// CountryCode
// ---------------------------------------------------------------------------

/// ISO 3166-1 alpha-2 country code: exactly two uppercase ASCII letters.
///
/// Regex: `^[A-Z]{2}$`. No lookup against the official country list is
/// performed here; that belongs in the validation engine.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CountryCode(String);

impl TryFrom<&str> for CountryCode {
    type Error = NewtypeError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        if COUNTRY_CODE_RE.is_match(s) {
            Ok(Self(s.to_owned()))
        } else {
            Err(NewtypeError::InvalidFormat {
                type_name: "CountryCode",
                expected: "two uppercase ASCII letters (e.g. US, DE)",
                got: s.to_owned(),
            })
        }
    }
}

impl Deref for CountryCode {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CountryCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for CountryCode {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for CountryCode {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Self::try_from(s.as_str()).map_err(de::Error::custom)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    // -- SemVer --------------------------------------------------------------

    #[test]
    fn semver_valid_basic() {
        let v = SemVer::try_from("1.0.0").expect("valid semver");
        assert_eq!(&*v, "1.0.0");
        assert_eq!(v.major(), 1);
        assert_eq!(v.minor(), 0);
        assert_eq!(v.patch(), 0);
    }

    #[test]
    fn semver_valid_large_numbers() {
        let v = SemVer::try_from("10.200.3000").expect("valid semver");
        assert_eq!(v.major(), 10);
        assert_eq!(v.minor(), 200);
        assert_eq!(v.patch(), 3000);
    }

    #[test]
    fn semver_valid_leading_zeros() {
        // The regex allows leading zeros (e.g. "01.0.0") — shape-only, not strict semver.
        SemVer::try_from("01.0.0").expect("leading zeros are accepted as valid shape");
    }

    #[test]
    fn semver_display() {
        let v = SemVer::try_from("2.3.4").expect("valid");
        assert_eq!(v.to_string(), "2.3.4");
    }

    #[test]
    fn semver_reject_missing_patch() {
        assert!(SemVer::try_from("1.0").is_err());
    }

    #[test]
    fn semver_reject_extra_component() {
        assert!(SemVer::try_from("1.0.0.0").is_err());
    }

    #[test]
    fn semver_reject_non_numeric() {
        assert!(SemVer::try_from("1.0.alpha").is_err());
    }

    #[test]
    fn semver_reject_empty() {
        assert!(SemVer::try_from("").is_err());
    }

    #[test]
    fn semver_reject_prerelease_suffix() {
        assert!(SemVer::try_from("1.0.0-beta").is_err());
    }

    #[test]
    fn semver_serde_roundtrip() {
        let v = SemVer::try_from("1.2.3").expect("valid");
        let json = serde_json::to_string(&v).expect("serialize");
        assert_eq!(json, "\"1.2.3\"");
        let back: SemVer = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(v, back);
    }

    #[test]
    fn semver_deserialize_rejects_invalid() {
        let result: Result<SemVer, _> = serde_json::from_str("\"not-a-semver\"");
        assert!(result.is_err());
    }

    // -- CalendarDate --------------------------------------------------------

    #[test]
    fn calendar_date_valid() {
        let d = CalendarDate::try_from("2026-02-19").expect("valid date");
        assert_eq!(&*d, "2026-02-19");
    }

    #[test]
    fn calendar_date_boundary_year_min() {
        CalendarDate::try_from("0001-01-01").expect("minimum representable date shape");
    }

    #[test]
    fn calendar_date_boundary_year_max() {
        CalendarDate::try_from("9999-12-31").expect("maximum representable date shape");
    }

    #[test]
    fn calendar_date_display() {
        let d = CalendarDate::try_from("2026-01-01").expect("valid");
        assert_eq!(d.to_string(), "2026-01-01");
    }

    #[test]
    fn calendar_date_reject_no_separator() {
        assert!(CalendarDate::try_from("20260219").is_err());
    }

    #[test]
    fn calendar_date_reject_short_year() {
        assert!(CalendarDate::try_from("26-02-19").is_err());
    }

    #[test]
    fn calendar_date_reject_slash_separator() {
        assert!(CalendarDate::try_from("2026/02/19").is_err());
    }

    #[test]
    fn calendar_date_reject_non_numeric() {
        assert!(CalendarDate::try_from("YYYY-MM-DD").is_err());
    }

    #[test]
    fn calendar_date_reject_empty() {
        assert!(CalendarDate::try_from("").is_err());
    }

    #[test]
    fn calendar_date_serde_roundtrip() {
        let d = CalendarDate::try_from("2026-02-19").expect("valid");
        let json = serde_json::to_string(&d).expect("serialize");
        assert_eq!(json, "\"2026-02-19\"");
        let back: CalendarDate = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(d, back);
    }

    #[test]
    fn calendar_date_deserialize_rejects_invalid() {
        let result: Result<CalendarDate, _> = serde_json::from_str("\"2026-2-1\"");
        assert!(result.is_err());
    }

    // -- FileSalt ------------------------------------------------------------

    #[test]
    fn file_salt_valid_all_zeros() {
        let s =
            FileSalt::try_from("0000000000000000000000000000000000000000000000000000000000000000")
                .expect("64 zeros is valid");
        assert_eq!(s.len(), 64);
    }

    #[test]
    fn file_salt_valid_all_hex_chars() {
        let s =
            FileSalt::try_from("abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789")
                .expect("valid 64 hex chars");
        assert_eq!(s.len(), 64);
    }

    #[test]
    fn file_salt_display() {
        let raw = "a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2";
        let s = FileSalt::try_from(raw).expect("valid");
        assert_eq!(s.to_string(), raw);
    }

    #[test]
    fn file_salt_reject_too_short() {
        assert!(FileSalt::try_from("abcdef").is_err());
    }

    #[test]
    fn file_salt_reject_too_long() {
        let long = "a".repeat(65);
        assert!(FileSalt::try_from(long.as_str()).is_err());
    }

    #[test]
    fn file_salt_reject_uppercase() {
        let upper = "A".repeat(64);
        assert!(FileSalt::try_from(upper.as_str()).is_err());
    }

    #[test]
    fn file_salt_reject_non_hex() {
        let non_hex = "g".repeat(64);
        assert!(FileSalt::try_from(non_hex.as_str()).is_err());
    }

    #[test]
    fn file_salt_reject_empty() {
        assert!(FileSalt::try_from("").is_err());
    }

    #[test]
    fn file_salt_serde_roundtrip() {
        let raw = "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef";
        let s = FileSalt::try_from(raw).expect("valid");
        let json = serde_json::to_string(&s).expect("serialize");
        let back: FileSalt = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(s, back);
    }

    #[test]
    fn file_salt_deserialize_rejects_invalid() {
        let result: Result<FileSalt, _> = serde_json::from_str("\"tooshort\"");
        assert!(result.is_err());
    }

    // -- NodeId --------------------------------------------------------------

    #[test]
    fn node_id_valid_simple() {
        let id = NodeId::try_from("node-1").expect("valid id");
        assert_eq!(&*id, "node-1");
    }

    #[test]
    fn node_id_valid_uuid_style() {
        let id =
            NodeId::try_from("550e8400-e29b-41d4-a716-446655440000").expect("valid uuid-style id");
        assert_eq!(id.to_string(), "550e8400-e29b-41d4-a716-446655440000");
    }

    #[test]
    fn node_id_valid_arbitrary_chars() {
        // Spec only requires non-empty; any characters are allowed.
        NodeId::try_from("org:acme/division.east").expect("arbitrary non-empty is valid");
    }

    #[test]
    fn node_id_reject_empty() {
        assert!(NodeId::try_from("").is_err());
    }

    #[test]
    fn node_id_display() {
        let id = NodeId::try_from("my-node").expect("valid");
        assert_eq!(id.to_string(), "my-node");
    }

    #[test]
    fn node_id_serde_roundtrip() {
        let id = NodeId::try_from("edge-42").expect("valid");
        let json = serde_json::to_string(&id).expect("serialize");
        assert_eq!(json, "\"edge-42\"");
        let back: NodeId = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(id, back);
    }

    #[test]
    fn node_id_deserialize_rejects_empty() {
        let result: Result<NodeId, _> = serde_json::from_str("\"\"");
        assert!(result.is_err());
    }

    #[test]
    fn edge_id_is_node_id_alias() {
        // EdgeId is a type alias; ensure it compiles and behaves identically.
        let eid: EdgeId = NodeId::try_from("edge-1").expect("valid");
        assert_eq!(&*eid, "edge-1");
    }

    // -- CountryCode ---------------------------------------------------------

    #[test]
    fn country_code_valid_us() {
        let c = CountryCode::try_from("US").expect("valid country code");
        assert_eq!(&*c, "US");
    }

    #[test]
    fn country_code_valid_de() {
        CountryCode::try_from("DE").expect("valid country code");
    }

    #[test]
    fn country_code_valid_zz() {
        // ZZ is user-assigned but passes the shape check.
        CountryCode::try_from("ZZ").expect("valid shape");
    }

    #[test]
    fn country_code_display() {
        let c = CountryCode::try_from("GB").expect("valid");
        assert_eq!(c.to_string(), "GB");
    }

    #[test]
    fn country_code_reject_lowercase() {
        assert!(CountryCode::try_from("us").is_err());
    }

    #[test]
    fn country_code_reject_mixed_case() {
        assert!(CountryCode::try_from("Us").is_err());
    }

    #[test]
    fn country_code_reject_too_short() {
        assert!(CountryCode::try_from("U").is_err());
    }

    #[test]
    fn country_code_reject_too_long() {
        assert!(CountryCode::try_from("USA").is_err());
    }

    #[test]
    fn country_code_reject_digits() {
        assert!(CountryCode::try_from("U1").is_err());
    }

    #[test]
    fn country_code_reject_empty() {
        assert!(CountryCode::try_from("").is_err());
    }

    #[test]
    fn country_code_serde_roundtrip() {
        let c = CountryCode::try_from("JP").expect("valid");
        let json = serde_json::to_string(&c).expect("serialize");
        assert_eq!(json, "\"JP\"");
        let back: CountryCode = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(c, back);
    }

    #[test]
    fn country_code_deserialize_rejects_invalid() {
        let result: Result<CountryCode, _> = serde_json::from_str("\"us\"");
        assert!(result.is_err());
    }

    // -- NewtypeError --------------------------------------------------------

    #[test]
    fn newtype_error_display() {
        let err = NewtypeError::InvalidFormat {
            type_name: "SemVer",
            expected: "MAJOR.MINOR.PATCH",
            got: "bad".to_owned(),
        };
        let msg = err.to_string();
        assert!(msg.contains("SemVer"));
        assert!(msg.contains("MAJOR.MINOR.PATCH"));
        assert!(msg.contains("bad"));
    }

    #[test]
    fn newtype_error_is_std_error() {
        let err: Box<dyn std::error::Error> = Box::new(NewtypeError::InvalidFormat {
            type_name: "NodeId",
            expected: "non-empty string",
            got: String::new(),
        });
        assert!(!err.to_string().is_empty());
    }

    // -- Deref (no DerefMut) -------------------------------------------------

    #[test]
    fn deref_gives_str_access() {
        let v = SemVer::try_from("1.0.0").expect("valid");
        // Deref to &str allows string methods.
        assert!(v.starts_with("1."));
        assert_eq!(v.len(), 5);
    }
}
