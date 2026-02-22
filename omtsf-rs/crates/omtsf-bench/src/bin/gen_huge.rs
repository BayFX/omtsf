//! Generates the huge-tier benchmark fixtures to disk.
//!
//! Run via `just gen-huge`. Writes both JSON and CBOR fixtures to
//! `target/bench-fixtures/` and they are loaded by `benches/huge_file.rs`
//! at benchmark time.

use std::error::Error;
use std::fs;
use std::io::BufWriter;

use omtsf_bench::{SizeTier, generate_supply_chain, huge_cbor_fixture_path, huge_fixture_path};
use omtsf_core::cbor::encode_cbor;

fn main() -> Result<(), Box<dyn Error>> {
    let json_path = huge_fixture_path();
    let cbor_path = huge_cbor_fixture_path();

    if let Some(parent) = json_path.parent() {
        fs::create_dir_all(parent)?;
    }

    eprintln!("Generating Huge tier (~737K nodes)...");
    let file = generate_supply_chain(&SizeTier::Huge.config(42));

    let node_count = file.nodes.len();
    let edge_count = file.edges.len();
    eprintln!("Generated {node_count} nodes, {edge_count} edges");

    eprintln!("Writing JSON to {}...", json_path.display());
    let out = fs::File::create(&json_path)?;
    let writer = BufWriter::new(out);
    serde_json::to_writer(writer, &file)?;

    let json_meta = fs::metadata(&json_path)?;
    eprintln!("JSON: {:.1} MB", json_meta.len() as f64 / (1024.0 * 1024.0));

    eprintln!("Writing CBOR to {}...", cbor_path.display());
    let cbor_bytes = encode_cbor(&file)?;
    fs::write(&cbor_path, &cbor_bytes)?;

    eprintln!(
        "CBOR: {:.1} MB ({:.0}% of JSON)",
        cbor_bytes.len() as f64 / (1024.0 * 1024.0),
        cbor_bytes.len() as f64 / json_meta.len() as f64 * 100.0
    );

    Ok(())
}
