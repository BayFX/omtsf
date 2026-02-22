use serde::{Deserialize, Serialize};

use crate::dynvalue::{DynMap, DynValue};
use crate::enums::{
    AttestationOutcome, AttestationStatus, AttestationType, EmissionFactorSource, NodeTypeTag,
    OrganizationStatus, RiskLikelihood, RiskSeverity,
};
use crate::newtypes::{CalendarDate, CountryCode, NodeId};
use crate::types::parse_geo;
use crate::types::{DataQuality, Geo, GeoParseError, Identifier, Label};

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

    /// External or internal identifiers for this node (SPEC-002).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identifiers: Option<Vec<Identifier>>,

    /// Data quality metadata for this node (SPEC-001 Section 8.3).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_quality: Option<DataQuality>,

    /// Arbitrary key/value labels for tagging and filtering (SPEC-001 Section 8.4).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<Vec<Label>>,

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
    /// Stored as a raw dynamic value because `GovernanceStructure` is not yet
    /// defined in a completed task. Once that type is implemented, this field
    /// can be narrowed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub governance_structure: Option<DynValue>,

    /// [`NodeId`] of the organisation that operates this facility.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operator: Option<NodeId>,

    /// Human-readable address of the facility.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,

    /// Geographic location as a raw dynamic value (`{lat, lon}` or `GeoJSON`).
    ///
    /// Use [`Node::geo_parsed`] to convert this into a typed [`Geo`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub geo: Option<DynValue>,

    /// Commodity or product classification code (e.g. HS code, CN code).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commodity_code: Option<String>,

    /// Unit of measure for this good or consignment (e.g. `"kg"`, `"mt"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,

    /// Role of the individual within the supply chain context.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,

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

    /// Unknown fields preserved for round-trip fidelity (SPEC-001 Section 2.2).
    #[serde(flatten)]
    pub extra: DynMap,
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
