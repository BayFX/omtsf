/// All enums defined in the OMTSF graph data model (SPEC-001 Sections 4.1–4.4).
///
/// Each enum serializes to/from `snake_case` JSON strings. `NodeTypeTag` and
/// `EdgeTypeTag` additionally support extension strings via their `Extension`
/// variant.
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

/// File-level disclosure scope declaration (SPEC-001 Section 2, SPEC-004 Section 3).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DisclosureScope {
    /// Intended for internal use only.
    Internal,
    /// May be shared with direct trading partners.
    Partner,
    /// No restrictions on sharing.
    Public,
}

/// Known node types defined by the OMTSF core specification (SPEC-001 Section 4).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeType {
    /// A legal entity or organisation in the supply chain.
    Organization,
    /// A physical location where production, storage, or processing occurs.
    Facility,
    /// A product or commodity flowing through the supply chain.
    Good,
    /// An individual associated with an organisation or facility.
    Person,
    /// A certification, audit, due diligence statement, or similar record.
    Attestation,
    /// A specific batch or shipment of goods.
    Consignment,
    /// A placeholder for a node that exists outside the disclosure boundary.
    BoundaryRef,
}

/// The `type` field on a node: either a known [`NodeType`] or an extension string.
///
/// Extension types follow the reverse-domain notation convention defined in
/// SPEC-001 Section 8.1 (e.g. `"com.example.custom_node"`), but any unknown
/// string is accepted without error — rejection is a validation concern.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NodeTypeTag {
    /// A node type recognised by this version of omtsf-core.
    Known(NodeType),
    /// An extension or future node type not yet recognised by this crate.
    Extension(String),
}

impl NodeTypeTag {
    /// Returns the `snake_case` string representation of the tag.
    ///
    /// For known variants this is a `&'static str` with no allocation.
    /// For extension variants the inner `String` is returned by reference.
    pub fn as_str(&self) -> &str {
        match self {
            NodeTypeTag::Known(NodeType::Organization) => "organization",
            NodeTypeTag::Known(NodeType::Facility) => "facility",
            NodeTypeTag::Known(NodeType::Good) => "good",
            NodeTypeTag::Known(NodeType::Person) => "person",
            NodeTypeTag::Known(NodeType::Attestation) => "attestation",
            NodeTypeTag::Known(NodeType::Consignment) => "consignment",
            NodeTypeTag::Known(NodeType::BoundaryRef) => "boundary_ref",
            NodeTypeTag::Extension(s) => s.as_str(),
        }
    }
}

impl Default for NodeTypeTag {
    /// Returns `NodeTypeTag::Known(NodeType::Organization)` as the sentinel default.
    ///
    /// Used by [`Node::default`] so that struct update syntax works in tests
    /// without specifying a `node_type`.
    fn default() -> Self {
        Self::Known(NodeType::Organization)
    }
}

impl AsRef<str> for NodeTypeTag {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Serialize for NodeTypeTag {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            NodeTypeTag::Known(t) => t.serialize(serializer),
            NodeTypeTag::Extension(s) => serializer.serialize_str(s),
        }
    }
}

impl<'de> Deserialize<'de> for NodeTypeTag {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct NodeTypeTagVisitor;

        impl de::Visitor<'_> for NodeTypeTagVisitor {
            type Value = NodeTypeTag;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a string representing a node type")
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
                Ok(match v {
                    "organization" => NodeTypeTag::Known(NodeType::Organization),
                    "facility" => NodeTypeTag::Known(NodeType::Facility),
                    "good" => NodeTypeTag::Known(NodeType::Good),
                    "person" => NodeTypeTag::Known(NodeType::Person),
                    "attestation" => NodeTypeTag::Known(NodeType::Attestation),
                    "consignment" => NodeTypeTag::Known(NodeType::Consignment),
                    "boundary_ref" => NodeTypeTag::Known(NodeType::BoundaryRef),
                    other => NodeTypeTag::Extension(other.to_owned()),
                })
            }

            fn visit_string<E: de::Error>(self, v: String) -> Result<Self::Value, E> {
                match v.as_str() {
                    "organization" => Ok(NodeTypeTag::Known(NodeType::Organization)),
                    "facility" => Ok(NodeTypeTag::Known(NodeType::Facility)),
                    "good" => Ok(NodeTypeTag::Known(NodeType::Good)),
                    "person" => Ok(NodeTypeTag::Known(NodeType::Person)),
                    "attestation" => Ok(NodeTypeTag::Known(NodeType::Attestation)),
                    "consignment" => Ok(NodeTypeTag::Known(NodeType::Consignment)),
                    "boundary_ref" => Ok(NodeTypeTag::Known(NodeType::BoundaryRef)),
                    _ => Ok(NodeTypeTag::Extension(v)),
                }
            }
        }

        deserializer.deserialize_str(NodeTypeTagVisitor)
    }
}

/// Known edge types defined by the OMTSF core specification (SPEC-001 Sections 5-7).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeType {
    /// Equity ownership relationship.
    Ownership,
    /// Operational control without equity ownership.
    OperationalControl,
    /// Legal parentage / consolidation relationship.
    LegalParentage,
    /// Records a past identity event such as a merger or rename.
    FormerIdentity,
    /// Ultimate beneficial ownership relationship.
    BeneficialOwnership,
    /// Supply of goods or materials from one entity to another.
    Supplies,
    /// Subcontracting arrangement.
    Subcontracts,
    /// Tolling or processing arrangement.
    Tolls,
    /// Distribution or logistics service.
    Distributes,
    /// Brokering arrangement.
    Brokers,
    /// Operational relationship between an organisation and a facility.
    Operates,
    /// A facility or entity produces a specific good or consignment.
    Produces,
    /// Bill-of-materials composition link.
    ComposedOf,
    /// Commercial sale from one entity to another.
    SellsTo,
    /// A node is covered or certified by an attestation.
    AttestedBy,
    /// Two nodes represent the same real-world entity.
    SameAs,
}

/// The `type` field on an edge: either a known [`EdgeType`] or an extension string.
///
/// Mirrors the semantics of [`NodeTypeTag`] — unknown strings are accepted and
/// stored as `Extension` rather than causing a deserialization error.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EdgeTypeTag {
    /// An edge type recognised by this version of omtsf-core.
    Known(EdgeType),
    /// An extension or future edge type not yet recognised by this crate.
    Extension(String),
}

impl EdgeTypeTag {
    /// Returns the `snake_case` string representation of the tag.
    ///
    /// For known variants this is a `&'static str` with no allocation.
    /// For extension variants the inner `String` is returned by reference.
    pub fn as_str(&self) -> &str {
        match self {
            EdgeTypeTag::Known(EdgeType::Ownership) => "ownership",
            EdgeTypeTag::Known(EdgeType::OperationalControl) => "operational_control",
            EdgeTypeTag::Known(EdgeType::LegalParentage) => "legal_parentage",
            EdgeTypeTag::Known(EdgeType::FormerIdentity) => "former_identity",
            EdgeTypeTag::Known(EdgeType::BeneficialOwnership) => "beneficial_ownership",
            EdgeTypeTag::Known(EdgeType::Supplies) => "supplies",
            EdgeTypeTag::Known(EdgeType::Subcontracts) => "subcontracts",
            EdgeTypeTag::Known(EdgeType::Tolls) => "tolls",
            EdgeTypeTag::Known(EdgeType::Distributes) => "distributes",
            EdgeTypeTag::Known(EdgeType::Brokers) => "brokers",
            EdgeTypeTag::Known(EdgeType::Operates) => "operates",
            EdgeTypeTag::Known(EdgeType::Produces) => "produces",
            EdgeTypeTag::Known(EdgeType::ComposedOf) => "composed_of",
            EdgeTypeTag::Known(EdgeType::SellsTo) => "sells_to",
            EdgeTypeTag::Known(EdgeType::AttestedBy) => "attested_by",
            EdgeTypeTag::Known(EdgeType::SameAs) => "same_as",
            EdgeTypeTag::Extension(s) => s.as_str(),
        }
    }
}

impl AsRef<str> for EdgeTypeTag {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Serialize for EdgeTypeTag {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            EdgeTypeTag::Known(t) => t.serialize(serializer),
            EdgeTypeTag::Extension(s) => serializer.serialize_str(s),
        }
    }
}

impl<'de> Deserialize<'de> for EdgeTypeTag {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct EdgeTypeTagVisitor;

        impl de::Visitor<'_> for EdgeTypeTagVisitor {
            type Value = EdgeTypeTag;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a string representing an edge type")
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
                Ok(match v {
                    "ownership" => EdgeTypeTag::Known(EdgeType::Ownership),
                    "operational_control" => EdgeTypeTag::Known(EdgeType::OperationalControl),
                    "legal_parentage" => EdgeTypeTag::Known(EdgeType::LegalParentage),
                    "former_identity" => EdgeTypeTag::Known(EdgeType::FormerIdentity),
                    "beneficial_ownership" => EdgeTypeTag::Known(EdgeType::BeneficialOwnership),
                    "supplies" => EdgeTypeTag::Known(EdgeType::Supplies),
                    "subcontracts" => EdgeTypeTag::Known(EdgeType::Subcontracts),
                    "tolls" => EdgeTypeTag::Known(EdgeType::Tolls),
                    "distributes" => EdgeTypeTag::Known(EdgeType::Distributes),
                    "brokers" => EdgeTypeTag::Known(EdgeType::Brokers),
                    "operates" => EdgeTypeTag::Known(EdgeType::Operates),
                    "produces" => EdgeTypeTag::Known(EdgeType::Produces),
                    "composed_of" => EdgeTypeTag::Known(EdgeType::ComposedOf),
                    "sells_to" => EdgeTypeTag::Known(EdgeType::SellsTo),
                    "attested_by" => EdgeTypeTag::Known(EdgeType::AttestedBy),
                    "same_as" => EdgeTypeTag::Known(EdgeType::SameAs),
                    other => EdgeTypeTag::Extension(other.to_owned()),
                })
            }

            fn visit_string<E: de::Error>(self, v: String) -> Result<Self::Value, E> {
                match v.as_str() {
                    "ownership" => Ok(EdgeTypeTag::Known(EdgeType::Ownership)),
                    "operational_control" => Ok(EdgeTypeTag::Known(EdgeType::OperationalControl)),
                    "legal_parentage" => Ok(EdgeTypeTag::Known(EdgeType::LegalParentage)),
                    "former_identity" => Ok(EdgeTypeTag::Known(EdgeType::FormerIdentity)),
                    "beneficial_ownership" => Ok(EdgeTypeTag::Known(EdgeType::BeneficialOwnership)),
                    "supplies" => Ok(EdgeTypeTag::Known(EdgeType::Supplies)),
                    "subcontracts" => Ok(EdgeTypeTag::Known(EdgeType::Subcontracts)),
                    "tolls" => Ok(EdgeTypeTag::Known(EdgeType::Tolls)),
                    "distributes" => Ok(EdgeTypeTag::Known(EdgeType::Distributes)),
                    "brokers" => Ok(EdgeTypeTag::Known(EdgeType::Brokers)),
                    "operates" => Ok(EdgeTypeTag::Known(EdgeType::Operates)),
                    "produces" => Ok(EdgeTypeTag::Known(EdgeType::Produces)),
                    "composed_of" => Ok(EdgeTypeTag::Known(EdgeType::ComposedOf)),
                    "sells_to" => Ok(EdgeTypeTag::Known(EdgeType::SellsTo)),
                    "attested_by" => Ok(EdgeTypeTag::Known(EdgeType::AttestedBy)),
                    "same_as" => Ok(EdgeTypeTag::Known(EdgeType::SameAs)),
                    _ => Ok(EdgeTypeTag::Extension(v)),
                }
            }
        }

        deserializer.deserialize_str(EdgeTypeTagVisitor)
    }
}

/// Type of attestation record (SPEC-001 Section 4.5).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttestationType {
    /// A third-party certification against a defined standard.
    Certification,
    /// An independent audit finding.
    Audit,
    /// A due diligence statement (e.g. EUDR DDS).
    DueDiligenceStatement,
    /// A self-declaration by the entity itself.
    SelfDeclaration,
    /// Any other type of attestation.
    Other,
}

/// Data quality confidence level (SPEC-001 Section 8.3).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    /// Confirmed against an authoritative source.
    Verified,
    /// Reported by the entity without independent verification.
    Reported,
    /// Derived by inference from other data.
    Inferred,
    /// Estimated using a model or default factor.
    Estimated,
}

/// Identifier or property sensitivity level (SPEC-004 Section 2).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Sensitivity {
    /// No restrictions on sharing.
    Public,
    /// Share only with direct trading partners.
    Restricted,
    /// Do not share outside the originating organisation.
    Confidential,
}

/// Identifier verification status (SPEC-002 Section 3).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationStatus {
    /// Confirmed against an authoritative source.
    Verified,
    /// Reported by the entity without independent verification.
    Reported,
    /// Derived by inference from other data.
    Inferred,
    /// Not yet verified.
    Unverified,
}

/// Lifecycle state of an organisation node (SPEC-001 Section 4.1).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrganizationStatus {
    /// The organisation is currently operating.
    Active,
    /// The organisation has been wound up or struck off.
    Dissolved,
    /// The organisation has been absorbed into another entity.
    Merged,
    /// The organisation is temporarily suspended.
    Suspended,
}

/// Outcome of an attestation evaluation (SPEC-001 Section 4.5).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttestationOutcome {
    /// All criteria were satisfied.
    Pass,
    /// Criteria were satisfied subject to conditions.
    ConditionalPass,
    /// One or more criteria were not satisfied.
    Fail,
    /// Evaluation is still in progress.
    Pending,
    /// The attestation does not apply to this node.
    NotApplicable,
}

/// Lifecycle state of an attestation (SPEC-001 Section 4.5).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttestationStatus {
    /// The attestation is current and in force.
    Active,
    /// The attestation has been temporarily suspended.
    Suspended,
    /// The attestation has been permanently revoked.
    Revoked,
    /// The attestation has passed its `valid_to` date.
    Expired,
    /// The attesting party has voluntarily withdrawn the attestation.
    Withdrawn,
}

/// Risk severity classification (SPEC-001 Section 4.5).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskSeverity {
    /// Highest severity — immediate action required.
    Critical,
    /// High severity.
    High,
    /// Medium severity.
    Medium,
    /// Low severity.
    Low,
}

/// Likelihood of an identified risk materialising (SPEC-001 Section 4.5).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskLikelihood {
    /// Occurrence is considered near-certain.
    VeryLikely,
    /// Occurrence is considered probable.
    Likely,
    /// Occurrence is conceivable under plausible circumstances.
    Possible,
    /// Occurrence is considered improbable.
    Unlikely,
}

/// Source of an emissions factor used for `CO2e` calculations (SPEC-001 Section 4.6).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmissionFactorSource {
    /// Installation-level measurement data.
    Actual,
    /// EU default values per Annex III.
    DefaultEu,
    /// Third-country default values.
    DefaultCountry,
}

/// Type of operational control arrangement (SPEC-001 Section 5.2).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlType {
    /// Franchise arrangement.
    Franchise,
    /// Management contract.
    Management,
    /// Tolling arrangement.
    Tolling,
    /// Licensed manufacturing arrangement.
    LicensedManufacturing,
    /// Any other form of operational control.
    Other,
}

/// Accounting consolidation basis for legal parentage edges (SPEC-001 Section 5.3).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConsolidationBasis {
    /// Consolidated under IFRS 10.
    Ifrs10,
    /// Consolidated under US GAAP ASC 810.
    UsGaapAsc810,
    /// Other consolidation basis.
    Other,
    /// Consolidation basis is not known.
    Unknown,
}

/// Type of corporate identity event recorded on a `former_identity` edge (SPEC-001 Section 5.4).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    /// Two entities merged into one.
    Merger,
    /// One entity acquired another.
    Acquisition,
    /// An entity changed its name.
    Rename,
    /// Part of an entity was split off.
    Demerger,
    /// A subsidiary was separated and listed independently.
    SpinOff,
}

/// Type of logistics or distribution service (SPEC-001 Section 6.4).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceType {
    /// Warehousing and storage services.
    Warehousing,
    /// Transportation services.
    Transport,
    /// Order fulfilment services.
    Fulfillment,
    /// Any other distribution service.
    Other,
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

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

    #[test]
    fn disclosure_scope_round_trip() {
        assert_eq!(to_json(&DisclosureScope::Internal), r#""internal""#);
        assert_eq!(to_json(&DisclosureScope::Partner), r#""partner""#);
        assert_eq!(to_json(&DisclosureScope::Public), r#""public""#);
        round_trip(&DisclosureScope::Internal);
        round_trip(&DisclosureScope::Partner);
        round_trip(&DisclosureScope::Public);
    }

    #[test]
    fn node_type_round_trip() {
        assert_eq!(to_json(&NodeType::Organization), r#""organization""#);
        assert_eq!(to_json(&NodeType::Facility), r#""facility""#);
        assert_eq!(to_json(&NodeType::Good), r#""good""#);
        assert_eq!(to_json(&NodeType::Person), r#""person""#);
        assert_eq!(to_json(&NodeType::Attestation), r#""attestation""#);
        assert_eq!(to_json(&NodeType::Consignment), r#""consignment""#);
        assert_eq!(to_json(&NodeType::BoundaryRef), r#""boundary_ref""#);
        round_trip(&NodeType::Organization);
        round_trip(&NodeType::BoundaryRef);
    }

    #[test]
    fn node_type_tag_known_round_trip() {
        let tag = NodeTypeTag::Known(NodeType::Organization);
        assert_eq!(to_json(&tag), r#""organization""#);
        let back: NodeTypeTag = from_json(r#""organization""#);
        assert_eq!(back, NodeTypeTag::Known(NodeType::Organization));
    }

    #[test]
    fn node_type_tag_all_known_variants() {
        let variants = [
            (NodeType::Organization, "organization"),
            (NodeType::Facility, "facility"),
            (NodeType::Good, "good"),
            (NodeType::Person, "person"),
            (NodeType::Attestation, "attestation"),
            (NodeType::Consignment, "consignment"),
            (NodeType::BoundaryRef, "boundary_ref"),
        ];
        for (variant, expected_str) in variants {
            let tag = NodeTypeTag::Known(variant.clone());
            let json = to_json(&tag);
            assert_eq!(json, format!(r#""{expected_str}""#));
            let back: NodeTypeTag = from_json(&json);
            assert_eq!(back, NodeTypeTag::Known(variant));
        }
    }

    #[test]
    fn node_type_tag_extension_dot_notation() {
        let json = r#""com.example.custom_node""#;
        let tag: NodeTypeTag = from_json(json);
        assert_eq!(
            tag,
            NodeTypeTag::Extension("com.example.custom_node".to_owned())
        );
        assert_eq!(to_json(&tag), json);
    }

    #[test]
    fn node_type_tag_extension_unknown_no_dot() {
        let json = r#""mystery_type""#;
        let tag: NodeTypeTag = from_json(json);
        assert_eq!(tag, NodeTypeTag::Extension("mystery_type".to_owned()));
        assert_eq!(to_json(&tag), json);
    }

    #[test]
    fn edge_type_round_trip() {
        assert_eq!(to_json(&EdgeType::Ownership), r#""ownership""#);
        assert_eq!(
            to_json(&EdgeType::OperationalControl),
            r#""operational_control""#
        );
        assert_eq!(to_json(&EdgeType::LegalParentage), r#""legal_parentage""#);
        assert_eq!(to_json(&EdgeType::FormerIdentity), r#""former_identity""#);
        assert_eq!(
            to_json(&EdgeType::BeneficialOwnership),
            r#""beneficial_ownership""#
        );
        assert_eq!(to_json(&EdgeType::Supplies), r#""supplies""#);
        assert_eq!(to_json(&EdgeType::Subcontracts), r#""subcontracts""#);
        assert_eq!(to_json(&EdgeType::Tolls), r#""tolls""#);
        assert_eq!(to_json(&EdgeType::Distributes), r#""distributes""#);
        assert_eq!(to_json(&EdgeType::Brokers), r#""brokers""#);
        assert_eq!(to_json(&EdgeType::Operates), r#""operates""#);
        assert_eq!(to_json(&EdgeType::Produces), r#""produces""#);
        assert_eq!(to_json(&EdgeType::ComposedOf), r#""composed_of""#);
        assert_eq!(to_json(&EdgeType::SellsTo), r#""sells_to""#);
        assert_eq!(to_json(&EdgeType::AttestedBy), r#""attested_by""#);
        assert_eq!(to_json(&EdgeType::SameAs), r#""same_as""#);
        round_trip(&EdgeType::Ownership);
        round_trip(&EdgeType::SameAs);
    }

    #[test]
    fn edge_type_tag_known_round_trip() {
        let tag = EdgeTypeTag::Known(EdgeType::Supplies);
        assert_eq!(to_json(&tag), r#""supplies""#);
        let back: EdgeTypeTag = from_json(r#""supplies""#);
        assert_eq!(back, EdgeTypeTag::Known(EdgeType::Supplies));
    }

    #[test]
    fn edge_type_tag_all_known_variants() {
        let variants = [
            (EdgeType::Ownership, "ownership"),
            (EdgeType::OperationalControl, "operational_control"),
            (EdgeType::LegalParentage, "legal_parentage"),
            (EdgeType::FormerIdentity, "former_identity"),
            (EdgeType::BeneficialOwnership, "beneficial_ownership"),
            (EdgeType::Supplies, "supplies"),
            (EdgeType::Subcontracts, "subcontracts"),
            (EdgeType::Tolls, "tolls"),
            (EdgeType::Distributes, "distributes"),
            (EdgeType::Brokers, "brokers"),
            (EdgeType::Operates, "operates"),
            (EdgeType::Produces, "produces"),
            (EdgeType::ComposedOf, "composed_of"),
            (EdgeType::SellsTo, "sells_to"),
            (EdgeType::AttestedBy, "attested_by"),
            (EdgeType::SameAs, "same_as"),
        ];
        for (variant, expected_str) in variants {
            let tag = EdgeTypeTag::Known(variant.clone());
            let json = to_json(&tag);
            assert_eq!(json, format!(r#""{expected_str}""#));
            let back: EdgeTypeTag = from_json(&json);
            assert_eq!(back, EdgeTypeTag::Known(variant));
        }
    }

    #[test]
    fn edge_type_tag_extension_round_trip() {
        let json = r#""com.acme.custom_relationship""#;
        let tag: EdgeTypeTag = from_json(json);
        assert_eq!(
            tag,
            EdgeTypeTag::Extension("com.acme.custom_relationship".to_owned())
        );
        assert_eq!(to_json(&tag), json);
    }

    #[test]
    fn attestation_type_round_trip() {
        assert_eq!(
            to_json(&AttestationType::Certification),
            r#""certification""#
        );
        assert_eq!(to_json(&AttestationType::Audit), r#""audit""#);
        assert_eq!(
            to_json(&AttestationType::DueDiligenceStatement),
            r#""due_diligence_statement""#
        );
        assert_eq!(
            to_json(&AttestationType::SelfDeclaration),
            r#""self_declaration""#
        );
        assert_eq!(to_json(&AttestationType::Other), r#""other""#);
        round_trip(&AttestationType::DueDiligenceStatement);
    }

    #[test]
    fn confidence_round_trip() {
        assert_eq!(to_json(&Confidence::Verified), r#""verified""#);
        assert_eq!(to_json(&Confidence::Reported), r#""reported""#);
        assert_eq!(to_json(&Confidence::Inferred), r#""inferred""#);
        assert_eq!(to_json(&Confidence::Estimated), r#""estimated""#);
        round_trip(&Confidence::Estimated);
    }

    #[test]
    fn sensitivity_round_trip() {
        assert_eq!(to_json(&Sensitivity::Public), r#""public""#);
        assert_eq!(to_json(&Sensitivity::Restricted), r#""restricted""#);
        assert_eq!(to_json(&Sensitivity::Confidential), r#""confidential""#);
        round_trip(&Sensitivity::Confidential);
    }

    #[test]
    fn verification_status_round_trip() {
        assert_eq!(to_json(&VerificationStatus::Verified), r#""verified""#);
        assert_eq!(to_json(&VerificationStatus::Reported), r#""reported""#);
        assert_eq!(to_json(&VerificationStatus::Inferred), r#""inferred""#);
        assert_eq!(to_json(&VerificationStatus::Unverified), r#""unverified""#);
        round_trip(&VerificationStatus::Unverified);
    }

    #[test]
    fn organization_status_round_trip() {
        assert_eq!(to_json(&OrganizationStatus::Active), r#""active""#);
        assert_eq!(to_json(&OrganizationStatus::Dissolved), r#""dissolved""#);
        assert_eq!(to_json(&OrganizationStatus::Merged), r#""merged""#);
        assert_eq!(to_json(&OrganizationStatus::Suspended), r#""suspended""#);
        round_trip(&OrganizationStatus::Merged);
    }

    #[test]
    fn attestation_outcome_round_trip() {
        assert_eq!(to_json(&AttestationOutcome::Pass), r#""pass""#);
        assert_eq!(
            to_json(&AttestationOutcome::ConditionalPass),
            r#""conditional_pass""#
        );
        assert_eq!(to_json(&AttestationOutcome::Fail), r#""fail""#);
        assert_eq!(to_json(&AttestationOutcome::Pending), r#""pending""#);
        assert_eq!(
            to_json(&AttestationOutcome::NotApplicable),
            r#""not_applicable""#
        );
        round_trip(&AttestationOutcome::ConditionalPass);
    }

    #[test]
    fn attestation_status_round_trip() {
        assert_eq!(to_json(&AttestationStatus::Active), r#""active""#);
        assert_eq!(to_json(&AttestationStatus::Suspended), r#""suspended""#);
        assert_eq!(to_json(&AttestationStatus::Revoked), r#""revoked""#);
        assert_eq!(to_json(&AttestationStatus::Expired), r#""expired""#);
        assert_eq!(to_json(&AttestationStatus::Withdrawn), r#""withdrawn""#);
        round_trip(&AttestationStatus::Revoked);
    }

    #[test]
    fn risk_severity_round_trip() {
        assert_eq!(to_json(&RiskSeverity::Critical), r#""critical""#);
        assert_eq!(to_json(&RiskSeverity::High), r#""high""#);
        assert_eq!(to_json(&RiskSeverity::Medium), r#""medium""#);
        assert_eq!(to_json(&RiskSeverity::Low), r#""low""#);
        round_trip(&RiskSeverity::Critical);
    }

    #[test]
    fn risk_likelihood_round_trip() {
        assert_eq!(to_json(&RiskLikelihood::VeryLikely), r#""very_likely""#);
        assert_eq!(to_json(&RiskLikelihood::Likely), r#""likely""#);
        assert_eq!(to_json(&RiskLikelihood::Possible), r#""possible""#);
        assert_eq!(to_json(&RiskLikelihood::Unlikely), r#""unlikely""#);
        round_trip(&RiskLikelihood::VeryLikely);
    }

    #[test]
    fn emission_factor_source_round_trip() {
        assert_eq!(to_json(&EmissionFactorSource::Actual), r#""actual""#);
        assert_eq!(to_json(&EmissionFactorSource::DefaultEu), r#""default_eu""#);
        assert_eq!(
            to_json(&EmissionFactorSource::DefaultCountry),
            r#""default_country""#
        );
        round_trip(&EmissionFactorSource::DefaultEu);
    }

    #[test]
    fn control_type_round_trip() {
        assert_eq!(to_json(&ControlType::Franchise), r#""franchise""#);
        assert_eq!(to_json(&ControlType::Management), r#""management""#);
        assert_eq!(to_json(&ControlType::Tolling), r#""tolling""#);
        assert_eq!(
            to_json(&ControlType::LicensedManufacturing),
            r#""licensed_manufacturing""#
        );
        assert_eq!(to_json(&ControlType::Other), r#""other""#);
        round_trip(&ControlType::LicensedManufacturing);
    }

    #[test]
    fn consolidation_basis_round_trip() {
        assert_eq!(to_json(&ConsolidationBasis::Ifrs10), r#""ifrs10""#);
        assert_eq!(
            to_json(&ConsolidationBasis::UsGaapAsc810),
            r#""us_gaap_asc810""#
        );
        assert_eq!(to_json(&ConsolidationBasis::Other), r#""other""#);
        assert_eq!(to_json(&ConsolidationBasis::Unknown), r#""unknown""#);
        round_trip(&ConsolidationBasis::UsGaapAsc810);
    }

    #[test]
    fn event_type_round_trip() {
        assert_eq!(to_json(&EventType::Merger), r#""merger""#);
        assert_eq!(to_json(&EventType::Acquisition), r#""acquisition""#);
        assert_eq!(to_json(&EventType::Rename), r#""rename""#);
        assert_eq!(to_json(&EventType::Demerger), r#""demerger""#);
        assert_eq!(to_json(&EventType::SpinOff), r#""spin_off""#);
        round_trip(&EventType::SpinOff);
    }

    #[test]
    fn service_type_round_trip() {
        assert_eq!(to_json(&ServiceType::Warehousing), r#""warehousing""#);
        assert_eq!(to_json(&ServiceType::Transport), r#""transport""#);
        assert_eq!(to_json(&ServiceType::Fulfillment), r#""fulfillment""#);
        assert_eq!(to_json(&ServiceType::Other), r#""other""#);
        round_trip(&ServiceType::Fulfillment);
    }
}
