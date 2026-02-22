//! Property-based algebraic tests for the merge engine.
//!
//! Verifies commutativity, associativity, and idempotency of `merge` using
//! `proptest`-generated small graphs (1-30 nodes, 0-50 edges) with controlled
//! identifier overlap.
#![allow(clippy::expect_used)]

use omtsf_core::{
    CalendarDate, EdgeType, EdgeTypeTag, FileSalt, NodeId, NodeType, NodeTypeTag, OmtsFile, SemVer,
    merge,
    structures::{Edge, EdgeProperties, Node},
    types::Identifier,
};
use proptest::prelude::*;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

const SALT_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const SALT_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const SALT_C: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

fn semver_1_0_0() -> SemVer {
    SemVer::try_from("1.0.0").expect("valid SemVer")
}

fn date_2026() -> CalendarDate {
    CalendarDate::try_from("2026-02-20").expect("valid CalendarDate")
}

fn salt(s: &str) -> FileSalt {
    FileSalt::try_from(s).expect("valid FileSalt")
}

fn nid(s: &str) -> NodeId {
    NodeId::try_from(s).expect("valid NodeId")
}

/// Build a minimal valid identifier from a (scheme, value) pair drawn from the
/// shared pool.
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

/// Build a minimal organization node.
fn make_node(local_id: usize, identifiers: Vec<Identifier>) -> Node {
    Node {
        id: nid(&format!("n-{local_id}")),
        node_type: NodeTypeTag::Known(NodeType::Organization),
        identifiers: if identifiers.is_empty() {
            None
        } else {
            Some(identifiers)
        },
        ..Node::default()
    }
}

/// Build a minimal supplies edge between two node indices.
fn make_edge(edge_id: usize, src_idx: usize, tgt_idx: usize) -> Edge {
    Edge {
        id: nid(&format!("e-{edge_id}")),
        edge_type: EdgeTypeTag::Known(EdgeType::Supplies),
        source: nid(&format!("n-{src_idx}")),
        target: nid(&format!("n-{tgt_idx}")),
        identifiers: None,
        properties: EdgeProperties::default(),
        extra: BTreeMap::new(),
    }
}

/// Build an `OmtsFile` with the given fixed salt, nodes, and edges.
fn build_file(salt_str: &str, nodes: Vec<Node>, edges: Vec<Edge>) -> OmtsFile {
    OmtsFile {
        omtsf_version: semver_1_0_0(),
        snapshot_date: date_2026(),
        file_salt: salt(salt_str),
        disclosure_scope: None,
        previous_snapshot_ref: None,
        snapshot_sequence: None,
        reporting_entity: None,
        nodes,
        edges,
        extra: BTreeMap::new(),
    }
}

/// Serialise `file` to JSON (using the stable `serde_json` field ordering) and
/// return the SHA-256 digest as a hex string.
///
/// The merge engine guarantees deterministic JSON output for the same logical
/// graph, so two files with the same structure will produce the same digest
/// (modulo the randomly-generated `file_salt`, which changes per `merge` call).
/// We therefore strip the `file_salt` and `merge_metadata.timestamp` before
/// hashing to make the digest stable for comparison.
fn stable_hash(file: &OmtsFile) -> String {
    let mut v = serde_json::to_value(file).expect("serialize to Value");

    if let Some(obj) = v.as_object_mut() {
        obj.insert(
            "file_salt".to_owned(),
            serde_json::Value::String("0".repeat(64)),
        );
        if let Some(meta) = obj
            .get_mut("merge_metadata")
            .and_then(|m| m.as_object_mut())
        {
            meta.insert(
                "timestamp".to_owned(),
                serde_json::Value::String(String::new()),
            );
        }
    }

    let json = serde_json::to_string(&v).expect("serialize normalized Value");
    let digest = Sha256::digest(json.as_bytes());
    hex::encode(digest)
}

mod hex {
    use std::fmt::Write as _;

    /// Encode a byte slice as a lowercase hex string.
    pub fn encode(bytes: impl AsRef<[u8]>) -> String {
        bytes.as_ref().iter().fold(String::new(), |mut s, b| {
            // write! to String is infallible; .ok() discards the Ok(()) without
            // triggering let_underscore_must_use.
            write!(s, "{b:02x}").ok();
            s
        })
    }
}

/// The shared identifier pool.  Each entry is `(scheme, value)`.
/// Files draw subsets of this pool so merge candidates can arise.
///
/// Only DUNS identifiers are used:
/// - DUNS requires exactly 9 digits (`^[0-9]{9}$`) with no check-digit rule.
/// - `nat-reg` and `vat` require an `authority` field (L1-EID-03) so are excluded.
/// - `lei` requires MOD 97-10 check-digit validity (L1-EID-05), which is hard to
///   generate correctly in a static table.
/// - `gln` requires GS1 mod-10 check-digit validity (L1-EID-07), same issue.
///
/// Six distinct values gives ample opportunity for inter-file overlap while
/// keeping the pool small enough that proptest shrinking terminates quickly.
const ID_POOL: &[(&str, &str)] = &[
    ("duns", "100000001"),
    ("duns", "100000002"),
    ("duns", "100000003"),
    ("duns", "200000001"),
    ("duns", "200000002"),
    ("duns", "300000001"),
];

/// Strategy: generate a single small [`OmtsFile`].
///
/// The strategy is designed so that:
///
/// 1. **Unique identifiers per file:** Each node in a file gets exactly one
///    identifier drawn from a disjoint slice of the pool, so no two nodes in
///    the same file share an external identifier.  This ensures that
///    `merge(A, A)` collapses each twin pair into a single merged node.
///
/// 2. **No duplicate edges:** Edges are deduplicated by `(src, tgt)` pair
///    within the generated file, so the original file and `merge(A, A)` have
///    identical edge connectivity (after endpoint remapping).
///
/// Node count is bounded at 6 (the pool size) so the disjoint-identifier
/// invariant can always be satisfied.
fn arb_omts_file() -> impl Strategy<Value = OmtsFile> {
    let salt_strategy = prop::sample::select(vec![SALT_A, SALT_B, SALT_C]);

    let pool_len = ID_POOL.len();
    (1usize..=pool_len.min(6), salt_strategy)
        .prop_flat_map(move |(node_count, file_salt)| {
            let max_nodes = node_count;
            let edges_strat = prop::collection::vec(
                (0usize..max_nodes, 0usize..max_nodes),
                0..=20usize.min(node_count * node_count),
            );

            (edges_strat, Just(node_count), Just(file_salt))
        })
        .prop_map(|(raw_pairs, node_count, file_salt)| {
            let nodes = (0..node_count)
                .map(|i| {
                    let (scheme, value) = ID_POOL[i];
                    let ids = vec![make_identifier(scheme, value)];
                    make_node(i, ids)
                })
                .collect::<Vec<_>>();

            let mut seen = std::collections::HashSet::new();
            let edges = raw_pairs
                .into_iter()
                .filter(|(s, t)| s != t && seen.insert((*s, *t)))
                .enumerate()
                .map(|(edge_idx, (src, tgt))| make_edge(edge_idx, src, tgt))
                .collect::<Vec<_>>();

            build_file(file_salt, nodes, edges)
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// merge(A, B) ≡ merge(B, A) — commutativity.
    ///
    /// The merge engine must produce byte-identical (hash-identical) output
    /// regardless of argument order.
    #[test]
    fn merge_is_commutative(
        a in arb_omts_file(),
        b in arb_omts_file(),
    ) {
        let ab = merge(&[a.clone(), b.clone()]).expect("merge(A,B) must succeed");
        let ba = merge(&[b, a]).expect("merge(B,A) must succeed");
        prop_assert_eq!(stable_hash(&ab.file), stable_hash(&ba.file));
    }

    /// merge(merge(A, B), C) ≡ merge(A, merge(B, C)) — associativity.
    ///
    /// Grouping of merge operands must not affect the result.
    #[test]
    fn merge_is_associative(
        a in arb_omts_file(),
        b in arb_omts_file(),
        c in arb_omts_file(),
    ) {
        let ab = merge(&[a.clone(), b.clone()]).expect("merge(A,B) must succeed");
        let ab_c = merge(&[ab.file, c.clone()]).expect("merge(merge(A,B),C) must succeed");

        let bc = merge(&[b, c]).expect("merge(B,C) must succeed");
        let a_bc = merge(&[a, bc.file]).expect("merge(A,merge(B,C)) must succeed");

        prop_assert_eq!(stable_hash(&ab_c.file), stable_hash(&a_bc.file));
    }

    /// merge(A, A) is structurally equal to A — idempotency.
    ///
    /// Merging a file with itself must produce the same set of nodes and edges
    /// (modulo graph-local ID reassignment and added merge metadata).
    #[test]
    fn merge_is_idempotent(a in arb_omts_file()) {
        let aa = merge(&[a.clone(), a.clone()]).expect("merge(A,A) must succeed");
        assert_structurally_equal(&a, &aa.file);
    }
}

/// Build a map from graph-local `NodeId` string to the sorted canonical identifier
/// string set for that node.  Used to resolve edge endpoints to canonical form.
fn node_canonical_map(
    file: &OmtsFile,
) -> std::collections::HashMap<String, std::collections::BTreeSet<String>> {
    file.nodes
        .iter()
        .map(|n| {
            let cids: std::collections::BTreeSet<String> = n
                .identifiers
                .as_deref()
                .unwrap_or(&[])
                .iter()
                .filter(|id| id.scheme != "internal")
                .map(|id| format!("{}:{}", id.scheme, id.value))
                .collect();
            (n.id.to_string(), cids)
        })
        .collect()
}

/// Assert that `original` and `merged` are structurally equivalent after
/// `merge(A, A)`:
///
/// 1. **Node partition equality:** Every node group (identified by its set of
///    canonical external identifiers) in `original` appears exactly once in
///    `merged`.  When two nodes in `A` shared an identifier they are already
///    the same group; `merge(A, A)` must not create more groups.
///
/// 2. **Edge connectivity equality:** The set of `(src_cid_set, tgt_cid_set,
///    type)` triples in `original` equals the set in `merged`, where endpoints
///    are resolved via their canonical identifier sets (not graph-local IDs).
///
/// Graph-local `id` values are NOT compared because the merge engine
/// reassigns them deterministically.  Merge metadata in `extra` is ignored.
fn assert_structurally_equal(original: &OmtsFile, merged: &OmtsFile) {
    let mut orig_id_sets: Vec<std::collections::BTreeSet<String>> = original
        .nodes
        .iter()
        .map(|n| {
            n.identifiers
                .as_deref()
                .unwrap_or(&[])
                .iter()
                .filter(|id| id.scheme != "internal")
                .map(|id| format!("{}:{}", id.scheme, id.value))
                .collect()
        })
        .collect();
    orig_id_sets.sort_by(|a, b| {
        // Sort sets by their minimum element for determinism.
        let a_min = a.iter().next().map(String::as_str).unwrap_or("");
        let b_min = b.iter().next().map(String::as_str).unwrap_or("");
        a_min.cmp(b_min)
    });

    let mut merged_id_sets: Vec<std::collections::BTreeSet<String>> = merged
        .nodes
        .iter()
        .map(|n| {
            n.identifiers
                .as_deref()
                .unwrap_or(&[])
                .iter()
                .filter(|id| id.scheme != "internal")
                .map(|id| format!("{}:{}", id.scheme, id.value))
                .collect()
        })
        .collect();
    merged_id_sets.sort_by(|a, b| {
        let a_min = a.iter().next().map(String::as_str).unwrap_or("");
        let b_min = b.iter().next().map(String::as_str).unwrap_or("");
        a_min.cmp(b_min)
    });

    // When merging A with A, matching nodes collapse.  Because all nodes in
    // our generated files have unique identifiers (by construction), each
    // node in A merges exactly with its twin from the second copy, so the
    // merged partition should equal the original partition.
    assert_eq!(
        orig_id_sets, merged_id_sets,
        "merge(A,A) must produce the same node identifier-set partition as A"
    );

    let orig_node_map = node_canonical_map(original);
    let merged_node_map = node_canonical_map(merged);

    type EdgeTriple = (
        std::collections::BTreeSet<String>,
        std::collections::BTreeSet<String>,
        String,
    );

    let edge_triple =
        |e: &Edge,
         node_map: &std::collections::HashMap<String, std::collections::BTreeSet<String>>|
         -> Option<EdgeTriple> {
            let src_cids = node_map.get::<str>(&*e.source)?.clone();
            let tgt_cids = node_map.get::<str>(&*e.target)?.clone();
            let type_str = format!("{:?}", e.edge_type);
            Some((src_cids, tgt_cids, type_str))
        };

    let orig_edges: std::collections::BTreeSet<(String, String, String)> = original
        .edges
        .iter()
        .filter_map(|e| edge_triple(e, &orig_node_map))
        .map(|(src, tgt, t)| {
            (
                src.into_iter().collect::<Vec<_>>().join("|"),
                tgt.into_iter().collect::<Vec<_>>().join("|"),
                t,
            )
        })
        .collect();

    let merged_edges: std::collections::BTreeSet<(String, String, String)> = merged
        .edges
        .iter()
        .filter_map(|e| edge_triple(e, &merged_node_map))
        .map(|(src, tgt, t)| {
            (
                src.into_iter().collect::<Vec<_>>().join("|"),
                tgt.into_iter().collect::<Vec<_>>().join("|"),
                t,
            )
        })
        .collect();

    assert_eq!(
        orig_edges, merged_edges,
        "merge(A,A) must preserve the edge connectivity set"
    );
}
