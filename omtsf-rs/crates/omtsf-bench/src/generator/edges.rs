//! Edge builders for all OMTSF edge types.

use std::collections::BTreeMap;

use omtsf_core::enums::{EdgeType, EdgeTypeTag, ServiceType};
use omtsf_core::newtypes::{EdgeId, NodeId};
use omtsf_core::structures::{Edge, EdgeProperties};
use rand::Rng;
use rand::rngs::StdRng;

use super::nodes;

fn edge_id(prefix: &str, index: usize) -> EdgeId {
    let s = format!("{prefix}-{index:06}");
    EdgeId::try_from(s.as_str()).unwrap_or_else(|_| unreachable!())
}

/// Builds a supplies edge.
pub fn build_supplies(
    rng: &mut StdRng,
    index: usize,
    source: &NodeId,
    target: &NodeId,
    fullness: f64,
) -> Edge {
    let mut props = EdgeProperties::default();
    if rng.gen_bool(fullness) {
        props.commodity = Some("raw-materials".to_owned());
        props.tier = Some(rng.gen_range(1..=8));
        props.volume = Some(rng.gen_range(100.0..100000.0));
        props.volume_unit = Some("tonne".to_owned());
        props.annual_value = Some(rng.gen_range(10000.0..10000000.0));
        props.value_currency = Some("USD".to_owned());
        props.share_of_buyer_demand = Some(rng.gen_range(0.01..1.0));
    }

    Edge {
        id: edge_id("e-sup", index),
        edge_type: EdgeTypeTag::Known(EdgeType::Supplies),
        source: source.clone(),
        target: target.clone(),
        identifiers: None,
        properties: props,
        extra: BTreeMap::new(),
    }
}

/// Builds an operates edge (facility → org operator link).
pub fn build_operates(index: usize, source: &NodeId, target: &NodeId) -> Edge {
    Edge {
        id: edge_id("e-ops", index),
        edge_type: EdgeTypeTag::Known(EdgeType::Operates),
        source: source.clone(),
        target: target.clone(),
        identifiers: None,
        properties: EdgeProperties::default(),
        extra: BTreeMap::new(),
    }
}

/// Builds an ownership edge.
pub fn build_ownership(
    rng: &mut StdRng,
    index: usize,
    source: &NodeId,
    target: &NodeId,
    fullness: f64,
) -> Edge {
    let mut props = EdgeProperties::default();
    if rng.gen_bool(fullness) {
        props.percentage = Some(rng.gen_range(1.0..100.0));
        props.direct = Some(rng.gen_bool(0.7));
    }

    Edge {
        id: edge_id("e-own", index),
        edge_type: EdgeTypeTag::Known(EdgeType::Ownership),
        source: source.clone(),
        target: target.clone(),
        identifiers: None,
        properties: props,
        extra: BTreeMap::new(),
    }
}

/// Builds a `beneficial_ownership` edge.
pub fn build_beneficial_ownership(
    rng: &mut StdRng,
    index: usize,
    source: &NodeId,
    target: &NodeId,
    fullness: f64,
) -> Edge {
    let mut props = EdgeProperties::default();
    if rng.gen_bool(fullness) {
        props.percentage = Some(rng.gen_range(1.0..50.0));
        props.direct = Some(rng.gen_bool(0.5));
    }

    Edge {
        id: edge_id("e-bo", index),
        edge_type: EdgeTypeTag::Known(EdgeType::BeneficialOwnership),
        source: source.clone(),
        target: target.clone(),
        identifiers: None,
        properties: props,
        extra: BTreeMap::new(),
    }
}

/// Builds a `legal_parentage` edge.
pub fn build_legal_parentage(index: usize, source: &NodeId, target: &NodeId) -> Edge {
    Edge {
        id: edge_id("e-lp", index),
        edge_type: EdgeTypeTag::Known(EdgeType::LegalParentage),
        source: source.clone(),
        target: target.clone(),
        identifiers: None,
        properties: EdgeProperties::default(),
        extra: BTreeMap::new(),
    }
}

/// Builds a produces edge (facility → good).
pub fn build_produces(index: usize, source: &NodeId, target: &NodeId) -> Edge {
    Edge {
        id: edge_id("e-prod", index),
        edge_type: EdgeTypeTag::Known(EdgeType::Produces),
        source: source.clone(),
        target: target.clone(),
        identifiers: None,
        properties: EdgeProperties::default(),
        extra: BTreeMap::new(),
    }
}

/// Builds an `attested_by` edge.
pub fn build_attested_by(
    rng: &mut StdRng,
    index: usize,
    source: &NodeId,
    target: &NodeId,
    fullness: f64,
) -> Edge {
    let mut props = EdgeProperties::default();
    if rng.gen_bool(fullness) {
        props.scope = Some("full-facility".to_owned());
    }

    Edge {
        id: edge_id("e-att", index),
        edge_type: EdgeTypeTag::Known(EdgeType::AttestedBy),
        source: source.clone(),
        target: target.clone(),
        identifiers: None,
        properties: props,
        extra: BTreeMap::new(),
    }
}

/// Builds a `composed_of` edge (good → consignment).
pub fn build_composed_of(
    rng: &mut StdRng,
    index: usize,
    source: &NodeId,
    target: &NodeId,
    fullness: f64,
) -> Edge {
    let mut props = EdgeProperties::default();
    if rng.gen_bool(fullness) {
        props.quantity = Some(rng.gen_range(1.0..1000.0));
        props.unit = Some("kg".to_owned());
    }

    Edge {
        id: edge_id("e-comp", index),
        edge_type: EdgeTypeTag::Known(EdgeType::ComposedOf),
        source: source.clone(),
        target: target.clone(),
        identifiers: None,
        properties: props,
        extra: BTreeMap::new(),
    }
}

/// Builds a `sells_to` edge.
pub fn build_sells_to(
    rng: &mut StdRng,
    index: usize,
    source: &NodeId,
    target: &NodeId,
    fullness: f64,
) -> Edge {
    let mut props = EdgeProperties::default();
    if rng.gen_bool(fullness) {
        props.commodity = Some("finished-goods".to_owned());
        props.annual_value = Some(rng.gen_range(10000.0..5000000.0));
        props.value_currency = Some("EUR".to_owned());
    }

    Edge {
        id: edge_id("e-sell", index),
        edge_type: EdgeTypeTag::Known(EdgeType::SellsTo),
        source: source.clone(),
        target: target.clone(),
        identifiers: None,
        properties: props,
        extra: BTreeMap::new(),
    }
}

/// Builds a subcontracts edge.
pub fn build_subcontracts(
    rng: &mut StdRng,
    index: usize,
    source: &NodeId,
    target: &NodeId,
    fullness: f64,
) -> Edge {
    let mut props = EdgeProperties::default();
    if rng.gen_bool(fullness) {
        props.commodity = Some("processing".to_owned());
        props.contract_ref = Some(format!("SC-{}", rng.gen_range(10000..99999)));
    }

    Edge {
        id: edge_id("e-sub", index),
        edge_type: EdgeTypeTag::Known(EdgeType::Subcontracts),
        source: source.clone(),
        target: target.clone(),
        identifiers: None,
        properties: props,
        extra: BTreeMap::new(),
    }
}

/// Builds a distributes edge.
pub fn build_distributes(
    rng: &mut StdRng,
    index: usize,
    source: &NodeId,
    target: &NodeId,
    fullness: f64,
) -> Edge {
    let mut props = EdgeProperties::default();
    if rng.gen_bool(fullness) {
        let services = [
            ServiceType::Transport,
            ServiceType::Warehousing,
            ServiceType::Fulfillment,
        ];
        props.service_type = Some(services[rng.gen_range(0..services.len())].clone());
    }

    Edge {
        id: edge_id("e-dist", index),
        edge_type: EdgeTypeTag::Known(EdgeType::Distributes),
        source: source.clone(),
        target: target.clone(),
        identifiers: None,
        properties: props,
        extra: BTreeMap::new(),
    }
}

/// Builds a brokers edge.
pub fn build_brokers(
    rng: &mut StdRng,
    index: usize,
    source: &NodeId,
    target: &NodeId,
    fullness: f64,
) -> Edge {
    let mut props = EdgeProperties::default();
    if rng.gen_bool(fullness) {
        props.commodity = Some("intermediation".to_owned());
    }

    Edge {
        id: edge_id("e-brk", index),
        edge_type: EdgeTypeTag::Known(EdgeType::Brokers),
        source: source.clone(),
        target: target.clone(),
        identifiers: None,
        properties: props,
        extra: BTreeMap::new(),
    }
}

/// Builds a `former_identity` edge.
pub fn build_former_identity(index: usize, source: &NodeId, target: &NodeId) -> Edge {
    Edge {
        id: edge_id("e-fid", index),
        edge_type: EdgeTypeTag::Known(EdgeType::FormerIdentity),
        source: source.clone(),
        target: target.clone(),
        identifiers: None,
        properties: EdgeProperties::default(),
        extra: BTreeMap::new(),
    }
}

/// Helper: pick a random pair of organization indices for mesh edges.
pub fn pick_org_pair(rng: &mut StdRng, num_orgs: usize) -> (NodeId, NodeId) {
    let a = rng.gen_range(0..num_orgs);
    let mut b = rng.gen_range(0..num_orgs);
    while b == a {
        b = rng.gen_range(0..num_orgs);
    }
    (nodes::org_node_id(a), nodes::org_node_id(b))
}
