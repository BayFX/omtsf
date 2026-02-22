#![allow(clippy::expect_used)]

use super::super::*;
use crate::enums::{EdgeType, EdgeTypeTag, NodeType, NodeTypeTag};
use crate::file::OmtsFile;
use crate::graph::build_graph;
use crate::graph::selectors::{Selector, SelectorSet};
use crate::newtypes::CountryCode;
use crate::structures::Node;
use crate::types::{Identifier, Label};
use std::collections::BTreeMap;

use super::{file_salt, minimal_file, node_id, org_node, ownership_edge, semver, supplies_edge};

const SALT: &str = "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef";

fn country_code(s: &str) -> CountryCode {
    CountryCode::try_from(s).expect("valid CountryCode")
}

fn label(key: &str, value: Option<&str>) -> Label {
    Label {
        key: key.to_owned(),
        value: value.map(str::to_owned),
        extra: BTreeMap::new(),
    }
}

fn identifier(scheme: &str, value: &str) -> Identifier {
    Identifier {
        scheme: scheme.to_owned(),
        value: value.to_owned(),
        authority: None,
        valid_from: None,
        valid_to: None,
        sensitivity: None,
        verification_status: None,
        verification_date: None,
        extra: BTreeMap::new(),
    }
}

fn facility_node(id: &str) -> Node {
    Node {
        id: node_id(id),
        node_type: NodeTypeTag::Known(NodeType::Facility),
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
        extra: BTreeMap::new(),
    }
}

/// `selector_match` with `NodeType` selector returns matching node indices.
#[test]
fn test_selector_match_node_type_returns_correct_indices() {
    let nodes = vec![org_node("org-1"), facility_node("fac-1"), org_node("org-2")];
    let file = minimal_file(nodes, vec![]);

    let ss = SelectorSet::from_selectors(vec![Selector::NodeType(NodeTypeTag::Known(
        NodeType::Organization,
    ))]);
    let result = selector_match(&file, &ss);

    assert_eq!(result.node_indices, vec![0, 2], "indices 0 and 2 are orgs");
    assert!(result.edge_indices.is_empty());
}

/// `selector_match` with `EdgeType` selector returns matching edge indices.
#[test]
fn test_selector_match_edge_type_returns_correct_indices() {
    let nodes = vec![org_node("a"), org_node("b"), org_node("c")];
    let edges = vec![
        supplies_edge("e-ab", "a", "b"),
        ownership_edge("e-bc", "b", "c"),
        supplies_edge("e-ac", "a", "c"),
    ];
    let file = minimal_file(nodes, edges);

    let ss = SelectorSet::from_selectors(vec![Selector::EdgeType(EdgeTypeTag::Known(
        EdgeType::Supplies,
    ))]);
    let result = selector_match(&file, &ss);

    assert!(result.node_indices.is_empty());
    assert_eq!(
        result.edge_indices,
        vec![0, 2],
        "indices 0 and 2 are supplies edges"
    );
}

/// `selector_match` with `LabelKey` selector matches nodes with that label.
#[test]
fn test_selector_match_label_key_matches_labeled_nodes() {
    let mut n1 = org_node("n1");
    n1.labels = Some(vec![label("certified", None)]);
    let n2 = org_node("n2");
    let mut n3 = facility_node("n3");
    n3.labels = Some(vec![label("certified", None)]);

    let file = minimal_file(vec![n1, n2, n3], vec![]);
    let ss = SelectorSet::from_selectors(vec![Selector::LabelKey("certified".to_owned())]);
    let result = selector_match(&file, &ss);

    assert_eq!(result.node_indices, vec![0, 2]);
}

/// `selector_match` with `LabelKeyValue` selector matches nodes with exact pair.
#[test]
fn test_selector_match_label_key_value_exact_match() {
    let mut n1 = org_node("n1");
    n1.labels = Some(vec![label("tier", Some("1"))]);
    let mut n2 = org_node("n2");
    n2.labels = Some(vec![label("tier", Some("2"))]);
    let mut n3 = org_node("n3");
    n3.labels = Some(vec![label("tier", Some("1"))]);

    let file = minimal_file(vec![n1, n2, n3], vec![]);
    let ss = SelectorSet::from_selectors(vec![Selector::LabelKeyValue(
        "tier".to_owned(),
        "1".to_owned(),
    )]);
    let result = selector_match(&file, &ss);

    assert_eq!(result.node_indices, vec![0, 2]);
}

/// `selector_match` with `IdentifierScheme` selector matches nodes with that scheme.
#[test]
fn test_selector_match_identifier_scheme_matches_nodes() {
    let mut n1 = org_node("n1");
    n1.identifiers = Some(vec![identifier("lei", "529900T8BM49AURSDO55")]);
    let n2 = org_node("n2");
    let mut n3 = org_node("n3");
    n3.identifiers = Some(vec![identifier("duns", "123456789")]);

    let file = minimal_file(vec![n1, n2, n3], vec![]);
    let ss = SelectorSet::from_selectors(vec![Selector::IdentifierScheme("lei".to_owned())]);
    let result = selector_match(&file, &ss);

    assert_eq!(result.node_indices, vec![0]);
}

/// `selector_match` with `IdentifierSchemeValue` selector matches exact (scheme, value).
#[test]
fn test_selector_match_identifier_scheme_value_exact() {
    let mut n1 = org_node("n1");
    n1.identifiers = Some(vec![identifier("duns", "111111111")]);
    let mut n2 = org_node("n2");
    n2.identifiers = Some(vec![identifier("duns", "222222222")]);

    let file = minimal_file(vec![n1, n2], vec![]);
    let ss = SelectorSet::from_selectors(vec![Selector::IdentifierSchemeValue(
        "duns".to_owned(),
        "111111111".to_owned(),
    )]);
    let result = selector_match(&file, &ss);

    assert_eq!(result.node_indices, vec![0]);
}

/// `selector_match` with `Jurisdiction` selector matches nodes by country code.
#[test]
fn test_selector_match_jurisdiction_matches_correct_nodes() {
    let mut n1 = org_node("n1");
    n1.jurisdiction = Some(country_code("DE"));
    let mut n2 = org_node("n2");
    n2.jurisdiction = Some(country_code("US"));
    let mut n3 = org_node("n3");
    n3.jurisdiction = Some(country_code("DE"));

    let file = minimal_file(vec![n1, n2, n3], vec![]);
    let ss = SelectorSet::from_selectors(vec![Selector::Jurisdiction(country_code("DE"))]);
    let result = selector_match(&file, &ss);

    assert_eq!(result.node_indices, vec![0, 2]);
}

/// `selector_match` with `Name` selector uses case-insensitive substring match.
#[test]
fn test_selector_match_name_case_insensitive_substring() {
    let mut n1 = org_node("n1");
    n1.name = Some("Acme Corp".to_owned());
    let mut n2 = org_node("n2");
    n2.name = Some("Global Logistics".to_owned());
    let mut n3 = org_node("n3");
    n3.name = Some("ACME GmbH".to_owned());

    let file = minimal_file(vec![n1, n2, n3], vec![]);
    let ss = SelectorSet::from_selectors(vec![Selector::Name("acme".to_owned())]);
    let result = selector_match(&file, &ss);

    assert_eq!(result.node_indices, vec![0, 2]);
}

/// `selector_match` returns empty result when nothing matches.
#[test]
fn test_selector_match_no_matches_returns_empty() {
    let file = minimal_file(vec![org_node("n1"), org_node("n2")], vec![]);
    let ss = SelectorSet::from_selectors(vec![Selector::NodeType(NodeTypeTag::Known(
        NodeType::Facility,
    ))]);
    let result = selector_match(&file, &ss);

    assert!(result.node_indices.is_empty());
    assert!(result.edge_indices.is_empty());
}

/// `selector_match` with empty selector set returns all nodes and all edges.
#[test]
fn test_selector_match_empty_selector_set_matches_everything() {
    let nodes = vec![org_node("n1"), org_node("n2")];
    let edges = vec![supplies_edge("e1", "n1", "n2")];
    let file = minimal_file(nodes, edges);

    let ss = SelectorSet::default();
    let result = selector_match(&file, &ss);

    assert_eq!(result.node_indices, vec![0, 1]);
    assert_eq!(result.edge_indices, vec![0]);
}

/// `selector_subgraph` with expand=0 returns seed nodes and their incident edges.
#[test]
fn test_selector_subgraph_expand_0_returns_seed_with_incident_edges() {
    // Graph: a(org) → b(facility) → c(org)
    // Select organizations with expand=0.
    // Seeds: {a, c}; their incident edges include e-ab and e-bc.
    let nodes = vec![org_node("a"), facility_node("b"), org_node("c")];
    let edges = vec![
        supplies_edge("e-ab", "a", "b"),
        supplies_edge("e-bc", "b", "c"),
    ];
    let file = minimal_file(nodes, edges);
    let graph = build_graph(&file).expect("builds");

    let ss = SelectorSet::from_selectors(vec![Selector::NodeType(NodeTypeTag::Known(
        NodeType::Organization,
    ))]);
    let sub = selector_subgraph(&graph, &file, &ss, 0).expect("should succeed");

    // Seeds are {a, c}. With expand=0, BFS does not expand beyond seeds.
    // assemble_subgraph computes the induced subgraph of {a, c},
    // which includes the edge e-ab only if b is in the set (it isn't).
    // So we get nodes {a, c} and no edges (e-ab has b not in set, e-bc has b not in set).
    let node_ids: Vec<String> = sub.nodes.iter().map(|n| n.id.to_string()).collect();
    assert!(node_ids.contains(&"a".to_owned()), "a must be included");
    assert!(node_ids.contains(&"c".to_owned()), "c must be included");
    assert!(
        !node_ids.contains(&"b".to_owned()),
        "b is not a seed and expand=0"
    );
    // No edge has both endpoints in {a, c}
    assert_eq!(sub.edges.len(), 0);
}

/// `selector_subgraph` with expand=1 includes one-hop neighbors.
#[test]
fn test_selector_subgraph_expand_1_includes_one_hop_neighbors() {
    // Graph: a(org) → b(facility) → c(org)
    // Select facilities with expand=1: seed={b}, expand gives {a, c}.
    let nodes = vec![org_node("a"), facility_node("b"), org_node("c")];
    let edges = vec![
        supplies_edge("e-ab", "a", "b"),
        supplies_edge("e-bc", "b", "c"),
    ];
    let file = minimal_file(nodes, edges);
    let graph = build_graph(&file).expect("builds");

    let ss = SelectorSet::from_selectors(vec![Selector::NodeType(NodeTypeTag::Known(
        NodeType::Facility,
    ))]);
    let sub = selector_subgraph(&graph, &file, &ss, 1).expect("should succeed");

    let node_ids: Vec<String> = sub.nodes.iter().map(|n| n.id.to_string()).collect();
    assert!(node_ids.contains(&"a".to_owned()), "a is 1 hop from b");
    assert!(node_ids.contains(&"b".to_owned()), "b is the seed");
    assert!(node_ids.contains(&"c".to_owned()), "c is 1 hop from b");
    // All edges are within the full node set.
    assert_eq!(sub.edges.len(), 2);
}

/// `selector_subgraph` with expand=3 expands multiple hops.
#[test]
fn test_selector_subgraph_expand_3_captures_multi_hop_neighbors() {
    // Chain: org-a → fac-b → org-c → fac-d → org-e → fac-f
    // Select organizations with expand=3: seeds={org-a, org-c, org-e}.
    // BFS from seeds: 1 hop → {fac-b, fac-d, fac-f}
    //                 2 hops → (nothing new in this chain from those)
    //                 3 hops → (nothing new)
    // Actually in a chain, 1 hop from org-a reaches fac-b, 1 hop from org-c
    // reaches fac-b and fac-d, 1 hop from org-e reaches fac-d and fac-f.
    // So expand=1 already covers everything in this 6-node chain.
    let nodes = vec![
        org_node("a"),
        facility_node("b"),
        org_node("c"),
        facility_node("d"),
        org_node("e"),
        facility_node("f"),
    ];
    let edges = vec![
        supplies_edge("e-ab", "a", "b"),
        supplies_edge("e-bc", "b", "c"),
        supplies_edge("e-cd", "c", "d"),
        supplies_edge("e-de", "d", "e"),
        supplies_edge("e-ef", "e", "f"),
    ];
    let file = minimal_file(nodes, edges);
    let graph = build_graph(&file).expect("builds");

    let ss = SelectorSet::from_selectors(vec![Selector::NodeType(NodeTypeTag::Known(
        NodeType::Organization,
    ))]);
    let sub = selector_subgraph(&graph, &file, &ss, 3).expect("should succeed");

    // All 6 nodes are reachable within 3 hops from at least one org seed.
    assert_eq!(sub.nodes.len(), 6, "all nodes included within 3 hops");
    assert_eq!(sub.edges.len(), 5, "all edges included");
}

/// `selector_subgraph` via seed edge: matched edge includes its endpoints.
#[test]
fn test_selector_subgraph_seed_edge_contributes_endpoints() {
    // Graph: a → b → c
    // Select edges of type Ownership (only e-bc is ownership here).
    // Seed edge = e-bc; endpoints b, c are added to seed_nodes.
    // With expand=0, result = induced subgraph of {b, c} = {b, c, e-bc}.
    let nodes = vec![org_node("a"), org_node("b"), org_node("c")];
    let edges = vec![
        supplies_edge("e-ab", "a", "b"),
        ownership_edge("e-bc", "b", "c"),
    ];
    let file = minimal_file(nodes, edges);
    let graph = build_graph(&file).expect("builds");

    let ss = SelectorSet::from_selectors(vec![Selector::EdgeType(EdgeTypeTag::Known(
        EdgeType::Ownership,
    ))]);
    let sub = selector_subgraph(&graph, &file, &ss, 0).expect("should succeed");

    let node_ids: Vec<String> = sub.nodes.iter().map(|n| n.id.to_string()).collect();
    assert!(
        node_ids.contains(&"b".to_owned()),
        "b is endpoint of seed edge"
    );
    assert!(
        node_ids.contains(&"c".to_owned()),
        "c is endpoint of seed edge"
    );
    assert!(
        !node_ids.contains(&"a".to_owned()),
        "a is not reachable with expand=0"
    );

    let edge_ids: Vec<String> = sub.edges.iter().map(|e| e.id.to_string()).collect();
    assert!(
        edge_ids.contains(&"e-bc".to_owned()),
        "seed edge must be included"
    );
    assert!(
        !edge_ids.contains(&"e-ab".to_owned()),
        "e-ab not in induced subgraph of {{b,c}}"
    );
}

/// `selector_subgraph` returns `EmptyResult` when no nodes or edges match.
#[test]
fn test_selector_subgraph_empty_result_error() {
    let nodes = vec![org_node("n1"), org_node("n2")];
    let edges = vec![supplies_edge("e1", "n1", "n2")];
    let file = minimal_file(nodes, edges);
    let graph = build_graph(&file).expect("builds");

    // Select facilities — there are none.
    let ss = SelectorSet::from_selectors(vec![Selector::NodeType(NodeTypeTag::Known(
        NodeType::Facility,
    ))]);
    let err = selector_subgraph(&graph, &file, &ss, 1).expect_err("should fail: no matches");
    assert_eq!(err, QueryError::EmptyResult);
}

/// `selector_subgraph` with OR composition on node types.
#[test]
fn test_selector_subgraph_or_within_group_node_types() {
    // Nodes: 2 orgs, 1 facility, 1 attestation.
    // Select org OR facility with expand=0.
    let nodes = vec![
        org_node("org-1"),
        org_node("org-2"),
        facility_node("fac-1"),
        {
            let mut n = org_node("attest-1");
            n.node_type = NodeTypeTag::Known(crate::enums::NodeType::Attestation);
            n
        },
    ];
    let file = minimal_file(nodes, vec![]);
    let graph = build_graph(&file).expect("builds");

    let ss = SelectorSet::from_selectors(vec![
        Selector::NodeType(NodeTypeTag::Known(NodeType::Organization)),
        Selector::NodeType(NodeTypeTag::Known(NodeType::Facility)),
    ]);
    let sub = selector_subgraph(&graph, &file, &ss, 0).expect("should succeed");

    let node_ids: Vec<String> = sub.nodes.iter().map(|n| n.id.to_string()).collect();
    assert!(node_ids.contains(&"org-1".to_owned()));
    assert!(node_ids.contains(&"org-2".to_owned()));
    assert!(node_ids.contains(&"fac-1".to_owned()));
    assert!(
        !node_ids.contains(&"attest-1".to_owned()),
        "attestation not selected"
    );
}

/// `selector_subgraph` with AND composition across groups.
#[test]
fn test_selector_subgraph_and_across_groups() {
    // org-de: org in DE; org-us: org in US; fac-de: facility in DE.
    // Select org AND DE → only org-de.
    let mut org_de = org_node("org-de");
    org_de.jurisdiction = Some(country_code("DE"));
    let mut org_us = org_node("org-us");
    org_us.jurisdiction = Some(country_code("US"));
    let mut fac_de = facility_node("fac-de");
    fac_de.jurisdiction = Some(country_code("DE"));

    let file = minimal_file(vec![org_de, org_us, fac_de], vec![]);
    let graph = build_graph(&file).expect("builds");

    let ss = SelectorSet::from_selectors(vec![
        Selector::NodeType(NodeTypeTag::Known(NodeType::Organization)),
        Selector::Jurisdiction(country_code("DE")),
    ]);
    let sub = selector_subgraph(&graph, &file, &ss, 0).expect("should succeed");

    let node_ids: Vec<String> = sub.nodes.iter().map(|n| n.id.to_string()).collect();
    assert!(node_ids.contains(&"org-de".to_owned()), "org-de must match");
    assert!(
        !node_ids.contains(&"org-us".to_owned()),
        "org-us wrong jurisdiction"
    );
    assert!(
        !node_ids.contains(&"fac-de".to_owned()),
        "fac-de is not an org"
    );
}

/// `selector_subgraph` handles cyclic graphs correctly (BFS terminates).
#[test]
fn test_selector_subgraph_cyclic_graph_terminates() {
    // Graph: a → b → c → a (cycle); select 'a' only.
    let nodes = vec![org_node("a"), facility_node("b"), org_node("c")];
    let edges = vec![
        supplies_edge("e-ab", "a", "b"),
        supplies_edge("e-bc", "b", "c"),
        supplies_edge("e-ca", "c", "a"),
    ];
    let file = minimal_file(nodes, edges);
    let graph = build_graph(&file).expect("builds");

    let ss = SelectorSet::from_selectors(vec![Selector::NodeType(NodeTypeTag::Known(
        NodeType::Organization,
    ))]);
    // Seeds: {a, c}. With expand=1, reaches b too.
    let sub = selector_subgraph(&graph, &file, &ss, 1).expect("must terminate");
    // All three nodes within 1 hop of the seeds.
    assert_eq!(sub.nodes.len(), 3, "all 3 nodes reachable");
}

/// `selector_subgraph` preserves header fields in the output.
#[test]
fn test_selector_subgraph_preserves_header_fields() {
    let nodes = vec![org_node("n1"), org_node("n2")];
    let file = OmtsFile {
        omtsf_version: semver("1.2.0"),
        snapshot_date: super::date("2025-06-01"),
        file_salt: file_salt(SALT),
        disclosure_scope: None,
        previous_snapshot_ref: Some("sha256:abc".to_owned()),
        snapshot_sequence: Some(5),
        reporting_entity: None,
        nodes,
        edges: vec![],
        extra: BTreeMap::new(),
    };
    let graph = build_graph(&file).expect("builds");

    let ss = SelectorSet::from_selectors(vec![Selector::NodeType(NodeTypeTag::Known(
        NodeType::Organization,
    ))]);
    let sub = selector_subgraph(&graph, &file, &ss, 0).expect("should succeed");

    assert_eq!(sub.omtsf_version, semver("1.2.0"));
    assert_eq!(sub.snapshot_date, super::date("2025-06-01"));
    assert_eq!(sub.previous_snapshot_ref.as_deref(), Some("sha256:abc"));
    assert_eq!(sub.snapshot_sequence, Some(5));
}

/// `QueryError::EmptyResult` has correct Display output.
#[test]
fn test_query_error_empty_result_display() {
    let err = QueryError::EmptyResult;
    let msg = err.to_string();
    assert!(!msg.is_empty());
    // Should mention something about selectors or matching.
    assert!(msg.contains("selector") || msg.contains("match") || msg.contains("element"));
}
