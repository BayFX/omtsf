#![allow(clippy::expect_used)]

use crate::dynvalue::DynValue;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;

use super::*;
use crate::enums::{
    AttestationOutcome, AttestationType, ConsolidationBasis, EdgeType, EdgeTypeTag, EventType,
    NodeType, NodeTypeTag, ServiceType,
};
use crate::newtypes::{CalendarDate, EdgeId, NodeId};
use crate::types::Geo;

fn to_json<T: Serialize>(v: &T) -> String {
    serde_json::to_string(v).expect("serialize")
}

fn from_json<T: for<'de> Deserialize<'de>>(s: &str) -> T {
    serde_json::from_str(s).expect("deserialize")
}

fn round_trip<T>(v: &T) -> T
where
    T: Serialize + for<'de> Deserialize<'de> + std::fmt::Debug + PartialEq,
{
    let json = to_json(v);
    let back: T = from_json(&json);
    assert_eq!(*v, back, "round-trip mismatch for {json}");
    back
}

fn node_id(s: &str) -> NodeId {
    NodeId::try_from(s).expect("valid NodeId")
}

fn edge_id(s: &str) -> EdgeId {
    NodeId::try_from(s).expect("valid EdgeId")
}

fn calendar_date(s: &str) -> CalendarDate {
    CalendarDate::try_from(s).expect("valid CalendarDate")
}

/// Minimal node with only the two required fields.
#[test]
fn node_minimal_round_trip() {
    let node = Node {
        id: node_id("org-1"),
        node_type: NodeTypeTag::Known(NodeType::Organization),
        ..Default::default()
    };
    let rt = round_trip(&node);
    assert_eq!(rt.id, node_id("org-1"));
    assert_eq!(rt.node_type, NodeTypeTag::Known(NodeType::Organization));
}

/// Node with a known type and multiple optional fields populated.
#[test]
fn node_known_type_all_fields_round_trip() {
    let raw = r#"{
        "id": "attest-001",
        "type": "attestation",
        "attestation_type": "certification",
        "standard": "ISO 14001",
        "issuer": "Bureau Veritas",
        "valid_from": "2025-01-01",
        "valid_to": "2027-01-01",
        "outcome": "pass",
        "attestation_status": "active",
        "reference": "BV-2025-12345",
        "risk_severity": "low",
        "risk_likelihood": "unlikely"
    }"#;
    let node: Node = serde_json::from_str(raw).expect("deserialize");
    assert_eq!(node.id, node_id("attest-001"));
    assert_eq!(node.node_type, NodeTypeTag::Known(NodeType::Attestation));
    assert_eq!(node.attestation_type, Some(AttestationType::Certification));
    assert_eq!(node.standard.as_deref(), Some("ISO 14001"));
    assert_eq!(node.valid_from, Some(calendar_date("2025-01-01")));
    assert_eq!(node.valid_to, Some(Some(calendar_date("2027-01-01"))));
    assert_eq!(node.outcome, Some(AttestationOutcome::Pass));
    round_trip(&node);
}

/// Node with an extension (non-built-in) type string.
#[test]
fn node_extension_type_round_trip() {
    let raw = r#"{"id":"custom-1","type":"com.example.custom_node"}"#;
    let node: Node = serde_json::from_str(raw).expect("deserialize");
    assert_eq!(
        node.node_type,
        NodeTypeTag::Extension("com.example.custom_node".to_owned())
    );
    let serialized = to_json(&node);
    assert!(serialized.contains("com.example.custom_node"));
    round_trip(&node);
}

/// `valid_to` absent → `None`.
#[test]
fn node_valid_to_absent_is_none() {
    let raw = r#"{"id":"n1","type":"organization"}"#;
    let node: Node = serde_json::from_str(raw).expect("deserialize");
    assert_eq!(node.valid_to, None);
    let serialized = to_json(&node);
    assert!(!serialized.contains("valid_to"));
}

/// `valid_to: null` → `Some(None)`.
#[test]
fn node_valid_to_null_is_some_none() {
    let raw = r#"{"id":"n1","type":"attestation","valid_to":null}"#;
    let node: Node = serde_json::from_str(raw).expect("deserialize");
    assert_eq!(node.valid_to, Some(None));
    let serialized = to_json(&node);
    assert!(serialized.contains(r#""valid_to":null"#));
}

/// `valid_to: "2030-12-31"` → `Some(Some(date))`.
#[test]
fn node_valid_to_date_is_some_some() {
    let raw = r#"{"id":"n1","type":"attestation","valid_to":"2030-12-31"}"#;
    let node: Node = serde_json::from_str(raw).expect("deserialize");
    assert_eq!(node.valid_to, Some(Some(calendar_date("2030-12-31"))));
}

/// Unknown fields are preserved in `extra`.
#[test]
fn node_unknown_fields_preserved() {
    let raw = r#"{"id":"n1","type":"organization","x_custom":"hello","x_version":42}"#;
    let node: Node = serde_json::from_str(raw).expect("deserialize");
    assert_eq!(
        node.extra.get("x_custom").and_then(|v| v.as_str()),
        Some("hello")
    );
    assert_eq!(
        node.extra.get("x_version").and_then(DynValue::as_u64),
        Some(42)
    );
    let serialized = to_json(&node);
    assert!(serialized.contains("x_custom"));
    assert!(serialized.contains("x_version"));
}

/// `geo_parsed()` returns `None` when `geo` is absent.
#[test]
fn node_geo_parsed_absent_is_none() {
    let raw = r#"{"id":"f1","type":"facility"}"#;
    let node: Node = serde_json::from_str(raw).expect("deserialize");
    assert!(node.geo_parsed().is_none());
}

/// `geo_parsed()` returns `Some(Ok(Point))` for a `{lat, lon}` object.
#[test]
fn node_geo_parsed_point() {
    let raw = r#"{"id":"f1","type":"facility","geo":{"lat":51.5074,"lon":-0.1278}}"#;
    let node: Node = serde_json::from_str(raw).expect("deserialize");
    let geo = node
        .geo_parsed()
        .expect("geo present")
        .expect("geo parses ok");
    assert_eq!(
        geo,
        Geo::Point {
            lat: 51.5074,
            lon: -0.1278
        }
    );
}

/// `geo_parsed()` returns `Some(Err(_))` for a non-object `geo` value.
#[test]
fn node_geo_parsed_error_on_non_object() {
    let raw = r#"{"id":"f1","type":"facility","geo":"not-an-object"}"#;
    let node: Node = serde_json::from_str(raw).expect("deserialize");
    assert!(node.geo_parsed().expect("geo present").is_err());
}

/// `geo_parsed()` returns `Some(Ok(GeoJson))` for a `GeoJSON` geometry.
#[test]
fn node_geo_parsed_geojson() {
    let raw = r#"{"id":"f1","type":"facility","geo":{"type":"Point","coordinates":[125.6,10.1]}}"#;
    let node: Node = serde_json::from_str(raw).expect("deserialize");
    let geo = node
        .geo_parsed()
        .expect("geo present")
        .expect("geo parses ok");
    assert!(matches!(geo, Geo::GeoJson(_)));
}

/// Facility node with address, operator, and geo.
#[test]
fn node_facility_fields_round_trip() {
    let raw = r#"{
        "id": "fac-42",
        "type": "facility",
        "name": "Acme Plant",
        "operator": "org-1",
        "address": "123 Industrial Ave",
        "geo": {"lat": 48.8566, "lon": 2.3522}
    }"#;
    let node: Node = serde_json::from_str(raw).expect("deserialize");
    assert_eq!(node.operator, Some(node_id("org-1")));
    assert_eq!(node.address.as_deref(), Some("123 Industrial Ave"));
    round_trip(&node);
}

/// Consignment node with emissions fields.
#[test]
fn node_consignment_fields_round_trip() {
    let raw = r#"{
        "id": "cons-7",
        "type": "consignment",
        "lot_id": "LOT-2025-001",
        "quantity": 1000.5,
        "unit": "mt",
        "production_date": "2025-03-15",
        "origin_country": "DE",
        "direct_emissions_co2e": 4500.0,
        "indirect_emissions_co2e": 220.0,
        "emission_factor_source": "actual"
    }"#;
    let node: Node = serde_json::from_str(raw).expect("deserialize");
    assert_eq!(node.lot_id.as_deref(), Some("LOT-2025-001"));
    assert_eq!(node.quantity, Some(1000.5_f64));
    assert_eq!(node.direct_emissions_co2e, Some(4500.0_f64));
    round_trip(&node);
}

/// Empty [`EdgeProperties`] round-trips correctly.
#[test]
fn edge_properties_empty_round_trip() {
    let props = EdgeProperties::default();
    round_trip(&props);
}

/// [`EdgeProperties`] with ownership fields.
#[test]
fn edge_properties_ownership_round_trip() {
    let raw = r#"{"percentage":51.0,"direct":true,"valid_from":"2020-01-01","valid_to":null}"#;
    let props: EdgeProperties = serde_json::from_str(raw).expect("deserialize");
    assert_eq!(props.percentage, Some(51.0_f64));
    assert_eq!(props.direct, Some(true));
    assert_eq!(props.valid_from, Some(calendar_date("2020-01-01")));
    assert_eq!(props.valid_to, Some(None));
    round_trip(&props);
}

/// `valid_to` absent → `None` on [`EdgeProperties`].
#[test]
fn edge_properties_valid_to_absent_is_none() {
    let props: EdgeProperties = serde_json::from_str("{}").expect("deserialize");
    assert_eq!(props.valid_to, None);
    let serialized = to_json(&props);
    assert!(!serialized.contains("valid_to"));
}

/// `valid_to: null` → `Some(None)` on [`EdgeProperties`].
#[test]
fn edge_properties_valid_to_null_is_some_none() {
    let raw = r#"{"valid_to":null}"#;
    let props: EdgeProperties = serde_json::from_str(raw).expect("deserialize");
    assert_eq!(props.valid_to, Some(None));
}

/// Unknown fields are preserved in [`EdgeProperties::extra`].
#[test]
fn edge_properties_unknown_fields_preserved() {
    let raw = r#"{"percentage":25.0,"x_notes":"audited"}"#;
    let props: EdgeProperties = serde_json::from_str(raw).expect("deserialize");
    assert_eq!(
        props.extra.get("x_notes").and_then(|v| v.as_str()),
        Some("audited")
    );
    let serialized = to_json(&props);
    assert!(serialized.contains("x_notes"));
}

/// Minimal edge with only required fields.
#[test]
fn edge_minimal_round_trip() {
    let edge = Edge {
        id: edge_id("e-1"),
        edge_type: EdgeTypeTag::Known(EdgeType::Supplies),
        source: node_id("org-1"),
        target: node_id("org-2"),
        identifiers: None,
        properties: EdgeProperties::default(),
        extra: BTreeMap::new(),
    };
    let rt = round_trip(&edge);
    assert_eq!(rt.id, edge_id("e-1"));
    assert_eq!(rt.edge_type, EdgeTypeTag::Known(EdgeType::Supplies));
}

/// Edge with a known type and properties populated.
#[test]
fn edge_known_type_with_properties_round_trip() {
    let raw = r#"{
        "id": "e-ownership-1",
        "type": "ownership",
        "source": "parent-org",
        "target": "child-org",
        "properties": {
            "percentage": 100.0,
            "direct": true,
            "valid_from": "2015-06-01"
        }
    }"#;
    let edge: Edge = serde_json::from_str(raw).expect("deserialize");
    assert_eq!(edge.edge_type, EdgeTypeTag::Known(EdgeType::Ownership));
    assert_eq!(edge.source, node_id("parent-org"));
    assert_eq!(edge.target, node_id("child-org"));
    assert_eq!(edge.properties.percentage, Some(100.0_f64));
    assert_eq!(edge.properties.direct, Some(true));
    round_trip(&edge);
}

/// Edge with an extension (non-built-in) type string.
#[test]
fn edge_extension_type_round_trip() {
    let raw = r#"{
        "id": "e-custom-1",
        "type": "com.acme.custom_relationship",
        "source": "node-a",
        "target": "node-b"
    }"#;
    let edge: Edge = serde_json::from_str(raw).expect("deserialize");
    assert_eq!(
        edge.edge_type,
        EdgeTypeTag::Extension("com.acme.custom_relationship".to_owned())
    );
    let serialized = to_json(&edge);
    assert!(serialized.contains("com.acme.custom_relationship"));
    round_trip(&edge);
}

/// Edge without explicit `properties` key uses default [`EdgeProperties`].
#[test]
fn edge_absent_properties_uses_default() {
    let raw = r#"{"id":"e1","type":"supplies","source":"s1","target":"t1"}"#;
    let edge: Edge = serde_json::from_str(raw).expect("deserialize");
    assert_eq!(edge.properties, EdgeProperties::default());
}

/// Unknown fields on [`Edge`] are preserved in `extra`.
#[test]
fn edge_unknown_fields_preserved() {
    let raw = r#"{
        "id": "e1",
        "type": "supplies",
        "source": "s1",
        "target": "t1",
        "x_annotation": "reviewed",
        "x_priority": 1
    }"#;
    let edge: Edge = serde_json::from_str(raw).expect("deserialize");
    assert_eq!(
        edge.extra.get("x_annotation").and_then(|v| v.as_str()),
        Some("reviewed")
    );
    let serialized = to_json(&edge);
    assert!(serialized.contains("x_annotation"));
}

/// Edge with identifiers array.
#[test]
fn edge_with_identifiers_round_trip() {
    let raw = r#"{
        "id": "e-attested-1",
        "type": "attested_by",
        "source": "facility-1",
        "target": "cert-1",
        "identifiers": [{"scheme": "internal", "value": "ref-99"}],
        "properties": {"scope": "full site"}
    }"#;
    let edge: Edge = serde_json::from_str(raw).expect("deserialize");
    assert!(edge.identifiers.is_some());
    let ids = edge.identifiers.as_ref().expect("identifiers present");
    assert_eq!(ids.len(), 1);
    assert_eq!(ids[0].scheme, "internal");
    assert_eq!(edge.properties.scope.as_deref(), Some("full site"));
    round_trip(&edge);
}

/// Composed-of edge with quantity and unit in properties.
#[test]
fn edge_composed_of_properties_round_trip() {
    let raw = r#"{
        "id": "e-bom-1",
        "type": "composed_of",
        "source": "product-a",
        "target": "component-b",
        "properties": {
            "quantity": 3.0,
            "unit": "pcs"
        }
    }"#;
    let edge: Edge = serde_json::from_str(raw).expect("deserialize");
    assert_eq!(edge.properties.quantity, Some(3.0_f64));
    assert_eq!(edge.properties.unit.as_deref(), Some("pcs"));
    round_trip(&edge);
}

/// Supplies edge with volume and value fields.
#[test]
fn edge_supplies_properties_round_trip() {
    let raw = r#"{
        "id": "e-sup-1",
        "type": "supplies",
        "source": "supplier-1",
        "target": "buyer-1",
        "properties": {
            "commodity": "steel_coil",
            "volume": 5000.0,
            "volume_unit": "mt",
            "annual_value": 2500000.0,
            "value_currency": "EUR",
            "tier": 1,
            "share_of_buyer_demand": 0.35
        }
    }"#;
    let edge: Edge = serde_json::from_str(raw).expect("deserialize");
    assert_eq!(edge.properties.commodity.as_deref(), Some("steel_coil"));
    assert_eq!(edge.properties.tier, Some(1_u32));
    assert_eq!(edge.properties.share_of_buyer_demand, Some(0.35_f64));
    round_trip(&edge);
}

/// Distributes edge with `service_type` in properties.
#[test]
fn edge_distributes_service_type_round_trip() {
    let raw = r#"{
        "id": "e-dist-1",
        "type": "distributes",
        "source": "3pl-1",
        "target": "retailer-1",
        "properties": {"service_type": "transport"}
    }"#;
    let edge: Edge = serde_json::from_str(raw).expect("deserialize");
    assert_eq!(edge.properties.service_type, Some(ServiceType::Transport));
    round_trip(&edge);
}

/// Node: required fields must be present; missing `id` fails.
#[test]
fn node_missing_id_fails() {
    let raw = r#"{"type":"organization"}"#;
    let result: Result<Node, _> = serde_json::from_str(raw);
    assert!(result.is_err(), "missing id should fail deserialization");
}

/// Node: required fields must be present; missing `type` fails.
#[test]
fn node_missing_type_fails() {
    let raw = r#"{"id":"n1"}"#;
    let result: Result<Node, _> = serde_json::from_str(raw);
    assert!(result.is_err(), "missing type should fail deserialization");
}

/// Edge: required fields must be present; missing `source` fails.
#[test]
fn edge_missing_source_fails() {
    let raw = r#"{"id":"e1","type":"supplies","target":"t1"}"#;
    let result: Result<Edge, _> = serde_json::from_str(raw);
    assert!(
        result.is_err(),
        "missing source should fail deserialization"
    );
}

/// Verify that [`Node`] serializes `type` (not `node_type`) as the JSON key.
#[test]
fn node_type_serializes_as_type_key() {
    let raw = r#"{"id":"n1","type":"good"}"#;
    let node: Node = serde_json::from_str(raw).expect("deserialize");
    let serialized = to_json(&node);
    assert!(serialized.contains(r#""type":"good""#));
    assert!(!serialized.contains("node_type"));
}

/// Verify that [`Edge`] serializes `type` (not `edge_type`) as the JSON key.
#[test]
fn edge_type_serializes_as_type_key() {
    let raw = r#"{"id":"e1","type":"sells_to","source":"s1","target":"t1"}"#;
    let edge: Edge = serde_json::from_str(raw).expect("deserialize");
    let serialized = to_json(&edge);
    assert!(serialized.contains(r#""type":"sells_to""#));
    assert!(!serialized.contains("edge_type"));
}

/// Former identity edge with `event_type` and `effective_date`.
#[test]
fn edge_former_identity_round_trip() {
    let raw = r#"{
        "id": "e-fi-1",
        "type": "former_identity",
        "source": "old-org",
        "target": "new-org",
        "properties": {
            "event_type": "merger",
            "effective_date": "2022-07-01",
            "description": "Merged into NewCo"
        }
    }"#;
    let edge: Edge = serde_json::from_str(raw).expect("deserialize");
    assert_eq!(edge.properties.event_type, Some(EventType::Merger));
    assert_eq!(
        edge.properties.effective_date,
        Some(calendar_date("2022-07-01"))
    );
    assert_eq!(
        edge.properties.description.as_deref(),
        Some("Merged into NewCo")
    );
    round_trip(&edge);
}

/// Legal parentage edge with `consolidation_basis`.
#[test]
fn edge_legal_parentage_round_trip() {
    let raw = r#"{
        "id": "e-lp-1",
        "type": "legal_parentage",
        "source": "parent-1",
        "target": "subsidiary-1",
        "properties": {"consolidation_basis": "ifrs10"}
    }"#;
    let edge: Edge = serde_json::from_str(raw).expect("deserialize");
    assert_eq!(
        edge.properties.consolidation_basis,
        Some(ConsolidationBasis::Ifrs10)
    );
    round_trip(&edge);
}

/// [`EdgeProperties`] `control_type` stored as raw JSON value.
#[test]
fn edge_properties_control_type_raw_value() {
    let raw = r#"{"control_type":"franchise"}"#;
    let props: EdgeProperties = serde_json::from_str(raw).expect("deserialize");
    assert_eq!(props.control_type, Some(DynValue::from(json!("franchise"))));
    round_trip(&props);
}
