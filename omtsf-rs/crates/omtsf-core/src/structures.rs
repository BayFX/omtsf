/// Node, Edge, and `EdgeProperties` structs for the OMTSF graph data model.
///
/// This module defines the primary graph entity types as specified in
/// data-model.md Sections 5.1–5.3 and 6.1–6.3.
///
/// Key design decisions:
/// - All fields beyond `id` and `node_type`/`edge_type` are `Option<T>` so a
///   single struct covers all node/edge subtypes without enum overhead.
/// - `valid_to` uses `Option<Option<CalendarDate>>` to distinguish absent (field
///   omitted) from explicit `null` (open-ended validity). See
///   [`crate::serde_helpers::deserialize_optional_nullable`].
/// - `#[serde(flatten)] pub extra` on all three structs preserves unknown JSON
///   fields across round trips (SPEC-001 Section 2.2).
use serde::{Deserialize, Serialize};

use crate::enums::EdgeTypeTag;
use crate::enums::{
    AttestationOutcome, AttestationStatus, AttestationType, ConsolidationBasis,
    EmissionFactorSource, EventType, NodeTypeTag, OrganizationStatus, RiskLikelihood, RiskSeverity,
    ServiceType,
};
use crate::newtypes::{CalendarDate, CountryCode, EdgeId, NodeId};
use crate::types::parse_geo;
use crate::types::{DataQuality, Geo, GeoParseError, Identifier, Label};

// ---------------------------------------------------------------------------
// Node
// ---------------------------------------------------------------------------

/// A single node in an OMTSF supply-chain graph.
///
/// Corresponds to the node structure defined in data-model.md Section 5.1.
/// The `id` and `node_type` fields are required; every other field is optional
/// so that the same struct represents all built-in node subtypes (organization,
/// facility, good, person, attestation, consignment, `boundary_ref`) as well as
/// any extension types.
///
/// Unknown JSON fields are preserved in [`Node::extra`] for round-trip fidelity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Node {
    /// Unique identifier for this node within the file.
    pub id: NodeId,

    /// Node subtype (known built-in or extension string).
    #[serde(rename = "type")]
    pub node_type: NodeTypeTag,

    // ---- universal optional fields ----------------------------------------
    /// External or internal identifiers for this node (SPEC-002).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identifiers: Option<Vec<Identifier>>,

    /// Data quality metadata for this node (SPEC-001 Section 8.3).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_quality: Option<DataQuality>,

    /// Arbitrary key/value labels for tagging and filtering (SPEC-001 Section 8.4).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<Vec<Label>>,

    // ---- organization fields -----------------------------------------------
    /// Display name of the organisation or entity.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// ISO 3166-1 alpha-2 jurisdiction code of the organisation's registration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jurisdiction: Option<CountryCode>,

    /// Lifecycle status of the organisation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<OrganizationStatus>,

    /// Governance or ownership structure of the organisation.
    ///
    /// Stored as a raw JSON value because `GovernanceStructure` is not yet
    /// defined in a completed task. Once that type is implemented, this field
    /// can be narrowed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub governance_structure: Option<serde_json::Value>,

    // ---- facility fields ---------------------------------------------------
    /// [`NodeId`] of the organisation that operates this facility.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operator: Option<NodeId>,

    /// Human-readable address of the facility.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,

    /// Geographic location as a raw JSON value (`{lat, lon}` or `GeoJSON`).
    ///
    /// Use [`Node::geo_parsed`] to convert this into a typed [`Geo`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub geo: Option<serde_json::Value>,

    // ---- good fields -------------------------------------------------------
    /// Commodity or product classification code (e.g. HS code, CN code).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commodity_code: Option<String>,

    /// Unit of measure for this good or consignment (e.g. `"kg"`, `"mt"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,

    // ---- person fields -----------------------------------------------------
    /// Role of the individual within the supply chain context.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,

    // ---- attestation fields ------------------------------------------------
    /// Category of attestation (certification, audit, due diligence, etc.).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attestation_type: Option<AttestationType>,

    /// Name of the standard or scheme under which the attestation was issued.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub standard: Option<String>,

    /// Issuing body or certification authority.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub issuer: Option<String>,

    /// Date from which this attestation or identifier is valid.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub valid_from: Option<CalendarDate>,

    /// Expiry date of this attestation.
    ///
    /// Three-way distinction:
    /// - absent → `None` (not provided)
    /// - `null` → `Some(None)` (explicitly open-ended)
    /// - `"YYYY-MM-DD"` → `Some(Some(date))`
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::serde_helpers::deserialize_optional_nullable"
    )]
    pub valid_to: Option<Option<CalendarDate>>,

    /// Evaluation outcome of the attestation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outcome: Option<AttestationOutcome>,

    /// Lifecycle status of the attestation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attestation_status: Option<AttestationStatus>,

    /// External reference number or URL for this attestation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,

    /// Severity of the risk described by this node (for risk-type nodes).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub risk_severity: Option<RiskSeverity>,

    /// Likelihood of the risk described by this node materialising.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub risk_likelihood: Option<RiskLikelihood>,

    // ---- consignment fields ------------------------------------------------
    /// Production or shipment lot identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lot_id: Option<String>,

    /// Quantity of goods in this consignment (measured in [`Node::unit`]).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quantity: Option<f64>,

    /// Date on which the consignment was produced or dispatched.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub production_date: Option<CalendarDate>,

    /// ISO 3166-1 alpha-2 country of origin for this consignment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin_country: Option<CountryCode>,

    /// Direct (scope 1) CO2-equivalent emissions in kg.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direct_emissions_co2e: Option<f64>,

    /// Indirect (scope 2) CO2-equivalent emissions in kg.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indirect_emissions_co2e: Option<f64>,

    /// Source of the emissions factor used for CO2-equivalent calculations.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub emission_factor_source: Option<EmissionFactorSource>,

    /// [`NodeId`] of the CBAM installation associated with this consignment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub installation_id: Option<NodeId>,

    // ---- extension catch-all -----------------------------------------------
    /// Unknown fields preserved for round-trip fidelity (SPEC-001 Section 2.2).
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl Node {
    /// Parses the raw `geo` field into a typed [`Geo`] value.
    ///
    /// Returns `None` when the `geo` field is absent, or
    /// `Some(Ok(geo))` / `Some(Err(e))` when it is present.
    ///
    /// See [`parse_geo`] for the parsing heuristic (point vs. `GeoJSON`).
    pub fn geo_parsed(&self) -> Option<Result<Geo, GeoParseError>> {
        self.geo.as_ref().map(parse_geo)
    }
}

// ---------------------------------------------------------------------------
// EdgeProperties
// ---------------------------------------------------------------------------

/// Optional properties carried by an edge in an OMTSF supply-chain graph.
///
/// Corresponds to the edge properties defined in data-model.md Section 6.2.
/// All fields are optional; the applicable subset depends on the edge type.
/// Unknown JSON fields are preserved in [`EdgeProperties::extra`].
///
/// [`EdgeProperties`] implements [`Default`] so that an [`Edge`] whose JSON
/// representation omits the `properties` key still deserialises without error.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct EdgeProperties {
    // ---- universal optional fields ----------------------------------------
    /// Data quality metadata for this edge (SPEC-001 Section 8.3).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_quality: Option<DataQuality>,

    /// Arbitrary key/value labels for tagging and filtering (SPEC-001 Section 8.4).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<Vec<Label>>,

    /// Date from which this edge relationship is valid.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub valid_from: Option<CalendarDate>,

    /// Date on which this edge relationship ceases to be valid.
    ///
    /// Three-way distinction:
    /// - absent → `None`
    /// - `null` → `Some(None)` (open-ended)
    /// - `"YYYY-MM-DD"` → `Some(Some(date))`
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::serde_helpers::deserialize_optional_nullable"
    )]
    pub valid_to: Option<Option<CalendarDate>>,

    // ---- ownership / beneficial_ownership fields ---------------------------
    /// Percentage of ownership or beneficial ownership.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub percentage: Option<f64>,

    /// Whether the ownership or control is direct (as opposed to indirect).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direct: Option<bool>,

    // ---- operational_control / beneficial_ownership fields ----------------
    /// Type of operational control arrangement.
    ///
    /// Stored as a raw JSON value because extension strings are possible beyond
    /// the [`crate::ControlType`] enum variants.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub control_type: Option<serde_json::Value>,

    // ---- legal_parentage fields --------------------------------------------
    /// Accounting standard under which this parentage is consolidated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consolidation_basis: Option<ConsolidationBasis>,

    // ---- former_identity fields --------------------------------------------
    /// Type of corporate identity event (merger, acquisition, rename, etc.).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_type: Option<EventType>,

    /// Date on which the identity event took effect.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effective_date: Option<CalendarDate>,

    /// Human-readable description of the identity event.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    // ---- supplies / subcontracts / brokers / sells_to fields ---------------
    /// Commodity or material flowing along this edge.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commodity: Option<String>,

    /// Reference number for the underlying contract.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contract_ref: Option<String>,

    /// Volume of goods transferred.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume: Option<f64>,

    /// Unit of measure for [`EdgeProperties::volume`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume_unit: Option<String>,

    /// Annualised monetary value of this relationship.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annual_value: Option<f64>,

    /// Currency code for [`EdgeProperties::annual_value`] (ISO 4217).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_currency: Option<String>,

    /// Supply-chain tier number (1 = direct supplier).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tier: Option<u32>,

    /// This supplier's estimated share of the buyer's total demand (0–1).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub share_of_buyer_demand: Option<f64>,

    // ---- distributes fields ------------------------------------------------
    /// Type of distribution or logistics service provided.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_type: Option<ServiceType>,

    // ---- composed_of fields ------------------------------------------------
    /// Quantity of this component in the parent bill of materials.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quantity: Option<f64>,

    /// Unit of measure for [`EdgeProperties::quantity`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,

    // ---- attested_by fields ------------------------------------------------
    /// Scope or coverage description of the attestation relationship.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,

    // ---- extension catch-all -----------------------------------------------
    /// Unknown fields preserved for round-trip fidelity (SPEC-001 Section 2.2).
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

// ---------------------------------------------------------------------------
// Edge
// ---------------------------------------------------------------------------

/// A directed relationship between two nodes in an OMTSF supply-chain graph.
///
/// Corresponds to the edge structure defined in data-model.md Section 6.1.
/// The `id`, `edge_type`, `source`, and `target` fields are required; all
/// other fields are optional or have defaults.
///
/// Unknown JSON fields are preserved in [`Edge::extra`] for round-trip fidelity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Edge {
    /// Unique identifier for this edge within the file.
    pub id: EdgeId,

    /// Edge subtype (known built-in or extension string).
    #[serde(rename = "type")]
    pub edge_type: EdgeTypeTag,

    /// [`NodeId`] of the source (tail) node.
    pub source: NodeId,

    /// [`NodeId`] of the target (head) node.
    pub target: NodeId,

    /// External or internal identifiers for this edge (SPEC-002).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identifiers: Option<Vec<Identifier>>,

    /// Type-specific and universal edge properties.
    ///
    /// Defaults to an empty [`EdgeProperties`] when the field is absent.
    #[serde(default)]
    pub properties: EdgeProperties,

    /// Unknown fields preserved for round-trip fidelity (SPEC-001 Section 2.2).
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use serde_json::json;

    use super::*;
    use crate::enums::{EdgeType, NodeType};

    // --- helpers ------------------------------------------------------------

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

    // -----------------------------------------------------------------------
    // Node tests
    // -----------------------------------------------------------------------

    /// Minimal node with only the two required fields.
    #[test]
    fn node_minimal_round_trip() {
        let node = Node {
            id: node_id("org-1"),
            node_type: NodeTypeTag::Known(NodeType::Organization),
            identifiers: None,
            data_quality: None,
            labels: None,
            name: None,
            jurisdiction: None,
            status: None,
            governance_structure: None,
            operator: None,
            address: None,
            geo: None,
            commodity_code: None,
            unit: None,
            role: None,
            attestation_type: None,
            standard: None,
            issuer: None,
            valid_from: None,
            valid_to: None,
            outcome: None,
            attestation_status: None,
            reference: None,
            risk_severity: None,
            risk_likelihood: None,
            lot_id: None,
            quantity: None,
            production_date: None,
            origin_country: None,
            direct_emissions_co2e: None,
            indirect_emissions_co2e: None,
            emission_factor_source: None,
            installation_id: None,
            extra: serde_json::Map::new(),
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
            node.extra
                .get("x_version")
                .and_then(serde_json::Value::as_u64),
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
        let raw =
            r#"{"id":"f1","type":"facility","geo":{"type":"Point","coordinates":[125.6,10.1]}}"#;
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

    // -----------------------------------------------------------------------
    // EdgeProperties tests
    // -----------------------------------------------------------------------

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

    // -----------------------------------------------------------------------
    // Edge tests
    // -----------------------------------------------------------------------

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
            extra: serde_json::Map::new(),
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
        assert_eq!(props.control_type, Some(json!("franchise")));
        round_trip(&props);
    }
}
