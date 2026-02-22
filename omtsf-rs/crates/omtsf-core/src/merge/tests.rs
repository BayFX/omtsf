#![allow(clippy::expect_used)]
#![allow(clippy::panic)]

use crate::dynvalue::DynValue;
use serde_json::json;
use std::collections::BTreeMap;

use super::*;
use crate::enums::{EdgeType, EdgeTypeTag};
use crate::newtypes::NodeId;
use crate::structures::{Edge, EdgeProperties};
use crate::types::{Identifier, Label};

fn make_identifier(scheme: &str, value: &str) -> Identifier {
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

fn make_label(key: &str, value: Option<&str>) -> Label {
    Label {
        key: key.to_owned(),
        value: value.map(str::to_owned),
        extra: BTreeMap::new(),
    }
}

fn make_same_as_edge(id: &str, src: &str, tgt: &str, confidence: Option<&str>) -> Edge {
    let mut props = EdgeProperties::default();
    if let Some(c) = confidence {
        props
            .extra
            .insert("confidence".to_owned(), DynValue::from(json!(c)));
    }
    Edge {
        id: NodeId::try_from(id).expect("valid edge id"),
        edge_type: EdgeTypeTag::Known(EdgeType::SameAs),
        source: NodeId::try_from(src).expect("valid node id"),
        target: NodeId::try_from(tgt).expect("valid node id"),
        identifiers: None,
        properties: props,
        extra: BTreeMap::new(),
    }
}

fn make_supplies_edge(id: &str, src: &str, tgt: &str) -> Edge {
    Edge {
        id: NodeId::try_from(id).expect("valid edge id"),
        edge_type: EdgeTypeTag::Known(EdgeType::Supplies),
        source: NodeId::try_from(src).expect("valid node id"),
        target: NodeId::try_from(tgt).expect("valid node id"),
        identifiers: None,
        properties: EdgeProperties::default(),
        extra: BTreeMap::new(),
    }
}

#[test]
fn threshold_definite_honours_definite_only() {
    let t = SameAsThreshold::Definite;
    assert!(t.honours(Some("definite")));
    assert!(!t.honours(Some("probable")));
    assert!(!t.honours(Some("possible")));
    assert!(!t.honours(None));
}

#[test]
fn threshold_probable_honours_definite_and_probable() {
    let t = SameAsThreshold::Probable;
    assert!(t.honours(Some("definite")));
    assert!(t.honours(Some("probable")));
    assert!(!t.honours(Some("possible")));
    assert!(!t.honours(None));
}

#[test]
fn threshold_possible_honours_all() {
    let t = SameAsThreshold::Possible;
    assert!(t.honours(Some("definite")));
    assert!(t.honours(Some("probable")));
    assert!(t.honours(Some("possible")));
    assert!(t.honours(None)); // absent treated as "possible"
}

#[test]
fn threshold_unrecognised_string_treated_as_possible() {
    let t = SameAsThreshold::Possible;
    assert!(t.honours(Some("unknown_level")));
    let t2 = SameAsThreshold::Definite;
    assert!(!t2.honours(Some("unknown_level")));
}

#[test]
fn threshold_default_is_definite() {
    assert_eq!(SameAsThreshold::default(), SameAsThreshold::Definite);
}

#[test]
fn scalars_both_none_agrees_on_none() {
    let inputs: Vec<(Option<String>, &str)> = vec![(None, "file_a.json"), (None, "file_b.json")];
    let result = merge_scalars(&inputs);
    assert_eq!(result, ScalarMergeResult::Agreed(None));
}

#[test]
fn scalars_one_none_one_some_agrees_on_some() {
    let inputs: Vec<(Option<String>, &str)> = vec![
        (None, "file_a.json"),
        (Some("Acme Corp".to_owned()), "file_b.json"),
    ];
    let result = merge_scalars(&inputs);
    assert_eq!(
        result,
        ScalarMergeResult::Agreed(Some("Acme Corp".to_owned()))
    );
}

#[test]
fn scalars_identical_values_agree() {
    let inputs: Vec<(Option<String>, &str)> = vec![
        (Some("Acme Corp".to_owned()), "file_a.json"),
        (Some("Acme Corp".to_owned()), "file_b.json"),
    ];
    let result = merge_scalars(&inputs);
    assert_eq!(
        result,
        ScalarMergeResult::Agreed(Some("Acme Corp".to_owned()))
    );
}

#[test]
fn scalars_three_identical_values_agree() {
    let inputs: Vec<(Option<u64>, &str)> = vec![
        (Some(42), "a.json"),
        (Some(42), "b.json"),
        (Some(42), "c.json"),
    ];
    let result = merge_scalars(&inputs);
    assert_eq!(result, ScalarMergeResult::Agreed(Some(42u64)));
}

#[test]
fn scalars_different_values_conflict() {
    let inputs: Vec<(Option<String>, &str)> = vec![
        (Some("Acme Corp".to_owned()), "file_a.json"),
        (Some("ACME Corporation".to_owned()), "file_b.json"),
    ];
    let result = merge_scalars(&inputs);
    match result {
        ScalarMergeResult::Conflict(entries) => {
            assert_eq!(entries.len(), 2);
            // Sorted by source_file
            assert_eq!(entries[0].source_file, "file_a.json");
            assert_eq!(entries[0].value, json!("Acme Corp"));
            assert_eq!(entries[1].source_file, "file_b.json");
            assert_eq!(entries[1].value, json!("ACME Corporation"));
        }
        ScalarMergeResult::Agreed(_) => panic!("expected Conflict"),
    }
}

#[test]
fn scalars_conflict_sorted_by_source_file() {
    // Source files arrive out of order; output must be sorted.
    let inputs: Vec<(Option<String>, &str)> = vec![
        (Some("Z".to_owned()), "z_file.json"),
        (Some("A".to_owned()), "a_file.json"),
    ];
    let result = merge_scalars(&inputs);
    match result {
        ScalarMergeResult::Conflict(entries) => {
            assert_eq!(entries[0].source_file, "a_file.json");
            assert_eq!(entries[1].source_file, "z_file.json");
        }
        ScalarMergeResult::Agreed(_) => panic!("expected Conflict"),
    }
}

#[test]
fn scalars_conflict_deduplicates_same_source_same_value() {
    let inputs: Vec<(Option<String>, &str)> = vec![
        (Some("X".to_owned()), "file_a.json"),
        (Some("X".to_owned()), "file_a.json"), // duplicate — should be merged with above
        (Some("Y".to_owned()), "file_b.json"),
    ];
    let result = merge_scalars(&inputs);
    match result {
        ScalarMergeResult::Conflict(entries) => {
            // "X" from file_a appears once despite two inputs
            let file_a_entries: Vec<_> = entries
                .iter()
                .filter(|e| e.source_file == "file_a.json")
                .collect();
            assert_eq!(file_a_entries.len(), 1);
            assert_eq!(file_a_entries[0].value, json!("X"));
        }
        ScalarMergeResult::Agreed(_) => panic!("expected Conflict"),
    }
}

#[test]
fn scalars_numeric_conflict() {
    let inputs: Vec<(Option<f64>, &str)> =
        vec![(Some(51.0_f64), "a.json"), (Some(49.0_f64), "b.json")];
    let result = merge_scalars(&inputs);
    assert!(matches!(result, ScalarMergeResult::Conflict(_)));
}

#[test]
fn identifiers_empty_inputs_produces_empty() {
    let result = merge_identifiers(&[]);
    assert!(result.is_empty());
}

#[test]
fn identifiers_all_none_produces_empty() {
    let result = merge_identifiers(&[None, None]);
    assert!(result.is_empty());
}

#[test]
fn identifiers_single_source_passthrough() {
    let ids = vec![
        make_identifier("lei", "LEI_A"),
        make_identifier("duns", "DUNS_A"),
    ];
    let result = merge_identifiers(&[Some(ids.as_slice())]);
    assert_eq!(result.len(), 2);
}

#[test]
fn identifiers_dedup_by_canonical_string() {
    let ids_a = vec![make_identifier("lei", "SAME_LEI")];
    let ids_b = vec![make_identifier("lei", "SAME_LEI")];
    let result = merge_identifiers(&[Some(ids_a.as_slice()), Some(ids_b.as_slice())]);
    assert_eq!(result.len(), 1, "duplicate canonical id should be deduped");
    assert_eq!(result[0].scheme, "lei");
    assert_eq!(result[0].value, "SAME_LEI");
}

#[test]
fn identifiers_union_non_overlapping() {
    let ids_a = vec![make_identifier("lei", "LEI_A")];
    let ids_b = vec![make_identifier("duns", "DUNS_B")];
    let result = merge_identifiers(&[Some(ids_a.as_slice()), Some(ids_b.as_slice())]);
    assert_eq!(result.len(), 2);
}

#[test]
fn identifiers_sorted_by_canonical_string() {
    // "lei:Z" > "duns:A" lexicographically; output must sort them
    let ids_a = vec![make_identifier("lei", "Z")];
    let ids_b = vec![make_identifier("duns", "A")];
    let result = merge_identifiers(&[Some(ids_a.as_slice()), Some(ids_b.as_slice())]);
    assert_eq!(result.len(), 2);
    // "duns:A" < "lei:Z" lexicographically
    assert_eq!(result[0].scheme, "duns");
    assert_eq!(result[1].scheme, "lei");
}

#[test]
fn identifiers_multiple_sources_union_deduplicated_sorted() {
    // Three sources: overlapping identifiers from different sources.
    let ids_a = vec![
        make_identifier("lei", "LEI_1"),
        make_identifier("duns", "DUNS_1"),
    ];
    let ids_b = vec![
        make_identifier("lei", "LEI_1"), // duplicate
        make_identifier("gln", "GLN_1"),
    ];
    let ids_c = vec![make_identifier("duns", "DUNS_1")]; // duplicate

    let result = merge_identifiers(&[
        Some(ids_a.as_slice()),
        Some(ids_b.as_slice()),
        Some(ids_c.as_slice()),
    ]);

    // Should have: duns:DUNS_1, gln:GLN_1, lei:LEI_1
    assert_eq!(result.len(), 3);
    assert_eq!(result[0].scheme, "duns");
    assert_eq!(result[1].scheme, "gln");
    assert_eq!(result[2].scheme, "lei");
}

#[test]
fn labels_empty_inputs_produces_empty() {
    let result = merge_labels(&[]);
    assert!(result.is_empty());
}

#[test]
fn labels_all_none_produces_empty() {
    let result = merge_labels(&[None, None]);
    assert!(result.is_empty());
}

#[test]
fn labels_single_source_passthrough() {
    let labels = vec![make_label("env", Some("prod")), make_label("tier", None)];
    let result = merge_labels(&[Some(labels.as_slice())]);
    assert_eq!(result.len(), 2);
}

#[test]
fn labels_dedup_exact_key_value_pair() {
    let labels_a = vec![make_label("env", Some("prod"))];
    let labels_b = vec![make_label("env", Some("prod"))];
    let result = merge_labels(&[Some(labels_a.as_slice()), Some(labels_b.as_slice())]);
    assert_eq!(
        result.len(),
        1,
        "duplicate (key, value) pair should be deduped"
    );
}

#[test]
fn labels_same_key_different_values_both_kept() {
    let labels_a = vec![make_label("env", Some("prod"))];
    let labels_b = vec![make_label("env", Some("staging"))];
    let result = merge_labels(&[Some(labels_a.as_slice()), Some(labels_b.as_slice())]);
    assert_eq!(result.len(), 2);
}

#[test]
fn labels_sorted_by_key_ascending() {
    let labels = vec![
        make_label("z_tag", Some("v")),
        make_label("a_tag", Some("v")),
    ];
    let result = merge_labels(&[Some(labels.as_slice())]);
    assert_eq!(result[0].key, "a_tag");
    assert_eq!(result[1].key, "z_tag");
}

#[test]
fn labels_none_value_sorts_before_some_value() {
    // Same key, one label has no value and the other has a value.
    let labels = vec![
        make_label("flag", Some("present")),
        make_label("flag", None),
    ];
    let result = merge_labels(&[Some(labels.as_slice())]);
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].value, None); // None sorts first
    assert_eq!(result[1].value, Some("present".to_owned()));
}

#[test]
fn labels_sorted_by_key_then_value() {
    let labels_a = vec![make_label("env", Some("prod"))];
    let labels_b = vec![
        make_label("env", Some("dev")),
        make_label("app", Some("service-a")),
    ];
    let result = merge_labels(&[Some(labels_a.as_slice()), Some(labels_b.as_slice())]);
    // Expected order: ("app","service-a"), ("env","dev"), ("env","prod")
    assert_eq!(result.len(), 3);
    assert_eq!(result[0].key, "app");
    assert_eq!(result[1].key, "env");
    assert_eq!(result[1].value.as_deref(), Some("dev"));
    assert_eq!(result[2].key, "env");
    assert_eq!(result[2].value.as_deref(), Some("prod"));
}

fn ordinal_lookup<'a>(ids: &'a [&'a str]) -> impl Fn(&str) -> Option<usize> + 'a {
    |id: &str| ids.iter().position(|&s| s == id)
}

#[test]
fn same_as_definite_threshold_honours_definite_only() {
    let node_ids = ["n0", "n1", "n2"];
    let edges = vec![
        make_same_as_edge("e1", "n0", "n1", Some("definite")),
        make_same_as_edge("e2", "n1", "n2", Some("probable")),
    ];

    let mut uf = crate::union_find::UnionFind::new(3);
    let lookup = ordinal_lookup(&node_ids);
    let honoured = apply_same_as_edges(&edges, lookup, &mut uf, SameAsThreshold::Definite);

    assert_eq!(honoured.len(), 1);
    assert_eq!(&*honoured[0].id, "e1");
    // n0 and n1 should be in the same set
    assert_eq!(uf.find(0), uf.find(1));
    // n2 should be separate
    assert_ne!(uf.find(0), uf.find(2));
}

#[test]
fn same_as_probable_threshold_honours_definite_and_probable() {
    let node_ids = ["n0", "n1", "n2"];
    let edges = vec![
        make_same_as_edge("e1", "n0", "n1", Some("definite")),
        make_same_as_edge("e2", "n1", "n2", Some("probable")),
    ];

    let mut uf = crate::union_find::UnionFind::new(3);
    let lookup = ordinal_lookup(&node_ids);
    let honoured = apply_same_as_edges(&edges, lookup, &mut uf, SameAsThreshold::Probable);

    assert_eq!(honoured.len(), 2);
    assert_eq!(uf.find(0), uf.find(1));
    assert_eq!(uf.find(1), uf.find(2));
}

#[test]
fn same_as_possible_threshold_honours_all() {
    let node_ids = ["n0", "n1", "n2", "n3"];
    let edges = vec![
        make_same_as_edge("e1", "n0", "n1", Some("definite")),
        make_same_as_edge("e2", "n1", "n2", Some("probable")),
        make_same_as_edge("e3", "n2", "n3", Some("possible")),
    ];

    let mut uf = crate::union_find::UnionFind::new(4);
    let lookup = ordinal_lookup(&node_ids);
    let honoured = apply_same_as_edges(&edges, lookup, &mut uf, SameAsThreshold::Possible);

    assert_eq!(honoured.len(), 3);
    let root = uf.find(0);
    assert_eq!(uf.find(1), root);
    assert_eq!(uf.find(2), root);
    assert_eq!(uf.find(3), root);
}

#[test]
fn same_as_no_confidence_treated_as_possible() {
    let node_ids = ["n0", "n1"];
    let edges = vec![make_same_as_edge("e1", "n0", "n1", None)];

    let mut uf = crate::union_find::UnionFind::new(2);
    let lookup = ordinal_lookup(&node_ids);

    // With Definite threshold: absent confidence = possible → not honoured
    let honoured = apply_same_as_edges(&edges, lookup, &mut uf, SameAsThreshold::Definite);
    assert!(honoured.is_empty());
    assert_ne!(uf.find(0), uf.find(1));

    // With Possible threshold: honoured
    let lookup2 = ordinal_lookup(&node_ids);
    let mut uf2 = crate::union_find::UnionFind::new(2);
    let honoured2 = apply_same_as_edges(&edges, lookup2, &mut uf2, SameAsThreshold::Possible);
    assert_eq!(honoured2.len(), 1);
    assert_eq!(uf2.find(0), uf2.find(1));
}

#[test]
fn same_as_non_same_as_edges_ignored() {
    let node_ids = ["n0", "n1"];
    let edges = vec![make_supplies_edge("e1", "n0", "n1")];

    let mut uf = crate::union_find::UnionFind::new(2);
    let lookup = ordinal_lookup(&node_ids);
    let honoured = apply_same_as_edges(&edges, lookup, &mut uf, SameAsThreshold::Possible);

    assert!(honoured.is_empty());
    assert_ne!(uf.find(0), uf.find(1));
}

#[test]
fn same_as_unknown_source_skipped() {
    let node_ids = ["n0", "n1"];
    let edges = vec![make_same_as_edge("e1", "UNKNOWN", "n1", Some("definite"))];

    let mut uf = crate::union_find::UnionFind::new(2);
    let lookup = ordinal_lookup(&node_ids);
    let honoured = apply_same_as_edges(&edges, lookup, &mut uf, SameAsThreshold::Possible);

    assert!(honoured.is_empty());
}

#[test]
fn same_as_cycle_handled_idempotently() {
    // A→B, B→C, C→A: union-find handles cycles as redundant unions.
    let node_ids = ["n0", "n1", "n2"];
    let edges = vec![
        make_same_as_edge("e1", "n0", "n1", Some("definite")),
        make_same_as_edge("e2", "n1", "n2", Some("definite")),
        make_same_as_edge("e3", "n2", "n0", Some("definite")),
    ];

    let mut uf = crate::union_find::UnionFind::new(3);
    let lookup = ordinal_lookup(&node_ids);
    let honoured = apply_same_as_edges(&edges, lookup, &mut uf, SameAsThreshold::Definite);

    assert_eq!(honoured.len(), 3);
    let root = uf.find(0);
    assert_eq!(uf.find(1), root);
    assert_eq!(uf.find(2), root);
}

#[test]
fn build_conflicts_empty_returns_none() {
    let result = build_conflicts_value(vec![]);
    assert!(result.is_none());
}

#[test]
fn build_conflicts_sorted_by_field() {
    let conflicts = vec![
        Conflict {
            field: "z_field".to_owned(),
            values: vec![ConflictEntry {
                value: json!("z"),
                source_file: "a.json".to_owned(),
            }],
        },
        Conflict {
            field: "a_field".to_owned(),
            values: vec![ConflictEntry {
                value: json!("a"),
                source_file: "a.json".to_owned(),
            }],
        },
    ];
    let val = build_conflicts_value(conflicts).expect("non-empty conflicts");
    let arr = val.as_array().expect("array");
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0]["field"].as_str(), Some("a_field"));
    assert_eq!(arr[1]["field"].as_str(), Some("z_field"));
}

#[test]
fn build_conflicts_single_conflict_serialises() {
    let conflicts = vec![Conflict {
        field: "name".to_owned(),
        values: vec![
            ConflictEntry {
                value: json!("Acme"),
                source_file: "a.json".to_owned(),
            },
            ConflictEntry {
                value: json!("ACME Corp"),
                source_file: "b.json".to_owned(),
            },
        ],
    }];
    let val = build_conflicts_value(conflicts).expect("non-empty");
    let arr = val.as_array().expect("array");
    assert_eq!(arr[0]["field"].as_str(), Some("name"));
    let entries = arr[0]["values"].as_array().expect("values array");
    assert_eq!(entries.len(), 2);
}

#[test]
fn merge_metadata_round_trip() {
    let meta = MergeMetadata {
        source_files: vec!["a.omts".to_owned(), "b.omts".to_owned()],
        reporting_entities: vec!["org-acme".to_owned()],
        timestamp: "2026-02-19T00:00:00Z".to_owned(),
        merged_node_count: 10,
        merged_edge_count: 5,
        conflict_count: 2,
    };
    let json = serde_json::to_string(&meta).expect("serialize");
    let back: MergeMetadata = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(meta, back);
}
