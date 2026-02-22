use std::collections::{BTreeMap, HashMap};

use crate::boundary_hash::generate_file_salt;
use crate::canonical::CanonicalId;
use crate::dynvalue::DynValue;
use crate::enums::{EdgeType, EdgeTypeTag};
use crate::file::OmtsFile;
use crate::identity::{edges_match, identifiers_match, is_lei_annulled};
use crate::merge::{
    Conflict, MergeMetadata, ScalarMergeResult, build_conflicts_value, merge_identifiers,
    merge_labels, merge_scalars,
};
use crate::newtypes::{CalendarDate, NodeId};
use crate::structures::{Edge, EdgeProperties, Node};
use crate::types::Identifier;
use crate::union_find::UnionFind;
use crate::validation::{ValidationConfig, validate};

use super::types::{MergeConfig, MergeError, MergeOutput, MergeWarning};

/// Merges two or more OMTSF files into a single deduplicated file.
///
/// Uses the default [`MergeConfig`] (group size limit 50, `same_as` threshold
/// `Definite`). Call [`merge_with_config`] for custom configuration.
///
/// # Errors
///
/// Returns [`MergeError::NoInputFiles`] when `files` is empty.
/// Returns [`MergeError::PostMergeValidationFailed`] if the merged output
/// fails L1 validation (should not occur in normal operation).
/// Returns [`MergeError::SaltGenerationFailed`] if the CSPRNG is unavailable.
pub fn merge(files: &[OmtsFile]) -> Result<MergeOutput, MergeError> {
    merge_with_config(files, &MergeConfig::default())
}

/// Merges two or more OMTSF files using the given configuration.
///
/// # Errors
///
/// See [`merge`].
pub fn merge_with_config(
    files: &[OmtsFile],
    config: &MergeConfig,
) -> Result<MergeOutput, MergeError> {
    if files.is_empty() {
        return Err(MergeError::NoInputFiles);
    }

    let source_labels: Vec<String> = files
        .iter()
        .enumerate()
        .map(|(i, _)| format!("file_{i}"))
        .collect();

    let mut all_nodes: Vec<Node> = Vec::new();
    let mut node_origins: Vec<usize> = Vec::new();

    for (file_idx, file) in files.iter().enumerate() {
        for node in &file.nodes {
            all_nodes.push(node.clone());
            node_origins.push(file_idx);
        }
    }

    let total_nodes = all_nodes.len();

    let mut id_index: HashMap<CanonicalId, Vec<usize>> = HashMap::new();
    for (node_idx, node) in all_nodes.iter().enumerate() {
        let Some(identifiers) = node.identifiers.as_ref() else {
            continue;
        };
        for id in identifiers {
            if id.scheme == "internal" {
                continue;
            }
            if is_lei_annulled(id) {
                continue;
            }
            let canonical = CanonicalId::from_identifier(id);
            id_index.entry(canonical).or_default().push(node_idx);
        }
    }

    let mut uf = UnionFind::new(total_nodes);

    for node_indices in id_index.values() {
        if node_indices.len() < 2 {
            continue;
        }
        for i in 0..node_indices.len() {
            for j in (i + 1)..node_indices.len() {
                let idx_a = node_indices[i];
                let idx_b = node_indices[j];
                let node_a = &all_nodes[idx_a];
                let node_b = &all_nodes[idx_b];
                let ids_a = node_a.identifiers.as_deref().unwrap_or(&[]);
                let ids_b = node_b.identifiers.as_deref().unwrap_or(&[]);
                let mut matched = false;
                'outer: for id_a in ids_a {
                    for id_b in ids_b {
                        if identifiers_match(id_a, id_b) {
                            matched = true;
                            break 'outer;
                        }
                    }
                }
                if matched {
                    uf.union(idx_a, idx_b);
                }
            }
        }
    }

    let mut all_edges: Vec<Edge> = Vec::new();
    let mut edge_origins: Vec<usize> = Vec::new();

    let mut file_node_offsets: Vec<usize> = Vec::with_capacity(files.len());
    {
        let mut offset = 0usize;
        for file in files.iter() {
            file_node_offsets.push(offset);
            offset += file.nodes.len();
        }
    }

    let mut per_file_id_maps: Vec<HashMap<&str, usize>> = Vec::with_capacity(files.len());
    for (file_idx, file) in files.iter().enumerate() {
        let offset = file_node_offsets[file_idx];
        let mut map: HashMap<&str, usize> = HashMap::new();
        for (local_idx, node) in file.nodes.iter().enumerate() {
            map.insert(node.id.as_ref(), offset + local_idx);
        }
        per_file_id_maps.push(map);
    }

    for (file_idx, file) in files.iter().enumerate() {
        for edge in &file.edges {
            all_edges.push(edge.clone());
            edge_origins.push(file_idx);
        }
    }

    for (edge_idx, edge) in all_edges.iter().enumerate() {
        let is_same_as = matches!(&edge.edge_type, EdgeTypeTag::Known(EdgeType::SameAs));
        if !is_same_as {
            continue;
        }

        let file_idx = edge_origins[edge_idx];
        let id_map = &per_file_id_maps[file_idx];

        let confidence_str: Option<&str> = edge
            .properties
            .extra
            .get("confidence")
            .and_then(|v| v.as_str())
            .or_else(|| edge.extra.get("confidence").and_then(|v| v.as_str()));

        if !config.same_as_threshold.honours(confidence_str) {
            continue;
        }

        let Some(&src_ord) = id_map.get(&*edge.source as &str) else {
            continue;
        };
        let Some(&tgt_ord) = id_map.get(&*edge.target as &str) else {
            continue;
        };

        uf.union(src_ord, tgt_ord);
    }

    let mut warnings: Vec<MergeWarning> = Vec::new();

    if total_nodes > 0 {
        let mut group_sizes: HashMap<usize, usize> = HashMap::new();
        for i in 0..total_nodes {
            let rep = uf.find(i);
            *group_sizes.entry(rep).or_insert(0) += 1;
        }
        let mut reps: Vec<usize> = group_sizes.keys().copied().collect();
        reps.sort_unstable();
        for rep in reps {
            let size = group_sizes[&rep];
            if size > config.group_size_limit {
                warnings.push(MergeWarning::OversizedMergeGroup {
                    representative_ordinal: rep,
                    group_size: size,
                    limit: config.group_size_limit,
                });
            }
        }
    }

    let mut groups: HashMap<usize, Vec<usize>> = HashMap::new();
    for i in 0..total_nodes {
        let rep = uf.find(i);
        groups.entry(rep).or_default().push(i);
    }

    let mut group_sort_keys: Vec<(String, usize)> = groups
        .iter()
        .map(|(&rep, member_ordinals)| {
            let min_canonical = member_ordinals
                .iter()
                .flat_map(|&ord| {
                    all_nodes[ord]
                        .identifiers
                        .as_deref()
                        .unwrap_or(&[])
                        .iter()
                        .filter(|id| id.scheme != "internal" && !is_lei_annulled(id))
                        .map(|id| CanonicalId::from_identifier(id).into_string())
                })
                .min()
                .unwrap_or_default();
            (min_canonical, rep)
        })
        .collect();

    group_sort_keys.sort_unstable_by(|(key_a, rep_a), (key_b, rep_b)| {
        key_a.cmp(key_b).then_with(|| rep_a.cmp(rep_b))
    });

    let mut rep_to_new_id: HashMap<usize, NodeId> = HashMap::new();
    let mut merged_nodes: Vec<(NodeId, usize)> = Vec::new();

    let mut conflict_count = 0usize;

    for (idx, (_key, rep)) in group_sort_keys.iter().enumerate() {
        let new_id_str = format!("n-{idx}");
        let new_id = NodeId::try_from(new_id_str.as_str())
            .map_err(|e| MergeError::InternalDataError(e.to_string()))?;
        rep_to_new_id.insert(*rep, new_id.clone());
        merged_nodes.push((new_id, *rep));
    }

    let mut output_nodes: Vec<Node> = Vec::new();

    for (new_id, rep) in &merged_nodes {
        let member_ordinals = &groups[rep];

        let src_labels_for_group: Vec<&str> = member_ordinals
            .iter()
            .map(|&ord| source_labels[node_origins[ord]].as_str())
            .collect();

        let id_slices: Vec<Option<&[Identifier]>> = member_ordinals
            .iter()
            .map(|&ord| all_nodes[ord].identifiers.as_deref())
            .collect();
        let merged_ids = merge_identifiers(&id_slices);

        let label_slices: Vec<Option<&[crate::types::Label]>> = member_ordinals
            .iter()
            .map(|&ord| all_nodes[ord].labels.as_deref())
            .collect();
        let merged_labels = merge_labels(&label_slices);

        let name_inputs: Vec<(Option<String>, &str)> = member_ordinals
            .iter()
            .zip(src_labels_for_group.iter())
            .map(|(&ord, &src)| (all_nodes[ord].name.clone(), src))
            .collect();
        let (merged_name, name_conflict) = resolve_scalar_merge(&name_inputs, "name");

        let node_type = all_nodes[member_ordinals[0]].node_type.clone();

        let jurisdiction_inputs: Vec<(Option<crate::newtypes::CountryCode>, &str)> =
            member_ordinals
                .iter()
                .zip(src_labels_for_group.iter())
                .map(|(&ord, &src)| (all_nodes[ord].jurisdiction.clone(), src))
                .collect();
        let (merged_jurisdiction, jurisdiction_conflict) =
            resolve_scalar_merge(&jurisdiction_inputs, "jurisdiction");

        let status_inputs: Vec<(Option<crate::enums::OrganizationStatus>, &str)> = member_ordinals
            .iter()
            .zip(src_labels_for_group.iter())
            .map(|(&ord, &src)| (all_nodes[ord].status.clone(), src))
            .collect();
        let (merged_status, status_conflict) = resolve_scalar_merge(&status_inputs, "status");

        let mut node_conflicts: Vec<Conflict> = Vec::new();
        if let Some(c) = name_conflict {
            node_conflicts.push(c);
        }
        if let Some(c) = jurisdiction_conflict {
            node_conflicts.push(c);
        }
        if let Some(c) = status_conflict {
            node_conflicts.push(c);
        }
        conflict_count += node_conflicts.len();

        let mut extra = BTreeMap::new();
        if let Some(conflicts_val) = build_conflicts_value(node_conflicts) {
            extra.insert("_conflicts".to_owned(), DynValue::from(conflicts_val));
        }

        let mut merged_node = Node {
            id: new_id.clone(),
            node_type,
            identifiers: if merged_ids.is_empty() {
                None
            } else {
                Some(merged_ids)
            },
            data_quality: None,
            labels: if merged_labels.is_empty() {
                None
            } else {
                Some(merged_labels)
            },
            name: merged_name,
            jurisdiction: merged_jurisdiction,
            status: merged_status,
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
            extra,
        };

        let rep_node = &all_nodes[member_ordinals[0]];
        merged_node.governance_structure = rep_node.governance_structure.clone();
        merged_node.operator = rep_node.operator.clone();
        merged_node.address = rep_node.address.clone();
        merged_node.geo = rep_node.geo.clone();
        merged_node.commodity_code = rep_node.commodity_code.clone();
        merged_node.unit = rep_node.unit.clone();
        merged_node.role = rep_node.role.clone();
        merged_node.attestation_type = rep_node.attestation_type.clone();
        merged_node.standard = rep_node.standard.clone();
        merged_node.issuer = rep_node.issuer.clone();
        merged_node.valid_from = rep_node.valid_from.clone();
        merged_node.valid_to = rep_node.valid_to.clone();
        merged_node.outcome = rep_node.outcome.clone();
        merged_node.attestation_status = rep_node.attestation_status.clone();
        merged_node.reference = rep_node.reference.clone();
        merged_node.risk_severity = rep_node.risk_severity.clone();
        merged_node.risk_likelihood = rep_node.risk_likelihood.clone();
        merged_node.lot_id = rep_node.lot_id.clone();
        merged_node.quantity = rep_node.quantity;
        merged_node.production_date = rep_node.production_date.clone();
        merged_node.origin_country = rep_node.origin_country.clone();
        merged_node.direct_emissions_co2e = rep_node.direct_emissions_co2e;
        merged_node.indirect_emissions_co2e = rep_node.indirect_emissions_co2e;
        merged_node.emission_factor_source = rep_node.emission_factor_source.clone();
        merged_node.installation_id = rep_node.installation_id.clone();

        output_nodes.push(merged_node);
    }

    let node_representatives: Vec<usize> = (0..total_nodes).map(|i| uf.find(i)).collect();

    let edge_node_ordinal = |edge_idx: usize, id: &str| -> Option<usize> {
        let file_idx = edge_origins[edge_idx];
        per_file_id_maps[file_idx].get(id).copied()
    };

    let edge_candidate_index = {
        use crate::identity::{EdgeCompositeKey, edge_composite_key};
        let mut index: HashMap<EdgeCompositeKey, Vec<usize>> = HashMap::new();
        for (edge_idx, edge) in all_edges.iter().enumerate() {
            let Some(src_ord) = edge_node_ordinal(edge_idx, edge.source.as_ref()) else {
                continue;
            };
            let Some(tgt_ord) = edge_node_ordinal(edge_idx, edge.target.as_ref()) else {
                continue;
            };
            let src_rep = node_representatives[src_ord];
            let tgt_rep = node_representatives[tgt_ord];
            let Some(key) = edge_composite_key(src_rep, tgt_rep, edge) else {
                continue;
            };
            index.entry(key).or_default().push(edge_idx);
        }
        index
    };

    let total_edges = all_edges.len();
    let mut edge_uf = UnionFind::new(total_edges);

    for bucket in edge_candidate_index.values() {
        if bucket.len() < 2 {
            continue;
        }
        for i in 0..bucket.len() {
            for j in (i + 1)..bucket.len() {
                let ei = bucket[i];
                let ej = bucket[j];
                let edge_a = &all_edges[ei];
                let edge_b = &all_edges[ej];

                let src_rep_a = edge_node_ordinal(ei, edge_a.source.as_ref())
                    .map(|o| node_representatives[o])
                    .unwrap_or(usize::MAX);
                let tgt_rep_a = edge_node_ordinal(ei, edge_a.target.as_ref())
                    .map(|o| node_representatives[o])
                    .unwrap_or(usize::MAX);
                let src_rep_b = edge_node_ordinal(ej, edge_b.source.as_ref())
                    .map(|o| node_representatives[o])
                    .unwrap_or(usize::MAX);
                let tgt_rep_b = edge_node_ordinal(ej, edge_b.target.as_ref())
                    .map(|o| node_representatives[o])
                    .unwrap_or(usize::MAX);

                if edges_match(src_rep_a, tgt_rep_a, src_rep_b, tgt_rep_b, edge_a, edge_b) {
                    edge_uf.union(ei, ej);
                }
            }
        }
    }

    let mut edge_groups: HashMap<usize, Vec<usize>> = HashMap::new();
    for i in 0..total_edges {
        let rep = edge_uf.find(i);
        edge_groups.entry(rep).or_default().push(i);
    }

    let mut node_rep_to_canonical: HashMap<usize, String> = HashMap::new();
    for (idx, node) in all_nodes.iter().enumerate() {
        let rep = uf.find(idx);
        let cids: Vec<String> = node
            .identifiers
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .filter(|id| id.scheme != "internal" && !is_lei_annulled(id))
            .map(|id| CanonicalId::from_identifier(id).into_string())
            .collect();
        for cid in cids {
            let entry = node_rep_to_canonical
                .entry(rep)
                .or_insert_with(|| cid.clone());
            if cid < *entry {
                *entry = cid;
            }
        }
    }

    let edge_type_str = |et: &EdgeTypeTag| -> String {
        match et {
            EdgeTypeTag::Known(t) => format!("{t:?}"),
            EdgeTypeTag::Extension(s) => s.clone(),
        }
    };

    let mut edge_group_sort_keys: Vec<(String, String, String, String, usize)> = edge_groups
        .iter()
        .map(|(&rep, member_ordinals)| {
            let first_edge_idx = member_ordinals[0];
            let first_edge = &all_edges[first_edge_idx];

            let src_canonical = edge_node_ordinal(first_edge_idx, first_edge.source.as_ref())
                .map(|o| node_representatives[o])
                .and_then(|node_rep| node_rep_to_canonical.get(&node_rep).cloned())
                .unwrap_or_default();

            let tgt_canonical = edge_node_ordinal(first_edge_idx, first_edge.target.as_ref())
                .map(|o| node_representatives[o])
                .and_then(|node_rep| node_rep_to_canonical.get(&node_rep).cloned())
                .unwrap_or_default();

            let type_str = edge_type_str(&first_edge.edge_type);

            let lowest_edge_cid = member_ordinals
                .iter()
                .flat_map(|&ord| {
                    all_edges[ord]
                        .identifiers
                        .as_deref()
                        .unwrap_or(&[])
                        .iter()
                        .filter(|id| id.scheme != "internal")
                        .map(|id| CanonicalId::from_identifier(id).into_string())
                })
                .min()
                .unwrap_or_default();

            (src_canonical, tgt_canonical, type_str, lowest_edge_cid, rep)
        })
        .collect();

    edge_group_sort_keys.sort_unstable_by(|a, b| {
        a.0.cmp(&b.0)
            .then_with(|| a.1.cmp(&b.1))
            .then_with(|| a.2.cmp(&b.2))
            .then_with(|| a.3.cmp(&b.3))
            .then_with(|| a.4.cmp(&b.4))
    });

    let mut output_edges: Vec<Edge> = Vec::new();
    let mut edge_new_id_counter = 0usize;

    for (_src_cid, _tgt_cid, _type_str, _edge_cid, rep) in &edge_group_sort_keys {
        let member_ordinals = &edge_groups[rep];

        let first_edge = &all_edges[member_ordinals[0]];
        let is_same_as = matches!(&first_edge.edge_type, EdgeTypeTag::Known(EdgeType::SameAs));

        if is_same_as {
            for &ord in member_ordinals {
                let edge = &all_edges[ord];
                let file_idx = edge_origins[ord];
                let file_map = &per_file_id_maps[file_idx];

                let new_src = file_map
                    .get(&*edge.source as &str)
                    .copied()
                    .and_then(|node_ord| {
                        let node_rep = uf.find(node_ord);
                        rep_to_new_id.get(&node_rep).cloned()
                    });
                let new_tgt = file_map
                    .get(&*edge.target as &str)
                    .copied()
                    .and_then(|node_ord| {
                        let node_rep = uf.find(node_ord);
                        rep_to_new_id.get(&node_rep).cloned()
                    });

                let (Some(new_src_id), Some(new_tgt_id)) = (new_src, new_tgt) else {
                    continue;
                };

                let new_edge_id_str = format!("e-{edge_new_id_counter}");
                edge_new_id_counter += 1;
                let Ok(new_edge_id) = NodeId::try_from(new_edge_id_str.as_str()) else {
                    continue;
                };

                output_edges.push(Edge {
                    id: new_edge_id,
                    edge_type: edge.edge_type.clone(),
                    source: new_src_id,
                    target: new_tgt_id,
                    identifiers: edge.identifiers.clone(),
                    properties: edge.properties.clone(),
                    extra: edge.extra.clone(),
                });
            }
            continue;
        }

        let id_slices: Vec<Option<&[Identifier]>> = member_ordinals
            .iter()
            .map(|&ord| all_edges[ord].identifiers.as_deref())
            .collect();
        let merged_ids = merge_identifiers(&id_slices);

        let label_slices: Vec<Option<&[crate::types::Label]>> = member_ordinals
            .iter()
            .map(|&ord| all_edges[ord].properties.labels.as_deref())
            .collect();
        let merged_labels = merge_labels(&label_slices);

        let file_idx_0 = edge_origins[member_ordinals[0]];
        let file_map_0 = &per_file_id_maps[file_idx_0];

        let new_src_id = file_map_0
            .get(&*first_edge.source as &str)
            .copied()
            .and_then(|node_ord| {
                let node_rep = uf.find(node_ord);
                rep_to_new_id.get(&node_rep).cloned()
            });
        let new_tgt_id = file_map_0
            .get(&*first_edge.target as &str)
            .copied()
            .and_then(|node_ord| {
                let node_rep = uf.find(node_ord);
                rep_to_new_id.get(&node_rep).cloned()
            });

        let (Some(new_src), Some(new_tgt)) = (new_src_id, new_tgt_id) else {
            continue;
        };

        let rep_props = &all_edges[member_ordinals[0]].properties;
        let mut merged_props = EdgeProperties {
            data_quality: rep_props.data_quality.clone(),
            labels: if merged_labels.is_empty() {
                None
            } else {
                Some(merged_labels)
            },
            valid_from: rep_props.valid_from.clone(),
            valid_to: rep_props.valid_to.clone(),
            percentage: rep_props.percentage,
            direct: rep_props.direct,
            control_type: rep_props.control_type.clone(),
            consolidation_basis: rep_props.consolidation_basis.clone(),
            event_type: rep_props.event_type.clone(),
            effective_date: rep_props.effective_date.clone(),
            description: rep_props.description.clone(),
            commodity: rep_props.commodity.clone(),
            contract_ref: rep_props.contract_ref.clone(),
            volume: rep_props.volume,
            volume_unit: rep_props.volume_unit.clone(),
            annual_value: rep_props.annual_value,
            value_currency: rep_props.value_currency.clone(),
            tier: rep_props.tier,
            share_of_buyer_demand: rep_props.share_of_buyer_demand,
            service_type: rep_props.service_type.clone(),
            quantity: rep_props.quantity,
            unit: rep_props.unit.clone(),
            scope: rep_props.scope.clone(),
            extra: BTreeMap::new(),
        };

        let edge_conflicts: Vec<Conflict> = Vec::new();
        conflict_count += edge_conflicts.len();
        if let Some(conflicts_val) = build_conflicts_value(edge_conflicts) {
            merged_props
                .extra
                .insert("_conflicts".to_owned(), DynValue::from(conflicts_val));
        }

        let new_edge_id_str = format!("e-{edge_new_id_counter}");
        edge_new_id_counter += 1;
        let Ok(new_edge_id) = NodeId::try_from(new_edge_id_str.as_str()) else {
            continue;
        };

        output_edges.push(Edge {
            id: new_edge_id,
            edge_type: first_edge.edge_type.clone(),
            source: new_src,
            target: new_tgt,
            identifiers: if merged_ids.is_empty() {
                None
            } else {
                Some(merged_ids)
            },
            properties: merged_props,
            extra: BTreeMap::new(),
        });
    }

    let mut reporting_entities: Vec<String> = files
        .iter()
        .filter_map(|f| f.reporting_entity.as_ref().map(ToString::to_string))
        .collect();
    reporting_entities.sort();
    reporting_entities.dedup();

    let output_reporting_entity: Option<NodeId> = if reporting_entities.len() == 1 {
        NodeId::try_from(reporting_entities[0].as_str()).ok()
    } else {
        None
    };

    let latest_date: Option<CalendarDate> = files.iter().map(|f| f.snapshot_date.clone()).max();

    let snapshot_date = match latest_date {
        Some(d) => d,
        None => CalendarDate::try_from("2026-02-20")
            .map_err(|e| MergeError::InternalDataError(e.to_string()))?,
    };

    let omtsf_version = files[0].omtsf_version.clone();

    let file_salt =
        generate_file_salt().map_err(|e| MergeError::SaltGenerationFailed(e.to_string()))?;

    let mut source_files = source_labels.clone();
    source_files.sort();
    source_files.dedup();

    let metadata = MergeMetadata {
        source_files: source_files.clone(),
        reporting_entities: reporting_entities.clone(),
        timestamp: "2026-02-20T00:00:00Z".to_owned(),
        merged_node_count: output_nodes.len(),
        merged_edge_count: output_edges.len(),
        conflict_count,
    };

    let mut file_extra = BTreeMap::new();
    if let Ok(meta_val) = serde_json::to_value(&metadata) {
        file_extra.insert("merge_metadata".to_owned(), DynValue::from(meta_val));
    }

    let merged_file = OmtsFile {
        omtsf_version,
        snapshot_date,
        file_salt,
        disclosure_scope: None,
        previous_snapshot_ref: None,
        snapshot_sequence: None,
        reporting_entity: output_reporting_entity,
        nodes: output_nodes,
        edges: output_edges,
        extra: file_extra,
    };

    let l1_only_config = ValidationConfig {
        run_l1: true,
        run_l2: false,
        run_l3: false,
    };
    let validation_result = validate(&merged_file, &l1_only_config, None);
    if validation_result.has_errors() {
        let first_error = validation_result
            .errors()
            .next()
            .map(|d| d.message.clone())
            .unwrap_or_else(|| "unknown error".to_owned());
        return Err(MergeError::PostMergeValidationFailed(first_error));
    }

    Ok(MergeOutput {
        file: merged_file,
        metadata,
        warnings,
        conflict_count,
    })
}

/// Merges N optional scalar values using [`merge_scalars`], returning the
/// agreed value and an optional [`crate::merge::Conflict`] record.
pub(super) fn resolve_scalar_merge<T>(
    inputs: &[(Option<T>, &str)],
    field_name: &str,
) -> (Option<T>, Option<Conflict>)
where
    T: serde::Serialize + Clone,
{
    match merge_scalars(inputs) {
        ScalarMergeResult::Agreed(val) => (val, None),
        ScalarMergeResult::Conflict(entries) => {
            let conflict = Conflict {
                field: field_name.to_owned(),
                values: entries,
            };
            (None, Some(conflict))
        }
    }
}
