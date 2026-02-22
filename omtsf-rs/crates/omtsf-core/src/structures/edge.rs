use serde::{Deserialize, Serialize};

use crate::dynvalue::{DynMap, DynValue};
use crate::enums::{ConsolidationBasis, EdgeTypeTag, EventType, ServiceType};
use crate::newtypes::{CalendarDate, EdgeId, NodeId};
use crate::types::{DataQuality, Identifier, Label};

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

    /// Percentage of ownership or beneficial ownership.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub percentage: Option<f64>,

    /// Whether the ownership or control is direct (as opposed to indirect).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direct: Option<bool>,

    /// Type of operational control arrangement.
    ///
    /// Stored as a raw dynamic value because extension strings are possible beyond
    /// the [`crate::ControlType`] enum variants.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub control_type: Option<DynValue>,

    /// Accounting standard under which this parentage is consolidated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consolidation_basis: Option<ConsolidationBasis>,

    /// Type of corporate identity event (merger, acquisition, rename, etc.).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_type: Option<EventType>,

    /// Date on which the identity event took effect.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effective_date: Option<CalendarDate>,

    /// Human-readable description of the identity event.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

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

    /// Type of distribution or logistics service provided.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_type: Option<ServiceType>,

    /// Quantity of this component in the parent bill of materials.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quantity: Option<f64>,

    /// Unit of measure for [`EdgeProperties::quantity`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,

    /// Scope or coverage description of the attestation relationship.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,

    /// Unknown fields preserved for round-trip fidelity (SPEC-001 Section 2.2).
    #[serde(flatten)]
    pub extra: DynMap,
}

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
    pub extra: DynMap,
}
