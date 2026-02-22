/// Sensitivity classification for identifiers and edge properties.
///
/// This module implements Sections 2.1–2.3 of the redaction specification:
/// - Scheme-based defaults for identifier sensitivity (Section 2.1)
/// - Person-node override: all identifiers default to `confidential` (Section 2.2)
/// - Edge-property sensitivity defaults with per-property-name and edge-type
///   rules, plus `_property_sensitivity` override maps (Section 2.3)
///
/// Explicit `sensitivity` values on identifier records and entries in
/// `_property_sensitivity` always win over the defaults computed here.
use crate::enums::{EdgeType, EdgeTypeTag, NodeType, NodeTypeTag, Sensitivity};
use crate::structures::Edge;
use crate::types::Identifier;

/// Returns the effective sensitivity for an identifier, taking into account:
///
/// 1. **Explicit override** — if `identifier.sensitivity` is `Some(s)`, return
///    `s` directly (overrides both scheme default and person-node rule).
/// 2. **Person-node rule** (Section 2.2) — if `node_type` is
///    [`NodeType::Person`], default to [`Sensitivity::Confidential`].
/// 3. **Scheme default** (Section 2.1) — otherwise, apply the table:
///    - `lei`, `duns`, `gln` → `Public`
///    - `nat-reg`, `vat`, `internal` → `Restricted`
///    - Any unrecognised scheme → `Public`
///
/// The person-node rule and scheme default are only used when no explicit
/// `sensitivity` field is present on the identifier record.
///
/// # Arguments
///
/// * `identifier` — the identifier whose sensitivity is being determined.
/// * `node_type` — the type tag of the node that owns this identifier.
pub fn effective_sensitivity(identifier: &Identifier, node_type: &NodeTypeTag) -> Sensitivity {
    if let Some(explicit) = &identifier.sensitivity {
        return explicit.clone();
    }

    if let NodeTypeTag::Known(NodeType::Person) = node_type {
        return Sensitivity::Confidential;
    }

    scheme_default(&identifier.scheme)
}

/// Returns the scheme-based default sensitivity for a given scheme string.
///
/// | Scheme | Default |
/// |--------|---------|
/// | `lei` | `Public` |
/// | `duns` | `Public` |
/// | `gln` | `Public` |
/// | `nat-reg` | `Restricted` |
/// | `vat` | `Restricted` |
/// | `internal` | `Restricted` |
/// | Any other | `Public` |
fn scheme_default(scheme: &str) -> Sensitivity {
    match scheme {
        "lei" | "duns" | "gln" => Sensitivity::Public,
        "nat-reg" | "vat" | "internal" => Sensitivity::Restricted,
        // Any unrecognised scheme defaults to Public per Section 2.1.
        _ => Sensitivity::Public,
    }
}

/// Returns the effective sensitivity for a named property on an edge.
///
/// Resolution order (first match wins):
///
/// 1. **`_property_sensitivity` map** — if the edge's `properties.extra` field
///    contains a `_property_sensitivity` JSON object whose key matches
///    `property_name`, use that value (must be a JSON string recognisable as
///    a [`Sensitivity`] variant: `"public"`, `"restricted"`, or
///    `"confidential"`). If the key is present but not a recognised string, it
///    is ignored and resolution falls through.
/// 2. **Property/edge-type default table** (Section 2.3):
///    - `contract_ref` → `Restricted`
///    - `annual_value` → `Restricted`
///    - `value_currency` → `Restricted`
///    - `volume` → `Restricted`
///    - `volume_unit` → `Public`
///    - `percentage` on `ownership` → `Public`
///    - `percentage` on `beneficial_ownership` → `Confidential`
///    - All other properties → `Public`
///
/// # Arguments
///
/// * `edge` — the edge whose property is being classified.
/// * `property_name` — the JSON property name to classify (e.g. `"contract_ref"`).
pub fn effective_property_sensitivity(edge: &Edge, property_name: &str) -> Sensitivity {
    if let Some(override_sensitivity) = read_property_sensitivity_override(edge, property_name) {
        return override_sensitivity;
    }

    property_default(&edge.edge_type, property_name)
}

/// Attempts to read a per-property sensitivity override from the edge's
/// `_property_sensitivity` JSON object (stored in `properties.extra`).
///
/// Returns `Some(Sensitivity)` if the map contains the key and its value is a
/// recognised sensitivity string; `None` otherwise (key absent, value not a
/// string, or unrecognised string).
fn read_property_sensitivity_override(edge: &Edge, property_name: &str) -> Option<Sensitivity> {
    let override_map = edge
        .properties
        .extra
        .get("_property_sensitivity")?
        .as_object()?;

    let raw = override_map.get(property_name)?.as_str()?;

    match raw {
        "public" => Some(Sensitivity::Public),
        "restricted" => Some(Sensitivity::Restricted),
        "confidential" => Some(Sensitivity::Confidential),
        _ => None,
    }
}

/// Returns the default sensitivity for a property given the edge type.
///
/// Implements the table in Section 2.3 of the redaction specification.
fn property_default(edge_type: &EdgeTypeTag, property_name: &str) -> Sensitivity {
    match property_name {
        "contract_ref" => Sensitivity::Restricted,
        "annual_value" => Sensitivity::Restricted,
        "value_currency" => Sensitivity::Restricted,
        "volume" => Sensitivity::Restricted,
        "volume_unit" => Sensitivity::Public,
        "percentage" => percentage_default(edge_type),
        _ => Sensitivity::Public,
    }
}

/// Returns the default sensitivity for the `percentage` property, which
/// depends on edge type:
/// - `ownership` → `Public`
/// - `beneficial_ownership` → `Confidential`
/// - All other edge types → `Public`
fn percentage_default(edge_type: &EdgeTypeTag) -> Sensitivity {
    match edge_type {
        EdgeTypeTag::Known(EdgeType::Ownership) => Sensitivity::Public,
        EdgeTypeTag::Known(EdgeType::BeneficialOwnership) => Sensitivity::Confidential,
        EdgeTypeTag::Known(EdgeType::OperationalControl) => Sensitivity::Public,
        EdgeTypeTag::Known(EdgeType::LegalParentage) => Sensitivity::Public,
        EdgeTypeTag::Known(EdgeType::FormerIdentity) => Sensitivity::Public,
        EdgeTypeTag::Known(EdgeType::Supplies) => Sensitivity::Public,
        EdgeTypeTag::Known(EdgeType::Subcontracts) => Sensitivity::Public,
        EdgeTypeTag::Known(EdgeType::Tolls) => Sensitivity::Public,
        EdgeTypeTag::Known(EdgeType::Distributes) => Sensitivity::Public,
        EdgeTypeTag::Known(EdgeType::Brokers) => Sensitivity::Public,
        EdgeTypeTag::Known(EdgeType::Operates) => Sensitivity::Public,
        EdgeTypeTag::Known(EdgeType::Produces) => Sensitivity::Public,
        EdgeTypeTag::Known(EdgeType::ComposedOf) => Sensitivity::Public,
        EdgeTypeTag::Known(EdgeType::SellsTo) => Sensitivity::Public,
        EdgeTypeTag::Known(EdgeType::AttestedBy) => Sensitivity::Public,
        EdgeTypeTag::Known(EdgeType::SameAs) => Sensitivity::Public,
        EdgeTypeTag::Extension(_) => Sensitivity::Public,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use serde_json::json;
    use std::collections::BTreeMap;

    use super::*;
    use crate::dynvalue::DynValue;
    use crate::enums::{EdgeType, EdgeTypeTag, NodeType, NodeTypeTag};
    use crate::newtypes::{EdgeId, NodeId};
    use crate::structures::{Edge, EdgeProperties};
    use crate::types::Identifier;

    fn make_identifier(scheme: &str, explicit_sensitivity: Option<Sensitivity>) -> Identifier {
        Identifier {
            scheme: scheme.to_owned(),
            value: "test-value".to_owned(),
            authority: None,
            valid_from: None,
            valid_to: None,
            sensitivity: explicit_sensitivity,
            verification_status: None,
            verification_date: None,
            extra: BTreeMap::new(),
        }
    }

    fn known_node_type(t: NodeType) -> NodeTypeTag {
        NodeTypeTag::Known(t)
    }

    fn extension_node_type(s: &str) -> NodeTypeTag {
        NodeTypeTag::Extension(s.to_owned())
    }

    fn make_edge(edge_type: EdgeType, extra_properties: BTreeMap<String, DynValue>) -> Edge {
        Edge {
            id: EdgeId::try_from("e-test").expect("valid EdgeId"),
            edge_type: EdgeTypeTag::Known(edge_type),
            source: NodeId::try_from("src").expect("valid NodeId"),
            target: NodeId::try_from("tgt").expect("valid NodeId"),
            identifiers: None,
            properties: EdgeProperties {
                extra: extra_properties,
                ..Default::default()
            },
            extra: BTreeMap::new(),
        }
    }

    fn make_edge_extension(extra_properties: BTreeMap<String, DynValue>) -> Edge {
        Edge {
            id: EdgeId::try_from("e-test-ext").expect("valid EdgeId"),
            edge_type: EdgeTypeTag::Extension("com.example.custom".to_owned()),
            source: NodeId::try_from("src").expect("valid NodeId"),
            target: NodeId::try_from("tgt").expect("valid NodeId"),
            identifiers: None,
            properties: EdgeProperties {
                extra: extra_properties,
                ..Default::default()
            },
            extra: BTreeMap::new(),
        }
    }

    fn no_extra() -> BTreeMap<String, DynValue> {
        BTreeMap::new()
    }

    fn property_sensitivity_extra(overrides: &[(&str, &str)]) -> BTreeMap<String, DynValue> {
        let mut obj = BTreeMap::new();
        for (k, v) in overrides {
            obj.insert(k.to_string(), DynValue::from(json!(v)));
        }
        let mut extra = BTreeMap::new();
        extra.insert("_property_sensitivity".to_owned(), DynValue::Object(obj));
        extra
    }

    #[test]
    fn scheme_lei_defaults_to_public() {
        let id = make_identifier("lei", None);
        let result = effective_sensitivity(&id, &known_node_type(NodeType::Organization));
        assert_eq!(result, Sensitivity::Public);
    }

    #[test]
    fn scheme_duns_defaults_to_public() {
        let id = make_identifier("duns", None);
        let result = effective_sensitivity(&id, &known_node_type(NodeType::Organization));
        assert_eq!(result, Sensitivity::Public);
    }

    #[test]
    fn scheme_gln_defaults_to_public() {
        let id = make_identifier("gln", None);
        let result = effective_sensitivity(&id, &known_node_type(NodeType::Facility));
        assert_eq!(result, Sensitivity::Public);
    }

    #[test]
    fn scheme_nat_reg_defaults_to_restricted() {
        let id = make_identifier("nat-reg", None);
        let result = effective_sensitivity(&id, &known_node_type(NodeType::Organization));
        assert_eq!(result, Sensitivity::Restricted);
    }

    #[test]
    fn scheme_vat_defaults_to_restricted() {
        let id = make_identifier("vat", None);
        let result = effective_sensitivity(&id, &known_node_type(NodeType::Organization));
        assert_eq!(result, Sensitivity::Restricted);
    }

    #[test]
    fn scheme_internal_defaults_to_restricted() {
        let id = make_identifier("internal", None);
        let result = effective_sensitivity(&id, &known_node_type(NodeType::Organization));
        assert_eq!(result, Sensitivity::Restricted);
    }

    #[test]
    fn unrecognised_scheme_defaults_to_public() {
        let id = make_identifier("com.example.custom", None);
        let result = effective_sensitivity(&id, &known_node_type(NodeType::Organization));
        assert_eq!(result, Sensitivity::Public);
    }

    #[test]
    fn unrecognised_scheme_on_extension_node_defaults_to_public() {
        let id = make_identifier("com.example.custom", None);
        let result = effective_sensitivity(&id, &extension_node_type("com.example.custom_node"));
        assert_eq!(result, Sensitivity::Public);
    }

    #[test]
    fn explicit_public_overrides_scheme_default_restricted() {
        // nat-reg defaults to restricted, but explicit public wins.
        let id = make_identifier("nat-reg", Some(Sensitivity::Public));
        let result = effective_sensitivity(&id, &known_node_type(NodeType::Organization));
        assert_eq!(result, Sensitivity::Public);
    }

    #[test]
    fn explicit_restricted_overrides_scheme_default_public() {
        // lei defaults to public, but explicit restricted wins.
        let id = make_identifier("lei", Some(Sensitivity::Restricted));
        let result = effective_sensitivity(&id, &known_node_type(NodeType::Organization));
        assert_eq!(result, Sensitivity::Restricted);
    }

    #[test]
    fn explicit_confidential_overrides_scheme_default_public() {
        let id = make_identifier("duns", Some(Sensitivity::Confidential));
        let result = effective_sensitivity(&id, &known_node_type(NodeType::Organization));
        assert_eq!(result, Sensitivity::Confidential);
    }

    #[test]
    fn person_node_lei_defaults_to_confidential() {
        let id = make_identifier("lei", None);
        let result = effective_sensitivity(&id, &known_node_type(NodeType::Person));
        assert_eq!(result, Sensitivity::Confidential);
    }

    #[test]
    fn person_node_gln_defaults_to_confidential() {
        let id = make_identifier("gln", None);
        let result = effective_sensitivity(&id, &known_node_type(NodeType::Person));
        assert_eq!(result, Sensitivity::Confidential);
    }

    #[test]
    fn person_node_nat_reg_defaults_to_confidential() {
        // Even schemes that default to restricted → confidential on person.
        let id = make_identifier("nat-reg", None);
        let result = effective_sensitivity(&id, &known_node_type(NodeType::Person));
        assert_eq!(result, Sensitivity::Confidential);
    }

    #[test]
    fn person_node_explicit_restricted_override_respected() {
        // Spec Section 2.2: explicit restricted override is permitted.
        let id = make_identifier("lei", Some(Sensitivity::Restricted));
        let result = effective_sensitivity(&id, &known_node_type(NodeType::Person));
        assert_eq!(result, Sensitivity::Restricted);
    }

    #[test]
    fn person_node_explicit_public_override_respected() {
        // Spec notes validators should flag this as suspect, but the engine
        // respects what the file declares.
        let id = make_identifier("lei", Some(Sensitivity::Public));
        let result = effective_sensitivity(&id, &known_node_type(NodeType::Person));
        assert_eq!(result, Sensitivity::Public);
    }

    #[test]
    fn person_node_explicit_confidential_override_is_same_as_default() {
        let id = make_identifier("lei", Some(Sensitivity::Confidential));
        let result = effective_sensitivity(&id, &known_node_type(NodeType::Person));
        assert_eq!(result, Sensitivity::Confidential);
    }

    #[test]
    fn facility_node_uses_scheme_default() {
        let id = make_identifier("lei", None);
        let result = effective_sensitivity(&id, &known_node_type(NodeType::Facility));
        assert_eq!(result, Sensitivity::Public);
    }

    #[test]
    fn good_node_uses_scheme_default() {
        let id = make_identifier("internal", None);
        let result = effective_sensitivity(&id, &known_node_type(NodeType::Good));
        assert_eq!(result, Sensitivity::Restricted);
    }

    #[test]
    fn attestation_node_uses_scheme_default() {
        let id = make_identifier("gln", None);
        let result = effective_sensitivity(&id, &known_node_type(NodeType::Attestation));
        assert_eq!(result, Sensitivity::Public);
    }

    #[test]
    fn consignment_node_uses_scheme_default() {
        let id = make_identifier("vat", None);
        let result = effective_sensitivity(&id, &known_node_type(NodeType::Consignment));
        assert_eq!(result, Sensitivity::Restricted);
    }

    #[test]
    fn boundary_ref_node_uses_scheme_default() {
        let id = make_identifier("lei", None);
        let result = effective_sensitivity(&id, &known_node_type(NodeType::BoundaryRef));
        assert_eq!(result, Sensitivity::Public);
    }

    #[test]
    fn contract_ref_defaults_to_restricted() {
        let edge = make_edge(EdgeType::Supplies, no_extra());
        assert_eq!(
            effective_property_sensitivity(&edge, "contract_ref"),
            Sensitivity::Restricted
        );
    }

    #[test]
    fn annual_value_defaults_to_restricted() {
        let edge = make_edge(EdgeType::Supplies, no_extra());
        assert_eq!(
            effective_property_sensitivity(&edge, "annual_value"),
            Sensitivity::Restricted
        );
    }

    #[test]
    fn value_currency_defaults_to_restricted() {
        let edge = make_edge(EdgeType::Supplies, no_extra());
        assert_eq!(
            effective_property_sensitivity(&edge, "value_currency"),
            Sensitivity::Restricted
        );
    }

    #[test]
    fn volume_defaults_to_restricted() {
        let edge = make_edge(EdgeType::Supplies, no_extra());
        assert_eq!(
            effective_property_sensitivity(&edge, "volume"),
            Sensitivity::Restricted
        );
    }

    #[test]
    fn volume_unit_defaults_to_public() {
        let edge = make_edge(EdgeType::Supplies, no_extra());
        assert_eq!(
            effective_property_sensitivity(&edge, "volume_unit"),
            Sensitivity::Public
        );
    }

    #[test]
    fn percentage_on_ownership_defaults_to_public() {
        let edge = make_edge(EdgeType::Ownership, no_extra());
        assert_eq!(
            effective_property_sensitivity(&edge, "percentage"),
            Sensitivity::Public
        );
    }

    #[test]
    fn percentage_on_beneficial_ownership_defaults_to_confidential() {
        let edge = make_edge(EdgeType::BeneficialOwnership, no_extra());
        assert_eq!(
            effective_property_sensitivity(&edge, "percentage"),
            Sensitivity::Confidential
        );
    }

    #[test]
    fn percentage_on_other_edge_type_defaults_to_public() {
        let edge = make_edge(EdgeType::Supplies, no_extra());
        assert_eq!(
            effective_property_sensitivity(&edge, "percentage"),
            Sensitivity::Public
        );
    }

    #[test]
    fn percentage_on_extension_edge_defaults_to_public() {
        let edge = make_edge_extension(no_extra());
        assert_eq!(
            effective_property_sensitivity(&edge, "percentage"),
            Sensitivity::Public
        );
    }

    #[test]
    fn unknown_property_defaults_to_public() {
        let edge = make_edge(EdgeType::Supplies, no_extra());
        assert_eq!(
            effective_property_sensitivity(&edge, "commodity"),
            Sensitivity::Public
        );
    }

    #[test]
    fn tier_property_defaults_to_public() {
        let edge = make_edge(EdgeType::Supplies, no_extra());
        assert_eq!(
            effective_property_sensitivity(&edge, "tier"),
            Sensitivity::Public
        );
    }

    #[test]
    fn property_sensitivity_override_public_on_restricted_default() {
        // volume normally restricted; override to public.
        let extra = property_sensitivity_extra(&[("volume", "public")]);
        let edge = make_edge(EdgeType::Supplies, extra);
        assert_eq!(
            effective_property_sensitivity(&edge, "volume"),
            Sensitivity::Public
        );
    }

    #[test]
    fn property_sensitivity_override_confidential_on_public_default() {
        // volume_unit normally public; override to confidential.
        let extra = property_sensitivity_extra(&[("volume_unit", "confidential")]);
        let edge = make_edge(EdgeType::Supplies, extra);
        assert_eq!(
            effective_property_sensitivity(&edge, "volume_unit"),
            Sensitivity::Confidential
        );
    }

    #[test]
    fn property_sensitivity_override_restricted_on_confidential_default() {
        // percentage on beneficial_ownership defaults to confidential; override to restricted.
        let extra = property_sensitivity_extra(&[("percentage", "restricted")]);
        let edge = make_edge(EdgeType::BeneficialOwnership, extra);
        assert_eq!(
            effective_property_sensitivity(&edge, "percentage"),
            Sensitivity::Restricted
        );
    }

    #[test]
    fn property_sensitivity_override_applies_only_to_named_property() {
        // Override volume_unit but volume should still be restricted.
        let extra = property_sensitivity_extra(&[("volume_unit", "confidential")]);
        let edge = make_edge(EdgeType::Supplies, extra);
        assert_eq!(
            effective_property_sensitivity(&edge, "volume"),
            Sensitivity::Restricted,
            "volume should use its default, not the volume_unit override"
        );
    }

    #[test]
    fn property_sensitivity_override_unrecognised_value_falls_through_to_default() {
        // Unrecognised sensitivity string falls through to the property default.
        let mut obj = BTreeMap::new();
        obj.insert("volume".to_owned(), DynValue::from(json!("ultra-secret")));
        let mut extra = BTreeMap::new();
        extra.insert("_property_sensitivity".to_owned(), DynValue::Object(obj));
        let edge = make_edge(EdgeType::Supplies, extra);
        // volume default is restricted.
        assert_eq!(
            effective_property_sensitivity(&edge, "volume"),
            Sensitivity::Restricted
        );
    }

    #[test]
    fn property_sensitivity_override_map_not_an_object_falls_through() {
        // _property_sensitivity is present but not a JSON object.
        let mut extra = BTreeMap::new();
        extra.insert(
            "_property_sensitivity".to_owned(),
            DynValue::from(json!("not-an-object")),
        );
        let edge = make_edge(EdgeType::Supplies, extra);
        assert_eq!(
            effective_property_sensitivity(&edge, "volume"),
            Sensitivity::Restricted
        );
    }

    #[test]
    fn property_sensitivity_override_value_not_a_string_falls_through() {
        // _property_sensitivity.volume is a number, not a string.
        let mut obj = BTreeMap::new();
        obj.insert("volume".to_owned(), DynValue::from(json!(1)));
        let mut extra = BTreeMap::new();
        extra.insert("_property_sensitivity".to_owned(), DynValue::Object(obj));
        let edge = make_edge(EdgeType::Supplies, extra);
        assert_eq!(
            effective_property_sensitivity(&edge, "volume"),
            Sensitivity::Restricted
        );
    }

    #[test]
    fn property_sensitivity_override_missing_key_falls_through_to_default() {
        // _property_sensitivity map present but the property we query is absent.
        let extra = property_sensitivity_extra(&[("contract_ref", "public")]);
        let edge = make_edge(EdgeType::Supplies, extra);
        // volume was not overridden → restricted default.
        assert_eq!(
            effective_property_sensitivity(&edge, "volume"),
            Sensitivity::Restricted
        );
    }

    #[test]
    fn no_property_sensitivity_map_uses_defaults() {
        let edge = make_edge(EdgeType::Supplies, no_extra());
        assert_eq!(
            effective_property_sensitivity(&edge, "contract_ref"),
            Sensitivity::Restricted
        );
        assert_eq!(
            effective_property_sensitivity(&edge, "volume_unit"),
            Sensitivity::Public
        );
    }

    #[test]
    fn percentage_on_operational_control_defaults_to_public() {
        let edge = make_edge(EdgeType::OperationalControl, no_extra());
        assert_eq!(
            effective_property_sensitivity(&edge, "percentage"),
            Sensitivity::Public
        );
    }

    #[test]
    fn percentage_on_legal_parentage_defaults_to_public() {
        let edge = make_edge(EdgeType::LegalParentage, no_extra());
        assert_eq!(
            effective_property_sensitivity(&edge, "percentage"),
            Sensitivity::Public
        );
    }

    #[test]
    fn percentage_on_subcontracts_defaults_to_public() {
        let edge = make_edge(EdgeType::Subcontracts, no_extra());
        assert_eq!(
            effective_property_sensitivity(&edge, "percentage"),
            Sensitivity::Public
        );
    }
}
