/// Full merge pipeline for combining multiple OMTSF files.
///
/// This module implements the eight-step merge procedure described in
/// merge.md, orchestrating:
///
/// 1. Identifier-index construction and union-find for node identity resolution.
/// 2. `same_as` edge processing to extend merge groups.
/// 3. Merge-group safety-limit warnings.
/// 4. Per-group property merge (scalars, identifiers, labels, conflicts).
/// 5. Edge candidate grouping and property merge.
/// 6. Deterministic output ordering.
/// 7. Post-merge L1 validation.
///
/// The primary entry point is [`merge`].
use std::collections::HashMap;

use crate::boundary_hash::generate_file_salt;
use crate::canonical::CanonicalId;
use crate::enums::{EdgeType, EdgeTypeTag};
use crate::file::OmtsFile;
use crate::identity::{edges_match, identifiers_match, is_lei_annulled};
use crate::merge::{
    Conflict, MergeMetadata, SameAsThreshold, ScalarMergeResult, build_conflicts_value,
    merge_identifiers, merge_labels, merge_scalars,
};
use crate::newtypes::{CalendarDate, NodeId};
use crate::structures::{Edge, EdgeProperties, Node};
use crate::types::Identifier;
use crate::union_find::UnionFind;
use crate::validation::{ValidationConfig, validate};

// ---------------------------------------------------------------------------
// MergeError
// ---------------------------------------------------------------------------

/// Errors that can occur during the merge pipeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeError {
    /// The input slice was empty; at least one file is required.
    NoInputFiles,
    /// Post-merge L1 validation found structural errors in the merged output.
    ///
    /// The inner string describes the first error found. This should not occur
    /// under normal operation; if it does it indicates a bug in the pipeline.
    PostMergeValidationFailed(String),
    /// The random file salt could not be generated (platform CSPRNG failure).
    SaltGenerationFailed(String),
    /// A required OMTSF version or date string could not be constructed.
    InternalDataError(String),
}

impl std::fmt::Display for MergeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoInputFiles => f.write_str("merge requires at least one input file"),
            Self::PostMergeValidationFailed(msg) => {
                write!(f, "post-merge L1 validation failed: {msg}")
            }
            Self::SaltGenerationFailed(msg) => {
                write!(f, "could not generate file salt: {msg}")
            }
            Self::InternalDataError(msg) => {
                write!(f, "internal data error during merge: {msg}")
            }
        }
    }
}

impl std::error::Error for MergeError {}

// ---------------------------------------------------------------------------
// MergeWarning
// ---------------------------------------------------------------------------

/// Non-fatal warning produced during the merge pipeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeWarning {
    /// A merge group exceeded the configured size limit.
    ///
    /// This may indicate a false-positive cascade where a single erroneous
    /// identifier match pulls unrelated entities into the same group.
    OversizedMergeGroup {
        /// The representative node ordinal for the group.
        representative_ordinal: usize,
        /// The number of nodes in the group.
        group_size: usize,
        /// The configured limit that was exceeded.
        limit: usize,
    },
}

impl std::fmt::Display for MergeWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OversizedMergeGroup {
                representative_ordinal,
                group_size,
                limit,
            } => write!(
                f,
                "merge group (representative ordinal {representative_ordinal}) has {group_size} \
                 nodes, exceeding the limit of {limit}"
            ),
        }
    }
}

// ---------------------------------------------------------------------------
// MergeConfig
// ---------------------------------------------------------------------------

/// Configuration for the merge pipeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeConfig {
    /// Maximum number of nodes allowed in a single merge group before a
    /// [`MergeWarning::OversizedMergeGroup`] is emitted.
    ///
    /// Default: 50.
    pub group_size_limit: usize,

    /// Confidence threshold for honouring `same_as` edges.
    ///
    /// Default: [`SameAsThreshold::Definite`].
    pub same_as_threshold: SameAsThreshold,

    /// Source-file label used in conflict entries when a file has no path.
    ///
    /// Default: `"<unknown>"`.
    pub default_source_label: String,
}

impl Default for MergeConfig {
    fn default() -> Self {
        Self {
            group_size_limit: 50,
            same_as_threshold: SameAsThreshold::default(),
            default_source_label: "<unknown>".to_owned(),
        }
    }
}

// ---------------------------------------------------------------------------
// MergeOutput
// ---------------------------------------------------------------------------

/// The result of a successful merge operation.
#[derive(Debug, Clone)]
pub struct MergeOutput {
    /// The merged OMTSF file.
    pub file: OmtsFile,
    /// Provenance metadata written into [`MergeOutput::file`]'s `extra` map.
    pub metadata: MergeMetadata,
    /// Non-fatal warnings produced during the merge.
    pub warnings: Vec<MergeWarning>,
    /// Total number of conflict records across all merged nodes and edges.
    pub conflict_count: usize,
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

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

    // Source file labels: use index as fallback since OmtsFile has no path field.
    let source_labels: Vec<String> = files
        .iter()
        .enumerate()
        .map(|(i, _)| format!("file_{i}"))
        .collect();

    // -----------------------------------------------------------------------
    // Step 1: Concatenate all nodes and build a flat node slice with provenance.
    // -----------------------------------------------------------------------
    // node_origins[i] = index into `files` for the file that contributed node i.
    let mut all_nodes: Vec<Node> = Vec::new();
    let mut node_origins: Vec<usize> = Vec::new();

    for (file_idx, file) in files.iter().enumerate() {
        for node in &file.nodes {
            all_nodes.push(node.clone());
            node_origins.push(file_idx);
        }
    }

    let total_nodes = all_nodes.len();

    // -----------------------------------------------------------------------
    // Step 2: Build identifier index, filtering out internal and ANNULLED LEIs.
    // -----------------------------------------------------------------------
    // We build a filtered index that skips ANNULLED LEIs.
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

    // -----------------------------------------------------------------------
    // Step 3: Run union-find over identifier matches.
    // -----------------------------------------------------------------------
    let mut uf = UnionFind::new(total_nodes);

    for node_indices in id_index.values() {
        if node_indices.len() < 2 {
            continue;
        }
        // Evaluate pairwise identity predicate; union matching pairs.
        for i in 0..node_indices.len() {
            for j in (i + 1)..node_indices.len() {
                let idx_a = node_indices[i];
                let idx_b = node_indices[j];
                let node_a = &all_nodes[idx_a];
                let node_b = &all_nodes[idx_b];
                // Check if any identifier from node_a matches any from node_b
                // sharing this canonical key.
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

    // -----------------------------------------------------------------------
    // Step 4: Concatenate all edges and apply same_as edges to union-find.
    // -----------------------------------------------------------------------
    let mut all_edges: Vec<Edge> = Vec::new();
    let mut edge_origins: Vec<usize> = Vec::new();

    // Build a node-id → ordinal map for same_as edge processing.
    // We need per-file offset to correctly map node ids within each file.
    let mut file_node_offsets: Vec<usize> = Vec::with_capacity(files.len());
    {
        let mut offset = 0usize;
        for file in files.iter() {
            file_node_offsets.push(offset);
            offset += file.nodes.len();
        }
    }

    // Build a combined map: for each file, map (file_idx, node_id_str) → ordinal.
    // For same_as edge resolution we need per-file lookups.
    // We'll build a per-file node_id → ordinal map for same_as processing,
    // then concatenate edges.
    let mut per_file_id_maps: Vec<HashMap<&str, usize>> = Vec::with_capacity(files.len());
    for (file_idx, file) in files.iter().enumerate() {
        let offset = file_node_offsets[file_idx];
        let mut map: HashMap<&str, usize> = HashMap::new();
        for (local_idx, node) in file.nodes.iter().enumerate() {
            map.insert(node.id.as_ref(), offset + local_idx);
        }
        per_file_id_maps.push(map);
    }

    // Concatenate all edges.
    for (file_idx, file) in files.iter().enumerate() {
        for edge in &file.edges {
            all_edges.push(edge.clone());
            edge_origins.push(file_idx);
        }
    }

    // Apply same_as edges per file (same_as edges only reference nodes in
    // their own file, since IDs are file-local).
    // We process same_as edges from all_edges using file-specific lookups.
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

    // -----------------------------------------------------------------------
    // Step 5: Compute merge groups and check size limits.
    // -----------------------------------------------------------------------
    let mut warnings: Vec<MergeWarning> = Vec::new();

    if total_nodes > 0 {
        // Count group sizes.
        let mut group_sizes: HashMap<usize, usize> = HashMap::new();
        for i in 0..total_nodes {
            let rep = uf.find(i);
            *group_sizes.entry(rep).or_insert(0) += 1;
        }
        // Sort by representative for deterministic warning order.
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

    // -----------------------------------------------------------------------
    // Step 6: Merge node groups → output nodes.
    // -----------------------------------------------------------------------
    // Collect each group's node ordinals.
    let mut groups: HashMap<usize, Vec<usize>> = HashMap::new();
    for i in 0..total_nodes {
        let rep = uf.find(i);
        groups.entry(rep).or_default().push(i);
    }

    // For each group, determine the canonical ordering key (lowest canonical id
    // among all external identifiers of nodes in the group, or the merged node's
    // assigned id if no external identifiers exist).
    // We assign new sequential IDs: "n-0", "n-1", ... after sorting groups.

    // Build a sorted list of (sort_key, group_representative) pairs.
    // sort_key: lowest canonical identifier string among group members,
    //           or "" if no external identifiers (sorts last, then by rep).
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

    // Sort by (canonical_key, representative) for determinism.
    group_sort_keys.sort_unstable_by(|(key_a, rep_a), (key_b, rep_b)| {
        key_a.cmp(key_b).then_with(|| rep_a.cmp(rep_b))
    });

    // Map from representative ordinal → new NodeId string.
    let mut rep_to_new_id: HashMap<usize, NodeId> = HashMap::new();
    let mut merged_nodes: Vec<(NodeId, usize)> = Vec::new(); // (new_id, rep)

    let mut conflict_count = 0usize;

    for (idx, (_key, rep)) in group_sort_keys.iter().enumerate() {
        let new_id_str = format!("n-{idx}");
        let new_id = NodeId::try_from(new_id_str.as_str())
            .map_err(|e| MergeError::InternalDataError(e.to_string()))?;
        rep_to_new_id.insert(*rep, new_id.clone());
        merged_nodes.push((new_id, *rep));
    }

    // Now build the merged Node for each group.
    let mut output_nodes: Vec<Node> = Vec::new();

    for (new_id, rep) in &merged_nodes {
        let member_ordinals = &groups[rep];

        // Collect source labels for this group.
        let src_labels_for_group: Vec<&str> = member_ordinals
            .iter()
            .map(|&ord| source_labels[node_origins[ord]].as_str())
            .collect();

        // Merge identifiers (set union, sorted by canonical string).
        let id_slices: Vec<Option<&[Identifier]>> = member_ordinals
            .iter()
            .map(|&ord| all_nodes[ord].identifiers.as_deref())
            .collect();
        let merged_ids = merge_identifiers(&id_slices);

        // Merge labels.
        let label_slices: Vec<Option<&[crate::types::Label]>> = member_ordinals
            .iter()
            .map(|&ord| all_nodes[ord].labels.as_deref())
            .collect();
        let merged_labels = merge_labels(&label_slices);

        // Merge scalar: name.
        let name_inputs: Vec<(Option<String>, &str)> = member_ordinals
            .iter()
            .zip(src_labels_for_group.iter())
            .map(|(&ord, &src)| (all_nodes[ord].name.clone(), src))
            .collect();
        let (merged_name, name_conflict) = resolve_scalar_merge(&name_inputs, "name");

        // Merge scalar: node_type — use the first encountered (they should all
        // agree after identity resolution; if not, take the representative's).
        let node_type = all_nodes[member_ordinals[0]].node_type.clone();

        // Merge scalar: jurisdiction.
        let jurisdiction_inputs: Vec<(Option<crate::newtypes::CountryCode>, &str)> =
            member_ordinals
                .iter()
                .zip(src_labels_for_group.iter())
                .map(|(&ord, &src)| (all_nodes[ord].jurisdiction.clone(), src))
                .collect();
        let (merged_jurisdiction, jurisdiction_conflict) =
            resolve_scalar_merge(&jurisdiction_inputs, "jurisdiction");

        // Merge scalar: status.
        let status_inputs: Vec<(Option<crate::enums::OrganizationStatus>, &str)> = member_ordinals
            .iter()
            .zip(src_labels_for_group.iter())
            .map(|(&ord, &src)| (all_nodes[ord].status.clone(), src))
            .collect();
        let (merged_status, status_conflict) = resolve_scalar_merge(&status_inputs, "status");

        // Collect all conflicts for this node.
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

        // Build extra map with _conflicts if any.
        let mut extra = serde_json::Map::new();
        if let Some(conflicts_val) = build_conflicts_value(node_conflicts) {
            extra.insert("_conflicts".to_owned(), conflicts_val);
        }

        // Build the merged node.
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

        // Merge additional scalar fields from the representative node (first in group).
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

    // -----------------------------------------------------------------------
    // Step 7: Merge edges.
    // -----------------------------------------------------------------------
    // Node IDs are file-local: two files can both have a node named "org-1"
    // that refer to entirely different entities.  We must resolve each edge's
    // source/target through the id-map of the file that owns that edge, not
    // through a global map that would silently clobber later files' entries.
    //
    // `edge_node_ordinal(edge_idx, id_str)` performs that per-file lookup.

    // Pre-compute a snapshot of union-find representatives for all node ordinals.
    // We need this as a plain Vec (not &mut uf) so we can call it from closures.
    let node_representatives: Vec<usize> = (0..total_nodes).map(|i| uf.find(i)).collect();

    // Per-file node ordinal lookup: resolves an id string to a global ordinal
    // using the file that owns edge `edge_idx`.
    let edge_node_ordinal = |edge_idx: usize, id: &str| -> Option<usize> {
        let file_idx = edge_origins[edge_idx];
        per_file_id_maps[file_idx].get(id).copied()
    };

    // Build the edge candidate index using per-file resolution.
    // We inline the logic from `build_edge_candidate_index` so we can pass
    // the edge index to the node-ordinal lookup.
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
                // same_as — skip
                continue;
            };
            index.entry(key).or_default().push(edge_idx);
        }
        index
    };

    // For each bucket in the edge candidate index, run pairwise edges_match
    // and build merge groups using a second union-find for edges.
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

                // Resolve representatives using per-file maps.
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

    // Collect edge groups.
    let mut edge_groups: HashMap<usize, Vec<usize>> = HashMap::new();
    for i in 0..total_edges {
        let rep = edge_uf.find(i);
        edge_groups.entry(rep).or_default().push(i);
    }

    // For edge output ordering: sort by
    // (source_canonical, target_canonical, type, lowest_edge_canonical).
    // source_canonical = lowest canonical id of the merged source node group.
    // target_canonical = lowest canonical id of the merged target node group.

    // Build rep_ordinal → lowest canonical id for node groups (for edge sorting).
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

    // Build sort key for each edge group representative.
    // key = (src_canonical, tgt_canonical, type_str, lowest_edge_canonical)
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

    // Build merged edges.
    let mut output_edges: Vec<Edge> = Vec::new();
    let mut edge_new_id_counter = 0usize;

    for (_src_cid, _tgt_cid, _type_str, _edge_cid, rep) in &edge_group_sort_keys {
        let member_ordinals = &edge_groups[rep];

        // Retain same_as edges as-is (one per source edge, rewriting IDs).
        let first_edge = &all_edges[member_ordinals[0]];
        let is_same_as = matches!(&first_edge.edge_type, EdgeTypeTag::Known(EdgeType::SameAs));

        if is_same_as {
            // same_as edges are never merged; retain each one with rewritten endpoints.
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

        // Merge non-same_as edge group.

        // Merged identifiers.
        let id_slices: Vec<Option<&[Identifier]>> = member_ordinals
            .iter()
            .map(|&ord| all_edges[ord].identifiers.as_deref())
            .collect();
        let merged_ids = merge_identifiers(&id_slices);

        // Merged labels (from properties).
        let label_slices: Vec<Option<&[crate::types::Label]>> = member_ordinals
            .iter()
            .map(|&ord| all_edges[ord].properties.labels.as_deref())
            .collect();
        let merged_labels = merge_labels(&label_slices);

        // Rewrite source and target to new merged node IDs.
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
            // Dangling edge — skip.
            continue;
        };

        // Build merged properties from the representative edge.
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
            extra: serde_json::Map::new(),
        };

        // Edge-level conflict recording is minimal in this implementation:
        // we take the representative edge's scalar properties and do not
        // compare across group members beyond identifier merging.
        // build_conflicts_value returns None for an empty vec, so no key
        // is written when there are no conflicts.
        let edge_conflicts: Vec<Conflict> = Vec::new();
        conflict_count += edge_conflicts.len();
        if let Some(conflicts_val) = build_conflicts_value(edge_conflicts) {
            merged_props
                .extra
                .insert("_conflicts".to_owned(), conflicts_val);
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
            extra: serde_json::Map::new(),
        });
    }

    // -----------------------------------------------------------------------
    // Step 8: Build output file.
    // -----------------------------------------------------------------------

    // Collect reporting_entity values from all source files.
    let mut reporting_entities: Vec<String> = files
        .iter()
        .filter_map(|f| f.reporting_entity.as_ref().map(ToString::to_string))
        .collect();
    reporting_entities.sort();
    reporting_entities.dedup();

    // Output reporting_entity: set only if all files agree.
    let output_reporting_entity: Option<NodeId> = if reporting_entities.len() == 1 {
        NodeId::try_from(reporting_entities[0].as_str()).ok()
    } else {
        None
    };

    // Snapshot date: use the latest among all source files.
    let latest_date: Option<CalendarDate> = files.iter().map(|f| f.snapshot_date.clone()).max();

    let snapshot_date = match latest_date {
        Some(d) => d,
        None => CalendarDate::try_from("2026-02-20")
            .map_err(|e| MergeError::InternalDataError(e.to_string()))?,
    };

    // Use the version from the first file.
    let omtsf_version = files[0].omtsf_version.clone();

    // Generate a fresh file salt.
    let file_salt =
        generate_file_salt().map_err(|e| MergeError::SaltGenerationFailed(e.to_string()))?;

    // Source files list for metadata.
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

    // Build extra map with merge_metadata.
    let mut file_extra = serde_json::Map::new();
    if let Ok(meta_val) = serde_json::to_value(&metadata) {
        file_extra.insert("merge_metadata".to_owned(), meta_val);
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

    // -----------------------------------------------------------------------
    // Step 9: Post-merge L1 validation.
    // -----------------------------------------------------------------------
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

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Merges N optional scalar values using [`merge_scalars`], returning the
/// agreed value and an optional [`Conflict`] record.
fn resolve_scalar_merge<T>(
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use crate::enums::{EdgeType, EdgeTypeTag, NodeType, NodeTypeTag};
    use crate::newtypes::{FileSalt, NodeId, SemVer};
    use crate::structures::{Edge, EdgeProperties};
    use crate::types::Identifier;

    const SALT_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const SALT_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const SALT_C: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

    fn semver(s: &str) -> SemVer {
        SemVer::try_from(s).expect("valid SemVer")
    }

    fn date(s: &str) -> CalendarDate {
        CalendarDate::try_from(s).expect("valid CalendarDate")
    }

    fn file_salt(s: &str) -> FileSalt {
        FileSalt::try_from(s).expect("valid FileSalt")
    }

    fn node_id(s: &str) -> NodeId {
        NodeId::try_from(s).expect("valid NodeId")
    }

    fn make_org_node(id: &str, name: Option<&str>, identifiers: Option<Vec<Identifier>>) -> Node {
        Node {
            id: node_id(id),
            node_type: NodeTypeTag::Known(NodeType::Organization),
            identifiers,
            data_quality: None,
            labels: None,
            name: name.map(str::to_owned),
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
            extra: serde_json::Map::new(),
        }
    }

    fn make_supplies_edge(id: &str, src: &str, tgt: &str) -> Edge {
        Edge {
            id: node_id(id),
            edge_type: EdgeTypeTag::Known(EdgeType::Supplies),
            source: node_id(src),
            target: node_id(tgt),
            identifiers: None,
            properties: EdgeProperties::default(),
            extra: serde_json::Map::new(),
        }
    }

    fn minimal_file(salt: &str, nodes: Vec<Node>, edges: Vec<Edge>) -> OmtsFile {
        OmtsFile {
            omtsf_version: semver("1.0.0"),
            snapshot_date: date("2026-02-20"),
            file_salt: file_salt(salt),
            disclosure_scope: None,
            previous_snapshot_ref: None,
            snapshot_sequence: None,
            reporting_entity: None,
            nodes,
            edges,
            extra: serde_json::Map::new(),
        }
    }

    // -----------------------------------------------------------------------
    // Test: empty input
    // -----------------------------------------------------------------------

    #[test]
    fn merge_empty_input_returns_error() {
        let result = merge(&[]);
        assert!(matches!(result, Err(MergeError::NoInputFiles)));
    }

    // -----------------------------------------------------------------------
    // Test: single file passthrough
    // -----------------------------------------------------------------------

    #[test]
    fn merge_single_file_passthrough() {
        let nodes = vec![make_org_node("org-1", Some("Acme"), None)];
        let file = minimal_file(SALT_A, nodes, vec![]);
        let output = merge(&[file]).expect("merge should succeed");
        assert_eq!(output.file.nodes.len(), 1);
        assert_eq!(output.file.edges.len(), 0);
        assert_eq!(output.warnings.len(), 0);
    }

    // -----------------------------------------------------------------------
    // Test: disjoint merge (no overlapping identifiers)
    // -----------------------------------------------------------------------

    #[test]
    fn merge_disjoint_graphs() {
        let node_a = make_org_node(
            "org-1",
            Some("Alpha Corp"),
            Some(vec![make_identifier("lei", "TESTLEIALPHATEST0091")]),
        );
        let node_b = make_org_node(
            "org-2",
            Some("Beta Ltd"),
            Some(vec![make_identifier("duns", "012345678")]),
        );

        // (No edges in this test — we only check that disjoint nodes stay separate.)

        let file_a = minimal_file(SALT_A, vec![node_a], vec![]);
        let file_b = minimal_file(SALT_B, vec![node_b], vec![]);

        let output = merge(&[file_a, file_b]).expect("disjoint merge should succeed");

        // Two distinct nodes — no merge happened.
        assert_eq!(
            output.file.nodes.len(),
            2,
            "disjoint nodes should both appear"
        );
        assert_eq!(output.conflict_count, 0);
        assert_eq!(output.warnings.len(), 0);
    }

    // -----------------------------------------------------------------------
    // Test: full overlap (identical files)
    // -----------------------------------------------------------------------

    #[test]
    fn merge_full_overlap_identical_files() {
        let lei = make_identifier("lei", "TESTLEISHAREDTEST062");
        let node = make_org_node("org-1", Some("SharedCorp"), Some(vec![lei]));
        let file_a = minimal_file(SALT_A, vec![node.clone()], vec![]);
        let file_b = minimal_file(SALT_B, vec![node], vec![]);

        let output = merge(&[file_a, file_b]).expect("full overlap merge should succeed");

        // Two identical nodes → merged into one.
        assert_eq!(
            output.file.nodes.len(),
            1,
            "identical nodes should merge into one"
        );
        assert_eq!(
            output.conflict_count, 0,
            "identical files produce no conflicts"
        );
        assert_eq!(output.warnings.len(), 0);
    }

    // -----------------------------------------------------------------------
    // Test: partial overlap with name conflict
    // -----------------------------------------------------------------------

    #[test]
    fn merge_partial_overlap_with_conflict() {
        let lei = make_identifier("lei", "TESTLEICONFLICT00069");
        let node_a = make_org_node("org-a", Some("Acme Corp"), Some(vec![lei.clone()]));
        let node_b = make_org_node("org-b", Some("ACME Corporation"), Some(vec![lei]));

        let file_a = minimal_file(SALT_A, vec![node_a], vec![]);
        let file_b = minimal_file(SALT_B, vec![node_b], vec![]);

        let output = merge(&[file_a, file_b]).expect("conflict merge should succeed");

        // They share a LEI → merged into one node.
        assert_eq!(
            output.file.nodes.len(),
            1,
            "nodes with shared LEI must merge"
        );
        // Name conflict recorded.
        assert!(
            output.conflict_count > 0,
            "conflicting names must generate a conflict"
        );
        // The merged node's name should be absent (conflict).
        assert!(
            output.file.nodes[0].name.is_none(),
            "conflicting name field should be absent from output"
        );
        // _conflicts entry present.
        assert!(
            output.file.nodes[0].extra.contains_key("_conflicts"),
            "_conflicts should be present on merged node"
        );
    }

    // -----------------------------------------------------------------------
    // Test: three-file merge
    // -----------------------------------------------------------------------

    #[test]
    fn merge_three_files() {
        let lei = make_identifier("lei", "TESTLEITHREETEST0059");
        let duns = make_identifier("duns", "987654321");

        let node_a = make_org_node("n1", Some("Corp A"), Some(vec![lei.clone()]));
        let node_b = make_org_node("n2", Some("Corp A"), Some(vec![lei, duns.clone()]));
        let node_c = make_org_node("n3", Some("Corp A"), Some(vec![duns]));

        let file_a = minimal_file(SALT_A, vec![node_a], vec![]);
        let file_b = minimal_file(SALT_B, vec![node_b], vec![]);
        let file_c = minimal_file(SALT_C, vec![node_c], vec![]);

        let output = merge(&[file_a, file_b, file_c]).expect("three-file merge should succeed");

        // Transitive: A-B via LEI, B-C via DUNS → all three in one group.
        assert_eq!(
            output.file.nodes.len(),
            1,
            "transitive chain must merge all three nodes into one"
        );
        // Name agreed ("Corp A") across all three.
        assert_eq!(
            output.file.nodes[0].name.as_deref(),
            Some("Corp A"),
            "agreed name must be present"
        );
        assert_eq!(output.conflict_count, 0, "no conflicts when names agree");
    }

    // -----------------------------------------------------------------------
    // Test: merge preserves edges with rewritten endpoints
    // -----------------------------------------------------------------------

    #[test]
    fn merge_rewrites_edge_endpoints() {
        let lei = make_identifier("lei", "TESTLEIEDGETEST00051");
        let node_a = make_org_node("supplier", Some("Supplier"), Some(vec![lei.clone()]));
        let node_b = make_org_node("buyer", Some("Buyer"), None);
        let edge = make_supplies_edge("e1", "supplier", "buyer");

        // Same node in file_b under different local ID.
        let node_a2 = make_org_node("supplier2", Some("Supplier"), Some(vec![lei]));
        let node_b2 = make_org_node("buyer2", Some("Buyer"), None);
        let edge2 = make_supplies_edge("e2", "supplier2", "buyer2");

        let file_a = minimal_file(SALT_A, vec![node_a, node_b], vec![edge]);
        let file_b = minimal_file(SALT_B, vec![node_a2, node_b2], vec![edge2]);

        let output = merge(&[file_a, file_b]).expect("edge rewrite merge should succeed");

        // supplier merges (shared LEI); buyer stays separate (2 buyers,
        // different files, no shared identifiers).
        // We expect: 1 supplier group + 2 buyer groups = 3 nodes.
        assert_eq!(output.file.nodes.len(), 3);

        // All edges must reference existing node IDs.
        let node_ids: std::collections::HashSet<&str> =
            output.file.nodes.iter().map(|n| &n.id as &str).collect();
        for edge in &output.file.edges {
            assert!(
                node_ids.contains(&edge.source as &str),
                "edge source {} must reference existing node",
                &edge.source as &str,
            );
            assert!(
                node_ids.contains(&edge.target as &str),
                "edge target {} must reference existing node",
                &edge.target as &str,
            );
        }
    }

    // -----------------------------------------------------------------------
    // Test: oversized group warning
    // -----------------------------------------------------------------------

    #[test]
    fn merge_oversized_group_emits_warning() {
        // Create many nodes sharing the same LEI to trigger group size warning.
        let lei_val = "TESTLEIOVERSIZED0089";
        let nodes: Vec<Node> = (0..5)
            .map(|i| {
                make_org_node(
                    &format!("org-{i}"),
                    Some("OverCorp"),
                    Some(vec![make_identifier("lei", lei_val)]),
                )
            })
            .collect();

        let file = minimal_file(SALT_A, nodes, vec![]);

        let config = MergeConfig {
            group_size_limit: 3, // trigger warning at > 3 nodes
            ..MergeConfig::default()
        };
        let output = merge_with_config(&[file], &config).expect("oversized merge should succeed");

        // 5 nodes with same LEI → 1 group of size 5 > limit 3.
        assert!(
            !output.warnings.is_empty(),
            "oversized group should emit warning"
        );
        let warning = &output.warnings[0];
        assert!(matches!(
            warning,
            MergeWarning::OversizedMergeGroup {
                group_size: 5,
                limit: 3,
                ..
            }
        ));
    }

    // -----------------------------------------------------------------------
    // Test: post-merge output passes L1 validation
    // -----------------------------------------------------------------------

    #[test]
    fn merge_output_passes_l1_validation() {
        let node_a = make_org_node(
            "org-1",
            Some("Alpha"),
            Some(vec![make_identifier("lei", "TESTLEIL1TEST0000069")]),
        );
        let node_b = make_org_node("org-2", Some("Beta"), None);
        let edge = make_supplies_edge("e1", "org-1", "org-2");

        let file = minimal_file(SALT_A, vec![node_a, node_b], vec![edge]);
        let output = merge(&[file]).expect("single file merge should succeed");

        // Validate the output.
        let cfg = ValidationConfig {
            run_l1: true,
            run_l2: false,
            run_l3: false,
        };
        let result = validate(&output.file, &cfg, None);
        assert!(
            result.is_conformant(),
            "merged output must pass L1 validation; errors: {:?}",
            result.errors().collect::<Vec<_>>()
        );
    }

    // -----------------------------------------------------------------------
    // Test: merge metadata written to output
    // -----------------------------------------------------------------------

    #[test]
    fn merge_metadata_in_output() {
        let file_a = minimal_file(SALT_A, vec![], vec![]);
        let file_b = minimal_file(SALT_B, vec![], vec![]);

        let output = merge(&[file_a, file_b]).expect("merge should succeed");

        assert!(
            output.file.extra.contains_key("merge_metadata"),
            "merge_metadata must be present in output file extra"
        );
        assert_eq!(output.metadata.source_files.len(), 2);
    }

    // -----------------------------------------------------------------------
    // Test: colliding node IDs across files are treated as distinct entities
    // -----------------------------------------------------------------------

    /// Two files each have a node named "org-1", but they refer to different
    /// real-world entities (different names, no shared external identifiers).
    /// Each file also has an edge from "org-1" to its own distinct buyer node.
    ///
    /// After merging, the pipeline must produce two separate supplier nodes (one
    /// per file) and two separate edges, not mistakenly bucket both files'
    /// edges as candidates for the same merge group because they share the
    /// local string "org-1".
    #[test]
    fn merge_colliding_node_ids_across_files_are_distinct() {
        // File A: org-1 (Alpha Supplier) → buyer-1 (Alpha Buyer)
        let supplier_a = make_org_node(
            "org-1",
            Some("Alpha Supplier"),
            Some(vec![make_identifier("duns", "111111111")]),
        );
        let buyer_a = make_org_node("buyer-1", Some("Alpha Buyer"), None);
        let edge_a = make_supplies_edge("e-1", "org-1", "buyer-1");

        // File B: org-1 (Beta Supplier) → buyer-1 (Beta Buyer)
        // Same local node ID strings, entirely different entities.
        let supplier_b = make_org_node(
            "org-1",
            Some("Beta Supplier"),
            Some(vec![make_identifier("duns", "222222222")]),
        );
        let buyer_b = make_org_node("buyer-1", Some("Beta Buyer"), None);
        let edge_b = make_supplies_edge("e-1", "org-1", "buyer-1");

        let file_a = minimal_file(SALT_A, vec![supplier_a, buyer_a], vec![edge_a]);
        let file_b = minimal_file(SALT_B, vec![supplier_b, buyer_b], vec![edge_b]);

        let output = merge(&[file_a, file_b]).expect("colliding-id merge should succeed");

        // No shared external identifiers → all four nodes stay distinct.
        assert_eq!(
            output.file.nodes.len(),
            4,
            "four distinct nodes expected (2 suppliers + 2 buyers)"
        );

        // Each file contributes one edge; they connect different node pairs, so
        // they must NOT be merged together.
        assert_eq!(
            output.file.edges.len(),
            2,
            "two distinct edges expected — one per file"
        );

        // Every edge must reference nodes that exist in the output.
        let node_ids: std::collections::HashSet<&str> =
            output.file.nodes.iter().map(|n| &n.id as &str).collect();
        for edge in &output.file.edges {
            assert!(
                node_ids.contains(&edge.source as &str),
                "edge source {} must reference an existing merged node",
                &edge.source as &str,
            );
            assert!(
                node_ids.contains(&edge.target as &str),
                "edge target {} must reference an existing merged node",
                &edge.target as &str,
            );
        }

        // The two output edges must connect different source/target pairs,
        // confirming that file A's "org-1" and file B's "org-1" were resolved
        // to different merged node IDs.
        let edge_pairs: Vec<(&str, &str)> = output
            .file
            .edges
            .iter()
            .map(|e| (&e.source as &str, &e.target as &str))
            .collect();
        assert_eq!(edge_pairs.len(), 2, "there must be exactly two edge pairs");
        assert_ne!(
            edge_pairs[0], edge_pairs[1],
            "the two edges must connect different (source, target) pairs"
        );
    }
}
