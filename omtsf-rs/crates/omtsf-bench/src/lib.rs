//! Supply chain graph generator and benchmark utilities for OMTSF.
//!
//! This crate provides deterministic generation of realistic `.omts` files
//! for benchmarking and property-based testing of `omtsf-core`.

use std::path::PathBuf;

pub mod correctness;
pub mod generator;

pub use generator::{GeneratorConfig, SizeTier, generate_supply_chain};

/// Returns the path where the huge-tier JSON fixture is stored on disk.
///
/// The file lives under `target/bench-fixtures/huge.omts.json` so it is
/// automatically gitignored and shared between the generator binary and
/// the benchmark harness.
pub fn huge_fixture_path() -> PathBuf {
    huge_fixtures_dir().join("huge.omts.json")
}

/// Returns the path where the huge-tier CBOR fixture is stored on disk.
///
/// The file lives under `target/bench-fixtures/huge.omts.cbor`.
pub fn huge_cbor_fixture_path() -> PathBuf {
    huge_fixtures_dir().join("huge.omts.cbor")
}

fn huge_fixtures_dir() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .join("..")
        .join("..")
        .join("target")
        .join("bench-fixtures")
}
