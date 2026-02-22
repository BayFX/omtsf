/// A format-neutral dynamic value that works with any serde backend.
///
/// Unlike `serde_json::Value`, [`DynValue`] does not depend on JSON-specific
/// representations. This allows structs using `#[serde(flatten)]` with `DynValue`
/// fields to be directly serialized/deserialized with any serde-compatible format
/// (JSON, CBOR, `MessagePack`, etc.) without intermediate conversion.
///
/// The integer/float distinction preserves numeric fidelity across formats that
/// distinguish integer and floating-point types (e.g. CBOR).
use std::collections::BTreeMap;
use std::fmt;

use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A format-neutral dynamic value that works with any serde backend.
///
/// Unlike `serde_json::Value`, `DynValue` does not depend on JSON-specific
/// representations. This allows structs using `#[serde(flatten)]` with `DynValue`
/// fields to be directly serialized/deserialized with any serde-compatible format
/// (JSON, CBOR, `MessagePack`, etc.) without intermediate conversion.
///
/// The integer/float distinction preserves numeric fidelity across formats that
/// distinguish integer and floating-point types (e.g. CBOR).
#[derive(Debug, Clone)]
pub enum DynValue {
    /// JSON `null` / CBOR `null`.
    Null,
    /// Boolean value.
    Bool(bool),
    /// Signed integer (fits in i64).
    Integer(i64),
    /// Unsigned integer that does not fit in i64 (u64 range above `i64::MAX`).
    UnsignedInteger(u64),
    /// IEEE 754 double-precision float.
    Float(f64),
    /// UTF-8 string.
    String(String),
    /// Ordered sequence of values.
    Array(Vec<DynValue>),
    /// String-keyed map preserving insertion order via `BTreeMap`.
    Object(BTreeMap<String, DynValue>),
}

/// A string-keyed map of dynamic values, used as the type for `extra` fields.
pub type DynMap = BTreeMap<String, DynValue>;

impl PartialEq for DynValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Null, Self::Null) => true,
            (Self::Bool(a), Self::Bool(b)) => a == b,
            (Self::Integer(a), Self::Integer(b)) => a == b,
            (Self::UnsignedInteger(a), Self::UnsignedInteger(b)) => a == b,
            (Self::Integer(a), Self::UnsignedInteger(b)) => {
                if *a >= 0 {
                    *a as u64 == *b
                } else {
                    false
                }
            }
            (Self::UnsignedInteger(a), Self::Integer(b)) => {
                if *b >= 0 {
                    *a == *b as u64
                } else {
                    false
                }
            }
            (Self::Float(a), Self::Float(b)) => a.to_bits() == b.to_bits(),
            (Self::String(a), Self::String(b)) => a == b,
            (Self::Array(a), Self::Array(b)) => a == b,
            (Self::Object(a), Self::Object(b)) => a == b,
            _ => false,
        }
    }
}

impl DynValue {
    /// Returns the string value if this is a `DynValue::String`.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s.as_str()),
            Self::Null
            | Self::Bool(_)
            | Self::Integer(_)
            | Self::UnsignedInteger(_)
            | Self::Float(_)
            | Self::Array(_)
            | Self::Object(_) => None,
        }
    }

    /// Returns the i64 value if this is an integer type.
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Integer(n) => Some(*n),
            Self::UnsignedInteger(n) => i64::try_from(*n).ok(),
            Self::Null
            | Self::Bool(_)
            | Self::Float(_)
            | Self::String(_)
            | Self::Array(_)
            | Self::Object(_) => None,
        }
    }

    /// Returns the u64 value if this is an integer type.
    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Self::Integer(n) => u64::try_from(*n).ok(),
            Self::UnsignedInteger(n) => Some(*n),
            Self::Null
            | Self::Bool(_)
            | Self::Float(_)
            | Self::String(_)
            | Self::Array(_)
            | Self::Object(_) => None,
        }
    }

    /// Returns the f64 value if this is a `DynValue::Float` or an integer type.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Float(f) => Some(*f),
            Self::Integer(n) => Some(*n as f64),
            Self::UnsignedInteger(n) => Some(*n as f64),
            Self::Null | Self::Bool(_) | Self::String(_) | Self::Array(_) | Self::Object(_) => None,
        }
    }

    /// Returns the bool value if this is a `DynValue::Bool`.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(b) => Some(*b),
            Self::Null
            | Self::Integer(_)
            | Self::UnsignedInteger(_)
            | Self::Float(_)
            | Self::String(_)
            | Self::Array(_)
            | Self::Object(_) => None,
        }
    }

    /// Returns the inner map if this is a `DynValue::Object`.
    pub fn as_object(&self) -> Option<&BTreeMap<String, DynValue>> {
        match self {
            Self::Object(m) => Some(m),
            Self::Null
            | Self::Bool(_)
            | Self::Integer(_)
            | Self::UnsignedInteger(_)
            | Self::Float(_)
            | Self::String(_)
            | Self::Array(_) => None,
        }
    }

    /// Returns the inner array if this is a `DynValue::Array`.
    pub fn as_array(&self) -> Option<&Vec<DynValue>> {
        match self {
            Self::Array(a) => Some(a),
            Self::Null
            | Self::Bool(_)
            | Self::Integer(_)
            | Self::UnsignedInteger(_)
            | Self::Float(_)
            | Self::String(_)
            | Self::Object(_) => None,
        }
    }

    /// Returns `true` if this is `DynValue::Null`.
    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    /// Index into an object by key.
    pub fn get(&self, key: &str) -> Option<&DynValue> {
        match self {
            Self::Object(m) => m.get(key),
            Self::Null
            | Self::Bool(_)
            | Self::Integer(_)
            | Self::UnsignedInteger(_)
            | Self::Float(_)
            | Self::String(_)
            | Self::Array(_) => None,
        }
    }
}

impl From<serde_json::Value> for DynValue {
    fn from(v: serde_json::Value) -> Self {
        match v {
            serde_json::Value::Null => Self::Null,
            serde_json::Value::Bool(b) => Self::Bool(b),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Self::Integer(i)
                } else if let Some(u) = n.as_u64() {
                    Self::UnsignedInteger(u)
                } else if let Some(f) = n.as_f64() {
                    Self::Float(f)
                } else {
                    Self::Null
                }
            }
            serde_json::Value::String(s) => Self::String(s),
            serde_json::Value::Array(a) => Self::Array(a.into_iter().map(DynValue::from).collect()),
            serde_json::Value::Object(m) => {
                Self::Object(m.into_iter().map(|(k, v)| (k, DynValue::from(v))).collect())
            }
        }
    }
}

impl From<DynValue> for serde_json::Value {
    fn from(v: DynValue) -> Self {
        match v {
            DynValue::Null => serde_json::Value::Null,
            DynValue::Bool(b) => serde_json::Value::Bool(b),
            DynValue::Integer(i) => serde_json::Value::Number(i.into()),
            DynValue::UnsignedInteger(u) => serde_json::Value::Number(u.into()),
            DynValue::Float(f) => serde_json::Number::from_f64(f)
                .map(serde_json::Value::Number)
                .unwrap_or(serde_json::Value::Null),
            DynValue::String(s) => serde_json::Value::String(s),
            DynValue::Array(a) => {
                serde_json::Value::Array(a.into_iter().map(serde_json::Value::from).collect())
            }
            DynValue::Object(m) => {
                let map: serde_json::Map<String, serde_json::Value> = m
                    .into_iter()
                    .map(|(k, v)| (k, serde_json::Value::from(v)))
                    .collect();
                serde_json::Value::Object(map)
            }
        }
    }
}

impl Serialize for DynValue {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Null => serializer.serialize_none(),
            Self::Bool(b) => serializer.serialize_bool(*b),
            Self::Integer(i) => serializer.serialize_i64(*i),
            Self::UnsignedInteger(u) => serializer.serialize_u64(*u),
            Self::Float(f) => serializer.serialize_f64(*f),
            Self::String(s) => serializer.serialize_str(s),
            Self::Array(arr) => arr.serialize(serializer),
            Self::Object(map) => {
                let mut m = serializer.serialize_map(Some(map.len()))?;
                for (k, v) in map {
                    m.serialize_entry(k, v)?;
                }
                m.end()
            }
        }
    }
}

impl<'de> Deserialize<'de> for DynValue {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(DynValueVisitor)
    }
}

struct DynValueVisitor;

impl<'de> Visitor<'de> for DynValueVisitor {
    type Value = DynValue;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("any valid value")
    }

    fn visit_bool<E: de::Error>(self, v: bool) -> Result<DynValue, E> {
        Ok(DynValue::Bool(v))
    }

    fn visit_i8<E: de::Error>(self, v: i8) -> Result<DynValue, E> {
        Ok(DynValue::Integer(i64::from(v)))
    }

    fn visit_i16<E: de::Error>(self, v: i16) -> Result<DynValue, E> {
        Ok(DynValue::Integer(i64::from(v)))
    }

    fn visit_i32<E: de::Error>(self, v: i32) -> Result<DynValue, E> {
        Ok(DynValue::Integer(i64::from(v)))
    }

    fn visit_i64<E: de::Error>(self, v: i64) -> Result<DynValue, E> {
        Ok(DynValue::Integer(v))
    }

    fn visit_u8<E: de::Error>(self, v: u8) -> Result<DynValue, E> {
        Ok(DynValue::Integer(i64::from(v)))
    }

    fn visit_u16<E: de::Error>(self, v: u16) -> Result<DynValue, E> {
        Ok(DynValue::Integer(i64::from(v)))
    }

    fn visit_u32<E: de::Error>(self, v: u32) -> Result<DynValue, E> {
        Ok(DynValue::Integer(i64::from(v)))
    }

    fn visit_u64<E: de::Error>(self, v: u64) -> Result<DynValue, E> {
        match i64::try_from(v) {
            Ok(i) => Ok(DynValue::Integer(i)),
            Err(_) => Ok(DynValue::UnsignedInteger(v)),
        }
    }

    fn visit_f32<E: de::Error>(self, v: f32) -> Result<DynValue, E> {
        Ok(DynValue::Float(f64::from(v)))
    }

    fn visit_f64<E: de::Error>(self, v: f64) -> Result<DynValue, E> {
        Ok(DynValue::Float(v))
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<DynValue, E> {
        Ok(DynValue::String(v.to_owned()))
    }

    fn visit_string<E: de::Error>(self, v: String) -> Result<DynValue, E> {
        Ok(DynValue::String(v))
    }

    fn visit_unit<E: de::Error>(self) -> Result<DynValue, E> {
        Ok(DynValue::Null)
    }

    fn visit_none<E: de::Error>(self) -> Result<DynValue, E> {
        Ok(DynValue::Null)
    }

    fn visit_some<D: Deserializer<'de>>(self, deserializer: D) -> Result<DynValue, D::Error> {
        DynValue::deserialize(deserializer)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<DynValue, A::Error> {
        let mut arr = Vec::new();
        while let Some(elem) = seq.next_element()? {
            arr.push(elem);
        }
        Ok(DynValue::Array(arr))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<DynValue, A::Error> {
        let mut obj = BTreeMap::new();
        while let Some((key, value)) = map.next_entry::<String, DynValue>()? {
            obj.insert(key, value);
        }
        Ok(DynValue::Object(obj))
    }
}

impl fmt::Display for DynValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Null => write!(f, "null"),
            Self::Bool(b) => write!(f, "{b}"),
            Self::Integer(i) => write!(f, "{i}"),
            Self::UnsignedInteger(u) => write!(f, "{u}"),
            Self::Float(v) => write!(f, "{v}"),
            Self::String(s) => write!(f, "{s}"),
            Self::Array(_) => write!(f, "[...]"),
            Self::Object(_) => write!(f, "{{...}}"),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    #[test]
    fn null_round_trips_json() {
        let v = DynValue::Null;
        let json = serde_json::to_string(&v).expect("serialize");
        assert_eq!(json, "null");
        let back: DynValue = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(v, back);
    }

    #[test]
    fn bool_round_trips_json() {
        for b in [true, false] {
            let v = DynValue::Bool(b);
            let json = serde_json::to_string(&v).expect("serialize");
            let back: DynValue = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(v, back);
        }
    }

    #[test]
    fn integer_round_trips_json() {
        for i in [-1_i64, 0, 42, i64::MAX] {
            let v = DynValue::Integer(i);
            let json = serde_json::to_string(&v).expect("serialize");
            let back: DynValue = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(v, back);
        }
    }

    #[test]
    fn float_round_trips_json() {
        let v = DynValue::Float(1.5);
        let json = serde_json::to_string(&v).expect("serialize");
        let back: DynValue = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(v, back);
    }

    #[test]
    fn string_round_trips_json() {
        let v = DynValue::String("hello".to_owned());
        let json = serde_json::to_string(&v).expect("serialize");
        let back: DynValue = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(v, back);
    }

    #[test]
    fn object_round_trips_json() {
        let mut map = BTreeMap::new();
        map.insert("key".to_owned(), DynValue::Integer(1));
        let v = DynValue::Object(map);
        let json = serde_json::to_string(&v).expect("serialize");
        let back: DynValue = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(v, back);
    }

    #[test]
    fn array_round_trips_json() {
        let v = DynValue::Array(vec![
            DynValue::Integer(1),
            DynValue::String("two".to_owned()),
        ]);
        let json = serde_json::to_string(&v).expect("serialize");
        let back: DynValue = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(v, back);
    }

    #[test]
    fn from_serde_json_value_null() {
        let v = DynValue::from(serde_json::Value::Null);
        assert_eq!(v, DynValue::Null);
    }

    #[test]
    fn from_serde_json_value_string() {
        let v = DynValue::from(serde_json::Value::String("hello".to_owned()));
        assert_eq!(v, DynValue::String("hello".to_owned()));
    }

    #[test]
    fn into_serde_json_value_integer() {
        let v = serde_json::Value::from(DynValue::Integer(42));
        assert_eq!(v, serde_json::json!(42));
    }

    #[test]
    fn into_serde_json_value_null() {
        let v = serde_json::Value::from(DynValue::Null);
        assert_eq!(v, serde_json::Value::Null);
    }

    #[test]
    fn display_variants() {
        assert_eq!(DynValue::Null.to_string(), "null");
        assert_eq!(DynValue::Bool(true).to_string(), "true");
        assert_eq!(DynValue::Integer(-5).to_string(), "-5");
        assert_eq!(
            DynValue::UnsignedInteger(u64::MAX).to_string(),
            u64::MAX.to_string()
        );
        assert_eq!(DynValue::String("hi".to_owned()).to_string(), "hi");
        assert_eq!(DynValue::Array(vec![]).to_string(), "[...]");
        assert_eq!(DynValue::Object(BTreeMap::new()).to_string(), "{...}");
    }

    #[test]
    fn accessors_return_correct_values() {
        assert_eq!(DynValue::String("x".to_owned()).as_str(), Some("x"));
        assert_eq!(DynValue::Integer(7).as_i64(), Some(7));
        assert_eq!(DynValue::Integer(7).as_u64(), Some(7));
        assert_eq!(DynValue::Float(1.5).as_f64(), Some(1.5));
        assert_eq!(DynValue::Bool(false).as_bool(), Some(false));
        assert!(DynValue::Null.is_null());
        assert!(!DynValue::Bool(true).is_null());
    }

    #[test]
    fn cross_integer_type_equality() {
        assert_eq!(DynValue::Integer(42), DynValue::UnsignedInteger(42));
        assert_eq!(DynValue::UnsignedInteger(0), DynValue::Integer(0));
        assert_ne!(DynValue::Integer(-1), DynValue::UnsignedInteger(u64::MAX));
    }

    #[test]
    fn get_on_object() {
        let mut map = BTreeMap::new();
        map.insert("k".to_owned(), DynValue::Bool(true));
        let v = DynValue::Object(map);
        assert_eq!(v.get("k"), Some(&DynValue::Bool(true)));
        assert_eq!(v.get("missing"), None);
    }

    #[test]
    fn nan_float_equality_uses_bits() {
        let nan1 = DynValue::Float(f64::NAN);
        let nan2 = DynValue::Float(f64::NAN);
        assert_eq!(nan1, nan2);
    }

    #[test]
    fn null_round_trips_cbor() {
        let v = DynValue::Null;
        let cbor = cbor4ii::serde::to_vec(Vec::new(), &v).expect("serialize");
        assert_eq!(cbor, [0xF6], "DynValue::Null must encode as CBOR null");
        let back: DynValue = cbor4ii::serde::from_slice(&cbor).expect("deserialize");
        assert_eq!(v, back);
    }
}
