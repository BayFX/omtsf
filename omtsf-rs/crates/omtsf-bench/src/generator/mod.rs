//! Supply chain graph generator.
//!
//! Produces valid [`OmtsFile`] instances with realistic topology and
//! identifier density for benchmarking.

pub mod edges;
pub mod identifiers;
pub mod nodes;
pub mod topology;

use omtsf_core::OmtsFile;
use rand::SeedableRng;
use rand::rngs::StdRng;

use topology::build_supply_chain;

/// Configuration for the supply chain generator.
#[derive(Debug, Clone)]
pub struct GeneratorConfig {
    /// Seed for the random number generator (deterministic).
    pub seed: u64,
    /// Number of organization nodes.
    pub num_organizations: usize,
    /// Number of facility nodes.
    pub num_facilities: usize,
    /// Number of good nodes.
    pub num_goods: usize,
    /// Number of person nodes.
    pub num_persons: usize,
    /// Number of attestation nodes.
    pub num_attestations: usize,
    /// Number of consignment nodes.
    pub num_consignments: usize,
    /// Depth of the supplier hierarchy (tiers).
    pub supply_chain_depth: usize,
    /// Depth of the ownership tree.
    pub ownership_depth: usize,
    /// Average children per org in supply tree.
    pub branching_factor: usize,
    /// Average identifiers per node (1.0-3.0).
    pub identifier_density: f64,
    /// Average labels per node (1.0-3.0).
    pub label_density: f64,
    /// Fraction of edges with full properties (0.0-1.0).
    pub edge_property_fullness: f64,
    /// Fraction of cross-tier edges (0.0-0.3).
    pub mesh_density: f64,
    /// Whether to inject cycles for cycle detection benchmarks.
    pub inject_cycles: bool,
    /// Number of `boundary_ref` nodes.
    pub num_boundary_refs: usize,
}

/// Predefined size tiers for benchmarking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SizeTier {
    /// ~50 nodes, ~80 edges, ~40KB JSON
    Small,
    /// ~500 nodes, ~1200 edges, ~500KB JSON
    Medium,
    /// ~2000 nodes, ~5000 edges, ~2MB JSON
    Large,
    /// ~5000 nodes, ~15000 edges, ~5MB JSON
    XLarge,
}

impl SizeTier {
    /// Returns the default `GeneratorConfig` for this size tier.
    pub fn config(self, seed: u64) -> GeneratorConfig {
        match self {
            SizeTier::Small => GeneratorConfig {
                seed,
                num_organizations: 22,
                num_facilities: 10,
                num_goods: 8,
                num_persons: 2,
                num_attestations: 5,
                num_consignments: 2,
                supply_chain_depth: 3,
                ownership_depth: 2,
                branching_factor: 3,
                identifier_density: 1.5,
                label_density: 1.5,
                edge_property_fullness: 0.5,
                mesh_density: 0.1,
                inject_cycles: false,
                num_boundary_refs: 1,
            },
            SizeTier::Medium => GeneratorConfig {
                seed,
                num_organizations: 225,
                num_facilities: 100,
                num_goods: 75,
                num_persons: 15,
                num_attestations: 50,
                num_consignments: 25,
                supply_chain_depth: 5,
                ownership_depth: 3,
                branching_factor: 4,
                identifier_density: 2.0,
                label_density: 1.5,
                edge_property_fullness: 0.6,
                mesh_density: 0.15,
                inject_cycles: false,
                num_boundary_refs: 10,
            },
            SizeTier::Large => GeneratorConfig {
                seed,
                num_organizations: 900,
                num_facilities: 400,
                num_goods: 300,
                num_persons: 60,
                num_attestations: 200,
                num_consignments: 100,
                supply_chain_depth: 7,
                ownership_depth: 4,
                branching_factor: 5,
                identifier_density: 2.0,
                label_density: 2.0,
                edge_property_fullness: 0.7,
                mesh_density: 0.15,
                inject_cycles: false,
                num_boundary_refs: 40,
            },
            SizeTier::XLarge => GeneratorConfig {
                seed,
                num_organizations: 2250,
                num_facilities: 1000,
                num_goods: 750,
                num_persons: 150,
                num_attestations: 500,
                num_consignments: 250,
                supply_chain_depth: 8,
                ownership_depth: 5,
                branching_factor: 6,
                identifier_density: 2.5,
                label_density: 2.0,
                edge_property_fullness: 0.8,
                mesh_density: 0.2,
                inject_cycles: false,
                num_boundary_refs: 100,
            },
        }
    }
}

/// Generates a supply chain `OmtsFile` from the given configuration.
///
/// All randomness is deterministic, seeded from `config.seed`.
pub fn generate_supply_chain(config: &GeneratorConfig) -> OmtsFile {
    let mut rng = StdRng::seed_from_u64(config.seed);
    build_supply_chain(config, &mut rng)
}
