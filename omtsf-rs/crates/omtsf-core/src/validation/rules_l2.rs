/// L2-GDM-01 through L2-GDM-04 and L2-EID-01 through L2-EID-04:
/// Semantic warning rules enforcing SHOULD constraints from SPEC-001 and SPEC-002.
///
/// These rules are stateless structs implementing [`crate::validation::ValidationRule`].
/// All rules produce [`crate::validation::Severity::Warning`] diagnostics and collect
/// every violation without early exit.
///
/// Rules are registered in [`crate::validation::build_registry`] when
/// [`crate::validation::ValidationConfig::run_l2`] is `true`.
use std::collections::HashSet;

use crate::enums::{EdgeType, EdgeTypeTag, NodeType, NodeTypeTag};
use crate::file::OmtsFile;

use super::{Diagnostic, Level, Location, RuleId, Severity, ValidationRule};

// ---------------------------------------------------------------------------
// Static ISO 3166-1 alpha-2 country code table
//
// Embedded to keep the dependency tree light and WASM-compatible.
// Source: ISO 3166-1 alpha-2 list (249 codes as of 2026-01-01).
// ---------------------------------------------------------------------------

/// Returns `true` if `code` is a valid ISO 3166-1 alpha-2 country code.
///
/// The list is a static snapshot embedded at compile time. Codes are the
/// 249 officially assigned alpha-2 codes per ISO 3166 Maintenance Agency.
pub fn is_valid_iso3166_alpha2(code: &str) -> bool {
    // 249 officially assigned ISO 3166-1 alpha-2 codes (uppercase, 2 ASCII letters).
    // Sorted for readability; membership tested via binary search below.
    const CODES: &[&str] = &[
        "AD", "AE", "AF", "AG", "AI", "AL", "AM", "AO", "AQ", "AR", "AS", "AT", "AU", "AW", "AX",
        "AZ", "BA", "BB", "BD", "BE", "BF", "BG", "BH", "BI", "BJ", "BL", "BM", "BN", "BO", "BQ",
        "BR", "BS", "BT", "BV", "BW", "BY", "BZ", "CA", "CC", "CD", "CF", "CG", "CH", "CI", "CK",
        "CL", "CM", "CN", "CO", "CR", "CU", "CV", "CW", "CX", "CY", "CZ", "DE", "DJ", "DK", "DM",
        "DO", "DZ", "EC", "EE", "EG", "EH", "ER", "ES", "ET", "FI", "FJ", "FK", "FM", "FO", "FR",
        "GA", "GB", "GD", "GE", "GF", "GG", "GH", "GI", "GL", "GM", "GN", "GP", "GQ", "GR", "GS",
        "GT", "GU", "GW", "GY", "HK", "HM", "HN", "HR", "HT", "HU", "ID", "IE", "IL", "IM", "IN",
        "IO", "IQ", "IR", "IS", "IT", "JE", "JM", "JO", "JP", "KE", "KG", "KH", "KI", "KM", "KN",
        "KP", "KR", "KW", "KY", "KZ", "LA", "LB", "LC", "LI", "LK", "LR", "LS", "LT", "LU", "LV",
        "LY", "MA", "MC", "MD", "ME", "MF", "MG", "MH", "MK", "ML", "MM", "MN", "MO", "MP", "MQ",
        "MR", "MS", "MT", "MU", "MV", "MW", "MX", "MY", "MZ", "NA", "NC", "NE", "NF", "NG", "NI",
        "NL", "NO", "NP", "NR", "NU", "NZ", "OM", "PA", "PE", "PF", "PG", "PH", "PK", "PL", "PM",
        "PN", "PR", "PS", "PT", "PW", "PY", "QA", "RE", "RO", "RS", "RU", "RW", "SA", "SB", "SC",
        "SD", "SE", "SG", "SH", "SI", "SJ", "SK", "SL", "SM", "SN", "SO", "SR", "SS", "ST", "SV",
        "SX", "SY", "SZ", "TC", "TD", "TF", "TG", "TH", "TJ", "TK", "TL", "TM", "TN", "TO", "TR",
        "TT", "TV", "TW", "TZ", "UA", "UG", "UM", "US", "UY", "UZ", "VA", "VC", "VE", "VG", "VI",
        "VN", "VU", "WF", "WS", "YE", "YT", "ZA", "ZM", "ZW",
    ];
    CODES.binary_search(&code).is_ok()
}

// ---------------------------------------------------------------------------
// Helper: collect IDs of organisation nodes that are referenced by edges
// connecting a facility to an org (operates, operational_control, tolls target).
// ---------------------------------------------------------------------------

/// Returns the set of facility node IDs that have at least one edge connecting
/// them to an organisation node, either via an `operates` or `operational_control`
/// edge (where the facility is the target) or via the `Node::operator` field.
///
/// Used by [`L2Gdm01`] to detect isolated facilities.
fn facility_ids_with_org_connection<'a>(file: &'a OmtsFile) -> HashSet<&'a str> {
    // Collect all organisation node IDs for quick lookup.
    let org_ids: HashSet<&str> = file
        .nodes
        .iter()
        .filter(|n| n.node_type == NodeTypeTag::Known(NodeType::Organization))
        .map(|n| n.id.as_ref() as &str)
        .collect();

    let mut connected: HashSet<&'a str> = HashSet::new();

    // Check the `operator` property on facility nodes.
    for node in &file.nodes {
        if node.node_type != NodeTypeTag::Known(NodeType::Facility) {
            continue;
        }
        if let Some(ref op) = node.operator {
            let op_str: &str = op.as_ref() as &str;
            if org_ids.contains(op_str) {
                connected.insert(node.id.as_ref() as &str);
            }
        }
    }

    // Check edges that connect a facility to an organisation.
    for edge in &file.edges {
        let edge_type = match &edge.edge_type {
            EdgeTypeTag::Known(et) => et,
            EdgeTypeTag::Extension(_) => continue,
        };

        let src: &str = edge.source.as_ref() as &str;
        let tgt: &str = edge.target.as_ref() as &str;

        let (facility_side, org_side): (&str, &str) = match edge_type {
            // operates: source=org, target=facility
            EdgeType::Operates => (tgt, src),
            // operational_control: source=org, target=org|facility
            EdgeType::OperationalControl => (tgt, src),
            // tolls: source=org|facility, target=org — facility can be source
            EdgeType::Tolls => (src, tgt),
            // For other edge types we do not count as connecting facility↔org.
            EdgeType::Ownership
            | EdgeType::LegalParentage
            | EdgeType::FormerIdentity
            | EdgeType::BeneficialOwnership
            | EdgeType::Supplies
            | EdgeType::Subcontracts
            | EdgeType::Distributes
            | EdgeType::Brokers
            | EdgeType::Produces
            | EdgeType::ComposedOf
            | EdgeType::SellsTo
            | EdgeType::AttestedBy
            | EdgeType::SameAs => continue,
        };

        // Only count the connection when the "org side" is actually an organisation
        // and the "facility side" is actually a facility.
        let facility_is_facility = file.nodes.iter().any(|n| {
            (n.id.as_ref() as &str) == facility_side
                && n.node_type == NodeTypeTag::Known(NodeType::Facility)
        });

        if facility_is_facility && org_ids.contains(org_side) {
            connected.insert(facility_side);
        }
    }

    connected
}

// ---------------------------------------------------------------------------
// L2-GDM-01: Facility with no edge connecting it to an organisation
// ---------------------------------------------------------------------------

/// L2-GDM-01 — Every `facility` node SHOULD be connected to an `organization`
/// node via an edge or the `operator` property (SPEC-001 Section 9.2).
///
/// An isolated facility (one with no `operates`, `operational_control`, or `tolls`
/// edge to an organisation, and no `operator` field referencing an organisation)
/// is likely an incomplete graph. Each such facility produces one warning.
pub struct L2Gdm01;

impl ValidationRule for L2Gdm01 {
    fn id(&self) -> RuleId {
        RuleId::L2Gdm01
    }

    fn level(&self) -> Level {
        Level::L2
    }

    fn check(&self, file: &OmtsFile, diags: &mut Vec<Diagnostic>) {
        let connected = facility_ids_with_org_connection(file);

        for node in &file.nodes {
            if node.node_type != NodeTypeTag::Known(NodeType::Facility) {
                continue;
            }
            let id: &str = &node.id;
            if !connected.contains(id) {
                diags.push(Diagnostic::new(
                    RuleId::L2Gdm01,
                    Severity::Warning,
                    Location::Node {
                        node_id: id.to_owned(),
                        field: None,
                    },
                    format!(
                        "facility \"{id}\" has no edge or `operator` field connecting it to \
                         an organisation; consider adding an `operates` or `operational_control` \
                         edge"
                    ),
                ));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// L2-GDM-02: Ownership edge missing valid_from
// ---------------------------------------------------------------------------

/// L2-GDM-02 — `ownership` edges SHOULD have `valid_from` set
/// (SPEC-001 Section 9.2).
///
/// An ownership edge without `valid_from` is ambiguous in temporal merges.
/// Each such edge produces one warning.
pub struct L2Gdm02;

impl ValidationRule for L2Gdm02 {
    fn id(&self) -> RuleId {
        RuleId::L2Gdm02
    }

    fn level(&self) -> Level {
        Level::L2
    }

    fn check(&self, file: &OmtsFile, diags: &mut Vec<Diagnostic>) {
        for edge in &file.edges {
            if edge.edge_type != EdgeTypeTag::Known(EdgeType::Ownership) {
                continue;
            }
            if edge.properties.valid_from.is_none() {
                let id: &str = &edge.id;
                diags.push(Diagnostic::new(
                    RuleId::L2Gdm02,
                    Severity::Warning,
                    Location::Edge {
                        edge_id: id.to_owned(),
                        field: Some("properties.valid_from".to_owned()),
                    },
                    format!(
                        "ownership edge \"{id}\" is missing `valid_from`; temporal merge \
                         correctness requires a start date on ownership relationships"
                    ),
                ));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// L2-GDM-03: Organisation/facility/supply edges SHOULD carry data_quality
// ---------------------------------------------------------------------------

/// L2-GDM-03 — Every `organization` and `facility` node, and every `supplies`,
/// `subcontracts`, and `tolls` edge, SHOULD carry a `data_quality` object
/// (SPEC-001 Section 9.2).
///
/// Provenance metadata is essential for merge conflict resolution and regulatory
/// audit trails. Nodes and edges without `data_quality` each produce one warning.
pub struct L2Gdm03;

impl ValidationRule for L2Gdm03 {
    fn id(&self) -> RuleId {
        RuleId::L2Gdm03
    }

    fn level(&self) -> Level {
        Level::L2
    }

    fn check(&self, file: &OmtsFile, diags: &mut Vec<Diagnostic>) {
        // Check organization and facility nodes.
        for node in &file.nodes {
            let should_check = matches!(
                &node.node_type,
                NodeTypeTag::Known(NodeType::Organization) | NodeTypeTag::Known(NodeType::Facility)
            );
            if !should_check {
                continue;
            }
            if node.data_quality.is_none() {
                let id: &str = &node.id;
                let type_str = match &node.node_type {
                    NodeTypeTag::Known(NodeType::Organization) => "organization",
                    NodeTypeTag::Known(NodeType::Facility) => "facility",
                    // Unreachable due to the `should_check` guard above.
                    NodeTypeTag::Known(NodeType::Good)
                    | NodeTypeTag::Known(NodeType::Person)
                    | NodeTypeTag::Known(NodeType::Attestation)
                    | NodeTypeTag::Known(NodeType::Consignment)
                    | NodeTypeTag::Known(NodeType::BoundaryRef)
                    | NodeTypeTag::Extension(_) => continue,
                };
                diags.push(Diagnostic::new(
                    RuleId::L2Gdm03,
                    Severity::Warning,
                    Location::Node {
                        node_id: id.to_owned(),
                        field: Some("data_quality".to_owned()),
                    },
                    format!(
                        "{type_str} node \"{id}\" is missing a `data_quality` object; \
                         provenance metadata is essential for merge conflict resolution"
                    ),
                ));
            }
        }

        // Check supplies, subcontracts, and tolls edges.
        for edge in &file.edges {
            let should_check = matches!(
                &edge.edge_type,
                EdgeTypeTag::Known(EdgeType::Supplies)
                    | EdgeTypeTag::Known(EdgeType::Subcontracts)
                    | EdgeTypeTag::Known(EdgeType::Tolls)
            );
            if !should_check {
                continue;
            }
            if edge.properties.data_quality.is_none() {
                let id: &str = &edge.id;
                let type_str = match &edge.edge_type {
                    EdgeTypeTag::Known(EdgeType::Supplies) => "supplies",
                    EdgeTypeTag::Known(EdgeType::Subcontracts) => "subcontracts",
                    EdgeTypeTag::Known(EdgeType::Tolls) => "tolls",
                    // Unreachable due to should_check guard.
                    EdgeTypeTag::Known(EdgeType::Ownership)
                    | EdgeTypeTag::Known(EdgeType::OperationalControl)
                    | EdgeTypeTag::Known(EdgeType::LegalParentage)
                    | EdgeTypeTag::Known(EdgeType::FormerIdentity)
                    | EdgeTypeTag::Known(EdgeType::BeneficialOwnership)
                    | EdgeTypeTag::Known(EdgeType::Distributes)
                    | EdgeTypeTag::Known(EdgeType::Brokers)
                    | EdgeTypeTag::Known(EdgeType::Operates)
                    | EdgeTypeTag::Known(EdgeType::Produces)
                    | EdgeTypeTag::Known(EdgeType::ComposedOf)
                    | EdgeTypeTag::Known(EdgeType::SellsTo)
                    | EdgeTypeTag::Known(EdgeType::AttestedBy)
                    | EdgeTypeTag::Known(EdgeType::SameAs)
                    | EdgeTypeTag::Extension(_) => continue,
                };
                diags.push(Diagnostic::new(
                    RuleId::L2Gdm03,
                    Severity::Warning,
                    Location::Edge {
                        edge_id: id.to_owned(),
                        field: Some("properties.data_quality".to_owned()),
                    },
                    format!(
                        "{type_str} edge \"{id}\" is missing a `data_quality` object; \
                         provenance metadata is essential for merge conflict resolution"
                    ),
                ));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// L2-GDM-04: supplies edges with tier but no reporting_entity
// ---------------------------------------------------------------------------

/// L2-GDM-04 — If any `supplies` edge carries a `tier` property, the file
/// SHOULD declare `reporting_entity` in the file header (SPEC-001 Section 9.2).
///
/// Without a reporting entity, `tier` values are ambiguous. One warning is
/// emitted per offending `supplies` edge.
pub struct L2Gdm04;

impl ValidationRule for L2Gdm04 {
    fn id(&self) -> RuleId {
        RuleId::L2Gdm04
    }

    fn level(&self) -> Level {
        Level::L2
    }

    fn check(&self, file: &OmtsFile, diags: &mut Vec<Diagnostic>) {
        // If reporting_entity is present, all tier values are well-anchored.
        if file.reporting_entity.is_some() {
            return;
        }

        for edge in &file.edges {
            if edge.edge_type != EdgeTypeTag::Known(EdgeType::Supplies) {
                continue;
            }
            if edge.properties.tier.is_some() {
                let id: &str = &edge.id;
                diags.push(Diagnostic::new(
                    RuleId::L2Gdm04,
                    Severity::Warning,
                    Location::Edge {
                        edge_id: id.to_owned(),
                        field: Some("properties.tier".to_owned()),
                    },
                    format!(
                        "supplies edge \"{id}\" carries a `tier` property but the file has no \
                         `reporting_entity`; `tier` values are ambiguous without an anchor"
                    ),
                ));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// L2-EID-01: Organisation node with no external identifiers
// ---------------------------------------------------------------------------

/// L2-EID-01 — Every `organization` node SHOULD have at least one external
/// identifier (scheme other than `internal`) (SPEC-002 Section 6.2).
///
/// An organisation with only internal identifiers cannot participate in
/// cross-file merge. Each such node produces one warning.
pub struct L2Eid01;

impl ValidationRule for L2Eid01 {
    fn id(&self) -> RuleId {
        RuleId::L2Eid01
    }

    fn level(&self) -> Level {
        Level::L2
    }

    fn check(&self, file: &OmtsFile, diags: &mut Vec<Diagnostic>) {
        for node in &file.nodes {
            if node.node_type != NodeTypeTag::Known(NodeType::Organization) {
                continue;
            }
            let id: &str = &node.id;

            let has_external = match &node.identifiers {
                None => false,
                Some(ids) => ids.iter().any(|ident| ident.scheme != "internal"),
            };

            if !has_external {
                diags.push(Diagnostic::new(
                    RuleId::L2Eid01,
                    Severity::Warning,
                    Location::Node {
                        node_id: id.to_owned(),
                        field: Some("identifiers".to_owned()),
                    },
                    format!(
                        "organisation \"{id}\" has no external identifiers (non-`internal` \
                         scheme); cross-file merge requires at least one external identifier \
                         such as `lei`, `duns`, `nat-reg`, or `vat`"
                    ),
                ));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// L2-EID-04: vat authority not a valid ISO 3166-1 alpha-2 country code
// ---------------------------------------------------------------------------

/// L2-EID-04 — `vat` authority values SHOULD be valid ISO 3166-1 alpha-2
/// country codes (SPEC-002 Section 6.2).
///
/// The `vat` scheme requires the `authority` field to contain a 2-letter
/// country code. An invalid or unrecognised country code produces one warning
/// per identifier entry. Missing `authority` on `vat` identifiers is already
/// an L1 error (L1-EID-03) and is not re-reported here.
pub struct L2Eid04;

impl ValidationRule for L2Eid04 {
    fn id(&self) -> RuleId {
        RuleId::L2Eid04
    }

    fn level(&self) -> Level {
        Level::L2
    }

    fn check(&self, file: &OmtsFile, diags: &mut Vec<Diagnostic>) {
        for node in &file.nodes {
            let node_id: &str = &node.id;
            let Some(ref identifiers) = node.identifiers else {
                continue;
            };
            for (index, ident) in identifiers.iter().enumerate() {
                if ident.scheme != "vat" {
                    continue;
                }
                // Missing authority is an L1-EID-03 concern; skip silently.
                let Some(ref authority) = ident.authority else {
                    continue;
                };
                if !is_valid_iso3166_alpha2(authority.as_str()) {
                    diags.push(Diagnostic::new(
                        RuleId::L2Eid04,
                        Severity::Warning,
                        Location::Identifier {
                            node_id: node_id.to_owned(),
                            index,
                            field: Some("authority".to_owned()),
                        },
                        format!(
                            "node \"{node_id}\" identifiers[{index}]: `vat` authority \
                             \"{authority}\" is not a valid ISO 3166-1 alpha-2 country code"
                        ),
                    ));
                }
            }
        }
    }
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
    use crate::types::{DataQuality, Identifier};

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

    fn node_with_operator(id: &str, operator_id: &str) -> Node {
        let mut n = node(id, NodeType::Facility);
        n.operator = Some(NodeId::try_from(operator_id).expect("valid operator"));
        n
    }

    fn node_with_identifiers(id: &str, node_type: NodeType, identifiers: Vec<Identifier>) -> Node {
        let mut n = node(id, node_type);
        n.identifiers = Some(identifiers);
        n
    }

    fn node_with_data_quality(id: &str, node_type: NodeType) -> Node {
        let mut n = node(id, node_type);
        n.data_quality = Some(DataQuality {
            confidence: None,
            source: Some("test".to_owned()),
            last_verified: None,
            extra: serde_json::Map::new(),
        });
        n
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

    fn edge_with_valid_from(id: &str, edge_type: EdgeType, source: &str, target: &str) -> Edge {
        let mut e = edge(id, edge_type, source, target);
        e.properties.valid_from = Some(CalendarDate::try_from("2020-01-01").expect("valid date"));
        e
    }

    fn edge_with_tier(id: &str, source: &str, target: &str, tier: u32) -> Edge {
        let mut e = edge(id, EdgeType::Supplies, source, target);
        e.properties.tier = Some(tier);
        e
    }

    fn edge_with_data_quality(id: &str, edge_type: EdgeType, source: &str, target: &str) -> Edge {
        let mut e = edge(id, edge_type, source, target);
        e.properties.data_quality = Some(DataQuality {
            confidence: None,
            source: Some("test".to_owned()),
            last_verified: None,
            extra: serde_json::Map::new(),
        });
        e
    }

    fn identifier(scheme: &str, value: &str, authority: Option<&str>) -> Identifier {
        Identifier {
            scheme: scheme.to_owned(),
            value: value.to_owned(),
            authority: authority.map(str::to_owned),
            valid_from: None,
            valid_to: None,
            sensitivity: None,
            verification_status: None,
            verification_date: None,
            extra: serde_json::Map::new(),
        }
    }

    fn run_rule(rule: &dyn ValidationRule, file: &OmtsFile) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        rule.check(file, &mut diags);
        diags
    }

    // -----------------------------------------------------------------------
    // iso3166 helper tests
    // -----------------------------------------------------------------------

    #[test]
    fn iso3166_known_codes_accepted() {
        assert!(is_valid_iso3166_alpha2("DE"));
        assert!(is_valid_iso3166_alpha2("GB"));
        assert!(is_valid_iso3166_alpha2("US"));
        assert!(is_valid_iso3166_alpha2("FR"));
        assert!(is_valid_iso3166_alpha2("JP"));
        assert!(is_valid_iso3166_alpha2("CN"));
        assert!(is_valid_iso3166_alpha2("ZW")); // last code in the list
        assert!(is_valid_iso3166_alpha2("AD")); // first code in the list
    }

    #[test]
    fn iso3166_invalid_codes_rejected() {
        assert!(!is_valid_iso3166_alpha2("XX")); // not assigned
        assert!(!is_valid_iso3166_alpha2("de")); // lowercase
        assert!(!is_valid_iso3166_alpha2("DEU")); // three letters
        assert!(!is_valid_iso3166_alpha2("")); // empty
        assert!(!is_valid_iso3166_alpha2("1A")); // digit prefix
        assert!(!is_valid_iso3166_alpha2("EU")); // political union, not ISO 3166-1
    }

    // -----------------------------------------------------------------------
    // L2-GDM-01 tests
    // -----------------------------------------------------------------------

    #[test]
    fn gdm01_facility_with_operates_edge_passes() {
        let file = make_file(
            vec![
                node("org-1", NodeType::Organization),
                node("fac-1", NodeType::Facility),
            ],
            vec![edge("e-1", EdgeType::Operates, "org-1", "fac-1")],
        );
        let diags = run_rule(&L2Gdm01, &file);
        assert!(
            diags.is_empty(),
            "connected facility must produce no warning"
        );
    }

    #[test]
    fn gdm01_facility_with_operator_property_passes() {
        let file = make_file(
            vec![
                node("org-1", NodeType::Organization),
                node_with_operator("fac-1", "org-1"),
            ],
            vec![],
        );
        let diags = run_rule(&L2Gdm01, &file);
        assert!(
            diags.is_empty(),
            "facility with operator property must produce no warning"
        );
    }

    #[test]
    fn gdm01_isolated_facility_produces_warning() {
        let file = make_file(vec![node("fac-1", NodeType::Facility)], vec![]);
        let diags = run_rule(&L2Gdm01, &file);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule_id, RuleId::L2Gdm01);
        assert_eq!(diags[0].severity, Severity::Warning);
        assert!(diags[0].message.contains("fac-1"));
    }

    #[test]
    fn gdm01_multiple_isolated_facilities_all_warned() {
        let file = make_file(
            vec![
                node("fac-1", NodeType::Facility),
                node("fac-2", NodeType::Facility),
                node("org-1", NodeType::Organization),
            ],
            vec![],
        );
        let diags = run_rule(&L2Gdm01, &file);
        assert_eq!(diags.len(), 2, "both facilities should be warned");
        let ids: Vec<_> = diags.iter().map(|d| &d.rule_id).collect();
        assert!(ids.iter().all(|id| **id == RuleId::L2Gdm01));
    }

    #[test]
    fn gdm01_org_node_not_warned() {
        let file = make_file(vec![node("org-1", NodeType::Organization)], vec![]);
        let diags = run_rule(&L2Gdm01, &file);
        assert!(diags.is_empty(), "organisations are not subject to GDM-01");
    }

    #[test]
    fn gdm01_facility_with_operational_control_edge_passes() {
        let file = make_file(
            vec![
                node("org-1", NodeType::Organization),
                node("fac-1", NodeType::Facility),
            ],
            vec![edge("e-1", EdgeType::OperationalControl, "org-1", "fac-1")],
        );
        let diags = run_rule(&L2Gdm01, &file);
        assert!(
            diags.is_empty(),
            "facility connected via operational_control must pass"
        );
    }

    #[test]
    fn gdm01_empty_file_no_diagnostics() {
        let file = make_file(vec![], vec![]);
        let diags = run_rule(&L2Gdm01, &file);
        assert!(diags.is_empty());
    }

    // -----------------------------------------------------------------------
    // L2-GDM-02 tests
    // -----------------------------------------------------------------------

    #[test]
    fn gdm02_ownership_with_valid_from_passes() {
        let file = make_file(
            vec![
                node("org-1", NodeType::Organization),
                node("org-2", NodeType::Organization),
            ],
            vec![edge_with_valid_from(
                "e-1",
                EdgeType::Ownership,
                "org-1",
                "org-2",
            )],
        );
        let diags = run_rule(&L2Gdm02, &file);
        assert!(diags.is_empty());
    }

    #[test]
    fn gdm02_ownership_without_valid_from_warns() {
        let file = make_file(
            vec![
                node("org-1", NodeType::Organization),
                node("org-2", NodeType::Organization),
            ],
            vec![edge("e-1", EdgeType::Ownership, "org-1", "org-2")],
        );
        let diags = run_rule(&L2Gdm02, &file);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule_id, RuleId::L2Gdm02);
        assert_eq!(diags[0].severity, Severity::Warning);
        assert!(diags[0].message.contains("e-1"));
    }

    #[test]
    fn gdm02_non_ownership_edge_without_valid_from_not_warned() {
        // supplies edges do not require valid_from under GDM-02.
        let file = make_file(
            vec![
                node("org-1", NodeType::Organization),
                node("org-2", NodeType::Organization),
            ],
            vec![edge("e-1", EdgeType::Supplies, "org-1", "org-2")],
        );
        let diags = run_rule(&L2Gdm02, &file);
        assert!(diags.is_empty(), "GDM-02 only applies to ownership edges");
    }

    #[test]
    fn gdm02_multiple_ownership_edges_without_valid_from_all_warned() {
        let file = make_file(
            vec![
                node("org-1", NodeType::Organization),
                node("org-2", NodeType::Organization),
            ],
            vec![
                edge("e-1", EdgeType::Ownership, "org-1", "org-2"),
                edge("e-2", EdgeType::Ownership, "org-2", "org-1"),
            ],
        );
        let diags = run_rule(&L2Gdm02, &file);
        assert_eq!(diags.len(), 2);
    }

    #[test]
    fn gdm02_empty_file_no_diagnostics() {
        let file = make_file(vec![], vec![]);
        let diags = run_rule(&L2Gdm02, &file);
        assert!(diags.is_empty());
    }

    // -----------------------------------------------------------------------
    // L2-GDM-03 tests
    // -----------------------------------------------------------------------

    #[test]
    fn gdm03_org_with_data_quality_passes() {
        let file = make_file(
            vec![node_with_data_quality("org-1", NodeType::Organization)],
            vec![],
        );
        let diags = run_rule(&L2Gdm03, &file);
        assert!(diags.is_empty());
    }

    #[test]
    fn gdm03_org_without_data_quality_warns() {
        let file = make_file(vec![node("org-1", NodeType::Organization)], vec![]);
        let diags = run_rule(&L2Gdm03, &file);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule_id, RuleId::L2Gdm03);
        assert_eq!(diags[0].severity, Severity::Warning);
        assert!(diags[0].message.contains("org-1"));
    }

    #[test]
    fn gdm03_facility_without_data_quality_warns() {
        let file = make_file(vec![node("fac-1", NodeType::Facility)], vec![]);
        let diags = run_rule(&L2Gdm03, &file);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule_id, RuleId::L2Gdm03);
        assert!(diags[0].message.contains("fac-1"));
    }

    #[test]
    fn gdm03_good_node_without_data_quality_not_warned() {
        let file = make_file(vec![node("good-1", NodeType::Good)], vec![]);
        let diags = run_rule(&L2Gdm03, &file);
        assert!(diags.is_empty(), "good nodes are not subject to GDM-03");
    }

    #[test]
    fn gdm03_supplies_edge_without_data_quality_warns() {
        let file = make_file(
            vec![
                node("org-1", NodeType::Organization),
                node("org-2", NodeType::Organization),
            ],
            vec![edge("e-1", EdgeType::Supplies, "org-1", "org-2")],
        );
        let diags: Vec<_> = run_rule(&L2Gdm03, &file)
            .into_iter()
            .filter(|d| matches!(&d.location, Location::Edge { .. }))
            .collect();
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("e-1"));
    }

    #[test]
    fn gdm03_supplies_edge_with_data_quality_passes() {
        let file = make_file(
            vec![
                node("org-1", NodeType::Organization),
                node("org-2", NodeType::Organization),
            ],
            vec![edge_with_data_quality(
                "e-1",
                EdgeType::Supplies,
                "org-1",
                "org-2",
            )],
        );
        let edge_diags: Vec<_> = run_rule(&L2Gdm03, &file)
            .into_iter()
            .filter(|d| matches!(&d.location, Location::Edge { .. }))
            .collect();
        assert!(edge_diags.is_empty());
    }

    #[test]
    fn gdm03_ownership_edge_without_data_quality_not_warned() {
        // GDM-03 only covers supplies, subcontracts, tolls edges.
        let file = make_file(
            vec![
                node("org-1", NodeType::Organization),
                node("org-2", NodeType::Organization),
            ],
            vec![edge("e-1", EdgeType::Ownership, "org-1", "org-2")],
        );
        let edge_diags: Vec<_> = run_rule(&L2Gdm03, &file)
            .into_iter()
            .filter(|d| matches!(&d.location, Location::Edge { .. }))
            .collect();
        assert!(edge_diags.is_empty());
    }

    // -----------------------------------------------------------------------
    // L2-GDM-04 tests
    // -----------------------------------------------------------------------

    #[test]
    fn gdm04_supplies_tier_with_reporting_entity_passes() {
        let mut file = make_file(
            vec![
                node("org-1", NodeType::Organization),
                node("org-2", NodeType::Organization),
            ],
            vec![edge_with_tier("e-1", "org-1", "org-2", 1)],
        );
        file.reporting_entity = Some(NodeId::try_from("org-1").expect("valid"));
        let diags = run_rule(&L2Gdm04, &file);
        assert!(diags.is_empty());
    }

    #[test]
    fn gdm04_supplies_tier_without_reporting_entity_warns() {
        let file = make_file(
            vec![
                node("org-1", NodeType::Organization),
                node("org-2", NodeType::Organization),
            ],
            vec![edge_with_tier("e-1", "org-1", "org-2", 1)],
        );
        let diags = run_rule(&L2Gdm04, &file);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule_id, RuleId::L2Gdm04);
        assert_eq!(diags[0].severity, Severity::Warning);
        assert!(diags[0].message.contains("e-1"));
    }

    #[test]
    fn gdm04_supplies_without_tier_no_warning() {
        let file = make_file(
            vec![
                node("org-1", NodeType::Organization),
                node("org-2", NodeType::Organization),
            ],
            vec![edge("e-1", EdgeType::Supplies, "org-1", "org-2")],
        );
        let diags = run_rule(&L2Gdm04, &file);
        assert!(diags.is_empty());
    }

    #[test]
    fn gdm04_multiple_tier_edges_without_reporting_entity_all_warned() {
        let file = make_file(
            vec![
                node("org-1", NodeType::Organization),
                node("org-2", NodeType::Organization),
                node("org-3", NodeType::Organization),
            ],
            vec![
                edge_with_tier("e-1", "org-1", "org-2", 1),
                edge_with_tier("e-2", "org-2", "org-3", 2),
            ],
        );
        let diags = run_rule(&L2Gdm04, &file);
        assert_eq!(diags.len(), 2);
    }

    // -----------------------------------------------------------------------
    // L2-EID-01 tests
    // -----------------------------------------------------------------------

    #[test]
    fn eid01_org_with_external_identifier_passes() {
        let file = make_file(
            vec![node_with_identifiers(
                "org-1",
                NodeType::Organization,
                vec![identifier("lei", "5493006MHB84DD0ZWV18", None)],
            )],
            vec![],
        );
        let diags = run_rule(&L2Eid01, &file);
        assert!(diags.is_empty());
    }

    #[test]
    fn eid01_org_with_only_internal_identifier_warns() {
        let file = make_file(
            vec![node_with_identifiers(
                "org-1",
                NodeType::Organization,
                vec![identifier("internal", "V-100234", Some("sap-mm-prod"))],
            )],
            vec![],
        );
        let diags = run_rule(&L2Eid01, &file);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule_id, RuleId::L2Eid01);
        assert_eq!(diags[0].severity, Severity::Warning);
        assert!(diags[0].message.contains("org-1"));
    }

    #[test]
    fn eid01_org_with_no_identifiers_warns() {
        let file = make_file(vec![node("org-1", NodeType::Organization)], vec![]);
        let diags = run_rule(&L2Eid01, &file);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule_id, RuleId::L2Eid01);
        assert_eq!(diags[0].severity, Severity::Warning);
    }

    #[test]
    fn eid01_org_with_mixed_identifiers_passes() {
        // Has both internal and external → passes.
        let file = make_file(
            vec![node_with_identifiers(
                "org-1",
                NodeType::Organization,
                vec![
                    identifier("internal", "V-100234", Some("sap-mm-prod")),
                    identifier("duns", "081466849", None),
                ],
            )],
            vec![],
        );
        let diags = run_rule(&L2Eid01, &file);
        assert!(diags.is_empty());
    }

    #[test]
    fn eid01_facility_node_not_subject_to_rule() {
        // L2-EID-01 only applies to organization nodes.
        let file = make_file(vec![node("fac-1", NodeType::Facility)], vec![]);
        let diags = run_rule(&L2Eid01, &file);
        assert!(diags.is_empty(), "facility nodes are not subject to EID-01");
    }

    #[test]
    fn eid01_multiple_orgs_without_identifiers_all_warned() {
        let file = make_file(
            vec![
                node("org-1", NodeType::Organization),
                node("org-2", NodeType::Organization),
            ],
            vec![],
        );
        let diags = run_rule(&L2Eid01, &file);
        assert_eq!(diags.len(), 2);
    }

    // -----------------------------------------------------------------------
    // L2-EID-04 tests
    // -----------------------------------------------------------------------

    #[test]
    fn eid04_valid_vat_authority_passes() {
        let file = make_file(
            vec![node_with_identifiers(
                "org-1",
                NodeType::Organization,
                vec![identifier("vat", "DE123456789", Some("DE"))],
            )],
            vec![],
        );
        let diags = run_rule(&L2Eid04, &file);
        assert!(diags.is_empty());
    }

    #[test]
    fn eid04_invalid_vat_authority_warns() {
        let file = make_file(
            vec![node_with_identifiers(
                "org-1",
                NodeType::Organization,
                vec![identifier("vat", "XX123456789", Some("XX"))],
            )],
            vec![],
        );
        let diags = run_rule(&L2Eid04, &file);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule_id, RuleId::L2Eid04);
        assert_eq!(diags[0].severity, Severity::Warning);
        assert!(diags[0].message.contains("XX"));
    }

    #[test]
    fn eid04_missing_vat_authority_not_warned_here() {
        // L1-EID-03 handles missing authority; L2-EID-04 skips missing authority.
        let file = make_file(
            vec![node_with_identifiers(
                "org-1",
                NodeType::Organization,
                vec![identifier("vat", "DE123456789", None)],
            )],
            vec![],
        );
        let diags = run_rule(&L2Eid04, &file);
        assert!(diags.is_empty(), "missing authority is L1-EID-03's concern");
    }

    #[test]
    fn eid04_non_vat_scheme_not_warned() {
        // nat-reg authority is a GLEIF RA code, not an ISO 3166 code.
        let file = make_file(
            vec![node_with_identifiers(
                "org-1",
                NodeType::Organization,
                vec![identifier("nat-reg", "HRB86891", Some("RA000548"))],
            )],
            vec![],
        );
        let diags = run_rule(&L2Eid04, &file);
        assert!(diags.is_empty(), "nat-reg is not subject to EID-04");
    }

    #[test]
    fn eid04_multiple_invalid_vat_authorities_all_warned() {
        let file = make_file(
            vec![node_with_identifiers(
                "org-1",
                NodeType::Organization,
                vec![
                    identifier("vat", "XX123456789", Some("XX")),
                    identifier("vat", "EU123456789", Some("EU")),
                ],
            )],
            vec![],
        );
        let diags = run_rule(&L2Eid04, &file);
        assert_eq!(diags.len(), 2);
    }

    #[test]
    fn eid04_lowercase_country_code_warns() {
        // "de" is not a valid ISO 3166-1 alpha-2 code (must be uppercase "DE").
        let file = make_file(
            vec![node_with_identifiers(
                "org-1",
                NodeType::Organization,
                vec![identifier("vat", "de123456789", Some("de"))],
            )],
            vec![],
        );
        let diags = run_rule(&L2Eid04, &file);
        assert_eq!(diags.len(), 1);
    }

    // -----------------------------------------------------------------------
    // Severity invariant: all L2 rules produce Warning, never Error or Info
    // -----------------------------------------------------------------------

    #[test]
    fn all_l2_rules_produce_warnings_only() {
        // Construct a file that triggers at least one diagnostic from every rule.
        let file = make_file(
            vec![
                node("fac-1", NodeType::Facility),
                node("org-1", NodeType::Organization),
            ],
            vec![
                edge("e-own", EdgeType::Ownership, "org-1", "org-1"),
                edge_with_tier("e-sup", "org-1", "org-1", 1),
            ],
        );

        let rules: Vec<Box<dyn ValidationRule>> = vec![
            Box::new(L2Gdm01),
            Box::new(L2Gdm02),
            Box::new(L2Gdm03),
            Box::new(L2Gdm04),
            Box::new(L2Eid01),
        ];

        for rule in &rules {
            let mut diags = Vec::new();
            rule.check(&file, &mut diags);
            for d in &diags {
                assert_eq!(
                    d.severity,
                    Severity::Warning,
                    "rule {} produced a non-Warning diagnostic: {:?}",
                    rule.id().code(),
                    d
                );
            }
        }

        // L2-EID-04 triggered separately with a vat identifier.
        let file_eid04 = make_file(
            vec![node_with_identifiers(
                "org-2",
                NodeType::Organization,
                vec![identifier("vat", "XX123", Some("XX"))],
            )],
            vec![],
        );
        let mut diags = Vec::new();
        L2Eid04.check(&file_eid04, &mut diags);
        for d in &diags {
            assert_eq!(d.severity, Severity::Warning);
        }
    }
}
