/// Serde helper functions shared across the data model.
///
/// The primary export is [`deserialize_optional_nullable`], which handles the
/// three-way distinction between a field being absent, being `null`, and having
/// a value. This is used by `Identifier::valid_to`, `Node::valid_to`,
/// `EdgeProperties::valid_to`, and any other field where the spec assigns
/// distinct semantics to null vs. absent (data-model.md Section 8.3).
use serde::{Deserialize, Deserializer};

/// Deserializer for `Option<Option<T>>` that distinguishes absent from `null`.
///
/// | JSON                  | Rust result       |
/// |-----------------------|-------------------|
/// | field absent          | `None`            |
/// | `"field": null`       | `Some(None)`      |
/// | `"field": <value>`    | `Some(Some(v))`   |
///
/// Use with `#[serde(default, deserialize_with = "...")]`:
///
/// ```rust,ignore
/// #[serde(
///     default,
///     skip_serializing_if = "Option::is_none",
///     deserialize_with = "crate::serde_helpers::deserialize_optional_nullable"
/// )]
/// pub valid_to: Option<Option<CalendarDate>>,
/// ```
///
/// The `#[serde(default)]` attribute ensures the field is set to `None` (the
/// outer `Option`'s default) when the key is absent from the JSON object.
/// When the key is present, this function is called and returns `Some(None)`
/// for `null` or `Some(Some(v))` for any other value.
pub fn deserialize_optional_nullable<'de, T, D>(
    deserializer: D,
) -> Result<Option<Option<T>>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    // `Option<T>` deserializes as None for null, Some(v) for a value.
    let inner: Option<T> = Option::deserialize(deserializer)?;
    // Wrap in the outer Some to signal "field was present".
    Ok(Some(inner))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use serde::{Deserialize, Serialize};

    use crate::newtypes::CalendarDate;

    // A minimal struct that exercises the helper.
    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Holder {
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "super::deserialize_optional_nullable"
        )]
        valid_to: Option<Option<CalendarDate>>,
    }

    #[test]
    fn field_absent_gives_none() {
        let h: Holder = serde_json::from_str("{}").expect("deserialize");
        assert_eq!(h.valid_to, None);
    }

    #[test]
    fn field_null_gives_some_none() {
        let h: Holder = serde_json::from_str(r#"{"valid_to":null}"#).expect("deserialize");
        assert_eq!(h.valid_to, Some(None));
    }

    #[test]
    fn field_value_gives_some_some() {
        let h: Holder = serde_json::from_str(r#"{"valid_to":"2030-06-30"}"#).expect("deserialize");
        let date = CalendarDate::try_from("2030-06-30").expect("valid date");
        assert_eq!(h.valid_to, Some(Some(date)));
    }

    #[test]
    fn none_not_serialized() {
        let h = Holder { valid_to: None };
        let json = serde_json::to_string(&h).expect("serialize");
        assert!(
            !json.contains("valid_to"),
            "absent field should be omitted: {json}"
        );
    }

    #[test]
    fn some_none_serializes_as_null() {
        let h = Holder {
            valid_to: Some(None),
        };
        let json = serde_json::to_string(&h).expect("serialize");
        assert!(
            json.contains(r#""valid_to":null"#),
            "explicit null missing: {json}"
        );
    }

    #[test]
    fn some_some_serializes_as_value() {
        let date = CalendarDate::try_from("2030-06-30").expect("valid date");
        let h = Holder {
            valid_to: Some(Some(date)),
        };
        let json = serde_json::to_string(&h).expect("serialize");
        assert!(json.contains("2030-06-30"), "date missing: {json}");
    }
}
