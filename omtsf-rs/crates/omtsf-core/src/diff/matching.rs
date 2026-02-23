use std::collections::{HashMap, HashSet};

use crate::canonical::{CanonicalId, build_identifier_index};
use crate::identity::{edges_match, identifiers_match};
use crate::newtypes::NodeId;
use crate::structures::{Edge, Node};
use crate::union_find::UnionFind;

use super::helpers::{edge_type_str, node_type_str};
use super::types::{DiffFilter, NodeMatchResult};

/// Performs node matching for a diff.
///
/// Builds canonical identifier indices for both files, finds matching pairs
/// via shared identifiers, computes transitive closure using union-find,
/// detects ambiguous groups, and classifies unmatched nodes. Nodes that
/// lack external identifiers fall back to matching by node ID.
pub(super) fn match_nodes(
    nodes_a: &[Node],
    nodes_b: &[Node],
    filter: Option<&DiffFilter>,
) -> NodeMatchResult {
    let node_type_allowed = |node: &Node| -> bool {
        match filter.and_then(|f| f.node_types.as_ref()) {
            None => true,
            Some(allowed) => allowed.contains(node_type_str(&node.node_type)),
        }
    };

    let active_a: HashSet<usize> = (0..nodes_a.len())
        .filter(|&i| node_type_allowed(&nodes_a[i]))
        .collect();
    let active_b: HashSet<usize> = (0..nodes_b.len())
        .filter(|&i| node_type_allowed(&nodes_b[i]))
        .collect();

    let index_a = build_identifier_index(nodes_a);
    let index_b = build_identifier_index(nodes_b);

    let len_a = nodes_a.len();
    let len_b = nodes_b.len();
    let total = len_a + len_b;

    let mut uf = UnionFind::new(total);

    let mut pair_matched_by: HashMap<(usize, usize), Vec<String>> = HashMap::new();

    for (canonical_id, a_nodes) in &index_a {
        let Some(b_nodes) = index_b.get(canonical_id) else {
            continue;
        };

        for &ai in a_nodes {
            for &bi in b_nodes {
                if !active_a.contains(&ai) || !active_b.contains(&bi) {
                    continue;
                }

                let a_node = &nodes_a[ai];
                let b_node = &nodes_b[bi];

                let a_ids = a_node.identifiers.as_deref().unwrap_or(&[]);
                let b_ids = b_node.identifiers.as_deref().unwrap_or(&[]);

                let mut found_match = false;
                for id_a in a_ids {
                    if id_a.scheme == "internal" {
                        continue;
                    }
                    let cid_a = CanonicalId::from_identifier(id_a);
                    if cid_a != *canonical_id {
                        continue;
                    }
                    for id_b in b_ids {
                        if identifiers_match(id_a, id_b) {
                            found_match = true;
                            break;
                        }
                    }
                    if found_match {
                        break;
                    }
                }

                if found_match {
                    uf.union(ai, len_a + bi);
                    pair_matched_by
                        .entry((ai, bi))
                        .or_default()
                        .push(canonical_id.as_str().to_owned());
                }
            }
        }
    }

    // Fallback: match remaining nodes by node ID when identifier-based
    // matching didn't reach them (e.g. nodes with no external identifiers).
    let id_matched_a: HashSet<usize> = pair_matched_by.keys().map(|&(ai, _)| ai).collect();
    let id_matched_b: HashSet<usize> = pair_matched_by.keys().map(|&(_, bi)| bi).collect();

    let mut b_id_map: HashMap<&str, usize> = HashMap::new();
    for &bi in &active_b {
        if !id_matched_b.contains(&bi) {
            b_id_map.insert(&nodes_b[bi].id, bi);
        }
    }

    for &ai in &active_a {
        if id_matched_a.contains(&ai) {
            continue;
        }
        if let Some(&bi) = b_id_map.get(&*nodes_a[ai].id) {
            uf.union(ai, len_a + bi);
            pair_matched_by
                .entry((ai, bi))
                .or_default()
                .push(format!("node-id:{}", nodes_a[ai].id));
        }
    }

    let mut groups: HashMap<usize, (Vec<usize>, Vec<usize>)> = HashMap::new();

    for &ai in &active_a {
        let rep = uf.find(ai);
        groups.entry(rep).or_default().0.push(ai);
    }
    for &bi in &active_b {
        let rep = uf.find(len_a + bi);
        groups.entry(rep).or_default().1.push(bi);
    }

    let mut matched: Vec<(usize, usize, Vec<String>)> = Vec::new();
    let mut unmatched_a: Vec<usize> = Vec::new();
    let mut unmatched_b: Vec<usize> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    for (rep, (a_members, b_members)) in &groups {
        let _ = rep;

        match (a_members.as_slice(), b_members.as_slice()) {
            ([], []) => {}
            (a_list, []) => {
                unmatched_a.extend_from_slice(a_list);
            }

            ([], b_list) => {
                unmatched_b.extend_from_slice(b_list);
            }

            // Both sides present — matched (possibly ambiguous).
            (a_list, b_list) => {
                // Emit ambiguity warning if more than one node from the same file.
                if a_list.len() > 1 || b_list.len() > 1 {
                    let a_ids: Vec<&str> = a_list.iter().map(|&i| &*nodes_a[i].id).collect();
                    let b_ids: Vec<&str> = b_list.iter().map(|&i| &*nodes_b[i].id).collect();
                    warnings.push(format!(
                        "Ambiguous match group: A=[{}] B=[{}]",
                        a_ids.join(", "),
                        b_ids.join(", ")
                    ));
                }

                // Produce matched pairs: pair each A node with each B node in the group.
                for &ai in a_list {
                    for &bi in b_list {
                        let matched_by =
                            pair_matched_by.get(&(ai, bi)).cloned().unwrap_or_default();
                        matched.push((ai, bi, matched_by));
                    }
                }
            }
        }
    }

    NodeMatchResult {
        matched,
        unmatched_a,
        unmatched_b,
        warnings,
    }
}

/// Builds a map from `NodeId` string to a representative index in the unified
/// node space `[0, len_a + len_b)`.
///
/// For matched nodes, both the A-side and B-side `NodeId` strings map to the
/// same representative. For unmatched nodes, each maps to its own ordinal.
pub(super) fn build_node_rep_map(
    nodes_a: &[Node],
    nodes_b: &[Node],
    matched_pairs: &[(usize, usize, Vec<String>)],
) -> (HashMap<String, usize>, UnionFind) {
    let len_a = nodes_a.len();
    let len_b = nodes_b.len();
    let total = len_a + len_b;

    let mut uf = UnionFind::new(total);

    for &(ai, bi, _) in matched_pairs {
        uf.union(ai, len_a + bi);
    }

    let mut map: HashMap<String, usize> = HashMap::new();

    for (ai, node) in nodes_a.iter().enumerate() {
        let rep = uf.find(ai);
        map.insert(node.id.to_string(), rep);
    }
    for (bi, node) in nodes_b.iter().enumerate() {
        let rep = uf.find(len_a + bi);
        map.insert(node.id.to_string(), rep);
    }

    (map, uf)
}

/// Performs edge matching after node matching is complete.
///
/// Edges are matched by:
/// 1. Their source nodes belonging to the same match group.
/// 2. Their target nodes belonging to the same match group.
/// 3. Their type values being equal.
/// 4. Sharing an external identifier, OR lacking external identifiers and having
///    equal type-specific identity properties.
///
/// When multiple A-edges match the same B-edge bucket, they are paired by
/// order of appearance. Excess edges are reported as additions or deletions.
pub(super) fn match_edges(
    edges_a: &[Edge],
    edges_b: &[Edge],
    nodes_a: &[Node],
    nodes_b: &[Node],
    matched_node_pairs: &[(usize, usize, Vec<String>)],
    filter: Option<&DiffFilter>,
) -> (Vec<(usize, usize)>, Vec<usize>, Vec<usize>) {
    let (node_rep_map, _) = build_node_rep_map(nodes_a, nodes_b, matched_node_pairs);

    let edge_type_allowed = |edge: &Edge| -> bool {
        match filter.and_then(|f| f.edge_types.as_ref()) {
            None => true,
            Some(allowed) => allowed.contains(edge_type_str(&edge.edge_type)),
        }
    };

    let node_map_a: HashMap<&str, &Node> = nodes_a.iter().map(|n| (&*n.id as &str, n)).collect();
    let node_map_b: HashMap<&str, &Node> = nodes_b.iter().map(|n| (&*n.id as &str, n)).collect();

    let node_type_allowed_for_id = |node_id: &NodeId| -> bool {
        match filter.and_then(|f| f.node_types.as_ref()) {
            None => true,
            Some(allowed) => {
                let id_str: &str = node_id;
                if let Some(node) = node_map_a.get(id_str) {
                    return allowed.contains(node_type_str(&node.node_type));
                }
                if let Some(node) = node_map_b.get(id_str) {
                    return allowed.contains(node_type_str(&node.node_type));
                }
                false
            }
        }
    };

    let edge_is_active = |edge: &Edge| -> bool {
        edge_type_allowed(edge)
            && node_type_allowed_for_id(&edge.source)
            && node_type_allowed_for_id(&edge.target)
    };

    let active_a_edges: Vec<usize> = (0..edges_a.len())
        .filter(|&i| edge_is_active(&edges_a[i]))
        .collect();
    let active_b_edges: Vec<usize> = (0..edges_b.len())
        .filter(|&i| edge_is_active(&edges_b[i]))
        .collect();

    let resolve_rep = |node_id: &NodeId| -> Option<usize> {
        let key: &str = node_id;
        node_rep_map.get(key).copied()
    };

    // For same_as edges, edges_match returns false, so they won't be paired.
    type EdgeKey = (usize, usize, String);

    let mut a_buckets: HashMap<EdgeKey, Vec<usize>> = HashMap::new();
    for &ai in &active_a_edges {
        let edge = &edges_a[ai];
        let Some(src_rep) = resolve_rep(&edge.source) else {
            continue;
        };
        let Some(tgt_rep) = resolve_rep(&edge.target) else {
            continue;
        };
        let key = (src_rep, tgt_rep, edge_type_str(&edge.edge_type).to_owned());
        a_buckets.entry(key).or_default().push(ai);
    }

    let mut matched_pairs: Vec<(usize, usize)> = Vec::new();
    let mut unmatched_b_edges: Vec<usize> = Vec::new();
    let mut matched_a_set: HashSet<usize> = HashSet::new();

    for &bi in &active_b_edges {
        let edge_b = &edges_b[bi];
        let Some(src_rep_b) = resolve_rep(&edge_b.source) else {
            unmatched_b_edges.push(bi);
            continue;
        };
        let Some(tgt_rep_b) = resolve_rep(&edge_b.target) else {
            unmatched_b_edges.push(bi);
            continue;
        };
        let key_b = (
            src_rep_b,
            tgt_rep_b,
            edge_type_str(&edge_b.edge_type).to_owned(),
        );

        let Some(bucket) = a_buckets.get_mut(&key_b) else {
            unmatched_b_edges.push(bi);
            continue;
        };

        let mut found = false;
        for &ai in bucket.iter() {
            if matched_a_set.contains(&ai) {
                continue;
            }
            let edge_a = &edges_a[ai];
            let Some(src_rep_a) = resolve_rep(&edge_a.source) else {
                continue;
            };
            let Some(tgt_rep_a) = resolve_rep(&edge_a.target) else {
                continue;
            };
            if edges_match(src_rep_a, tgt_rep_a, src_rep_b, tgt_rep_b, edge_a, edge_b) {
                matched_pairs.push((ai, bi));
                matched_a_set.insert(ai);
                found = true;
                break;
            }
        }

        if !found {
            unmatched_b_edges.push(bi);
        }
    }

    let unmatched_a_edges: Vec<usize> = active_a_edges
        .into_iter()
        .filter(|ai| !matched_a_set.contains(ai))
        .collect();

    (matched_pairs, unmatched_a_edges, unmatched_b_edges)
}
