/// L1-GDM-01 through L1-GDM-06: Graph Data Model structural validation rules.
///
/// These rules enforce the MUST constraints from SPEC-001 Section 9.1 and 9.5.
/// Each rule is a stateless struct implementing [`crate::validation::ValidationRule`].
/// All rules collect every violation without early exit.
///
/// Rules are registered in [`crate::validation::build_registry`] when
/// [`crate::validation::ValidationConfig::run_l1`] is `true`.
use std::collections::{HashMap, HashSet};

use crate::enums::{EdgeType, EdgeTypeTag, NodeType, NodeTypeTag};
use crate::file::OmtsFile;
use crate::structures::Node;

use super::{Diagnostic, Level, Location, RuleId, Severity, ValidationRule};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a map from node id string to the node reference.
///
/// Used by multiple rules to avoid redundant iteration.
fn node_id_map(file: &OmtsFile) -> HashMap<&str, &Node> {
    file.nodes
        .iter()
        .map(|n| (n.id.as_ref() as &str, n))
        .collect()
}

/// Returns `true` if the string matches the reverse-domain extension convention
/// (contains at least one dot character, per SPEC-001 Section 8.2).
fn is_extension_type(s: &str) -> bool {
    s.contains('.')
}

// ---------------------------------------------------------------------------
// L1-GDM-01: Duplicate node ID detection
// ---------------------------------------------------------------------------

/// L1-GDM-01 — Every node has a non-empty `id`, unique within the file.
///
/// The non-empty constraint is enforced by the [`crate::newtypes::NodeId`]
/// newtype at deserialization time, so this rule only checks for duplicate
/// IDs across nodes. Each duplicate (beyond the first occurrence) produces
/// one diagnostic.
pub struct GdmRule01;

impl ValidationRule for GdmRule01 {
    fn id(&self) -> RuleId {
        RuleId::L1Gdm01
    }

    fn level(&self) -> Level {
        Level::L1
    }

    fn check(
        &self,
        file: &OmtsFile,
        diags: &mut Vec<Diagnostic>,
        _external_data: Option<&dyn super::external::ExternalDataSource>,
    ) {
        let mut seen: HashSet<&str> = HashSet::new();
        for node in &file.nodes {
            let id: &str = &node.id;
            if !seen.insert(id) {
                diags.push(Diagnostic::new(
                    RuleId::L1Gdm01,
                    Severity::Error,
                    Location::Node {
                        node_id: id.to_owned(),
                        field: None,
                    },
                    format!("duplicate node id \"{id}\""),
                ));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// L1-GDM-02: Duplicate edge ID detection
// ---------------------------------------------------------------------------

/// L1-GDM-02 — Every edge has a non-empty `id`, unique within the file.
///
/// The non-empty constraint is enforced by the [`crate::newtypes::NodeId`]
/// (aliased as `EdgeId`) newtype at deserialization time. This rule only
/// checks for duplicate IDs across edges. Each duplicate (beyond the first
/// occurrence) produces one diagnostic.
pub struct GdmRule02;

impl ValidationRule for GdmRule02 {
    fn id(&self) -> RuleId {
        RuleId::L1Gdm02
    }

    fn level(&self) -> Level {
        Level::L1
    }

    fn check(
        &self,
        file: &OmtsFile,
        diags: &mut Vec<Diagnostic>,
        _external_data: Option<&dyn super::external::ExternalDataSource>,
    ) {
        let mut seen: HashSet<&str> = HashSet::new();
        for edge in &file.edges {
            let id: &str = &edge.id;
            if !seen.insert(id) {
                diags.push(Diagnostic::new(
                    RuleId::L1Gdm02,
                    Severity::Error,
                    Location::Edge {
                        edge_id: id.to_owned(),
                        field: None,
                    },
                    format!("duplicate edge id \"{id}\""),
                ));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// L1-GDM-03: Dangling edge source/target references
// ---------------------------------------------------------------------------

/// L1-GDM-03 — Every edge `source` and `target` references an existing node `id`.
///
/// Both `source` and `target` are checked independently. Each dangling
/// reference produces a separate diagnostic with `field` set to `"source"` or
/// `"target"` respectively.
pub struct GdmRule03;

impl ValidationRule for GdmRule03 {
    fn id(&self) -> RuleId {
        RuleId::L1Gdm03
    }

    fn level(&self) -> Level {
        Level::L1
    }

    fn check(
        &self,
        file: &OmtsFile,
        diags: &mut Vec<Diagnostic>,
        _external_data: Option<&dyn super::external::ExternalDataSource>,
    ) {
        let node_ids: HashSet<&str> = file.nodes.iter().map(|n| n.id.as_ref() as &str).collect();

        for edge in &file.edges {
            let edge_id: &str = &edge.id;
            let source: &str = &edge.source;
            let target: &str = &edge.target;

            if !node_ids.contains(source) {
                diags.push(Diagnostic::new(
                    RuleId::L1Gdm03,
                    Severity::Error,
                    Location::Edge {
                        edge_id: edge_id.to_owned(),
                        field: Some("source".to_owned()),
                    },
                    format!(
                        "edge \"{edge_id}\" source \"{source}\" does not reference an existing node"
                    ),
                ));
            }

            if !node_ids.contains(target) {
                diags.push(Diagnostic::new(
                    RuleId::L1Gdm03,
                    Severity::Error,
                    Location::Edge {
                        edge_id: edge_id.to_owned(),
                        field: Some("target".to_owned()),
                    },
                    format!(
                        "edge \"{edge_id}\" target \"{target}\" does not reference an existing node"
                    ),
                ));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// L1-GDM-04: Edge type validation
// ---------------------------------------------------------------------------

/// L1-GDM-04 — Edge `type` is a recognised core type, `same_as`, or
/// reverse-domain extension.
///
/// All [`crate::enums::EdgeType`] variants (including `SameAs`) are accepted.
/// Extension strings that contain a dot are accepted per SPEC-001 Section 8.2.
/// Unrecognised strings without a dot are rejected.
pub struct GdmRule04;

impl ValidationRule for GdmRule04 {
    fn id(&self) -> RuleId {
        RuleId::L1Gdm04
    }

    fn level(&self) -> Level {
        Level::L1
    }

    fn check(
        &self,
        file: &OmtsFile,
        diags: &mut Vec<Diagnostic>,
        _external_data: Option<&dyn super::external::ExternalDataSource>,
    ) {
        for edge in &file.edges {
            match &edge.edge_type {
                EdgeTypeTag::Known(_) => {
                    // All known variants (including SameAs) are accepted.
                }
                EdgeTypeTag::Extension(s) => {
                    if !is_extension_type(s) {
                        diags.push(Diagnostic::new(
                            RuleId::L1Gdm04,
                            Severity::Error,
                            Location::Edge {
                                edge_id: edge.id.to_string(),
                                field: Some("type".to_owned()),
                            },
                            format!(
                                "edge \"{}\" has unrecognised type \"{s}\"; \
                                 must be a core type, \"same_as\", or a \
                                 reverse-domain extension (e.g. \"com.example.custom\")",
                                edge.id
                            ),
                        ));
                    }
                    // Dot-containing strings are extension types: accepted.
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// L1-GDM-05: reporting_entity references a valid organization node
// ---------------------------------------------------------------------------

/// L1-GDM-05 — `reporting_entity` if present references an existing
/// `organization` node.
///
/// The referenced node must both exist and have `type: "organization"`.
/// A missing node and a node of the wrong type each produce a distinct message.
pub struct GdmRule05;

impl ValidationRule for GdmRule05 {
    fn id(&self) -> RuleId {
        RuleId::L1Gdm05
    }

    fn level(&self) -> Level {
        Level::L1
    }

    fn check(
        &self,
        file: &OmtsFile,
        diags: &mut Vec<Diagnostic>,
        _external_data: Option<&dyn super::external::ExternalDataSource>,
    ) {
        let Some(ref reporting_entity) = file.reporting_entity else {
            return;
        };
        let ref_id: &str = reporting_entity;

        let node_map = node_id_map(file);

        match node_map.get(ref_id) {
            None => {
                diags.push(Diagnostic::new(
                    RuleId::L1Gdm05,
                    Severity::Error,
                    Location::Header {
                        field: "reporting_entity",
                    },
                    format!("reporting_entity \"{ref_id}\" does not reference an existing node"),
                ));
            }
            Some(node) => {
                if node.node_type != NodeTypeTag::Known(NodeType::Organization) {
                    diags.push(Diagnostic::new(
                        RuleId::L1Gdm05,
                        Severity::Error,
                        Location::Header {
                            field: "reporting_entity",
                        },
                        format!(
                            "reporting_entity \"{ref_id}\" references a node that is not an \
                             organization (found type: {})",
                            node_type_display(&node.node_type)
                        ),
                    ));
                }
            }
        }
    }
}

/// Returns a human-readable string for a [`NodeTypeTag`].
fn node_type_display(tag: &NodeTypeTag) -> String {
    match tag {
        NodeTypeTag::Known(NodeType::Organization) => "organization".to_owned(),
        NodeTypeTag::Known(NodeType::Facility) => "facility".to_owned(),
        NodeTypeTag::Known(NodeType::Good) => "good".to_owned(),
        NodeTypeTag::Known(NodeType::Person) => "person".to_owned(),
        NodeTypeTag::Known(NodeType::Attestation) => "attestation".to_owned(),
        NodeTypeTag::Known(NodeType::Consignment) => "consignment".to_owned(),
        NodeTypeTag::Known(NodeType::BoundaryRef) => "boundary_ref".to_owned(),
        NodeTypeTag::Extension(s) => s.clone(),
    }
}

// ---------------------------------------------------------------------------
// L1-GDM-06: Edge source/target node type compatibility
// ---------------------------------------------------------------------------

/// L1-GDM-06 — Edge source/target node types match the permitted types table
/// (SPEC-001 Section 9.5). Extension edges are exempt.
///
/// For each core edge type the permitted source and target [`NodeType`] sets
/// are encoded in [`permitted_types`]. Extension edges (those with
/// [`EdgeTypeTag::Extension`]) are skipped entirely.
///
/// A diagnostic is emitted per invalid endpoint — both `source` and `target`
/// are checked independently.
pub struct GdmRule06;

/// Permitted source and target node types for each core edge type.
///
/// Returns `None` when the edge type imposes no type constraint (e.g. `same_as`).
fn permitted_types(edge_type: &EdgeType) -> Option<(TypeSet, TypeSet)> {
    use NodeType::{Attestation, Consignment, Facility, Good, Organization, Person};

    // TypeSet is a small fixed-size slice reference; values live in static arrays.
    let org = &[Organization][..];
    let org_fac = &[Organization, Facility][..];
    let fac = &[Facility][..];
    let good_cons = &[Good, Consignment][..];
    let org_fac_good_cons = &[Organization, Facility, Good, Consignment][..];
    let att = &[Attestation][..];
    let person = &[Person][..];

    let (src, tgt): (&[NodeType], &[NodeType]) = match edge_type {
        EdgeType::Ownership => (org, org),
        EdgeType::OperationalControl => (org, org_fac),
        EdgeType::LegalParentage => (org, org),
        EdgeType::FormerIdentity => (org, org),
        EdgeType::BeneficialOwnership => (person, org),
        EdgeType::Supplies => (org, org),
        EdgeType::Subcontracts => (org, org),
        EdgeType::Tolls => (org_fac, org),
        EdgeType::Distributes => (org, org),
        EdgeType::Brokers => (org, org),
        EdgeType::Operates => (org, fac),
        EdgeType::Produces => (fac, good_cons),
        EdgeType::ComposedOf => (good_cons, good_cons),
        EdgeType::SellsTo => (org, org),
        EdgeType::AttestedBy => (org_fac_good_cons, att),
        EdgeType::SameAs => return None, // any → any
    };

    Some((src, tgt))
}

/// A slice of permitted node types.
type TypeSet = &'static [NodeType];

impl ValidationRule for GdmRule06 {
    fn id(&self) -> RuleId {
        RuleId::L1Gdm06
    }

    fn level(&self) -> Level {
        Level::L1
    }

    fn check(
        &self,
        file: &OmtsFile,
        diags: &mut Vec<Diagnostic>,
        _external_data: Option<&dyn super::external::ExternalDataSource>,
    ) {
        let node_map = node_id_map(file);

        for edge in &file.edges {
            let edge_id: &str = &edge.id;

            // Extension edges are exempt from type-compatibility checks.
            let edge_type = match &edge.edge_type {
                EdgeTypeTag::Extension(_) => continue,
                EdgeTypeTag::Known(et) => et,
            };

            let Some((permitted_src, permitted_tgt)) = permitted_types(edge_type) else {
                // `same_as` — no constraint.
                continue;
            };

            let source_id: &str = &edge.source;
            let target_id: &str = &edge.target;

            // Only check nodes that actually exist; dangling refs are L1-GDM-03's
            // responsibility.
            if let Some(src_node) = node_map.get(source_id) {
                if let NodeTypeTag::Known(ref src_type) = src_node.node_type {
                    // boundary_ref nodes may appear at any edge endpoint (SPEC-004
                    // Section 5.1): they preserve graph connectivity when a node is
                    // replaced during redaction, so the type-compatibility constraint
                    // does not apply to them.
                    let is_boundary_ref = *src_type == NodeType::BoundaryRef;
                    if !is_boundary_ref && !permitted_src.contains(src_type) {
                        diags.push(Diagnostic::new(
                            RuleId::L1Gdm06,
                            Severity::Error,
                            Location::Edge {
                                edge_id: edge_id.to_owned(),
                                field: Some("source".to_owned()),
                            },
                            format!(
                                "edge \"{edge_id}\" (type \"{}\") source \"{source_id}\" \
                                 has type \"{}\", which is not permitted; \
                                 expected one of: {}",
                                edge_type_display(edge_type),
                                node_type_display(&src_node.node_type),
                                format_type_set(permitted_src),
                            ),
                        ));
                    }
                }
                // Extension node types are not constrained.
            }

            if let Some(tgt_node) = node_map.get(target_id) {
                if let NodeTypeTag::Known(ref tgt_type) = tgt_node.node_type {
                    // boundary_ref nodes may appear at any edge endpoint (see above).
                    let is_boundary_ref = *tgt_type == NodeType::BoundaryRef;
                    if !is_boundary_ref && !permitted_tgt.contains(tgt_type) {
                        diags.push(Diagnostic::new(
                            RuleId::L1Gdm06,
                            Severity::Error,
                            Location::Edge {
                                edge_id: edge_id.to_owned(),
                                field: Some("target".to_owned()),
                            },
                            format!(
                                "edge \"{edge_id}\" (type \"{}\") target \"{target_id}\" \
                                 has type \"{}\", which is not permitted; \
                                 expected one of: {}",
                                edge_type_display(edge_type),
                                node_type_display(&tgt_node.node_type),
                                format_type_set(permitted_tgt),
                            ),
                        ));
                    }
                }
                // Extension node types are not constrained.
            }
        }
    }
}

/// Returns the `snake_case` string for an [`EdgeType`].
fn edge_type_display(et: &EdgeType) -> &'static str {
    match et {
        EdgeType::Ownership => "ownership",
        EdgeType::OperationalControl => "operational_control",
        EdgeType::LegalParentage => "legal_parentage",
        EdgeType::FormerIdentity => "former_identity",
        EdgeType::BeneficialOwnership => "beneficial_ownership",
        EdgeType::Supplies => "supplies",
        EdgeType::Subcontracts => "subcontracts",
        EdgeType::Tolls => "tolls",
        EdgeType::Distributes => "distributes",
        EdgeType::Brokers => "brokers",
        EdgeType::Operates => "operates",
        EdgeType::Produces => "produces",
        EdgeType::ComposedOf => "composed_of",
        EdgeType::SellsTo => "sells_to",
        EdgeType::AttestedBy => "attested_by",
        EdgeType::SameAs => "same_as",
    }
}

/// Formats a [`TypeSet`] as a comma-separated human-readable list.
fn format_type_set(types: &[NodeType]) -> String {
    types
        .iter()
        .map(|t| node_type_display(&NodeTypeTag::Known(t.clone())))
        .collect::<Vec<_>>()
        .join(", ")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use crate::enums::{EdgeType, EdgeTypeTag, NodeType, NodeTypeTag};
    use crate::file::OmtsFile;
    use crate::newtypes::{CalendarDate, EdgeId, FileSalt, NodeId, SemVer};
    use crate::structures::{Edge, EdgeProperties, Node};

    // -----------------------------------------------------------------------
    // Fixture helpers
    // -----------------------------------------------------------------------

    const SALT: &str = "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef";

    fn make_file(nodes: Vec<Node>, edges: Vec<Edge>) -> OmtsFile {
        OmtsFile {
            omtsf_version: SemVer::try_from("1.0.0").expect("valid"),
            snapshot_date: CalendarDate::try_from("2026-02-19").expect("valid"),
            file_salt: FileSalt::try_from(SALT).expect("valid"),
            disclosure_scope: None,
            previous_snapshot_ref: None,
            snapshot_sequence: None,
            reporting_entity: None,
            nodes,
            edges,
            extra: serde_json::Map::new(),
        }
    }

    fn node(id: &str, node_type: NodeType) -> Node {
        Node {
            id: NodeId::try_from(id).expect("valid id"),
            node_type: NodeTypeTag::Known(node_type),
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
        }
    }

    fn extension_node(id: &str, type_str: &str) -> Node {
        Node {
            id: NodeId::try_from(id).expect("valid id"),
            node_type: NodeTypeTag::Extension(type_str.to_owned()),
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
        }
    }

    fn edge(id: &str, edge_type: EdgeType, source: &str, target: &str) -> Edge {
        Edge {
            id: EdgeId::try_from(id).expect("valid id"),
            edge_type: EdgeTypeTag::Known(edge_type),
            source: NodeId::try_from(source).expect("valid source"),
            target: NodeId::try_from(target).expect("valid target"),
            identifiers: None,
            properties: EdgeProperties::default(),
            extra: serde_json::Map::new(),
        }
    }

    fn extension_edge(id: &str, type_str: &str, source: &str, target: &str) -> Edge {
        Edge {
            id: EdgeId::try_from(id).expect("valid id"),
            edge_type: EdgeTypeTag::Extension(type_str.to_owned()),
            source: NodeId::try_from(source).expect("valid source"),
            target: NodeId::try_from(target).expect("valid target"),
            identifiers: None,
            properties: EdgeProperties::default(),
            extra: serde_json::Map::new(),
        }
    }

    fn run_rule(rule: &dyn ValidationRule, file: &OmtsFile) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        rule.check(file, &mut diags, None);
        diags
    }

    fn rule_ids(diags: &[Diagnostic]) -> Vec<&RuleId> {
        diags.iter().map(|d| &d.rule_id).collect()
    }

    // -----------------------------------------------------------------------
    // L1-GDM-01 tests
    // -----------------------------------------------------------------------

    #[test]
    fn gdm01_clean_no_diagnostics() {
        let file = make_file(
            vec![
                node("org-1", NodeType::Organization),
                node("org-2", NodeType::Organization),
            ],
            vec![],
        );
        let diags = run_rule(&GdmRule01, &file);
        assert!(diags.is_empty(), "no duplicate ids → no diagnostics");
    }

    #[test]
    fn gdm01_duplicate_node_id_detected() {
        let file = make_file(
            vec![
                node("dup-id", NodeType::Organization),
                node("dup-id", NodeType::Facility),
            ],
            vec![],
        );
        let diags = run_rule(&GdmRule01, &file);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule_id, RuleId::L1Gdm01);
        assert_eq!(diags[0].severity, Severity::Error);
        assert!(diags[0].message.contains("dup-id"));
    }

    #[test]
    fn gdm01_multiple_duplicates_all_collected() {
        // Three nodes share the same id → two diagnostics (second and third occurrence).
        let file = make_file(
            vec![
                node("same", NodeType::Organization),
                node("same", NodeType::Facility),
                node("same", NodeType::Good),
                node("unique", NodeType::Person),
            ],
            vec![],
        );
        let diags = run_rule(&GdmRule01, &file);
        assert_eq!(diags.len(), 2, "second and third occurrence each trigger");
        assert!(rule_ids(&diags).iter().all(|id| **id == RuleId::L1Gdm01));
    }

    #[test]
    fn gdm01_empty_graph_no_diagnostics() {
        let file = make_file(vec![], vec![]);
        let diags = run_rule(&GdmRule01, &file);
        assert!(diags.is_empty());
    }

    // -----------------------------------------------------------------------
    // L1-GDM-02 tests
    // -----------------------------------------------------------------------

    #[test]
    fn gdm02_clean_no_diagnostics() {
        let file = make_file(
            vec![
                node("org-1", NodeType::Organization),
                node("org-2", NodeType::Organization),
            ],
            vec![edge("e-1", EdgeType::Supplies, "org-1", "org-2")],
        );
        let diags = run_rule(&GdmRule02, &file);
        assert!(diags.is_empty());
    }

    #[test]
    fn gdm02_duplicate_edge_id_detected() {
        let file = make_file(
            vec![
                node("org-1", NodeType::Organization),
                node("org-2", NodeType::Organization),
            ],
            vec![
                edge("dup-edge", EdgeType::Supplies, "org-1", "org-2"),
                edge("dup-edge", EdgeType::Ownership, "org-1", "org-2"),
            ],
        );
        let diags = run_rule(&GdmRule02, &file);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule_id, RuleId::L1Gdm02);
        assert_eq!(diags[0].severity, Severity::Error);
        assert!(diags[0].message.contains("dup-edge"));
    }

    #[test]
    fn gdm02_multiple_duplicate_edge_ids_all_collected() {
        let file = make_file(
            vec![
                node("org-1", NodeType::Organization),
                node("org-2", NodeType::Organization),
            ],
            vec![
                edge("dup", EdgeType::Supplies, "org-1", "org-2"),
                edge("dup", EdgeType::Supplies, "org-1", "org-2"),
                edge("dup", EdgeType::Supplies, "org-1", "org-2"),
                edge("unique", EdgeType::Ownership, "org-1", "org-2"),
            ],
        );
        let diags = run_rule(&GdmRule02, &file);
        assert_eq!(diags.len(), 2);
        assert!(rule_ids(&diags).iter().all(|id| **id == RuleId::L1Gdm02));
    }

    #[test]
    fn gdm02_empty_edges_no_diagnostics() {
        let file = make_file(vec![], vec![]);
        let diags = run_rule(&GdmRule02, &file);
        assert!(diags.is_empty());
    }

    // -----------------------------------------------------------------------
    // L1-GDM-03 tests
    // -----------------------------------------------------------------------

    #[test]
    fn gdm03_clean_no_diagnostics() {
        let file = make_file(
            vec![
                node("org-1", NodeType::Organization),
                node("org-2", NodeType::Organization),
            ],
            vec![edge("e-1", EdgeType::Supplies, "org-1", "org-2")],
        );
        let diags = run_rule(&GdmRule03, &file);
        assert!(diags.is_empty());
    }

    #[test]
    fn gdm03_dangling_source_detected() {
        let file = make_file(
            vec![node("org-1", NodeType::Organization)],
            vec![edge("e-1", EdgeType::Supplies, "missing-node", "org-1")],
        );
        let diags = run_rule(&GdmRule03, &file);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule_id, RuleId::L1Gdm03);
        assert!(matches!(
            &diags[0].location,
            Location::Edge { field: Some(f), .. } if f == "source"
        ));
        assert!(diags[0].message.contains("missing-node"));
    }

    #[test]
    fn gdm03_dangling_target_detected() {
        let file = make_file(
            vec![node("org-1", NodeType::Organization)],
            vec![edge("e-1", EdgeType::Supplies, "org-1", "missing-node")],
        );
        let diags = run_rule(&GdmRule03, &file);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule_id, RuleId::L1Gdm03);
        assert!(matches!(
            &diags[0].location,
            Location::Edge { field: Some(f), .. } if f == "target"
        ));
        assert!(diags[0].message.contains("missing-node"));
    }

    #[test]
    fn gdm03_both_dangling_two_diagnostics() {
        let file = make_file(
            vec![],
            vec![edge(
                "e-1",
                EdgeType::Supplies,
                "src-missing",
                "tgt-missing",
            )],
        );
        let diags = run_rule(&GdmRule03, &file);
        assert_eq!(diags.len(), 2);
        assert!(rule_ids(&diags).iter().all(|id| **id == RuleId::L1Gdm03));
    }

    #[test]
    fn gdm03_all_violations_collected_no_early_exit() {
        let file = make_file(
            vec![node("org-1", NodeType::Organization)],
            vec![
                edge("e-1", EdgeType::Supplies, "org-1", "ghost-1"),
                edge("e-2", EdgeType::Supplies, "ghost-2", "org-1"),
                edge("e-3", EdgeType::Supplies, "ghost-3", "ghost-4"),
            ],
        );
        let diags = run_rule(&GdmRule03, &file);
        // e-1: 1 (target), e-2: 1 (source), e-3: 2 (both)
        assert_eq!(diags.len(), 4);
    }

    // -----------------------------------------------------------------------
    // L1-GDM-04 tests
    // -----------------------------------------------------------------------

    #[test]
    fn gdm04_clean_all_known_types_no_diagnostics() {
        let file = make_file(
            vec![
                node("org-1", NodeType::Organization),
                node("org-2", NodeType::Organization),
            ],
            vec![
                edge("e-1", EdgeType::Supplies, "org-1", "org-2"),
                edge("e-2", EdgeType::SameAs, "org-1", "org-2"),
            ],
        );
        let diags = run_rule(&GdmRule04, &file);
        assert!(diags.is_empty());
    }

    #[test]
    fn gdm04_extension_type_with_dot_accepted() {
        let file = make_file(
            vec![
                node("org-1", NodeType::Organization),
                node("org-2", NodeType::Organization),
            ],
            vec![extension_edge(
                "e-1",
                "com.example.custom",
                "org-1",
                "org-2",
            )],
        );
        let diags = run_rule(&GdmRule04, &file);
        assert!(diags.is_empty(), "extension with dot must be accepted");
    }

    #[test]
    fn gdm04_unknown_type_no_dot_rejected() {
        let file = make_file(
            vec![
                node("org-1", NodeType::Organization),
                node("org-2", NodeType::Organization),
            ],
            vec![extension_edge("e-1", "mystery_type", "org-1", "org-2")],
        );
        let diags = run_rule(&GdmRule04, &file);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule_id, RuleId::L1Gdm04);
        assert_eq!(diags[0].severity, Severity::Error);
        assert!(diags[0].message.contains("mystery_type"));
    }

    #[test]
    fn gdm04_multiple_bad_types_all_collected() {
        let file = make_file(
            vec![
                node("n-1", NodeType::Organization),
                node("n-2", NodeType::Organization),
            ],
            vec![
                extension_edge("e-1", "bad_type_a", "n-1", "n-2"),
                extension_edge("e-2", "bad_type_b", "n-1", "n-2"),
                edge("e-3", EdgeType::Supplies, "n-1", "n-2"),
            ],
        );
        let diags = run_rule(&GdmRule04, &file);
        assert_eq!(diags.len(), 2);
    }

    // -----------------------------------------------------------------------
    // L1-GDM-05 tests
    // -----------------------------------------------------------------------

    #[test]
    fn gdm05_no_reporting_entity_no_diagnostics() {
        let file = make_file(vec![], vec![]);
        let diags = run_rule(&GdmRule05, &file);
        assert!(diags.is_empty());
    }

    #[test]
    fn gdm05_valid_reporting_entity_no_diagnostics() {
        let mut file = make_file(vec![node("org-acme", NodeType::Organization)], vec![]);
        file.reporting_entity = Some(NodeId::try_from("org-acme").expect("valid"));
        let diags = run_rule(&GdmRule05, &file);
        assert!(diags.is_empty());
    }

    #[test]
    fn gdm05_reporting_entity_missing_node_detected() {
        let mut file = make_file(vec![], vec![]);
        file.reporting_entity = Some(NodeId::try_from("ghost-org").expect("valid"));
        let diags = run_rule(&GdmRule05, &file);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule_id, RuleId::L1Gdm05);
        assert_eq!(diags[0].severity, Severity::Error);
        assert!(diags[0].message.contains("ghost-org"));
        assert!(matches!(
            diags[0].location,
            Location::Header {
                field: "reporting_entity"
            }
        ));
    }

    #[test]
    fn gdm05_reporting_entity_wrong_type_detected() {
        let mut file = make_file(vec![node("fac-1", NodeType::Facility)], vec![]);
        file.reporting_entity = Some(NodeId::try_from("fac-1").expect("valid"));
        let diags = run_rule(&GdmRule05, &file);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule_id, RuleId::L1Gdm05);
        assert!(diags[0].message.contains("fac-1"));
        assert!(diags[0].message.contains("organization"));
    }

    #[test]
    fn gdm05_reporting_entity_references_person_rejected() {
        let mut file = make_file(vec![node("person-1", NodeType::Person)], vec![]);
        file.reporting_entity = Some(NodeId::try_from("person-1").expect("valid"));
        let diags = run_rule(&GdmRule05, &file);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule_id, RuleId::L1Gdm05);
    }

    // -----------------------------------------------------------------------
    // L1-GDM-06 tests
    // -----------------------------------------------------------------------

    #[test]
    fn gdm06_clean_supplies_org_to_org_no_diagnostics() {
        let file = make_file(
            vec![
                node("org-1", NodeType::Organization),
                node("org-2", NodeType::Organization),
            ],
            vec![edge("e-1", EdgeType::Supplies, "org-1", "org-2")],
        );
        let diags = run_rule(&GdmRule06, &file);
        assert!(diags.is_empty());
    }

    #[test]
    fn gdm06_ownership_wrong_target_type_detected() {
        // ownership: source=org, target=org — using a facility as target is invalid.
        let file = make_file(
            vec![
                node("org-1", NodeType::Organization),
                node("fac-1", NodeType::Facility),
            ],
            vec![edge("e-1", EdgeType::Ownership, "org-1", "fac-1")],
        );
        let diags = run_rule(&GdmRule06, &file);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule_id, RuleId::L1Gdm06);
        assert_eq!(diags[0].severity, Severity::Error);
        assert!(matches!(
            &diags[0].location,
            Location::Edge { field: Some(f), .. } if f == "target"
        ));
    }

    #[test]
    fn gdm06_beneficial_ownership_person_to_org_accepted() {
        let file = make_file(
            vec![
                node("person-1", NodeType::Person),
                node("org-1", NodeType::Organization),
            ],
            vec![edge(
                "e-1",
                EdgeType::BeneficialOwnership,
                "person-1",
                "org-1",
            )],
        );
        let diags = run_rule(&GdmRule06, &file);
        assert!(diags.is_empty());
    }

    #[test]
    fn gdm06_beneficial_ownership_wrong_source_detected() {
        // source must be person; org is not permitted.
        let file = make_file(
            vec![
                node("org-1", NodeType::Organization),
                node("org-2", NodeType::Organization),
            ],
            vec![edge("e-1", EdgeType::BeneficialOwnership, "org-1", "org-2")],
        );
        let diags = run_rule(&GdmRule06, &file);
        assert_eq!(diags.len(), 1);
        assert!(matches!(
            &diags[0].location,
            Location::Edge { field: Some(f), .. } if f == "source"
        ));
    }

    #[test]
    fn gdm06_operates_org_to_facility_accepted() {
        let file = make_file(
            vec![
                node("org-1", NodeType::Organization),
                node("fac-1", NodeType::Facility),
            ],
            vec![edge("e-1", EdgeType::Operates, "org-1", "fac-1")],
        );
        let diags = run_rule(&GdmRule06, &file);
        assert!(diags.is_empty());
    }

    #[test]
    fn gdm06_produces_facility_to_good_accepted() {
        let file = make_file(
            vec![
                node("fac-1", NodeType::Facility),
                node("good-1", NodeType::Good),
            ],
            vec![edge("e-1", EdgeType::Produces, "fac-1", "good-1")],
        );
        let diags = run_rule(&GdmRule06, &file);
        assert!(diags.is_empty());
    }

    #[test]
    fn gdm06_produces_facility_to_consignment_accepted() {
        let file = make_file(
            vec![
                node("fac-1", NodeType::Facility),
                node("cons-1", NodeType::Consignment),
            ],
            vec![edge("e-1", EdgeType::Produces, "fac-1", "cons-1")],
        );
        let diags = run_rule(&GdmRule06, &file);
        assert!(diags.is_empty());
    }

    #[test]
    fn gdm06_attested_by_multiple_source_types_accepted() {
        let file = make_file(
            vec![
                node("org-1", NodeType::Organization),
                node("fac-1", NodeType::Facility),
                node("good-1", NodeType::Good),
                node("cons-1", NodeType::Consignment),
                node("att-1", NodeType::Attestation),
            ],
            vec![
                edge("e-1", EdgeType::AttestedBy, "org-1", "att-1"),
                edge("e-2", EdgeType::AttestedBy, "fac-1", "att-1"),
                edge("e-3", EdgeType::AttestedBy, "good-1", "att-1"),
                edge("e-4", EdgeType::AttestedBy, "cons-1", "att-1"),
            ],
        );
        let diags = run_rule(&GdmRule06, &file);
        assert!(
            diags.is_empty(),
            "all four source types are permitted for attested_by"
        );
    }

    #[test]
    fn gdm06_attested_by_wrong_target_type_detected() {
        let file = make_file(
            vec![
                node("org-1", NodeType::Organization),
                node("org-2", NodeType::Organization),
            ],
            vec![edge("e-1", EdgeType::AttestedBy, "org-1", "org-2")],
        );
        let diags = run_rule(&GdmRule06, &file);
        assert_eq!(diags.len(), 1);
        assert!(matches!(
            &diags[0].location,
            Location::Edge { field: Some(f), .. } if f == "target"
        ));
    }

    #[test]
    fn gdm06_same_as_any_types_accepted() {
        // same_as accepts any source and target node type.
        let file = make_file(
            vec![
                node("org-1", NodeType::Organization),
                node("fac-1", NodeType::Facility),
            ],
            vec![edge("e-1", EdgeType::SameAs, "org-1", "fac-1")],
        );
        let diags = run_rule(&GdmRule06, &file);
        assert!(
            diags.is_empty(),
            "same_as has no source/target type constraint"
        );
    }

    #[test]
    fn gdm06_extension_edge_type_exempt() {
        // Extension edge types bypass type-compatibility checks entirely.
        let file = make_file(
            vec![
                node("org-1", NodeType::Organization),
                node("att-1", NodeType::Attestation),
            ],
            vec![extension_edge(
                "e-1",
                "com.example.custom",
                "org-1",
                "att-1",
            )],
        );
        let diags = run_rule(&GdmRule06, &file);
        assert!(diags.is_empty(), "extension edges are exempt from GDM-06");
    }

    #[test]
    fn gdm06_dangling_ref_not_double_reported() {
        // When source doesn't exist, GDM-06 skips it (GDM-03 handles it).
        let file = make_file(
            vec![node("org-1", NodeType::Organization)],
            vec![edge("e-1", EdgeType::Supplies, "ghost", "org-1")],
        );
        let diags = run_rule(&GdmRule06, &file);
        assert!(
            diags.is_empty(),
            "missing nodes are GDM-03's concern, not GDM-06"
        );
    }

    #[test]
    fn gdm06_extension_node_type_not_constrained() {
        // A known edge type with an extension node type source: no GDM-06 diagnostic.
        let file = make_file(
            vec![
                extension_node("ext-1", "com.example.custom"),
                node("org-1", NodeType::Organization),
            ],
            vec![edge("e-1", EdgeType::Supplies, "ext-1", "org-1")],
        );
        let diags = run_rule(&GdmRule06, &file);
        assert!(
            diags.is_empty(),
            "extension node types are not constrained by GDM-06"
        );
    }

    #[test]
    fn gdm06_all_violations_collected_no_early_exit() {
        // Two edges each with a wrong source type → two diagnostics.
        let file = make_file(
            vec![
                node("fac-1", NodeType::Facility),
                node("fac-2", NodeType::Facility),
                node("org-1", NodeType::Organization),
            ],
            vec![
                // ownership expects org → org; facility source is wrong
                edge("e-1", EdgeType::Ownership, "fac-1", "org-1"),
                // ownership expects org → org; facility source is wrong
                edge("e-2", EdgeType::Ownership, "fac-2", "org-1"),
            ],
        );
        let diags = run_rule(&GdmRule06, &file);
        assert_eq!(diags.len(), 2);
        assert!(rule_ids(&diags).iter().all(|id| **id == RuleId::L1Gdm06));
    }

    #[test]
    fn gdm06_composed_of_good_to_consignment_accepted() {
        let file = make_file(
            vec![
                node("good-1", NodeType::Good),
                node("cons-1", NodeType::Consignment),
            ],
            vec![edge("e-1", EdgeType::ComposedOf, "good-1", "cons-1")],
        );
        let diags = run_rule(&GdmRule06, &file);
        assert!(diags.is_empty());
    }

    #[test]
    fn gdm06_operational_control_org_to_facility_accepted() {
        let file = make_file(
            vec![
                node("org-1", NodeType::Organization),
                node("fac-1", NodeType::Facility),
            ],
            vec![edge("e-1", EdgeType::OperationalControl, "org-1", "fac-1")],
        );
        let diags = run_rule(&GdmRule06, &file);
        assert!(diags.is_empty());
    }

    #[test]
    fn gdm06_tolls_facility_source_accepted() {
        // tolls permits organization or facility as source
        let file = make_file(
            vec![
                node("fac-1", NodeType::Facility),
                node("org-1", NodeType::Organization),
            ],
            vec![edge("e-1", EdgeType::Tolls, "fac-1", "org-1")],
        );
        let diags = run_rule(&GdmRule06, &file);
        assert!(diags.is_empty());
    }
}
