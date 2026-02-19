/// L1-SDI-01 and L1-SDI-02: Selective Disclosure structural validation rules.
///
/// These rules enforce the MUST constraints from SPEC-004 as listed in the
/// validation specification (docs/validation.md Section 4.1, L1-SDI table).
/// Each rule is a stateless struct implementing [`crate::validation::ValidationRule`].
/// All rules collect every violation without early exit.
///
/// Rules are registered in [`crate::validation::build_registry`] when
/// [`crate::validation::ValidationConfig::run_l1`] is `true`.
use crate::enums::{DisclosureScope, NodeType, NodeTypeTag, Sensitivity};
use crate::file::OmtsFile;
use crate::sensitivity::effective_sensitivity;

use super::{Diagnostic, Level, Location, RuleId, Severity, ValidationRule};

// ---------------------------------------------------------------------------
// L1-SDI-01: boundary_ref nodes have exactly one opaque identifier
// ---------------------------------------------------------------------------

/// L1-SDI-01 — `boundary_ref` nodes have exactly one identifier with scheme `opaque`.
///
/// Per SPEC-004 and the redaction specification (docs/redaction.md Section 5.1),
/// a `boundary_ref` node MUST carry exactly one identifier and that identifier's
/// `scheme` MUST be `"opaque"`. Zero identifiers, more than one identifier, or
/// any identifier whose scheme is not `"opaque"` are all violations.
pub struct L1Sdi01;

impl ValidationRule for L1Sdi01 {
    fn id(&self) -> RuleId {
        RuleId::L1Sdi01
    }

    fn level(&self) -> Level {
        Level::L1
    }

    fn check(&self, file: &OmtsFile, diags: &mut Vec<Diagnostic>) {
        for node in &file.nodes {
            // Only applies to boundary_ref nodes.
            if node.node_type != NodeTypeTag::Known(NodeType::BoundaryRef) {
                continue;
            }

            let node_id: &str = &node.id;

            let identifiers = match &node.identifiers {
                None => {
                    // No identifiers at all — missing the required opaque identifier.
                    diags.push(Diagnostic::new(
                        RuleId::L1Sdi01,
                        Severity::Error,
                        Location::Node {
                            node_id: node_id.to_owned(),
                            field: Some("identifiers".to_owned()),
                        },
                        format!(
                            "boundary_ref node \"{node_id}\" has no identifiers; \
                             must have exactly one identifier with scheme \"opaque\""
                        ),
                    ));
                    continue;
                }
                Some(ids) => ids,
            };

            // Count identifiers with scheme "opaque".
            let opaque_count = identifiers
                .iter()
                .filter(|id| id.scheme == "opaque")
                .count();

            let total_count = identifiers.len();

            if total_count == 0 {
                diags.push(Diagnostic::new(
                    RuleId::L1Sdi01,
                    Severity::Error,
                    Location::Node {
                        node_id: node_id.to_owned(),
                        field: Some("identifiers".to_owned()),
                    },
                    format!(
                        "boundary_ref node \"{node_id}\" has an empty identifiers array; \
                         must have exactly one identifier with scheme \"opaque\""
                    ),
                ));
                continue;
            }

            if opaque_count == 0 {
                // Has identifiers, but none are "opaque".
                diags.push(Diagnostic::new(
                    RuleId::L1Sdi01,
                    Severity::Error,
                    Location::Node {
                        node_id: node_id.to_owned(),
                        field: Some("identifiers".to_owned()),
                    },
                    format!(
                        "boundary_ref node \"{node_id}\" has no identifier with scheme \
                         \"opaque\"; must have exactly one"
                    ),
                ));
            } else if opaque_count > 1 {
                // Multiple opaque identifiers.
                diags.push(Diagnostic::new(
                    RuleId::L1Sdi01,
                    Severity::Error,
                    Location::Node {
                        node_id: node_id.to_owned(),
                        field: Some("identifiers".to_owned()),
                    },
                    format!(
                        "boundary_ref node \"{node_id}\" has {opaque_count} identifiers with \
                         scheme \"opaque\"; must have exactly one"
                    ),
                ));
            }

            if total_count > 1 {
                // Extra identifiers beyond the single opaque one.
                diags.push(Diagnostic::new(
                    RuleId::L1Sdi01,
                    Severity::Error,
                    Location::Node {
                        node_id: node_id.to_owned(),
                        field: Some("identifiers".to_owned()),
                    },
                    format!(
                        "boundary_ref node \"{node_id}\" has {total_count} identifiers; \
                         must have exactly one identifier with scheme \"opaque\""
                    ),
                ));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// L1-SDI-02: sensitivity constraints are satisfied when disclosure_scope is declared
// ---------------------------------------------------------------------------

/// L1-SDI-02 — If `disclosure_scope` is declared, sensitivity constraints are satisfied.
///
/// The constraints are derived from the redaction specification
/// (docs/redaction.md Sections 3.1 and 3.2):
///
/// - `partner` scope: identifiers with effective sensitivity `confidential` must
///   not appear.
/// - `public` scope: identifiers with effective sensitivity `confidential` OR
///   `restricted` must not appear.
/// - `internal` scope: no sensitivity constraints.
///
/// If `disclosure_scope` is absent from the file header, this rule emits no
/// diagnostics (the constraint only applies when a scope is explicitly declared).
pub struct L1Sdi02;

impl ValidationRule for L1Sdi02 {
    fn id(&self) -> RuleId {
        RuleId::L1Sdi02
    }

    fn level(&self) -> Level {
        Level::L1
    }

    fn check(&self, file: &OmtsFile, diags: &mut Vec<Diagnostic>) {
        let scope = match &file.disclosure_scope {
            None => return, // No scope declared — rule does not apply.
            Some(s) => s,
        };

        // Determine the maximum allowed sensitivity for the declared scope.
        let max_allowed = match scope {
            DisclosureScope::Internal => return, // No constraint on internal.
            DisclosureScope::Partner => Sensitivity::Restricted, // confidential not allowed
            DisclosureScope::Public => Sensitivity::Public, // restricted + confidential not allowed
        };

        for node in &file.nodes {
            let node_id: &str = &node.id;
            let identifiers = match &node.identifiers {
                None => continue,
                Some(ids) => ids,
            };

            for (index, identifier) in identifiers.iter().enumerate() {
                let eff = effective_sensitivity(identifier, &node.node_type);

                let violates = match max_allowed {
                    Sensitivity::Restricted => eff == Sensitivity::Confidential,
                    Sensitivity::Public => {
                        eff == Sensitivity::Confidential || eff == Sensitivity::Restricted
                    }
                    // Internal scope returns early above; this arm is unreachable
                    // but the exhaustive match is required by workspace rules.
                    Sensitivity::Confidential => false,
                };

                if violates {
                    let scope_label = match scope {
                        DisclosureScope::Internal => "internal",
                        DisclosureScope::Partner => "partner",
                        DisclosureScope::Public => "public",
                    };
                    let sensitivity_label = match eff {
                        Sensitivity::Public => "public",
                        Sensitivity::Restricted => "restricted",
                        Sensitivity::Confidential => "confidential",
                    };
                    diags.push(Diagnostic::new(
                        RuleId::L1Sdi02,
                        Severity::Error,
                        Location::Identifier {
                            node_id: node_id.to_owned(),
                            index,
                            field: Some("sensitivity".to_owned()),
                        },
                        format!(
                            "node \"{node_id}\" identifiers[{index}] has effective sensitivity \
                             \"{sensitivity_label}\" which violates disclosure_scope \
                             \"{scope_label}\""
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
    use crate::enums::{DisclosureScope, NodeType, NodeTypeTag, Sensitivity};
    use crate::file::OmtsFile;
    use crate::newtypes::{CalendarDate, FileSalt, NodeId, SemVer};
    use crate::structures::Node;
    use crate::types::Identifier;
    use crate::validation::{Diagnostic, ValidationRule};

    // -----------------------------------------------------------------------
    // Fixture helpers
    // -----------------------------------------------------------------------

    const SALT: &str = "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef";

    fn make_file(nodes: Vec<Node>, disclosure_scope: Option<DisclosureScope>) -> OmtsFile {
        OmtsFile {
            omtsf_version: SemVer::try_from("1.0.0").expect("valid"),
            snapshot_date: CalendarDate::try_from("2026-02-19").expect("valid"),
            file_salt: FileSalt::try_from(SALT).expect("valid"),
            disclosure_scope,
            previous_snapshot_ref: None,
            snapshot_sequence: None,
            reporting_entity: None,
            nodes,
            edges: vec![],
            extra: serde_json::Map::new(),
        }
    }

    fn node_no_identifiers(id: &str, node_type: NodeType) -> Node {
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

    fn node_with_identifiers(id: &str, node_type: NodeType, identifiers: Vec<Identifier>) -> Node {
        Node {
            identifiers: Some(identifiers),
            ..node_no_identifiers(id, node_type)
        }
    }

    fn opaque_identifier(value: &str) -> Identifier {
        Identifier {
            scheme: "opaque".to_owned(),
            value: value.to_owned(),
            authority: None,
            valid_from: None,
            valid_to: None,
            sensitivity: None,
            verification_status: None,
            verification_date: None,
            extra: serde_json::Map::new(),
        }
    }

    fn identifier_with_scheme(scheme: &str, sensitivity: Option<Sensitivity>) -> Identifier {
        Identifier {
            scheme: scheme.to_owned(),
            value: "test-value".to_owned(),
            authority: None,
            valid_from: None,
            valid_to: None,
            sensitivity,
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
    // L1-SDI-01 — satisfied cases
    // -----------------------------------------------------------------------

    #[test]
    fn sdi01_non_boundary_ref_nodes_ignored() {
        // Organization nodes are not subject to L1-SDI-01, even with no identifiers.
        let file = make_file(
            vec![node_no_identifiers("org-1", NodeType::Organization)],
            None,
        );
        let diags = run_rule(&L1Sdi01, &file);
        assert!(
            diags.is_empty(),
            "non-boundary_ref nodes must not trigger L1-SDI-01"
        );
    }

    #[test]
    fn sdi01_boundary_ref_exactly_one_opaque_passes() {
        // A boundary_ref with exactly one opaque identifier satisfies L1-SDI-01.
        let file = make_file(
            vec![node_with_identifiers(
                "br-1",
                NodeType::BoundaryRef,
                vec![opaque_identifier("abc123")],
            )],
            None,
        );
        let diags = run_rule(&L1Sdi01, &file);
        assert!(
            diags.is_empty(),
            "boundary_ref with exactly one opaque identifier must pass L1-SDI-01"
        );
    }

    #[test]
    fn sdi01_empty_graph_no_diagnostics() {
        let file = make_file(vec![], None);
        let diags = run_rule(&L1Sdi01, &file);
        assert!(diags.is_empty());
    }

    #[test]
    fn sdi01_multiple_valid_boundary_refs_pass() {
        let file = make_file(
            vec![
                node_with_identifiers(
                    "br-1",
                    NodeType::BoundaryRef,
                    vec![opaque_identifier("hash1")],
                ),
                node_with_identifiers(
                    "br-2",
                    NodeType::BoundaryRef,
                    vec![opaque_identifier("hash2")],
                ),
            ],
            None,
        );
        let diags = run_rule(&L1Sdi01, &file);
        assert!(
            diags.is_empty(),
            "multiple valid boundary_ref nodes must all pass"
        );
    }

    // -----------------------------------------------------------------------
    // L1-SDI-01 — violation: no identifiers field
    // -----------------------------------------------------------------------

    #[test]
    fn sdi01_boundary_ref_no_identifiers_field_is_error() {
        let file = make_file(
            vec![node_no_identifiers("br-1", NodeType::BoundaryRef)],
            None,
        );
        let diags = run_rule(&L1Sdi01, &file);
        assert_eq!(diags.len(), 1, "missing identifiers field must be an error");
        assert_eq!(diags[0].rule_id, RuleId::L1Sdi01);
        assert_eq!(diags[0].severity, Severity::Error);
        assert!(diags[0].message.contains("br-1"));
        assert!(diags[0].message.contains("opaque"));
    }

    // -----------------------------------------------------------------------
    // L1-SDI-01 — violation: empty identifiers array
    // -----------------------------------------------------------------------

    #[test]
    fn sdi01_boundary_ref_empty_identifiers_is_error() {
        let file = make_file(
            vec![node_with_identifiers("br-1", NodeType::BoundaryRef, vec![])],
            None,
        );
        let diags = run_rule(&L1Sdi01, &file);
        assert_eq!(diags.len(), 1, "empty identifiers array must be an error");
        assert_eq!(diags[0].rule_id, RuleId::L1Sdi01);
        assert_eq!(diags[0].severity, Severity::Error);
        assert!(diags[0].message.contains("br-1"));
    }

    // -----------------------------------------------------------------------
    // L1-SDI-01 — violation: identifier with wrong scheme
    // -----------------------------------------------------------------------

    #[test]
    fn sdi01_boundary_ref_non_opaque_scheme_is_error() {
        let file = make_file(
            vec![node_with_identifiers(
                "br-1",
                NodeType::BoundaryRef,
                vec![identifier_with_scheme("lei", None)],
            )],
            None,
        );
        let diags = run_rule(&L1Sdi01, &file);
        assert_eq!(
            diags.len(),
            1,
            "non-opaque scheme with no opaque identifier is an error"
        );
        assert_eq!(diags[0].rule_id, RuleId::L1Sdi01);
        assert!(diags[0].message.contains("br-1"));
        assert!(diags[0].message.contains("opaque"));
    }

    // -----------------------------------------------------------------------
    // L1-SDI-01 — violation: more than one opaque identifier
    // -----------------------------------------------------------------------

    #[test]
    fn sdi01_boundary_ref_two_opaque_identifiers_is_error() {
        let file = make_file(
            vec![node_with_identifiers(
                "br-1",
                NodeType::BoundaryRef,
                vec![opaque_identifier("hash1"), opaque_identifier("hash2")],
            )],
            None,
        );
        let diags = run_rule(&L1Sdi01, &file);
        // Expect two diagnostics: one for multiple opaque, one for total count > 1.
        assert!(
            !diags.is_empty(),
            "two opaque identifiers must produce at least one error"
        );
        assert!(diags.iter().all(|d| d.rule_id == RuleId::L1Sdi01));
        assert!(diags.iter().all(|d| d.severity == Severity::Error));
        assert!(diags.iter().any(|d| d.message.contains("br-1")));
    }

    // -----------------------------------------------------------------------
    // L1-SDI-01 — violation: one opaque + extra identifiers
    // -----------------------------------------------------------------------

    #[test]
    fn sdi01_boundary_ref_opaque_plus_extra_identifier_is_error() {
        // One opaque plus one non-opaque → total count > 1, which violates SDI-01.
        let file = make_file(
            vec![node_with_identifiers(
                "br-1",
                NodeType::BoundaryRef,
                vec![
                    opaque_identifier("hash1"),
                    identifier_with_scheme("lei", None),
                ],
            )],
            None,
        );
        let diags = run_rule(&L1Sdi01, &file);
        assert!(
            !diags.is_empty(),
            "opaque + extra identifier must produce an error"
        );
        assert!(diags.iter().all(|d| d.rule_id == RuleId::L1Sdi01));
        assert!(diags.iter().all(|d| d.severity == Severity::Error));
    }

    // -----------------------------------------------------------------------
    // L1-SDI-01 — multiple boundary_ref nodes, some violating
    // -----------------------------------------------------------------------

    #[test]
    fn sdi01_multiple_boundary_refs_all_violations_collected() {
        let file = make_file(
            vec![
                // Valid
                node_with_identifiers(
                    "br-ok",
                    NodeType::BoundaryRef,
                    vec![opaque_identifier("good")],
                ),
                // No identifiers
                node_no_identifiers("br-bad1", NodeType::BoundaryRef),
                // Wrong scheme
                node_with_identifiers(
                    "br-bad2",
                    NodeType::BoundaryRef,
                    vec![identifier_with_scheme("lei", None)],
                ),
            ],
            None,
        );
        let diags = run_rule(&L1Sdi01, &file);
        // br-ok passes; br-bad1 and br-bad2 each produce one diagnostic.
        assert_eq!(diags.len(), 2, "each violation must produce a diagnostic");
        assert!(diags.iter().any(|d| d.message.contains("br-bad1")));
        assert!(diags.iter().any(|d| d.message.contains("br-bad2")));
        assert!(!diags.iter().any(|d| d.message.contains("br-ok")));
    }

    // -----------------------------------------------------------------------
    // L1-SDI-02 — no disclosure_scope: rule does not fire
    // -----------------------------------------------------------------------

    #[test]
    fn sdi02_no_disclosure_scope_no_diagnostics() {
        // A confidential identifier on a partner-intended file without scope
        // declared must not be flagged — L1-SDI-02 only applies when scope is set.
        let file = make_file(
            vec![node_with_identifiers(
                "org-1",
                NodeType::Organization,
                vec![identifier_with_scheme(
                    "lei",
                    Some(Sensitivity::Confidential),
                )],
            )],
            None, // no disclosure_scope
        );
        let diags = run_rule(&L1Sdi02, &file);
        assert!(
            diags.is_empty(),
            "without disclosure_scope, L1-SDI-02 must not fire"
        );
    }

    // -----------------------------------------------------------------------
    // L1-SDI-02 — internal scope: no constraints
    // -----------------------------------------------------------------------

    #[test]
    fn sdi02_internal_scope_allows_all_sensitivities() {
        let file = make_file(
            vec![node_with_identifiers(
                "org-1",
                NodeType::Organization,
                vec![
                    identifier_with_scheme("lei", Some(Sensitivity::Confidential)),
                    identifier_with_scheme("vat", Some(Sensitivity::Restricted)),
                    identifier_with_scheme("duns", Some(Sensitivity::Public)),
                ],
            )],
            Some(DisclosureScope::Internal),
        );
        let diags = run_rule(&L1Sdi02, &file);
        assert!(
            diags.is_empty(),
            "internal scope imposes no sensitivity constraints"
        );
    }

    // -----------------------------------------------------------------------
    // L1-SDI-02 — partner scope: confidential not allowed, restricted allowed
    // -----------------------------------------------------------------------

    #[test]
    fn sdi02_partner_scope_public_identifier_passes() {
        let file = make_file(
            vec![node_with_identifiers(
                "org-1",
                NodeType::Organization,
                vec![identifier_with_scheme("lei", None)], // lei defaults to public
            )],
            Some(DisclosureScope::Partner),
        );
        let diags = run_rule(&L1Sdi02, &file);
        assert!(
            diags.is_empty(),
            "public identifier must pass partner scope"
        );
    }

    #[test]
    fn sdi02_partner_scope_restricted_identifier_passes() {
        let file = make_file(
            vec![node_with_identifiers(
                "org-1",
                NodeType::Organization,
                vec![identifier_with_scheme("nat-reg", None)], // nat-reg defaults to restricted
            )],
            Some(DisclosureScope::Partner),
        );
        let diags = run_rule(&L1Sdi02, &file);
        assert!(
            diags.is_empty(),
            "restricted identifier must pass partner scope"
        );
    }

    #[test]
    fn sdi02_partner_scope_confidential_identifier_is_error() {
        let file = make_file(
            vec![node_with_identifiers(
                "org-1",
                NodeType::Organization,
                vec![identifier_with_scheme(
                    "lei",
                    Some(Sensitivity::Confidential),
                )],
            )],
            Some(DisclosureScope::Partner),
        );
        let diags = run_rule(&L1Sdi02, &file);
        assert_eq!(
            diags.len(),
            1,
            "confidential identifier violates partner scope"
        );
        assert_eq!(diags[0].rule_id, RuleId::L1Sdi02);
        assert_eq!(diags[0].severity, Severity::Error);
        assert!(diags[0].message.contains("org-1"));
        assert!(diags[0].message.contains("confidential"));
        assert!(diags[0].message.contains("partner"));
    }

    #[test]
    fn sdi02_partner_scope_person_node_implicit_confidential_is_error() {
        // Person nodes default to confidential sensitivity for all identifiers
        // when no explicit sensitivity is set — this must also be flagged.
        let file = make_file(
            vec![node_with_identifiers(
                "person-1",
                NodeType::Person,
                vec![identifier_with_scheme("lei", None)], // implicitly confidential on Person
            )],
            Some(DisclosureScope::Partner),
        );
        let diags = run_rule(&L1Sdi02, &file);
        assert_eq!(
            diags.len(),
            1,
            "person node implicit confidential violates partner scope"
        );
        assert_eq!(diags[0].rule_id, RuleId::L1Sdi02);
        assert_eq!(diags[0].severity, Severity::Error);
        assert!(diags[0].message.contains("person-1"));
        assert!(diags[0].message.contains("confidential"));
    }

    // -----------------------------------------------------------------------
    // L1-SDI-02 — public scope: restricted + confidential not allowed
    // -----------------------------------------------------------------------

    #[test]
    fn sdi02_public_scope_public_identifier_passes() {
        let file = make_file(
            vec![node_with_identifiers(
                "org-1",
                NodeType::Organization,
                vec![identifier_with_scheme("lei", None)], // lei defaults to public
            )],
            Some(DisclosureScope::Public),
        );
        let diags = run_rule(&L1Sdi02, &file);
        assert!(diags.is_empty(), "public identifier must pass public scope");
    }

    #[test]
    fn sdi02_public_scope_restricted_identifier_is_error() {
        let file = make_file(
            vec![node_with_identifiers(
                "org-1",
                NodeType::Organization,
                vec![identifier_with_scheme("nat-reg", None)], // nat-reg defaults to restricted
            )],
            Some(DisclosureScope::Public),
        );
        let diags = run_rule(&L1Sdi02, &file);
        assert_eq!(
            diags.len(),
            1,
            "restricted identifier violates public scope"
        );
        assert_eq!(diags[0].rule_id, RuleId::L1Sdi02);
        assert_eq!(diags[0].severity, Severity::Error);
        assert!(diags[0].message.contains("org-1"));
        assert!(diags[0].message.contains("restricted"));
        assert!(diags[0].message.contains("public"));
    }

    #[test]
    fn sdi02_public_scope_confidential_identifier_is_error() {
        let file = make_file(
            vec![node_with_identifiers(
                "org-1",
                NodeType::Organization,
                vec![identifier_with_scheme(
                    "lei",
                    Some(Sensitivity::Confidential),
                )],
            )],
            Some(DisclosureScope::Public),
        );
        let diags = run_rule(&L1Sdi02, &file);
        assert_eq!(
            diags.len(),
            1,
            "confidential identifier violates public scope"
        );
        assert_eq!(diags[0].rule_id, RuleId::L1Sdi02);
        assert_eq!(diags[0].severity, Severity::Error);
        assert!(diags[0].message.contains("org-1"));
        assert!(diags[0].message.contains("confidential"));
    }

    #[test]
    fn sdi02_public_scope_explicit_restricted_on_lei_is_error() {
        // Explicit restricted override on a normally-public scheme is still restricted.
        let file = make_file(
            vec![node_with_identifiers(
                "org-1",
                NodeType::Organization,
                vec![identifier_with_scheme("lei", Some(Sensitivity::Restricted))],
            )],
            Some(DisclosureScope::Public),
        );
        let diags = run_rule(&L1Sdi02, &file);
        assert_eq!(
            diags.len(),
            1,
            "explicit restricted on lei still violates public scope"
        );
    }

    // -----------------------------------------------------------------------
    // L1-SDI-02 — multiple nodes and identifiers, all violations collected
    // -----------------------------------------------------------------------

    #[test]
    fn sdi02_public_scope_collects_all_violations() {
        let file = make_file(
            vec![
                node_with_identifiers(
                    "org-1",
                    NodeType::Organization,
                    vec![
                        identifier_with_scheme("lei", None),     // public — ok
                        identifier_with_scheme("nat-reg", None), // restricted — violation
                        identifier_with_scheme("vat", Some(Sensitivity::Confidential)), // confidential — violation
                    ],
                ),
                node_with_identifiers(
                    "org-2",
                    NodeType::Organization,
                    vec![
                        identifier_with_scheme("duns", None),     // public — ok
                        identifier_with_scheme("internal", None), // restricted — violation
                    ],
                ),
            ],
            Some(DisclosureScope::Public),
        );
        let diags = run_rule(&L1Sdi02, &file);
        // 3 violations total: nat-reg, confidential vat on org-1, internal on org-2.
        assert_eq!(diags.len(), 3, "all violations must be collected");
        assert!(diags.iter().all(|d| d.rule_id == RuleId::L1Sdi02));
        assert!(diags.iter().all(|d| d.severity == Severity::Error));
    }

    #[test]
    fn sdi02_partner_scope_collects_all_violations() {
        let file = make_file(
            vec![node_with_identifiers(
                "org-1",
                NodeType::Organization,
                vec![
                    identifier_with_scheme("lei", Some(Sensitivity::Confidential)), // violation
                    identifier_with_scheme("nat-reg", None), // restricted — ok
                    identifier_with_scheme("vat", Some(Sensitivity::Confidential)), // violation
                ],
            )],
            Some(DisclosureScope::Partner),
        );
        let diags = run_rule(&L1Sdi02, &file);
        assert_eq!(
            diags.len(),
            2,
            "both confidential identifiers must be flagged"
        );
        assert!(diags.iter().all(|d| d.rule_id == RuleId::L1Sdi02));
    }

    // -----------------------------------------------------------------------
    // L1-SDI-02 — location precision: correct index in Identifier location
    // -----------------------------------------------------------------------

    #[test]
    fn sdi02_location_points_to_correct_identifier_index() {
        let file = make_file(
            vec![node_with_identifiers(
                "org-1",
                NodeType::Organization,
                vec![
                    identifier_with_scheme("lei", None), // index 0 — public, ok
                    identifier_with_scheme("vat", Some(Sensitivity::Confidential)), // index 1 — violation
                    identifier_with_scheme("duns", None), // index 2 — public, ok
                ],
            )],
            Some(DisclosureScope::Partner),
        );
        let diags = run_rule(&L1Sdi02, &file);
        assert_eq!(diags.len(), 1);
        assert!(matches!(
            &diags[0].location,
            Location::Identifier { node_id, index: 1, .. } if node_id == "org-1"
        ));
    }

    // -----------------------------------------------------------------------
    // L1-SDI-02 — nodes with no identifiers are ignored
    // -----------------------------------------------------------------------

    #[test]
    fn sdi02_node_with_no_identifiers_is_ignored() {
        let file = make_file(
            vec![node_no_identifiers("org-1", NodeType::Organization)],
            Some(DisclosureScope::Public),
        );
        let diags = run_rule(&L1Sdi02, &file);
        assert!(
            diags.is_empty(),
            "nodes without identifiers must be ignored by L1-SDI-02"
        );
    }

    // -----------------------------------------------------------------------
    // L1-SDI-02 — boundary_ref opaque identifier is public by default
    // -----------------------------------------------------------------------

    #[test]
    fn sdi02_boundary_ref_opaque_identifier_passes_public_scope() {
        // The opaque scheme defaults to public per the redaction spec.
        // A valid boundary_ref node must pass L1-SDI-02 under public scope.
        let file = make_file(
            vec![node_with_identifiers(
                "br-1",
                NodeType::BoundaryRef,
                vec![opaque_identifier("abc123")],
            )],
            Some(DisclosureScope::Public),
        );
        let diags = run_rule(&L1Sdi02, &file);
        assert!(
            diags.is_empty(),
            "boundary_ref with opaque identifier must pass public scope"
        );
    }
}
