//! Shared selector flag parsing for `query` and `extract-subchain` commands.
//!
//! Provides [`build_selector_set`], which converts the raw string vectors
//! collected by clap into a [`SelectorSet`] ready for use with
//! [`omtsf_core::graph::selector_match`] and
//! [`omtsf_core::graph::selector_subgraph`].
//!
//! # Parsing Rules
//!
//! | Flag             | Input form          | Produces                             |
//! |------------------|---------------------|--------------------------------------|
//! | `--label`        | `key`               | `Selector::LabelKey("key")`          |
//! | `--label`        | `key=value`         | `Selector::LabelKeyValue("key","value")` |
//! | `--identifier`   | `scheme`            | `Selector::IdentifierScheme("scheme")` |
//! | `--identifier`   | `scheme:value`      | `Selector::IdentifierSchemeValue("scheme","value")` |
//! | `--node-type`    | `organization`      | `Selector::NodeType(NodeTypeTag::Known(…))` |
//! | `--node-type`    | `com.example.foo`   | `Selector::NodeType(NodeTypeTag::Extension(…))` |
//! | `--edge-type`    | `supplies`          | `Selector::EdgeType(EdgeTypeTag::Known(…))` |
//! | `--jurisdiction` | `DE`                | `Selector::Jurisdiction(CountryCode)` |
//! | `--name`         | `acme`              | `Selector::Name("acme")`             |

use omtsf_core::graph::selectors::{Selector, SelectorSet};
use omtsf_core::{CountryCode, EdgeTypeTag, NodeTypeTag};

use crate::error::CliError;

/// Builds a [`SelectorSet`] from the raw flag vectors collected by clap.
///
/// Each argument vector corresponds to one repeatable CLI flag. The function
/// returns an error if the resulting `SelectorSet` would be empty (i.e. no
/// selector flags were provided at all).
///
/// # Errors
///
/// Returns [`CliError`] with exit code 2 if:
/// - No selector flags were provided (empty result would be a universal match,
///   which is not the intended behaviour for an explicit query command).
/// - A `--jurisdiction` value is not a valid ISO 3166-1 alpha-2 country code.
pub fn build_selector_set(
    node_types: &[String],
    edge_types: &[String],
    labels: &[String],
    identifiers: &[String],
    jurisdictions: &[String],
    names: &[String],
) -> Result<SelectorSet, CliError> {
    let mut selectors: Vec<Selector> = Vec::new();

    for s in node_types {
        let tag = parse_node_type_tag(s)?;
        selectors.push(Selector::NodeType(tag));
    }

    for s in edge_types {
        let tag = parse_edge_type_tag(s)?;
        selectors.push(Selector::EdgeType(tag));
    }

    for s in labels {
        if let Some((k, v)) = s.split_once('=') {
            selectors.push(Selector::LabelKeyValue(k.to_owned(), v.to_owned()));
        } else {
            selectors.push(Selector::LabelKey(s.clone()));
        }
    }

    for s in identifiers {
        if let Some((scheme, value)) = s.split_once(':') {
            selectors.push(Selector::IdentifierSchemeValue(
                scheme.to_owned(),
                value.to_owned(),
            ));
        } else {
            selectors.push(Selector::IdentifierScheme(s.clone()));
        }
    }

    for s in jurisdictions {
        let cc = CountryCode::try_from(s.as_str()).map_err(|e| CliError::InvalidArgument {
            detail: format!("--jurisdiction: {e}"),
        })?;
        selectors.push(Selector::Jurisdiction(cc));
    }

    for s in names {
        selectors.push(Selector::Name(s.clone()));
    }

    let set = SelectorSet::from_selectors(selectors);

    if set.is_empty() {
        return Err(CliError::InvalidArgument {
            detail: "at least one selector flag is required \
                     (--node-type, --edge-type, --label, --identifier, --jurisdiction, --name)"
                .to_owned(),
        });
    }

    Ok(set)
}

/// Parses a string into a [`NodeTypeTag`] using serde deserialization.
///
/// Known `snake_case` node type strings (e.g. `"organization"`, `"facility"`)
/// produce `NodeTypeTag::Known(_)`; any unrecognised string produces
/// `NodeTypeTag::Extension(_)`.
///
/// Because `NodeTypeTag`'s serde impl always succeeds (unknown strings become
/// `Extension`), this function is infallible in practice.
fn parse_node_type_tag(s: &str) -> Result<NodeTypeTag, CliError> {
    serde_json::from_value(serde_json::Value::String(s.to_owned())).map_err(|e| {
        CliError::InvalidArgument {
            detail: format!("--node-type {s:?}: {e}"),
        }
    })
}

/// Parses a string into an [`EdgeTypeTag`] using serde deserialization.
///
/// Known `snake_case` edge type strings (e.g. `"supplies"`, `"ownership"`)
/// produce `EdgeTypeTag::Known(_)`; any unrecognised string produces
/// `EdgeTypeTag::Extension(_)`.
///
/// Because `EdgeTypeTag`'s serde impl always succeeds (unknown strings become
/// `Extension`), this function is infallible in practice.
fn parse_edge_type_tag(s: &str) -> Result<EdgeTypeTag, CliError> {
    serde_json::from_value(serde_json::Value::String(s.to_owned())).map_err(|e| {
        CliError::InvalidArgument {
            detail: format!("--edge-type {s:?}: {e}"),
        }
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    #![allow(clippy::panic)]

    use omtsf_core::{EdgeType, EdgeTypeTag, NodeType, NodeTypeTag};

    use super::*;

    fn empty() -> Vec<String> {
        vec![]
    }

    fn strs(v: &[&str]) -> Vec<String> {
        v.iter().map(std::string::ToString::to_string).collect()
    }

    /// No selector flags → error with exit code 2.
    #[test]
    fn test_empty_selectors_returns_error() {
        let err = build_selector_set(&empty(), &empty(), &empty(), &empty(), &empty(), &empty())
            .expect_err("should error on empty selector set");
        assert_eq!(err.exit_code(), 2);
        assert!(
            err.message().contains("selector"),
            "message should mention selectors: {}",
            err.message()
        );
    }

    /// `--label key` produces `LabelKey`.
    #[test]
    fn test_label_key_only() {
        let ss = build_selector_set(
            &empty(),
            &empty(),
            &strs(&["certified"]),
            &empty(),
            &empty(),
            &empty(),
        )
        .expect("should parse --label key");
        assert_eq!(ss.label_keys, vec!["certified"]);
        assert!(ss.label_key_values.is_empty());
    }

    /// `--label key=value` produces `LabelKeyValue`.
    #[test]
    fn test_label_key_value() {
        let ss = build_selector_set(
            &empty(),
            &empty(),
            &strs(&["tier=1"]),
            &empty(),
            &empty(),
            &empty(),
        )
        .expect("should parse --label key=value");
        assert!(ss.label_keys.is_empty());
        assert_eq!(
            ss.label_key_values,
            vec![("tier".to_owned(), "1".to_owned())]
        );
    }

    /// `--label key=val=extra` splits on the first `=` only; value contains second `=`.
    #[test]
    fn test_label_key_value_with_equals_in_value() {
        let ss = build_selector_set(
            &empty(),
            &empty(),
            &strs(&["url=https://example.com?a=b"]),
            &empty(),
            &empty(),
            &empty(),
        )
        .expect("should parse");
        assert_eq!(
            ss.label_key_values,
            vec![("url".to_owned(), "https://example.com?a=b".to_owned())]
        );
    }

    /// `--identifier scheme` produces `IdentifierScheme`.
    #[test]
    fn test_identifier_scheme_only() {
        let ss = build_selector_set(
            &empty(),
            &empty(),
            &empty(),
            &strs(&["lei"]),
            &empty(),
            &empty(),
        )
        .expect("should parse --identifier scheme");
        assert_eq!(ss.identifier_schemes, vec!["lei"]);
        assert!(ss.identifier_scheme_values.is_empty());
    }

    /// `--identifier scheme:value` produces `IdentifierSchemeValue`.
    #[test]
    fn test_identifier_scheme_value() {
        let ss = build_selector_set(
            &empty(),
            &empty(),
            &empty(),
            &strs(&["duns:123456789"]),
            &empty(),
            &empty(),
        )
        .expect("should parse --identifier scheme:value");
        assert!(ss.identifier_schemes.is_empty());
        assert_eq!(
            ss.identifier_scheme_values,
            vec![("duns".to_owned(), "123456789".to_owned())]
        );
    }

    /// Known node type string maps to `NodeTypeTag::Known`.
    #[test]
    fn test_node_type_known() {
        let ss = build_selector_set(
            &strs(&["organization"]),
            &empty(),
            &empty(),
            &empty(),
            &empty(),
            &empty(),
        )
        .expect("should parse organization");
        assert_eq!(
            ss.node_types,
            vec![NodeTypeTag::Known(NodeType::Organization)]
        );
    }

    /// Unknown node type string maps to `NodeTypeTag::Extension`.
    #[test]
    fn test_node_type_extension() {
        let ss = build_selector_set(
            &strs(&["com.example.custom"]),
            &empty(),
            &empty(),
            &empty(),
            &empty(),
            &empty(),
        )
        .expect("should parse extension node type");
        assert_eq!(
            ss.node_types,
            vec![NodeTypeTag::Extension("com.example.custom".to_owned())]
        );
    }

    /// All known node types parse without error.
    #[test]
    fn test_all_known_node_types_parse() {
        for type_str in [
            "organization",
            "facility",
            "good",
            "person",
            "attestation",
            "consignment",
            "boundary_ref",
        ] {
            build_selector_set(
                &strs(&[type_str]),
                &empty(),
                &empty(),
                &empty(),
                &empty(),
                &empty(),
            )
            .unwrap_or_else(|e| panic!("--node-type {type_str} should parse: {e}"));
        }
    }

    /// Known edge type string maps to `EdgeTypeTag::Known`.
    #[test]
    fn test_edge_type_known() {
        let ss = build_selector_set(
            &empty(),
            &strs(&["supplies"]),
            &empty(),
            &empty(),
            &empty(),
            &empty(),
        )
        .expect("should parse supplies");
        assert_eq!(ss.edge_types, vec![EdgeTypeTag::Known(EdgeType::Supplies)]);
    }

    /// Unknown edge type string maps to `EdgeTypeTag::Extension`.
    #[test]
    fn test_edge_type_extension() {
        let ss = build_selector_set(
            &empty(),
            &strs(&["com.acme.custom_rel"]),
            &empty(),
            &empty(),
            &empty(),
            &empty(),
        )
        .expect("should parse extension edge type");
        assert_eq!(
            ss.edge_types,
            vec![EdgeTypeTag::Extension("com.acme.custom_rel".to_owned())]
        );
    }

    /// Valid country code parses successfully.
    #[test]
    fn test_jurisdiction_valid() {
        let ss = build_selector_set(
            &empty(),
            &empty(),
            &empty(),
            &empty(),
            &strs(&["DE"]),
            &empty(),
        )
        .expect("should parse DE");
        assert_eq!(ss.jurisdictions.len(), 1);
        assert_eq!(ss.jurisdictions[0].to_string(), "DE");
    }

    /// Invalid country code returns error with exit code 2.
    #[test]
    fn test_jurisdiction_invalid() {
        let err = build_selector_set(
            &empty(),
            &empty(),
            &empty(),
            &empty(),
            &strs(&["123"]),
            &empty(),
        )
        .expect_err("123 is not a valid country code");
        assert_eq!(err.exit_code(), 2);
    }

    /// `--name pattern` produces `Name` selector.
    #[test]
    fn test_name_selector() {
        let ss = build_selector_set(
            &empty(),
            &empty(),
            &empty(),
            &empty(),
            &empty(),
            &strs(&["acme"]),
        )
        .expect("should parse --name");
        assert_eq!(ss.names, vec!["acme"]);
    }

    /// Multiple flags of different types produce a combined `SelectorSet`.
    #[test]
    fn test_multiple_flag_types_combine() {
        let ss = build_selector_set(
            &strs(&["facility"]),
            &empty(),
            &strs(&["certified"]),
            &empty(),
            &strs(&["US"]),
            &empty(),
        )
        .expect("should combine multiple selector types");
        assert_eq!(ss.node_types.len(), 1);
        assert_eq!(ss.label_keys.len(), 1);
        assert_eq!(ss.jurisdictions.len(), 1);
    }

    /// Multiple values for the same flag produce multiple selectors within one group.
    #[test]
    fn test_multiple_values_same_flag() {
        let ss = build_selector_set(
            &strs(&["organization", "facility"]),
            &empty(),
            &empty(),
            &empty(),
            &empty(),
            &empty(),
        )
        .expect("should accept multiple --node-type values");
        assert_eq!(ss.node_types.len(), 2);
    }

    /// The returned `SelectorSet` is never empty (function errors before that).
    #[test]
    fn test_non_empty_selector_set_is_never_empty() {
        let ss = build_selector_set(
            &strs(&["organization"]),
            &empty(),
            &empty(),
            &empty(),
            &empty(),
            &empty(),
        )
        .expect("valid selector");
        assert!(!ss.is_empty());
    }
}
