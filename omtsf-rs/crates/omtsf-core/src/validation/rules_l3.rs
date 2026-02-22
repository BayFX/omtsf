/// L3 enrichment rules: L3-EID-01, L3-MRG-01, and L3-MRG-02.
///
/// L3 rules are off by default and require a concrete [`ExternalDataSource`] to produce
/// any findings. When `external_data` is `None`, each rule skips its checks entirely.
/// When `external_data` is `Some`, the rule queries the data source and emits
/// [`Severity::Info`] diagnostics for findings that cannot be determined from the
/// file alone.
///
/// [`L3Mrg02`] is the exception: it does not use `external_data` at all. It builds
/// a petgraph representation of the file and runs cycle detection on the
/// `legal_parentage` subgraph. It is gated on `run_l3` only because cycle detection
/// is computationally heavier than L1/L2 checks and is considered an enrichment-level
/// quality observation rather than a hard conformance requirement.
///
/// Rules are registered in [`crate::validation::build_registry`] when
/// [`crate::validation::ValidationConfig::run_l3`] is `true`.
use std::collections::HashMap;

use crate::enums::{EdgeType, EdgeTypeTag};
use crate::file::OmtsFile;
use crate::graph::{build_graph, detect_cycles};

use super::external::ExternalDataSource;
use super::{Diagnostic, Level, Location, RuleId, Severity, ValidationRule};

/// L3-EID-01 — Every `lei` identifier on an organisation node SHOULD resolve
/// to an active LEI registration in the GLEIF database.
///
/// For each node that carries one or more `lei` scheme identifiers, this rule
/// queries [`ExternalDataSource::lei_status`]. If the status record is found
/// and `is_active` is `false`, an Info diagnostic is emitted. If the data
/// source returns `None` for a given LEI, the check is silently skipped for
/// that identifier.
///
/// When `external_data` is `None` the rule produces no diagnostics.
pub struct L3Eid01;

impl ValidationRule for L3Eid01 {
    fn id(&self) -> RuleId {
        RuleId::L3Eid01
    }

    fn level(&self) -> Level {
        Level::L3
    }

    fn check(
        &self,
        file: &OmtsFile,
        diags: &mut Vec<Diagnostic>,
        external_data: Option<&dyn ExternalDataSource>,
    ) {
        let Some(source) = external_data else {
            return;
        };

        for node in &file.nodes {
            let Some(ref identifiers) = node.identifiers else {
                continue;
            };

            for (index, ident) in identifiers.iter().enumerate() {
                if ident.scheme != "lei" {
                    continue;
                }

                let Some(record) = source.lei_status(&ident.value) else {
                    continue;
                };

                if !record.is_active {
                    let node_id: &str = &node.id;
                    diags.push(Diagnostic::new(
                        RuleId::L3Eid01,
                        Severity::Info,
                        Location::Identifier {
                            node_id: node_id.to_owned(),
                            index,
                            field: Some("value".to_owned()),
                        },
                        format!(
                            "node \"{node_id}\" identifiers[{index}]: LEI \"{}\" has \
                             registration status \"{}\" in the GLEIF database (is_active=false)",
                            ident.value, record.registration_status
                        ),
                    ));
                }
            }
        }
    }
}

/// L3-MRG-01 — For each organisation node, the sum of `percentage` values on
/// all inbound `ownership` edges SHOULD NOT exceed 100.0 when the external
/// data source confirms the organisation is an active legal entity.
///
/// The rule queries [`ExternalDataSource::lei_status`] (if an LEI is present)
/// to verify that the target exists in the registry before flagging percentage
/// anomalies. If the data source returns `None`, the percentage check still runs
/// using purely local data (the external lookup is opportunistic enrichment).
///
/// When `external_data` is `None` the rule produces no diagnostics.
pub struct L3Mrg01;

impl ValidationRule for L3Mrg01 {
    fn id(&self) -> RuleId {
        RuleId::L3Mrg01
    }

    fn level(&self) -> Level {
        Level::L3
    }

    fn check(
        &self,
        file: &OmtsFile,
        diags: &mut Vec<Diagnostic>,
        external_data: Option<&dyn ExternalDataSource>,
    ) {
        // Pure structural ownership-sum checks belong at L1/L2.
        let Some(_source) = external_data else {
            return;
        };

        let org_ids: std::collections::HashSet<&str> = file
            .nodes
            .iter()
            .filter(|n| {
                n.node_type
                    == crate::enums::NodeTypeTag::Known(crate::enums::NodeType::Organization)
            })
            .map(|n| n.id.as_ref() as &str)
            .collect();

        let mut ownership_by_target: HashMap<&str, Vec<&crate::structures::Edge>> = HashMap::new();
        for edge in &file.edges {
            if edge.edge_type == EdgeTypeTag::Known(EdgeType::Ownership) {
                ownership_by_target
                    .entry(edge.target.as_ref())
                    .or_default()
                    .push(edge);
            }
        }

        for org_node in &file.nodes {
            if org_node.node_type
                != crate::enums::NodeTypeTag::Known(crate::enums::NodeType::Organization)
            {
                continue;
            }

            let org_id: &str = &org_node.id;

            let mut total_pct: f64 = 0.0;
            let mut has_any_pct = false;

            let edges = ownership_by_target
                .get(org_id)
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            for edge in edges {
                if !org_ids.contains(edge.source.as_ref() as &str) {
                    continue;
                }
                if let Some(pct) = edge.properties.percentage {
                    has_any_pct = true;
                    total_pct += pct;
                }
            }

            if has_any_pct && total_pct > 100.0 {
                diags.push(Diagnostic::new(
                    RuleId::L3Mrg01,
                    Severity::Info,
                    Location::Node {
                        node_id: org_id.to_owned(),
                        field: None,
                    },
                    format!(
                        "organisation \"{org_id}\" has inbound ownership percentages summing to \
                         {total_pct:.2}%, which exceeds 100%; verify ownership structure with \
                         external registry data"
                    ),
                ));
            }
        }
    }
}

/// L3-MRG-02 — The `legal_parentage` subgraph SHOULD form a forest (directed
/// acyclic graph); a cycle indicates a subsidiary that is, directly or
/// indirectly, its own parent.
///
/// This rule builds the petgraph representation of the file and runs Kahn's
/// topological-sort cycle detection ([`detect_cycles`]) restricted to
/// `legal_parentage` edges. For each cycle found it emits one [`Severity::Info`]
/// diagnostic at the [`Location::Global`] level, listing the cycle participants
/// by their graph-local node IDs.
///
/// Unlike the other L3 rules this rule does not consult [`ExternalDataSource`]
/// at all; it operates on the graph structure alone. It is gated on
/// [`crate::validation::ValidationConfig::run_l3`] because cycle detection is
/// computationally heavier than L1/L2 per-element checks.
///
/// If `build_graph` fails (e.g. on a file that slipped past L1 checks), the
/// rule emits no diagnostics and returns silently.
pub struct L3Mrg02;

impl ValidationRule for L3Mrg02 {
    fn id(&self) -> RuleId {
        RuleId::L3Mrg02
    }

    fn level(&self) -> Level {
        Level::L3
    }

    fn check(
        &self,
        file: &OmtsFile,
        diags: &mut Vec<Diagnostic>,
        _external_data: Option<&dyn ExternalDataSource>,
    ) {
        let Ok(graph) = build_graph(file) else {
            return;
        };

        let lp_filter = [EdgeTypeTag::Known(EdgeType::LegalParentage)]
            .into_iter()
            .collect();
        let cycles = detect_cycles(&graph, &lp_filter);

        for cycle in cycles {
            let node_ids: Vec<String> = cycle
                .iter()
                .filter_map(|idx| graph.node_weight(*idx))
                .map(|w| w.local_id.clone())
                .collect();

            let cycle_str = node_ids.join(" → ");
            diags.push(Diagnostic::new(
                RuleId::L3Mrg02,
                Severity::Info,
                Location::Global,
                format!(
                    "legal_parentage cycle detected: {cycle_str}; \
                     a subsidiary cannot be its own parent"
                ),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use std::collections::BTreeMap;

    use super::*;
    use crate::enums::{EdgeType, EdgeTypeTag, NodeType, NodeTypeTag};
    use crate::file::OmtsFile;
    use crate::newtypes::{CalendarDate, EdgeId, FileSalt, NodeId, SemVer};
    use crate::structures::{Edge, EdgeProperties, Node};
    use crate::types::Identifier;
    use crate::validation::external::{LeiRecord, NatRegRecord};

    /// A simple mock that returns pre-configured LEI and nat-reg records.
    struct MockDataSource {
        lei_records: std::collections::HashMap<String, LeiRecord>,
        nat_reg_records: std::collections::HashMap<(String, String), NatRegRecord>,
    }

    impl MockDataSource {
        fn new() -> Self {
            Self {
                lei_records: std::collections::HashMap::new(),
                nat_reg_records: std::collections::HashMap::new(),
            }
        }

        fn with_lei(mut self, lei: &str, status: &str, is_active: bool) -> Self {
            self.lei_records.insert(
                lei.to_owned(),
                LeiRecord {
                    lei: lei.to_owned(),
                    registration_status: status.to_owned(),
                    is_active,
                },
            );
            self
        }

        fn with_nat_reg(mut self, authority: &str, value: &str, is_active: bool) -> Self {
            self.nat_reg_records.insert(
                (authority.to_owned(), value.to_owned()),
                NatRegRecord {
                    authority: authority.to_owned(),
                    value: value.to_owned(),
                    is_active,
                },
            );
            self
        }
    }

    impl ExternalDataSource for MockDataSource {
        fn lei_status(&self, lei: &str) -> Option<LeiRecord> {
            self.lei_records.get(lei).cloned()
        }

        fn nat_reg_lookup(&self, authority: &str, value: &str) -> Option<NatRegRecord> {
            self.nat_reg_records
                .get(&(authority.to_owned(), value.to_owned()))
                .cloned()
        }
    }

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
            extra: BTreeMap::new(),
        }
    }

    fn org_node(id: &str) -> Node {
        Node {
            id: NodeId::try_from(id).expect("valid id"),
            node_type: NodeTypeTag::Known(NodeType::Organization),
            ..Default::default()
        }
    }

    fn org_node_with_lei(id: &str, lei: &str) -> Node {
        let mut n = org_node(id);
        n.identifiers = Some(vec![Identifier {
            scheme: "lei".to_owned(),
            value: lei.to_owned(),
            authority: None,
            valid_from: None,
            valid_to: None,
            sensitivity: None,
            verification_status: None,
            verification_date: None,
            extra: BTreeMap::new(),
        }]);
        n
    }

    fn ownership_edge(id: &str, source: &str, target: &str, percentage: Option<f64>) -> Edge {
        let mut e = Edge {
            id: EdgeId::try_from(id).expect("valid id"),
            edge_type: EdgeTypeTag::Known(EdgeType::Ownership),
            source: NodeId::try_from(source).expect("valid source"),
            target: NodeId::try_from(target).expect("valid target"),
            identifiers: None,
            properties: EdgeProperties::default(),
            extra: BTreeMap::new(),
        };
        e.properties.percentage = percentage;
        e
    }

    fn run_l3(
        rule: &dyn ValidationRule,
        file: &OmtsFile,
        ext: Option<&dyn ExternalDataSource>,
    ) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        rule.check(file, &mut diags, ext);
        diags
    }

    #[test]
    fn eid01_no_external_source_produces_no_diagnostics() {
        let file = make_file(
            vec![org_node_with_lei("org-1", "5493006MHB84DD0ZWV18")],
            vec![],
        );
        let diags = run_l3(&L3Eid01, &file, None);
        assert!(
            diags.is_empty(),
            "L3-EID-01 must be silent when external_data is None"
        );
    }

    #[test]
    fn eid01_active_lei_produces_no_diagnostic() {
        let source = MockDataSource::new().with_lei("5493006MHB84DD0ZWV18", "ISSUED", true);
        let file = make_file(
            vec![org_node_with_lei("org-1", "5493006MHB84DD0ZWV18")],
            vec![],
        );
        let diags = run_l3(&L3Eid01, &file, Some(&source));
        assert!(
            diags.is_empty(),
            "active LEI must produce no diagnostic; got: {diags:?}"
        );
    }

    #[test]
    fn eid01_lapsed_lei_produces_info_diagnostic() {
        let source = MockDataSource::new().with_lei("5493006MHB84DD0ZWV18", "LAPSED", false);
        let file = make_file(
            vec![org_node_with_lei("org-1", "5493006MHB84DD0ZWV18")],
            vec![],
        );
        let diags = run_l3(&L3Eid01, &file, Some(&source));
        assert_eq!(
            diags.len(),
            1,
            "lapsed LEI should produce one Info diagnostic"
        );
        assert_eq!(diags[0].rule_id, RuleId::L3Eid01);
        assert_eq!(diags[0].severity, Severity::Info);
        assert!(diags[0].message.contains("LAPSED"));
        assert!(diags[0].message.contains("org-1"));
    }

    #[test]
    fn eid01_annulled_lei_produces_info_diagnostic() {
        let source = MockDataSource::new().with_lei("5493006MHB84DD0ZWV18", "ANNULLED", false);
        let file = make_file(
            vec![org_node_with_lei("org-1", "5493006MHB84DD0ZWV18")],
            vec![],
        );
        let diags = run_l3(&L3Eid01, &file, Some(&source));
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].severity, Severity::Info);
        assert!(diags[0].message.contains("ANNULLED"));
    }

    #[test]
    fn eid01_unknown_lei_in_data_source_skips_silently() {
        let source = MockDataSource::new();
        let file = make_file(
            vec![org_node_with_lei("org-1", "5493006MHB84DD0ZWV18")],
            vec![],
        );
        let diags = run_l3(&L3Eid01, &file, Some(&source));
        assert!(
            diags.is_empty(),
            "unknown LEI in data source should be skipped silently"
        );
    }

    #[test]
    fn eid01_node_with_no_identifiers_produces_no_diagnostic() {
        let source = MockDataSource::new();
        let file = make_file(vec![org_node("org-1")], vec![]);
        let diags = run_l3(&L3Eid01, &file, Some(&source));
        assert!(diags.is_empty());
    }

    #[test]
    fn eid01_multiple_nodes_lapsed_all_flagged() {
        let source = MockDataSource::new()
            .with_lei("5493006MHB84DD0ZWV18", "LAPSED", false)
            .with_lei("254900OPPU84GM83MG36", "LAPSED", false);
        let file = make_file(
            vec![
                org_node_with_lei("org-1", "5493006MHB84DD0ZWV18"),
                org_node_with_lei("org-2", "254900OPPU84GM83MG36"),
            ],
            vec![],
        );
        let diags = run_l3(&L3Eid01, &file, Some(&source));
        assert_eq!(
            diags.len(),
            2,
            "both lapsed LEIs should produce diagnostics"
        );
        assert!(diags.iter().all(|d| d.severity == Severity::Info));
    }

    #[test]
    fn mrg01_no_external_source_produces_no_diagnostics() {
        let file = make_file(
            vec![org_node("org-1"), org_node("org-2")],
            vec![ownership_edge("e-1", "org-1", "org-2", Some(60.0))],
        );
        let diags = run_l3(&L3Mrg01, &file, None);
        assert!(
            diags.is_empty(),
            "L3-MRG-01 must be silent when external_data is None"
        );
    }

    #[test]
    fn mrg01_ownership_sum_within_100_produces_no_diagnostic() {
        let source = MockDataSource::new();
        let file = make_file(
            vec![org_node("org-1"), org_node("org-2"), org_node("org-3")],
            vec![
                ownership_edge("e-1", "org-1", "org-3", Some(40.0)),
                ownership_edge("e-2", "org-2", "org-3", Some(60.0)),
            ],
        );
        let diags = run_l3(&L3Mrg01, &file, Some(&source));
        assert!(
            diags.is_empty(),
            "sum = 100.0 should not produce a diagnostic; got: {diags:?}"
        );
    }

    #[test]
    fn mrg01_ownership_sum_exceeds_100_produces_info_diagnostic() {
        let source = MockDataSource::new();
        let file = make_file(
            vec![org_node("org-1"), org_node("org-2"), org_node("org-3")],
            vec![
                ownership_edge("e-1", "org-1", "org-3", Some(70.0)),
                ownership_edge("e-2", "org-2", "org-3", Some(50.0)),
            ],
        );
        let diags = run_l3(&L3Mrg01, &file, Some(&source));
        assert_eq!(
            diags.len(),
            1,
            "sum = 120.0 should produce one Info diagnostic; got: {diags:?}"
        );
        assert_eq!(diags[0].rule_id, RuleId::L3Mrg01);
        assert_eq!(diags[0].severity, Severity::Info);
        assert!(diags[0].message.contains("org-3"));
        assert!(diags[0].message.contains("120.00%"));
    }

    #[test]
    fn mrg01_no_percentage_on_edges_produces_no_diagnostic() {
        let source = MockDataSource::new();
        let file = make_file(
            vec![org_node("org-1"), org_node("org-2")],
            vec![ownership_edge("e-1", "org-1", "org-2", None)],
        );
        let diags = run_l3(&L3Mrg01, &file, Some(&source));
        assert!(
            diags.is_empty(),
            "edges without percentage must not trigger MRG-01"
        );
    }

    #[test]
    fn mrg01_empty_file_produces_no_diagnostic() {
        let source = MockDataSource::new();
        let file = make_file(vec![], vec![]);
        let diags = run_l3(&L3Mrg01, &file, Some(&source));
        assert!(diags.is_empty());
    }

    #[test]
    fn mock_data_source_returns_configured_lei_records() {
        let source = MockDataSource::new()
            .with_lei("5493006MHB84DD0ZWV18", "ISSUED", true)
            .with_lei("254900OPPU84GM83MG36", "LAPSED", false);

        let active = source.lei_status("5493006MHB84DD0ZWV18");
        assert!(active.is_some());
        assert!(active.as_ref().map(|r| r.is_active).unwrap_or(false));

        let lapsed = source.lei_status("254900OPPU84GM83MG36");
        assert!(lapsed.is_some());
        assert!(!lapsed.as_ref().map(|r| r.is_active).unwrap_or(true));

        let unknown = source.lei_status("UNKNOWNLEI00000000000");
        assert!(unknown.is_none());
    }

    #[test]
    fn mock_data_source_returns_configured_nat_reg_records() {
        let source = MockDataSource::new().with_nat_reg("RA000548", "HRB86891", true);

        let found = source.nat_reg_lookup("RA000548", "HRB86891");
        assert!(found.is_some());
        assert!(found.as_ref().map(|r| r.is_active).unwrap_or(false));

        let not_found = source.nat_reg_lookup("RA000548", "UNKNOWN");
        assert!(not_found.is_none());
    }

    fn legal_parentage_edge(id: &str, source: &str, target: &str) -> Edge {
        Edge {
            id: EdgeId::try_from(id).expect("valid id"),
            edge_type: EdgeTypeTag::Known(EdgeType::LegalParentage),
            source: NodeId::try_from(source).expect("valid source"),
            target: NodeId::try_from(target).expect("valid target"),
            identifiers: None,
            properties: EdgeProperties::default(),
            extra: BTreeMap::new(),
        }
    }

    #[test]
    fn mrg02_acyclic_legal_parentage_produces_no_diagnostic() {
        // a → b → c (linear chain, no cycles)
        let file = make_file(
            vec![org_node("a"), org_node("b"), org_node("c")],
            vec![
                legal_parentage_edge("e-ab", "a", "b"),
                legal_parentage_edge("e-bc", "b", "c"),
            ],
        );
        let diags = run_l3(&L3Mrg02, &file, None);
        assert!(
            diags.is_empty(),
            "acyclic legal_parentage graph must produce no diagnostic; got: {diags:?}"
        );
    }

    #[test]
    fn mrg02_empty_file_produces_no_diagnostic() {
        let file = make_file(vec![], vec![]);
        let diags = run_l3(&L3Mrg02, &file, None);
        assert!(diags.is_empty());
    }

    #[test]
    fn mrg02_two_node_cycle_produces_info_diagnostic() {
        // a → b → a
        let file = make_file(
            vec![org_node("a"), org_node("b")],
            vec![
                legal_parentage_edge("e-ab", "a", "b"),
                legal_parentage_edge("e-ba", "b", "a"),
            ],
        );
        let diags = run_l3(&L3Mrg02, &file, None);
        assert!(
            !diags.is_empty(),
            "two-node cycle must produce at least one Info diagnostic"
        );
        assert_eq!(diags[0].rule_id, RuleId::L3Mrg02);
        assert_eq!(diags[0].severity, Severity::Info);
        assert_eq!(diags[0].location, Location::Global);
        assert!(
            diags[0].message.contains("legal_parentage cycle"),
            "message should mention cycle: {}",
            diags[0].message
        );
    }

    #[test]
    fn mrg02_three_node_cycle_produces_info_diagnostic() {
        // a → b → c → a
        let file = make_file(
            vec![org_node("a"), org_node("b"), org_node("c")],
            vec![
                legal_parentage_edge("e-ab", "a", "b"),
                legal_parentage_edge("e-bc", "b", "c"),
                legal_parentage_edge("e-ca", "c", "a"),
            ],
        );
        let diags = run_l3(&L3Mrg02, &file, None);
        assert!(
            !diags.is_empty(),
            "three-node cycle must produce at least one Info diagnostic"
        );
        assert!(diags.iter().all(|d| d.rule_id == RuleId::L3Mrg02));
        assert!(diags.iter().all(|d| d.severity == Severity::Info));
    }

    #[test]
    fn mrg02_ownership_cycle_does_not_trigger_rule() {
        // Ownership cycle is permitted; L3-MRG-02 only checks legal_parentage.
        let file = make_file(
            vec![org_node("a"), org_node("b")],
            vec![
                ownership_edge("e-ab", "a", "b", None),
                ownership_edge("e-ba", "b", "a", None),
            ],
        );
        let diags = run_l3(&L3Mrg02, &file, None);
        assert!(
            diags.is_empty(),
            "ownership cycles must not trigger MRG-02; got: {diags:?}"
        );
    }

    #[test]
    fn mrg02_external_data_not_required() {
        // MRG-02 should run even when external_data is None (it doesn't use it).
        let file = make_file(
            vec![org_node("a"), org_node("b")],
            vec![
                legal_parentage_edge("e-ab", "a", "b"),
                legal_parentage_edge("e-ba", "b", "a"),
            ],
        );
        let diags_none = run_l3(&L3Mrg02, &file, None);
        assert!(
            !diags_none.is_empty(),
            "MRG-02 should detect cycles regardless of external_data"
        );
    }

    #[test]
    fn mrg02_cycle_message_contains_participant_ids() {
        // a → b → a; the cycle message should name both nodes.
        let file = make_file(
            vec![org_node("subsidiary-x"), org_node("parent-y")],
            vec![
                legal_parentage_edge("e-1", "subsidiary-x", "parent-y"),
                legal_parentage_edge("e-2", "parent-y", "subsidiary-x"),
            ],
        );
        let diags = run_l3(&L3Mrg02, &file, None);
        assert!(!diags.is_empty());
        let msg = &diags[0].message;
        assert!(
            msg.contains("subsidiary-x") || msg.contains("parent-y"),
            "cycle message should contain at least one participant ID: {msg}"
        );
    }

    #[test]
    fn mrg02_tree_structure_produces_no_diagnostic() {
        // a → b, a → c, b → d: a valid corporate tree (no cycle)
        let file = make_file(
            vec![org_node("a"), org_node("b"), org_node("c"), org_node("d")],
            vec![
                legal_parentage_edge("e-ab", "a", "b"),
                legal_parentage_edge("e-ac", "a", "c"),
                legal_parentage_edge("e-bd", "b", "d"),
            ],
        );
        let diags = run_l3(&L3Mrg02, &file, None);
        assert!(
            diags.is_empty(),
            "tree-shaped legal_parentage graph must produce no diagnostic; got: {diags:?}"
        );
    }
}
