//! Topology strategies: multi-tier supplier tree, ownership hierarchy,
//! mesh overlay, and optional cycle injection.

use std::cmp::min;
use std::collections::BTreeMap;

use omtsf_core::enums::DisclosureScope;
use omtsf_core::file::OmtsFile;
use omtsf_core::newtypes::{CalendarDate, FileSalt, NodeId, SemVer};
use omtsf_core::structures::{Edge, Node};
use rand::Rng;
use rand::rngs::StdRng;

use super::GeneratorConfig;
use super::edges;
use super::nodes;

/// Builds a complete supply chain `OmtsFile` from the generator configuration.
pub fn build_supply_chain(config: &GeneratorConfig, rng: &mut StdRng) -> OmtsFile {
    let mut all_nodes: Vec<Node> = Vec::new();
    let mut all_edges: Vec<Edge> = Vec::new();
    let mut id_counter: usize = 0;
    let mut edge_counter: usize = 0;

    let mut org_ids: Vec<NodeId> = Vec::new();
    for i in 0..config.num_organizations {
        let node = nodes::build_organization(
            rng,
            i,
            &mut id_counter,
            config.identifier_density,
            config.label_density,
        );
        org_ids.push(node.id.clone());
        all_nodes.push(node);
    }

    let mut fac_ids: Vec<NodeId> = Vec::new();
    for i in 0..config.num_facilities {
        let operator = if org_ids.is_empty() {
            None
        } else {
            Some(&org_ids[rng.gen_range(0..org_ids.len())])
        };
        let node = nodes::build_facility(
            rng,
            i,
            &mut id_counter,
            config.identifier_density,
            config.label_density,
            operator,
        );
        fac_ids.push(node.id.clone());
        all_nodes.push(node);
    }

    let mut good_ids: Vec<NodeId> = Vec::new();
    for i in 0..config.num_goods {
        let node = nodes::build_good(
            rng,
            i,
            &mut id_counter,
            config.identifier_density,
            config.label_density,
        );
        good_ids.push(node.id.clone());
        all_nodes.push(node);
    }

    let mut person_ids: Vec<NodeId> = Vec::new();
    for i in 0..config.num_persons {
        let node = nodes::build_person(
            rng,
            i,
            &mut id_counter,
            config.identifier_density,
            config.label_density,
        );
        person_ids.push(node.id.clone());
        all_nodes.push(node);
    }

    let mut att_ids: Vec<NodeId> = Vec::new();
    for i in 0..config.num_attestations {
        let node = nodes::build_attestation(
            rng,
            i,
            &mut id_counter,
            config.identifier_density,
            config.label_density,
        );
        att_ids.push(node.id.clone());
        all_nodes.push(node);
    }

    let mut con_ids: Vec<NodeId> = Vec::new();
    for i in 0..config.num_consignments {
        let installation = if fac_ids.is_empty() {
            None
        } else {
            Some(&fac_ids[rng.gen_range(0..fac_ids.len())])
        };
        let node = nodes::build_consignment(
            rng,
            i,
            &mut id_counter,
            config.identifier_density,
            config.label_density,
            installation,
        );
        con_ids.push(node.id.clone());
        all_nodes.push(node);
    }

    let mut bref_ids: Vec<NodeId> = Vec::new();
    for i in 0..config.num_boundary_refs {
        let node = nodes::build_boundary_ref(rng, i);
        bref_ids.push(node.id.clone());
        all_nodes.push(node);
    }

    build_supplier_tree(rng, &org_ids, config, &mut all_edges, &mut edge_counter);
    build_operates_edges(rng, &org_ids, &fac_ids, &mut all_edges, &mut edge_counter);
    build_produces_edges(rng, &fac_ids, &good_ids, &mut all_edges, &mut edge_counter);
    build_attested_by_edges(
        rng,
        &org_ids,
        &fac_ids,
        &att_ids,
        config,
        &mut all_edges,
        &mut edge_counter,
    );

    build_composed_of_edges(
        rng,
        &good_ids,
        &con_ids,
        config,
        &mut all_edges,
        &mut edge_counter,
    );

    build_ownership_hierarchy(rng, &org_ids, config, &mut all_edges, &mut edge_counter);
    build_beneficial_ownership_edges(
        rng,
        &person_ids,
        &org_ids,
        config,
        &mut all_edges,
        &mut edge_counter,
    );

    build_mesh_overlay(rng, &org_ids, config, &mut all_edges, &mut edge_counter);

    if config.inject_cycles && org_ids.len() >= 4 {
        inject_cycles(rng, &org_ids, &mut all_edges, &mut edge_counter);
    }

    let salt = gen_file_salt(rng);
    let version = SemVer::try_from("1.0.0").unwrap_or_else(|_| unreachable!());
    let date = CalendarDate::try_from("2026-01-15").unwrap_or_else(|_| unreachable!());

    OmtsFile {
        omtsf_version: version,
        snapshot_date: date,
        file_salt: salt,
        disclosure_scope: Some(DisclosureScope::Internal),
        previous_snapshot_ref: None,
        snapshot_sequence: Some(1),
        reporting_entity: None,
        nodes: all_nodes,
        edges: all_edges,
        extra: BTreeMap::new(),
    }
}

/// Builds a multi-tier supplier tree using `supplies` edges.
fn build_supplier_tree(
    rng: &mut StdRng,
    org_ids: &[NodeId],
    config: &GeneratorConfig,
    edges_out: &mut Vec<Edge>,
    edge_counter: &mut usize,
) {
    if org_ids.is_empty() {
        return;
    }

    let remaining = org_ids.len().saturating_sub(1);
    if remaining == 0 {
        return;
    }

    let depth = config.supply_chain_depth.max(1);

    let per_tier = (remaining / depth).max(1);
    let mut tiers: Vec<Vec<usize>> = Vec::new();

    let mut org_idx = 1;
    for _ in 0..depth {
        let tier_size = min(per_tier + rng.gen_range(0..=1), remaining - (org_idx - 1));
        if tier_size == 0 {
            break;
        }
        let tier: Vec<usize> = (org_idx..org_idx + tier_size).collect();
        org_idx += tier_size;
        tiers.push(tier);
        if org_idx > remaining {
            break;
        }
    }

    if org_idx <= remaining {
        let last: Vec<usize> = (org_idx..=remaining).collect();
        if let Some(t) = tiers.last_mut() {
            t.extend(last);
        } else {
            tiers.push(last);
        }
    }

    if let Some(first_tier) = tiers.first() {
        for &supplier_idx in first_tier {
            let edge = edges::build_supplies(
                rng,
                *edge_counter,
                &org_ids[supplier_idx],
                &org_ids[0],
                config.edge_property_fullness,
            );
            *edge_counter += 1;
            edges_out.push(edge);
        }
    }

    for tier_idx in 1..tiers.len() {
        let prev_tier = &tiers[tier_idx - 1];
        let current_tier = &tiers[tier_idx];
        for &supplier_idx in current_tier {
            let target_idx = prev_tier[rng.gen_range(0..prev_tier.len())];
            let edge = edges::build_supplies(
                rng,
                *edge_counter,
                &org_ids[supplier_idx],
                &org_ids[target_idx],
                config.edge_property_fullness,
            );
            *edge_counter += 1;
            edges_out.push(edge);
        }
    }
}

/// Builds operates edges: each facility gets an operates edge from its operator org.
fn build_operates_edges(
    rng: &mut StdRng,
    org_ids: &[NodeId],
    fac_ids: &[NodeId],
    edges_out: &mut Vec<Edge>,
    edge_counter: &mut usize,
) {
    if org_ids.is_empty() || fac_ids.is_empty() {
        return;
    }
    for fac_id in fac_ids {
        let org_idx = rng.gen_range(0..org_ids.len());
        let edge = edges::build_operates(*edge_counter, &org_ids[org_idx], fac_id);
        *edge_counter += 1;
        edges_out.push(edge);
    }
}

/// Builds produces edges: assign each good to a random facility.
fn build_produces_edges(
    rng: &mut StdRng,
    fac_ids: &[NodeId],
    good_ids: &[NodeId],
    edges_out: &mut Vec<Edge>,
    edge_counter: &mut usize,
) {
    if fac_ids.is_empty() || good_ids.is_empty() {
        return;
    }
    for good_id in good_ids {
        let fac_idx = rng.gen_range(0..fac_ids.len());
        let edge = edges::build_produces(*edge_counter, &fac_ids[fac_idx], good_id);
        *edge_counter += 1;
        edges_out.push(edge);
    }
}

/// Builds `attested_by` edges: assign each attestation to a random facility or org.
fn build_attested_by_edges(
    rng: &mut StdRng,
    org_ids: &[NodeId],
    fac_ids: &[NodeId],
    att_ids: &[NodeId],
    config: &GeneratorConfig,
    edges_out: &mut Vec<Edge>,
    edge_counter: &mut usize,
) {
    if att_ids.is_empty() {
        return;
    }
    let mut sources: Vec<NodeId> = Vec::new();
    sources.extend(org_ids.iter().cloned());
    sources.extend(fac_ids.iter().cloned());

    if sources.is_empty() {
        return;
    }

    for att_id in att_ids {
        let src_idx = rng.gen_range(0..sources.len());
        let edge = edges::build_attested_by(
            rng,
            *edge_counter,
            &sources[src_idx],
            att_id,
            config.edge_property_fullness,
        );
        *edge_counter += 1;
        edges_out.push(edge);
    }
}

/// Builds `composed_of` edges: assign each consignment to a random good.
fn build_composed_of_edges(
    rng: &mut StdRng,
    good_ids: &[NodeId],
    con_ids: &[NodeId],
    config: &GeneratorConfig,
    edges_out: &mut Vec<Edge>,
    edge_counter: &mut usize,
) {
    if good_ids.is_empty() || con_ids.is_empty() {
        return;
    }
    for con_id in con_ids {
        let good_idx = rng.gen_range(0..good_ids.len());
        let edge = edges::build_composed_of(
            rng,
            *edge_counter,
            &good_ids[good_idx],
            con_id,
            config.edge_property_fullness,
        );
        *edge_counter += 1;
        edges_out.push(edge);
    }
}

/// Builds ownership hierarchy: a tree of ownership + `legal_parentage` edges.
fn build_ownership_hierarchy(
    rng: &mut StdRng,
    org_ids: &[NodeId],
    config: &GeneratorConfig,
    edges_out: &mut Vec<Edge>,
    edge_counter: &mut usize,
) {
    if org_ids.len() < 2 {
        return;
    }

    let depth = config.ownership_depth.max(1);
    let per_level = ((org_ids.len() - 1) / depth).max(1);

    let mut parent_start = 0;
    let mut parent_end = 1;
    let mut child_idx = 1;

    for _ in 0..depth {
        if child_idx >= org_ids.len() {
            break;
        }
        let level_size = min(per_level, org_ids.len() - child_idx);
        for _ in 0..level_size {
            if child_idx >= org_ids.len() {
                break;
            }
            let parent = rng.gen_range(parent_start..parent_end);
            let edge = edges::build_ownership(
                rng,
                *edge_counter,
                &org_ids[parent],
                &org_ids[child_idx],
                config.edge_property_fullness,
            );
            *edge_counter += 1;
            edges_out.push(edge);

            let lp_edge =
                edges::build_legal_parentage(*edge_counter, &org_ids[parent], &org_ids[child_idx]);
            *edge_counter += 1;
            edges_out.push(lp_edge);

            child_idx += 1;
        }
        parent_start = parent_end;
        parent_end = child_idx;
    }
}

/// Builds `beneficial_ownership` edges from persons to leaf orgs.
fn build_beneficial_ownership_edges(
    rng: &mut StdRng,
    person_ids: &[NodeId],
    org_ids: &[NodeId],
    config: &GeneratorConfig,
    edges_out: &mut Vec<Edge>,
    edge_counter: &mut usize,
) {
    if person_ids.is_empty() || org_ids.is_empty() {
        return;
    }
    for person_id in person_ids {
        let count = rng.gen_range(1..=min(3, org_ids.len()));
        for _ in 0..count {
            let org_idx = rng.gen_range(0..org_ids.len());
            let edge = edges::build_beneficial_ownership(
                rng,
                *edge_counter,
                person_id,
                &org_ids[org_idx],
                config.edge_property_fullness,
            );
            *edge_counter += 1;
            edges_out.push(edge);
        }
    }
}

/// Builds mesh overlay: random cross-tier edges (`sells_to`, brokers,
/// distributes, subcontracts).
fn build_mesh_overlay(
    rng: &mut StdRng,
    org_ids: &[NodeId],
    config: &GeneratorConfig,
    edges_out: &mut Vec<Edge>,
    edge_counter: &mut usize,
) {
    if org_ids.len() < 2 {
        return;
    }

    let mesh_count = (config.mesh_density * config.num_organizations as f64).round() as usize;

    for _ in 0..mesh_count {
        let (src, tgt) = edges::pick_org_pair(rng, org_ids.len());
        let kind = rng.gen_range(0..4);
        let edge = match kind {
            0 => edges::build_sells_to(
                rng,
                *edge_counter,
                &src,
                &tgt,
                config.edge_property_fullness,
            ),
            1 => edges::build_brokers(
                rng,
                *edge_counter,
                &src,
                &tgt,
                config.edge_property_fullness,
            ),
            2 => edges::build_distributes(
                rng,
                *edge_counter,
                &src,
                &tgt,
                config.edge_property_fullness,
            ),
            _ => edges::build_subcontracts(
                rng,
                *edge_counter,
                &src,
                &tgt,
                config.edge_property_fullness,
            ),
        };
        *edge_counter += 1;
        edges_out.push(edge);
    }
}

/// Injects 1-3 cycles into the `legal_parentage` subgraph for cycle detection benchmarks.
fn inject_cycles(
    rng: &mut StdRng,
    org_ids: &[NodeId],
    edges_out: &mut Vec<Edge>,
    edge_counter: &mut usize,
) {
    let num_cycles = rng.gen_range(1..=3);
    for _ in 0..num_cycles {
        let a = rng.gen_range(0..org_ids.len());
        let mut b = rng.gen_range(0..org_ids.len());
        while b == a {
            b = rng.gen_range(0..org_ids.len());
        }
        let edge = edges::build_legal_parentage(*edge_counter, &org_ids[b], &org_ids[a]);
        *edge_counter += 1;
        edges_out.push(edge);
    }
}

/// Generates a valid 64-character hex file salt.
fn gen_file_salt(rng: &mut StdRng) -> FileSalt {
    let hex_chars = b"0123456789abcdef";
    let s: String = (0..64)
        .map(|_| {
            let idx = rng.gen_range(0..hex_chars.len());
            hex_chars[idx] as char
        })
        .collect();
    FileSalt::try_from(s.as_str()).unwrap_or_else(|_| unreachable!())
}
