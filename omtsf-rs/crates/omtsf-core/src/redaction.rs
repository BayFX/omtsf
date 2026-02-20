/// Node classification, identifier filtering, edge handling, and the
/// top-level redaction pipeline for the selective disclosure engine.
///
/// This module implements Sections 3, 5, 6, and the orchestration layer
/// described in the redaction specification:
/// - Node classification into [`NodeAction`] dispositions (Section 5)
/// - Identifier filtering based on target scope (Section 3.1–3.2)
/// - Edge property filtering based on target scope (Section 6.5)
/// - Edge action classification (Sections 6.1–6.4)
/// - [`redact`]: the top-level pipeline that produces a valid redacted
///   [`OmtsFile`] from a higher-trust source file.
///
/// The lower-level functions (`classify_node`, `filter_identifiers`, etc.) are
/// deliberately pure-functional: they take inputs and return outputs without
/// side effects, making them easy to test in isolation.
use std::collections::{HashMap, HashSet};
use std::fmt;

use crate::boundary_hash::{BoundaryHashError, boundary_ref_value, decode_salt};
use crate::canonical::CanonicalId;
use crate::enums::{DisclosureScope, EdgeType, EdgeTypeTag, NodeType, NodeTypeTag, Sensitivity};
use crate::file::OmtsFile;
use crate::newtypes::NodeId;
use crate::sensitivity::{effective_property_sensitivity, effective_sensitivity};
use crate::structures::{Edge, EdgeProperties, Node};
use crate::types::Identifier;
use crate::validation::{ValidationConfig, validate};

// ---------------------------------------------------------------------------
// NodeAction
// ---------------------------------------------------------------------------

/// The disposition assigned to a node during redaction.
///
/// Classification follows the table in redaction.md Section 5 crossed with the
/// target disclosure scope.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NodeAction {
    /// Node appears in output, possibly with filtered identifiers.
    Retain,
    /// Node is replaced with a `boundary_ref` stub.
    ///
    /// This is a producer choice for nodes outside the exported subgraph.
    /// In the context of this module, the classification only determines
    /// *eligibility* for replacement; the caller decides which retained-eligible
    /// nodes to actually replace.
    Replace,
    /// Node is removed entirely. All edges referencing it are also removed.
    Omit,
}

// ---------------------------------------------------------------------------
// EdgeAction
// ---------------------------------------------------------------------------

/// The disposition assigned to an edge during redaction.
///
/// Derived from the actions of the source and target nodes plus the edge type
/// (Sections 6.1–6.4 of the redaction specification).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EdgeAction {
    /// Edge appears in output, possibly with filtered properties.
    Retain,
    /// Edge is removed entirely.
    Omit,
}

// ---------------------------------------------------------------------------
// Node classification (Section 5)
// ---------------------------------------------------------------------------

/// Classifies a node into a [`NodeAction`] based on its type and the target
/// disclosure scope.
///
/// Classification rules (redaction.md Section 5, table):
///
/// | Node Type       | `partner` Scope       | `public` Scope        |
/// |-----------------|------------------------|------------------------|
/// | `organization`  | `Retain` (or `Replace` by producer choice) | same |
/// | `facility`      | `Retain` (or `Replace`) | same |
/// | `good`          | `Retain` (or `Replace`) | same |
/// | `consignment`   | `Retain` (or `Replace`) | same |
/// | `attestation`   | `Retain` (or `Replace`) | same |
/// | `person`        | `Retain` (ids filtered) | `Omit` |
/// | `boundary_ref`  | `Retain` (pass-through) | `Retain` (pass-through) |
/// | Extension types | `Retain` (or `Replace`) | same |
///
/// The `Retain`-vs-`Replace` choice for non-person, non-boundary-ref nodes is
/// a producer decision not made here. This function returns [`NodeAction::Retain`]
/// for all nodes where the producer *may* retain or replace them — the caller
/// applies its own subgraph membership logic to promote `Retain` → `Replace`
/// for nodes outside the exported subgraph.
///
/// # Arguments
///
/// * `node` — the node to classify.
/// * `target_scope` — the disclosure scope being targeted.
pub fn classify_node(node: &Node, target_scope: &DisclosureScope) -> NodeAction {
    match target_scope {
        DisclosureScope::Internal => {
            // No-op path: all nodes are retained as-is.
            NodeAction::Retain
        }
        DisclosureScope::Partner => {
            // Person nodes are retained with filtered identifiers.
            // All other nodes are retained (producer may promote to Replace).
            // boundary_ref nodes pass through.
            NodeAction::Retain
        }
        DisclosureScope::Public => match &node.node_type {
            NodeTypeTag::Known(NodeType::Person) => NodeAction::Omit,
            NodeTypeTag::Known(NodeType::Organization)
            | NodeTypeTag::Known(NodeType::Facility)
            | NodeTypeTag::Known(NodeType::Good)
            | NodeTypeTag::Known(NodeType::Consignment)
            | NodeTypeTag::Known(NodeType::Attestation)
            | NodeTypeTag::Known(NodeType::BoundaryRef)
            | NodeTypeTag::Extension(_) => NodeAction::Retain,
        },
    }
}

// ---------------------------------------------------------------------------
// Identifier filtering (Sections 3.1 and 3.2)
// ---------------------------------------------------------------------------

/// Filters an identifier list to retain only identifiers within the sensitivity
/// threshold of the target scope.
///
/// Rules by scope (redaction.md Section 3):
/// - `internal`: retain all identifiers (no filtering).
/// - `partner`: retain identifiers with effective sensitivity `public` or
///   `restricted`; remove `confidential`.
/// - `public`: retain only identifiers with effective sensitivity `public`;
///   remove `confidential` and `restricted`.
///
/// The person-node override (Section 2.2) is applied automatically via
/// [`effective_sensitivity`]: all person-node identifiers default to
/// `confidential` unless explicitly overridden.
///
/// # Arguments
///
/// * `identifiers` — the identifiers to filter.
/// * `node_type` — the type tag of the owning node (needed for person-node rule).
/// * `target_scope` — the disclosure scope being targeted.
pub fn filter_identifiers(
    identifiers: &[Identifier],
    node_type: &NodeTypeTag,
    target_scope: &DisclosureScope,
) -> Vec<Identifier> {
    identifiers
        .iter()
        .filter(|id| {
            let sensitivity = effective_sensitivity(id, node_type);
            sensitivity_allowed(&sensitivity, target_scope)
        })
        .cloned()
        .collect()
}

/// Returns `true` if a given sensitivity level is allowed through at the
/// target scope.
///
/// | Scope      | Public | Restricted | Confidential |
/// |------------|--------|------------|--------------|
/// | `internal` | yes    | yes        | yes          |
/// | `partner`  | yes    | yes        | no           |
/// | `public`   | yes    | no         | no           |
fn sensitivity_allowed(sensitivity: &Sensitivity, scope: &DisclosureScope) -> bool {
    match scope {
        DisclosureScope::Internal => true,
        DisclosureScope::Partner => match sensitivity {
            Sensitivity::Public | Sensitivity::Restricted => true,
            Sensitivity::Confidential => false,
        },
        DisclosureScope::Public => match sensitivity {
            Sensitivity::Public => true,
            Sensitivity::Restricted | Sensitivity::Confidential => false,
        },
    }
}

// ---------------------------------------------------------------------------
// Edge property filtering (Section 6.5)
// ---------------------------------------------------------------------------

/// Filters edge properties based on the target scope's sensitivity threshold.
///
/// Rules (redaction.md Section 6.5):
/// - `internal`: no filtering.
/// - `partner`: remove properties with effective sensitivity `confidential`;
///   retain `_property_sensitivity` object.
/// - `public`: remove properties with effective sensitivity `confidential` or
///   `restricted`; also remove the `_property_sensitivity` object entirely.
///
/// The named struct fields on [`EdgeProperties`] are each checked individually.
/// Extension fields in `extra` are also filtered.
///
/// # Arguments
///
/// * `edge` — the edge whose properties are being filtered (used for
///   sensitivity lookups via the `_property_sensitivity` override map).
/// * `target_scope` — the disclosure scope being targeted.
pub fn filter_edge_properties(edge: &Edge, target_scope: &DisclosureScope) -> EdgeProperties {
    if matches!(target_scope, DisclosureScope::Internal) {
        return edge.properties.clone();
    }

    let props = &edge.properties;

    // Helper closure: returns true if this named property should be retained.
    let keep = |name: &str| -> bool {
        let s = effective_property_sensitivity(edge, name);
        sensitivity_allowed(&s, target_scope)
    };

    // Filter each named field.
    let percentage = if keep("percentage") {
        props.percentage
    } else {
        None
    };
    let contract_ref = if keep("contract_ref") {
        props.contract_ref.clone()
    } else {
        None
    };
    let annual_value = if keep("annual_value") {
        props.annual_value
    } else {
        None
    };
    let value_currency = if keep("value_currency") {
        props.value_currency.clone()
    } else {
        None
    };
    let volume = if keep("volume") { props.volume } else { None };
    let volume_unit = if keep("volume_unit") {
        props.volume_unit.clone()
    } else {
        None
    };

    // Fields with default Public sensitivity — always kept unless overridden.
    let data_quality = if keep("data_quality") {
        props.data_quality.clone()
    } else {
        None
    };
    let labels = if keep("labels") {
        props.labels.clone()
    } else {
        None
    };
    let valid_from = if keep("valid_from") {
        props.valid_from.clone()
    } else {
        None
    };
    let valid_to = if keep("valid_to") {
        props.valid_to.clone()
    } else {
        None
    };
    let direct = if keep("direct") { props.direct } else { None };
    let control_type = if keep("control_type") {
        props.control_type.clone()
    } else {
        None
    };
    let consolidation_basis = if keep("consolidation_basis") {
        props.consolidation_basis.clone()
    } else {
        None
    };
    let event_type = if keep("event_type") {
        props.event_type.clone()
    } else {
        None
    };
    let effective_date = if keep("effective_date") {
        props.effective_date.clone()
    } else {
        None
    };
    let description = if keep("description") {
        props.description.clone()
    } else {
        None
    };
    let commodity = if keep("commodity") {
        props.commodity.clone()
    } else {
        None
    };
    let tier = if keep("tier") { props.tier } else { None };
    let share_of_buyer_demand = if keep("share_of_buyer_demand") {
        props.share_of_buyer_demand
    } else {
        None
    };
    let service_type = if keep("service_type") {
        props.service_type.clone()
    } else {
        None
    };
    let quantity = if keep("quantity") {
        props.quantity
    } else {
        None
    };
    let unit = if keep("unit") {
        props.unit.clone()
    } else {
        None
    };
    let scope = if keep("scope") {
        props.scope.clone()
    } else {
        None
    };

    // Filter extra fields.
    let mut extra = serde_json::Map::new();
    for (key, value) in &props.extra {
        if key == "_property_sensitivity" {
            // Retained for `partner`, removed for `public` (Section 3.2).
            if matches!(target_scope, DisclosureScope::Partner) {
                extra.insert(key.clone(), value.clone());
            }
            // For `public`, skip this key entirely.
            continue;
        }
        if keep(key.as_str()) {
            extra.insert(key.clone(), value.clone());
        }
    }

    EdgeProperties {
        data_quality,
        labels,
        valid_from,
        valid_to,
        percentage,
        direct,
        control_type,
        consolidation_basis,
        event_type,
        effective_date,
        description,
        commodity,
        contract_ref,
        volume,
        volume_unit,
        annual_value,
        value_currency,
        tier,
        share_of_buyer_demand,
        service_type,
        quantity,
        unit,
        scope,
        extra,
    }
}

// ---------------------------------------------------------------------------
// Edge classification (Sections 6.1–6.4)
// ---------------------------------------------------------------------------

/// Classifies an edge into an [`EdgeAction`] based on the actions of its
/// source and target nodes and the target scope.
///
/// Rules (redaction.md Sections 6.1–6.4):
///
/// 1. **Section 6.4** — In `public` scope, `beneficial_ownership` edges are
///    unconditionally omitted regardless of endpoint disposition.
/// 2. **Section 6.3** — If either endpoint has action `Omit`, the edge is omitted.
/// 3. **Section 6.2** — If both endpoints have action `Replace`, the edge is omitted.
/// 4. **Section 6.1** — If one endpoint is `Retain` and the other is `Replace`
///    (boundary crossing), the edge is retained.
/// 5. If both endpoints are `Retain`, the edge is retained.
///
/// # Arguments
///
/// * `edge` — the edge to classify.
/// * `source_action` — the [`NodeAction`] assigned to the source node.
/// * `target_action` — the [`NodeAction`] assigned to the target node.
/// * `target_scope` — the disclosure scope being targeted.
pub fn classify_edge(
    edge: &Edge,
    source_action: &NodeAction,
    target_action: &NodeAction,
    target_scope: &DisclosureScope,
) -> EdgeAction {
    // Section 6.4: beneficial_ownership edges unconditionally omitted in public scope.
    if matches!(target_scope, DisclosureScope::Public) {
        if let EdgeTypeTag::Known(EdgeType::BeneficialOwnership) = &edge.edge_type {
            return EdgeAction::Omit;
        }
    }

    // Section 6.3: either endpoint omitted → edge omitted.
    if matches!(source_action, NodeAction::Omit) || matches!(target_action, NodeAction::Omit) {
        return EdgeAction::Omit;
    }

    // Section 6.2: both endpoints replaced → edge omitted.
    if matches!(source_action, NodeAction::Replace) && matches!(target_action, NodeAction::Replace)
    {
        return EdgeAction::Omit;
    }

    // Section 6.1: boundary crossing (one Retain, one Replace) → retained.
    // Also covers both-Retain case.
    EdgeAction::Retain
}

// ---------------------------------------------------------------------------
// RedactError
// ---------------------------------------------------------------------------

/// Errors that can occur during redaction.
#[derive(Debug)]
pub enum RedactError {
    /// The file salt could not be decoded or a CSPRNG call failed.
    BoundaryHash(BoundaryHashError),
    /// The redacted output failed L1 validation, indicating a logic error in
    /// the redaction engine.
    InvalidOutput(String),
}

impl fmt::Display for RedactError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BoundaryHash(e) => write!(f, "boundary hash error: {e}"),
            Self::InvalidOutput(msg) => write!(f, "redacted output failed validation: {msg}"),
        }
    }
}

impl std::error::Error for RedactError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::BoundaryHash(e) => Some(e),
            Self::InvalidOutput(_) => None,
        }
    }
}

impl From<BoundaryHashError> for RedactError {
    fn from(e: BoundaryHashError) -> Self {
        Self::BoundaryHash(e)
    }
}

// ---------------------------------------------------------------------------
// redact — top-level pipeline
// ---------------------------------------------------------------------------

/// Redacts an [`OmtsFile`] to the given `scope`, retaining nodes in
/// `retain_ids` and replacing all other non-person nodes with
/// `boundary_ref` stubs.
///
/// # Arguments
///
/// * `file` — the source file to redact (typically `internal` scope).
/// * `scope` — the target disclosure scope (`partner`, `public`, or `internal`).
/// * `retain_ids` — the set of node IDs the producer wants to keep in the
///   output.  Every node whose ID is in this set is classified `Retain` (if
///   it would not be `Omit` at the target scope); every other node whose
///   base classification is `Retain` is promoted to `Replace`.
///
/// # Algorithm
///
/// 1. **Internal scope short-circuit** — returns a clone with `disclosure_scope`
///    set; no filtering.
/// 2. **Decode salt** — decode `file.file_salt` to a 32-byte array.
/// 3. **Classify nodes** — build a `NodeId → NodeAction` map; apply
///    `retain_ids` to promote eligible nodes from `Retain` → `Replace`.
/// 4. **Compute boundary refs** — for each `Replace` node, compute the opaque
///    hash from the node's public identifiers and the decoded salt.  One hash
///    per node, deduplicated.
/// 5. **Build output nodes** — for each input node emit one of:
///    - `Retain`: the node with filtered identifiers (and `name` cleared for
///      `person` nodes with no remaining identifiers).
///    - `Replace`: a minimal `boundary_ref` node with the opaque identifier.
///    - `Omit`: nothing.
/// 6. **Build output edges** — classify each edge using `classify_edge`; for
///    `Retain` edges strip properties per scope threshold.
/// 7. **Assemble output** — copy the header, set `disclosure_scope` to `scope`,
///    preserve `file_salt`.
/// 8. **Post-redaction validation** — run L1 rules; return
///    [`RedactError::InvalidOutput`] if any errors are found.
///
/// # Errors
///
/// - [`RedactError::BoundaryHash`] if salt decoding or CSPRNG fails.
/// - [`RedactError::InvalidOutput`] if the produced file fails L1 validation.
pub fn redact(
    file: &OmtsFile,
    scope: DisclosureScope,
    retain_ids: &HashSet<NodeId>,
) -> Result<OmtsFile, RedactError> {
    // ------------------------------------------------------------------
    // 1. Internal scope: no-op path.
    // ------------------------------------------------------------------
    if matches!(scope, DisclosureScope::Internal) {
        let mut out = file.clone();
        out.disclosure_scope = Some(DisclosureScope::Internal);
        return Ok(out);
    }

    // ------------------------------------------------------------------
    // 2. Decode file salt for boundary hash computation.
    // ------------------------------------------------------------------
    let salt = decode_salt(&file.file_salt)?;

    // ------------------------------------------------------------------
    // 3. Classify every node.
    //
    // Base classification from classify_node:
    //   - Omit: person in public scope.
    //   - Retain: everything else (boundary_ref pass-through, etc.)
    //
    // Then apply producer choice: nodes NOT in retain_ids that have a
    // base classification of Retain are promoted to Replace, UNLESS they
    // are boundary_ref nodes (those pass through unconditionally).
    // ------------------------------------------------------------------
    let mut node_actions: HashMap<NodeId, NodeAction> = HashMap::new();
    for node in &file.nodes {
        let base = classify_node(node, &scope);
        let action = match base {
            NodeAction::Omit => NodeAction::Omit,
            NodeAction::Retain | NodeAction::Replace => {
                // Pass-through boundary_ref nodes regardless of retain_ids;
                // also retain nodes the producer explicitly placed in retain_ids.
                let is_bref = matches!(&node.node_type, NodeTypeTag::Known(NodeType::BoundaryRef));
                if is_bref || retain_ids.contains(&node.id) {
                    NodeAction::Retain
                } else {
                    NodeAction::Replace
                }
            }
        };
        node_actions.insert(node.id.clone(), action);
    }

    // ------------------------------------------------------------------
    // 4. Compute one boundary-ref value per replaced node (deduplicated).
    // ------------------------------------------------------------------
    let mut boundary_ref_values: HashMap<NodeId, String> = HashMap::new();
    for node in &file.nodes {
        let action = node_actions.get(&node.id);
        if !matches!(action, Some(NodeAction::Replace)) {
            continue;
        }
        // Collect public identifiers for the hash.
        let public_ids: Vec<CanonicalId> = node
            .identifiers
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .filter(|id| {
                matches!(
                    effective_sensitivity(id, &node.node_type),
                    Sensitivity::Public
                )
            })
            .map(CanonicalId::from_identifier)
            .collect();

        let hash = boundary_ref_value(&public_ids, &salt)?;
        boundary_ref_values.insert(node.id.clone(), hash);
    }

    // ------------------------------------------------------------------
    // 5. Build output nodes.
    // ------------------------------------------------------------------
    let mut output_nodes: Vec<Node> = Vec::with_capacity(file.nodes.len());
    for node in &file.nodes {
        let Some(action) = node_actions.get(&node.id) else {
            continue;
        };
        match action {
            NodeAction::Omit => {
                // Drop entirely.
            }
            NodeAction::Replace => {
                // Build a minimal boundary_ref stub.
                let opaque_value = match boundary_ref_values.get(&node.id) {
                    Some(v) => v.clone(),
                    None => continue,
                };
                let stub = build_boundary_ref_node(node.id.clone(), opaque_value);
                output_nodes.push(stub);
            }
            NodeAction::Retain => {
                // Emit the node with filtered identifiers.
                let filtered_ids = filter_identifiers(
                    node.identifiers.as_deref().unwrap_or(&[]),
                    &node.node_type,
                    &scope,
                );
                let mut retained = node.clone();
                retained.identifiers = if filtered_ids.is_empty() {
                    // Keep an explicit empty array (not None) to distinguish
                    // "filtered to zero" from "no identifiers field" — but the
                    // spec allows either. Use None to avoid an empty array in the
                    // output (smaller, cleaner JSON).
                    None
                } else {
                    Some(filtered_ids)
                };
                output_nodes.push(retained);
            }
        }
    }

    // ------------------------------------------------------------------
    // 6. Build output edges.
    // ------------------------------------------------------------------
    let mut output_edges: Vec<Edge> = Vec::with_capacity(file.edges.len());
    for edge in &file.edges {
        // Look up source and target actions; default to Omit for unknown nodes.
        let source_action = node_actions.get(&edge.source).unwrap_or(&NodeAction::Omit);
        let target_action = node_actions.get(&edge.target).unwrap_or(&NodeAction::Omit);

        let edge_action = classify_edge(edge, source_action, target_action, &scope);
        if matches!(edge_action, EdgeAction::Omit) {
            continue;
        }

        // Retain: strip properties per scope threshold.
        let mut retained_edge = edge.clone();
        retained_edge.properties = filter_edge_properties(edge, &scope);
        output_edges.push(retained_edge);
    }

    // ------------------------------------------------------------------
    // 7. Assemble output file.
    // ------------------------------------------------------------------
    let output = OmtsFile {
        omtsf_version: file.omtsf_version.clone(),
        snapshot_date: file.snapshot_date.clone(),
        file_salt: file.file_salt.clone(),
        disclosure_scope: Some(scope),
        previous_snapshot_ref: file.previous_snapshot_ref.clone(),
        snapshot_sequence: file.snapshot_sequence,
        reporting_entity: file.reporting_entity.clone(),
        nodes: output_nodes,
        edges: output_edges,
        extra: file.extra.clone(),
    };

    // ------------------------------------------------------------------
    // 8. Post-redaction L1 validation.
    // ------------------------------------------------------------------
    let config = ValidationConfig {
        run_l1: true,
        run_l2: false,
        run_l3: false,
    };
    let result = validate(&output, &config, None);
    if result.has_errors() {
        let messages: Vec<String> = result
            .errors()
            .map(|d| format!("{}: {}", d.rule_id, d.message))
            .collect();
        return Err(RedactError::InvalidOutput(messages.join("; ")));
    }

    Ok(output)
}

// ---------------------------------------------------------------------------
// Helper: build a boundary_ref stub node
// ---------------------------------------------------------------------------

/// Constructs a minimal `boundary_ref` node with a single `opaque` identifier.
///
/// The `id` is preserved from the original node so that existing edge
/// `source`/`target` references remain valid (Section 5.1 of redaction.md).
fn build_boundary_ref_node(id: NodeId, opaque_value: String) -> Node {
    let opaque_id = Identifier {
        scheme: "opaque".to_owned(),
        value: opaque_value,
        authority: None,
        valid_from: None,
        valid_to: None,
        sensitivity: None,
        verification_status: None,
        verification_date: None,
        extra: serde_json::Map::new(),
    };
    Node {
        id,
        node_type: NodeTypeTag::Known(NodeType::BoundaryRef),
        identifiers: Some(vec![opaque_id]),
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use crate::enums::{EdgeType, EdgeTypeTag, NodeType, NodeTypeTag, Sensitivity};
    use crate::newtypes::{EdgeId, NodeId};
    use crate::structures::{Edge, EdgeProperties};
    use crate::types::Identifier;

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn org_node(id: &str) -> Node {
        make_node(id, NodeTypeTag::Known(NodeType::Organization), None)
    }

    fn person_node(id: &str) -> Node {
        make_node(id, NodeTypeTag::Known(NodeType::Person), None)
    }

    fn boundary_ref_node(id: &str) -> Node {
        make_node(id, NodeTypeTag::Known(NodeType::BoundaryRef), None)
    }

    fn make_node(id: &str, node_type: NodeTypeTag, identifiers: Option<Vec<Identifier>>) -> Node {
        Node {
            id: NodeId::try_from(id).expect("valid NodeId"),
            node_type,
            identifiers,
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

    fn make_identifier(scheme: &str, sensitivity: Option<Sensitivity>) -> Identifier {
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

    fn make_edge(edge_type: EdgeType, source: &str, target: &str) -> Edge {
        Edge {
            id: EdgeId::try_from("e-test").expect("valid EdgeId"),
            edge_type: EdgeTypeTag::Known(edge_type),
            source: NodeId::try_from(source).expect("valid NodeId"),
            target: NodeId::try_from(target).expect("valid NodeId"),
            identifiers: None,
            properties: EdgeProperties::default(),
            extra: serde_json::Map::new(),
        }
    }

    fn make_edge_with_properties(
        edge_type: EdgeType,
        source: &str,
        target: &str,
        extra_props: serde_json::Map<String, serde_json::Value>,
    ) -> Edge {
        Edge {
            id: EdgeId::try_from("e-props").expect("valid EdgeId"),
            edge_type: EdgeTypeTag::Known(edge_type),
            source: NodeId::try_from(source).expect("valid NodeId"),
            target: NodeId::try_from(target).expect("valid NodeId"),
            identifiers: None,
            properties: EdgeProperties {
                extra: extra_props,
                ..Default::default()
            },
            extra: serde_json::Map::new(),
        }
    }

    // -----------------------------------------------------------------------
    // classify_node — partner scope
    // -----------------------------------------------------------------------

    #[test]
    fn classify_org_partner_scope_retain() {
        let node = org_node("org-1");
        assert_eq!(
            classify_node(&node, &DisclosureScope::Partner),
            NodeAction::Retain
        );
    }

    #[test]
    fn classify_facility_partner_scope_retain() {
        let node = make_node("fac-1", NodeTypeTag::Known(NodeType::Facility), None);
        assert_eq!(
            classify_node(&node, &DisclosureScope::Partner),
            NodeAction::Retain
        );
    }

    #[test]
    fn classify_good_partner_scope_retain() {
        let node = make_node("good-1", NodeTypeTag::Known(NodeType::Good), None);
        assert_eq!(
            classify_node(&node, &DisclosureScope::Partner),
            NodeAction::Retain
        );
    }

    #[test]
    fn classify_consignment_partner_scope_retain() {
        let node = make_node("cons-1", NodeTypeTag::Known(NodeType::Consignment), None);
        assert_eq!(
            classify_node(&node, &DisclosureScope::Partner),
            NodeAction::Retain
        );
    }

    #[test]
    fn classify_attestation_partner_scope_retain() {
        let node = make_node("attest-1", NodeTypeTag::Known(NodeType::Attestation), None);
        assert_eq!(
            classify_node(&node, &DisclosureScope::Partner),
            NodeAction::Retain
        );
    }

    #[test]
    fn classify_person_partner_scope_retain() {
        // Person nodes are retained in partner scope (identifiers get filtered).
        let node = person_node("person-1");
        assert_eq!(
            classify_node(&node, &DisclosureScope::Partner),
            NodeAction::Retain
        );
    }

    #[test]
    fn classify_boundary_ref_partner_scope_retain() {
        let node = boundary_ref_node("ref-1");
        assert_eq!(
            classify_node(&node, &DisclosureScope::Partner),
            NodeAction::Retain
        );
    }

    #[test]
    fn classify_extension_node_partner_scope_retain() {
        let node = make_node(
            "ext-1",
            NodeTypeTag::Extension("com.example.custom".to_owned()),
            None,
        );
        assert_eq!(
            classify_node(&node, &DisclosureScope::Partner),
            NodeAction::Retain
        );
    }

    // -----------------------------------------------------------------------
    // classify_node — public scope
    // -----------------------------------------------------------------------

    #[test]
    fn classify_org_public_scope_retain() {
        let node = org_node("org-1");
        assert_eq!(
            classify_node(&node, &DisclosureScope::Public),
            NodeAction::Retain
        );
    }

    #[test]
    fn classify_facility_public_scope_retain() {
        let node = make_node("fac-1", NodeTypeTag::Known(NodeType::Facility), None);
        assert_eq!(
            classify_node(&node, &DisclosureScope::Public),
            NodeAction::Retain
        );
    }

    #[test]
    fn classify_good_public_scope_retain() {
        let node = make_node("good-1", NodeTypeTag::Known(NodeType::Good), None);
        assert_eq!(
            classify_node(&node, &DisclosureScope::Public),
            NodeAction::Retain
        );
    }

    #[test]
    fn classify_consignment_public_scope_retain() {
        let node = make_node("cons-1", NodeTypeTag::Known(NodeType::Consignment), None);
        assert_eq!(
            classify_node(&node, &DisclosureScope::Public),
            NodeAction::Retain
        );
    }

    #[test]
    fn classify_attestation_public_scope_retain() {
        let node = make_node("attest-1", NodeTypeTag::Known(NodeType::Attestation), None);
        assert_eq!(
            classify_node(&node, &DisclosureScope::Public),
            NodeAction::Retain
        );
    }

    #[test]
    fn classify_person_public_scope_omit() {
        // Person nodes are OMITTED in public scope.
        let node = person_node("person-1");
        assert_eq!(
            classify_node(&node, &DisclosureScope::Public),
            NodeAction::Omit
        );
    }

    #[test]
    fn classify_boundary_ref_public_scope_retain() {
        let node = boundary_ref_node("ref-1");
        assert_eq!(
            classify_node(&node, &DisclosureScope::Public),
            NodeAction::Retain
        );
    }

    #[test]
    fn classify_extension_node_public_scope_retain() {
        let node = make_node(
            "ext-1",
            NodeTypeTag::Extension("com.acme.custom".to_owned()),
            None,
        );
        assert_eq!(
            classify_node(&node, &DisclosureScope::Public),
            NodeAction::Retain
        );
    }

    // -----------------------------------------------------------------------
    // classify_node — internal scope
    // -----------------------------------------------------------------------

    #[test]
    fn classify_all_node_types_internal_scope_retain() {
        let nodes = vec![
            make_node("org-1", NodeTypeTag::Known(NodeType::Organization), None),
            make_node("fac-1", NodeTypeTag::Known(NodeType::Facility), None),
            make_node("good-1", NodeTypeTag::Known(NodeType::Good), None),
            make_node("person-1", NodeTypeTag::Known(NodeType::Person), None),
            make_node("attest-1", NodeTypeTag::Known(NodeType::Attestation), None),
            make_node("cons-1", NodeTypeTag::Known(NodeType::Consignment), None),
            make_node("ref-1", NodeTypeTag::Known(NodeType::BoundaryRef), None),
        ];
        for node in &nodes {
            assert_eq!(
                classify_node(node, &DisclosureScope::Internal),
                NodeAction::Retain,
                "node {:?} should be Retain in internal scope",
                node.id
            );
        }
    }

    // -----------------------------------------------------------------------
    // filter_identifiers — partner scope
    // -----------------------------------------------------------------------

    #[test]
    fn filter_partner_retains_public_identifiers() {
        let ids = vec![make_identifier("lei", None)]; // lei defaults to Public
        let result = filter_identifiers(
            &ids,
            &NodeTypeTag::Known(NodeType::Organization),
            &DisclosureScope::Partner,
        );
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn filter_partner_retains_restricted_identifiers() {
        let ids = vec![make_identifier("nat-reg", None)]; // nat-reg defaults to Restricted
        let result = filter_identifiers(
            &ids,
            &NodeTypeTag::Known(NodeType::Organization),
            &DisclosureScope::Partner,
        );
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn filter_partner_removes_confidential_identifiers() {
        let ids = vec![make_identifier("lei", Some(Sensitivity::Confidential))];
        let result = filter_identifiers(
            &ids,
            &NodeTypeTag::Known(NodeType::Organization),
            &DisclosureScope::Partner,
        );
        assert!(result.is_empty());
    }

    #[test]
    fn filter_partner_person_node_removes_default_confidential() {
        // Person node identifiers default to confidential → removed in partner scope.
        let ids = vec![make_identifier("lei", None)];
        let result = filter_identifiers(
            &ids,
            &NodeTypeTag::Known(NodeType::Person),
            &DisclosureScope::Partner,
        );
        assert!(result.is_empty());
    }

    #[test]
    fn filter_partner_person_node_retains_explicit_restricted() {
        // Person node: explicit restricted override is permitted.
        let ids = vec![make_identifier("lei", Some(Sensitivity::Restricted))];
        let result = filter_identifiers(
            &ids,
            &NodeTypeTag::Known(NodeType::Person),
            &DisclosureScope::Partner,
        );
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn filter_partner_person_node_removes_explicit_confidential() {
        let ids = vec![make_identifier("lei", Some(Sensitivity::Confidential))];
        let result = filter_identifiers(
            &ids,
            &NodeTypeTag::Known(NodeType::Person),
            &DisclosureScope::Partner,
        );
        assert!(result.is_empty());
    }

    #[test]
    fn filter_partner_mixed_identifiers() {
        let ids = vec![
            make_identifier("lei", None),                            // Public → kept
            make_identifier("nat-reg", None),                        // Restricted → kept
            make_identifier("lei", Some(Sensitivity::Confidential)), // Confidential → removed
        ];
        let result = filter_identifiers(
            &ids,
            &NodeTypeTag::Known(NodeType::Organization),
            &DisclosureScope::Partner,
        );
        assert_eq!(result.len(), 2);
    }

    // -----------------------------------------------------------------------
    // filter_identifiers — public scope
    // -----------------------------------------------------------------------

    #[test]
    fn filter_public_retains_public_identifiers() {
        let ids = vec![make_identifier("lei", None)]; // Public
        let result = filter_identifiers(
            &ids,
            &NodeTypeTag::Known(NodeType::Organization),
            &DisclosureScope::Public,
        );
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn filter_public_removes_restricted_identifiers() {
        let ids = vec![make_identifier("nat-reg", None)]; // Restricted
        let result = filter_identifiers(
            &ids,
            &NodeTypeTag::Known(NodeType::Organization),
            &DisclosureScope::Public,
        );
        assert!(result.is_empty());
    }

    #[test]
    fn filter_public_removes_confidential_identifiers() {
        let ids = vec![make_identifier("lei", Some(Sensitivity::Confidential))];
        let result = filter_identifiers(
            &ids,
            &NodeTypeTag::Known(NodeType::Organization),
            &DisclosureScope::Public,
        );
        assert!(result.is_empty());
    }

    #[test]
    fn filter_public_person_node_removes_all_by_default() {
        // Person node: all identifiers default to confidential → all removed.
        let ids = vec![
            make_identifier("lei", None),
            make_identifier("nat-reg", None),
        ];
        let result = filter_identifiers(
            &ids,
            &NodeTypeTag::Known(NodeType::Person),
            &DisclosureScope::Public,
        );
        assert!(result.is_empty());
    }

    #[test]
    fn filter_public_person_node_removes_explicit_restricted() {
        // Even explicit restricted is removed in public scope.
        let ids = vec![make_identifier("lei", Some(Sensitivity::Restricted))];
        let result = filter_identifiers(
            &ids,
            &NodeTypeTag::Known(NodeType::Person),
            &DisclosureScope::Public,
        );
        assert!(result.is_empty());
    }

    #[test]
    fn filter_public_person_node_retains_explicit_public() {
        // Explicit public override is respected (though validators may flag it).
        let ids = vec![make_identifier("lei", Some(Sensitivity::Public))];
        let result = filter_identifiers(
            &ids,
            &NodeTypeTag::Known(NodeType::Person),
            &DisclosureScope::Public,
        );
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn filter_public_mixed_identifiers() {
        let ids = vec![
            make_identifier("lei", None),                            // Public → kept
            make_identifier("duns", None),                           // Public → kept
            make_identifier("nat-reg", None),                        // Restricted → removed
            make_identifier("vat", None),                            // Restricted → removed
            make_identifier("lei", Some(Sensitivity::Confidential)), // Confidential → removed
        ];
        let result = filter_identifiers(
            &ids,
            &NodeTypeTag::Known(NodeType::Organization),
            &DisclosureScope::Public,
        );
        assert_eq!(result.len(), 2);
    }

    // -----------------------------------------------------------------------
    // filter_identifiers — internal scope
    // -----------------------------------------------------------------------

    #[test]
    fn filter_internal_retains_all_identifiers() {
        let ids = vec![
            make_identifier("lei", None),
            make_identifier("nat-reg", None),
            make_identifier("lei", Some(Sensitivity::Confidential)),
        ];
        let result = filter_identifiers(
            &ids,
            &NodeTypeTag::Known(NodeType::Organization),
            &DisclosureScope::Internal,
        );
        assert_eq!(result.len(), 3);
    }

    // -----------------------------------------------------------------------
    // filter_edge_properties — partner scope
    // -----------------------------------------------------------------------

    #[test]
    fn filter_edge_props_partner_removes_confidential_percentage_on_beneficial_ownership() {
        // percentage on beneficial_ownership defaults to confidential.
        let mut edge = make_edge(EdgeType::BeneficialOwnership, "src", "tgt");
        edge.properties.percentage = Some(25.0);
        let result = filter_edge_properties(&edge, &DisclosureScope::Partner);
        assert!(result.percentage.is_none());
    }

    #[test]
    fn filter_edge_props_partner_retains_percentage_on_ownership() {
        // percentage on ownership defaults to public.
        let mut edge = make_edge(EdgeType::Ownership, "src", "tgt");
        edge.properties.percentage = Some(51.0);
        let result = filter_edge_properties(&edge, &DisclosureScope::Partner);
        assert_eq!(result.percentage, Some(51.0));
    }

    #[test]
    fn filter_edge_props_partner_removes_contract_ref() {
        // contract_ref defaults to restricted — kept in partner scope.
        // Wait — restricted is ALLOWED in partner. Let's re-check.
        // partner: remove confidential, retain restricted and public.
        let mut edge = make_edge(EdgeType::Supplies, "src", "tgt");
        edge.properties.contract_ref = Some("C-001".to_owned());
        let result = filter_edge_properties(&edge, &DisclosureScope::Partner);
        // contract_ref is restricted → kept in partner scope.
        assert_eq!(result.contract_ref.as_deref(), Some("C-001"));
    }

    #[test]
    fn filter_edge_props_partner_retains_volume_unit() {
        // volume_unit defaults to public.
        let mut edge = make_edge(EdgeType::Supplies, "src", "tgt");
        edge.properties.volume_unit = Some("mt".to_owned());
        let result = filter_edge_properties(&edge, &DisclosureScope::Partner);
        assert_eq!(result.volume_unit.as_deref(), Some("mt"));
    }

    #[test]
    fn filter_edge_props_partner_retains_property_sensitivity_map() {
        // _property_sensitivity is retained in partner scope.
        use serde_json::json;
        let mut extra = serde_json::Map::new();
        extra.insert(
            "_property_sensitivity".to_owned(),
            json!({"volume": "public"}),
        );
        let edge = make_edge_with_properties(EdgeType::Supplies, "src", "tgt", extra);
        let result = filter_edge_properties(&edge, &DisclosureScope::Partner);
        assert!(result.extra.contains_key("_property_sensitivity"));
    }

    // -----------------------------------------------------------------------
    // filter_edge_properties — public scope
    // -----------------------------------------------------------------------

    #[test]
    fn filter_edge_props_public_removes_restricted_contract_ref() {
        let mut edge = make_edge(EdgeType::Supplies, "src", "tgt");
        edge.properties.contract_ref = Some("C-001".to_owned());
        let result = filter_edge_properties(&edge, &DisclosureScope::Public);
        assert!(result.contract_ref.is_none());
    }

    #[test]
    fn filter_edge_props_public_removes_restricted_annual_value() {
        let mut edge = make_edge(EdgeType::Supplies, "src", "tgt");
        edge.properties.annual_value = Some(1_000_000.0);
        let result = filter_edge_properties(&edge, &DisclosureScope::Public);
        assert!(result.annual_value.is_none());
    }

    #[test]
    fn filter_edge_props_public_removes_restricted_volume() {
        let mut edge = make_edge(EdgeType::Supplies, "src", "tgt");
        edge.properties.volume = Some(5000.0);
        let result = filter_edge_properties(&edge, &DisclosureScope::Public);
        assert!(result.volume.is_none());
    }

    #[test]
    fn filter_edge_props_public_retains_public_volume_unit() {
        let mut edge = make_edge(EdgeType::Supplies, "src", "tgt");
        edge.properties.volume_unit = Some("mt".to_owned());
        let result = filter_edge_properties(&edge, &DisclosureScope::Public);
        assert_eq!(result.volume_unit.as_deref(), Some("mt"));
    }

    #[test]
    fn filter_edge_props_public_retains_public_percentage_on_ownership() {
        let mut edge = make_edge(EdgeType::Ownership, "src", "tgt");
        edge.properties.percentage = Some(51.0);
        let result = filter_edge_properties(&edge, &DisclosureScope::Public);
        assert_eq!(result.percentage, Some(51.0));
    }

    #[test]
    fn filter_edge_props_public_removes_property_sensitivity_map() {
        // _property_sensitivity is removed entirely in public scope.
        use serde_json::json;
        let mut extra = serde_json::Map::new();
        extra.insert(
            "_property_sensitivity".to_owned(),
            json!({"volume": "public"}),
        );
        let edge = make_edge_with_properties(EdgeType::Supplies, "src", "tgt", extra);
        let result = filter_edge_properties(&edge, &DisclosureScope::Public);
        assert!(!result.extra.contains_key("_property_sensitivity"));
    }

    #[test]
    fn filter_edge_props_public_removes_confidential_percentage_on_beneficial_ownership() {
        let mut edge = make_edge(EdgeType::BeneficialOwnership, "src", "tgt");
        edge.properties.percentage = Some(15.0);
        let result = filter_edge_properties(&edge, &DisclosureScope::Public);
        assert!(result.percentage.is_none());
    }

    // -----------------------------------------------------------------------
    // filter_edge_properties — internal scope
    // -----------------------------------------------------------------------

    #[test]
    fn filter_edge_props_internal_no_change() {
        let mut edge = make_edge(EdgeType::Supplies, "src", "tgt");
        edge.properties.contract_ref = Some("C-001".to_owned());
        edge.properties.volume = Some(5000.0);
        edge.properties.percentage = Some(10.0);
        let result = filter_edge_properties(&edge, &DisclosureScope::Internal);
        assert_eq!(result.contract_ref.as_deref(), Some("C-001"));
        assert_eq!(result.volume, Some(5000.0));
        assert_eq!(result.percentage, Some(10.0));
    }

    // -----------------------------------------------------------------------
    // classify_edge
    // -----------------------------------------------------------------------

    #[test]
    fn classify_edge_both_retain_is_retain() {
        let edge = make_edge(EdgeType::Supplies, "src", "tgt");
        let action = classify_edge(
            &edge,
            &NodeAction::Retain,
            &NodeAction::Retain,
            &DisclosureScope::Partner,
        );
        assert_eq!(action, EdgeAction::Retain);
    }

    #[test]
    fn classify_edge_boundary_crossing_retain_replace_is_retain() {
        let edge = make_edge(EdgeType::Supplies, "src", "tgt");
        let action = classify_edge(
            &edge,
            &NodeAction::Retain,
            &NodeAction::Replace,
            &DisclosureScope::Partner,
        );
        assert_eq!(action, EdgeAction::Retain);
    }

    #[test]
    fn classify_edge_boundary_crossing_replace_retain_is_retain() {
        let edge = make_edge(EdgeType::Supplies, "src", "tgt");
        let action = classify_edge(
            &edge,
            &NodeAction::Replace,
            &NodeAction::Retain,
            &DisclosureScope::Partner,
        );
        assert_eq!(action, EdgeAction::Retain);
    }

    #[test]
    fn classify_edge_both_replace_is_omit() {
        // Both endpoints replaced → edge omitted (Section 6.2).
        let edge = make_edge(EdgeType::Supplies, "src", "tgt");
        let action = classify_edge(
            &edge,
            &NodeAction::Replace,
            &NodeAction::Replace,
            &DisclosureScope::Partner,
        );
        assert_eq!(action, EdgeAction::Omit);
    }

    #[test]
    fn classify_edge_source_omit_is_omit() {
        // Source node omitted → edge omitted (Section 6.3).
        let edge = make_edge(EdgeType::Supplies, "src", "tgt");
        let action = classify_edge(
            &edge,
            &NodeAction::Omit,
            &NodeAction::Retain,
            &DisclosureScope::Public,
        );
        assert_eq!(action, EdgeAction::Omit);
    }

    #[test]
    fn classify_edge_target_omit_is_omit() {
        // Target node omitted → edge omitted (Section 6.3).
        let edge = make_edge(EdgeType::Supplies, "src", "tgt");
        let action = classify_edge(
            &edge,
            &NodeAction::Retain,
            &NodeAction::Omit,
            &DisclosureScope::Public,
        );
        assert_eq!(action, EdgeAction::Omit);
    }

    #[test]
    fn classify_edge_both_omit_is_omit() {
        let edge = make_edge(EdgeType::Supplies, "src", "tgt");
        let action = classify_edge(
            &edge,
            &NodeAction::Omit,
            &NodeAction::Omit,
            &DisclosureScope::Public,
        );
        assert_eq!(action, EdgeAction::Omit);
    }

    #[test]
    fn classify_edge_beneficial_ownership_public_scope_unconditionally_omit() {
        // beneficial_ownership edges omitted in public scope regardless of endpoints (Section 6.4).
        let edge = make_edge(EdgeType::BeneficialOwnership, "src", "tgt");
        let action = classify_edge(
            &edge,
            &NodeAction::Retain,
            &NodeAction::Retain,
            &DisclosureScope::Public,
        );
        assert_eq!(action, EdgeAction::Omit);
    }

    #[test]
    fn classify_edge_beneficial_ownership_partner_scope_not_unconditionally_omit() {
        // beneficial_ownership edges are NOT unconditionally omitted in partner scope.
        let edge = make_edge(EdgeType::BeneficialOwnership, "src", "tgt");
        let action = classify_edge(
            &edge,
            &NodeAction::Retain,
            &NodeAction::Retain,
            &DisclosureScope::Partner,
        );
        assert_eq!(action, EdgeAction::Retain);
    }

    #[test]
    fn classify_edge_beneficial_ownership_public_both_replace_still_omit() {
        // Even if we check both endpoints before Section 6.4 wouldn't matter;
        // Section 6.4 fires first and overrides.
        let edge = make_edge(EdgeType::BeneficialOwnership, "src", "tgt");
        let action = classify_edge(
            &edge,
            &NodeAction::Replace,
            &NodeAction::Replace,
            &DisclosureScope::Public,
        );
        assert_eq!(action, EdgeAction::Omit);
    }

    #[test]
    fn classify_edge_person_target_omit_causes_beneficial_ownership_omit() {
        // When target is a person node (omitted), the beneficial_ownership edge is omitted
        // via Section 6.3 (endpoint omit) as well as Section 6.4.
        let edge = make_edge(EdgeType::BeneficialOwnership, "org-1", "person-1");
        let action = classify_edge(
            &edge,
            &NodeAction::Retain,
            &NodeAction::Omit,
            &DisclosureScope::Public,
        );
        assert_eq!(action, EdgeAction::Omit);
    }

    #[test]
    fn classify_edge_supplies_with_omit_source_in_partner_scope_is_omit() {
        // Even non-person edge, if source is omitted somehow, edge is omitted.
        let edge = make_edge(EdgeType::Supplies, "src", "tgt");
        let action = classify_edge(
            &edge,
            &NodeAction::Omit,
            &NodeAction::Retain,
            &DisclosureScope::Partner,
        );
        assert_eq!(action, EdgeAction::Omit);
    }

    #[test]
    fn classify_edge_internal_scope_beneficial_ownership_retain() {
        // In internal scope, no filtering applies.
        let edge = make_edge(EdgeType::BeneficialOwnership, "src", "tgt");
        let action = classify_edge(
            &edge,
            &NodeAction::Retain,
            &NodeAction::Retain,
            &DisclosureScope::Internal,
        );
        assert_eq!(action, EdgeAction::Retain);
    }

    // -----------------------------------------------------------------------
    // sensitivity_allowed — sanity checks
    // -----------------------------------------------------------------------

    #[test]
    fn sensitivity_allowed_internal_allows_all() {
        assert!(sensitivity_allowed(
            &Sensitivity::Public,
            &DisclosureScope::Internal
        ));
        assert!(sensitivity_allowed(
            &Sensitivity::Restricted,
            &DisclosureScope::Internal
        ));
        assert!(sensitivity_allowed(
            &Sensitivity::Confidential,
            &DisclosureScope::Internal
        ));
    }

    #[test]
    fn sensitivity_allowed_partner_allows_public_restricted() {
        assert!(sensitivity_allowed(
            &Sensitivity::Public,
            &DisclosureScope::Partner
        ));
        assert!(sensitivity_allowed(
            &Sensitivity::Restricted,
            &DisclosureScope::Partner
        ));
        assert!(!sensitivity_allowed(
            &Sensitivity::Confidential,
            &DisclosureScope::Partner
        ));
    }

    #[test]
    fn sensitivity_allowed_public_allows_only_public() {
        assert!(sensitivity_allowed(
            &Sensitivity::Public,
            &DisclosureScope::Public
        ));
        assert!(!sensitivity_allowed(
            &Sensitivity::Restricted,
            &DisclosureScope::Public
        ));
        assert!(!sensitivity_allowed(
            &Sensitivity::Confidential,
            &DisclosureScope::Public
        ));
    }
}
