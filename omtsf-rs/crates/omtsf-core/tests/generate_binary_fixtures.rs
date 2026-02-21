//! Generator: write CBOR and zstd-compressed fixture files to `tests/fixtures/`.
//!
//! Run with:
//!   cargo test -p omtsf-core --test `generate_binary_fixtures` -- --ignored
//!
//! The generated files are committed to git and read back by `binary_fixture_tests.rs`.
//! Running this test again after the files exist is idempotent.
#![allow(clippy::expect_used)]

use std::path::PathBuf;

use omtsf_core::{OmtsFile, compress_zstd, encode_cbor};

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .canonicalize()
        .expect("fixtures directory should exist")
}

fn read_fixture_bytes(name: &str) -> Vec<u8> {
    let path = fixtures_dir().join(name);
    std::fs::read(&path).expect("fixture file should be readable")
}

fn parse_fixture(name: &str) -> OmtsFile {
    let bytes = read_fixture_bytes(name);
    serde_json::from_slice(&bytes).expect("fixture should parse as OmtsFile")
}

fn write_fixture(name: &str, data: &[u8]) {
    let path = fixtures_dir().join(name);
    std::fs::write(&path, data).expect("should be able to write fixture file");
}

/// Generate all CBOR and zstd-compressed fixtures from the existing JSON fixtures.
///
/// This test is marked `#[ignore]` so it does not run in the normal test suite.
/// Run it once to produce the binary fixtures, then commit those files.
#[test]
#[ignore]
fn generate_cbor_and_compressed_fixtures() {
    let minimal_json = read_fixture_bytes("minimal.omts");
    let full_json = read_fixture_bytes("full-featured.omts");
    let _ext_json = read_fixture_bytes("extension-types.omts");

    let minimal: OmtsFile = parse_fixture("minimal.omts");
    let full: OmtsFile = parse_fixture("full-featured.omts");
    let ext: OmtsFile = parse_fixture("extension-types.omts");

    let minimal_cbor = encode_cbor(&minimal).expect("encode minimal to CBOR");
    let full_cbor = encode_cbor(&full).expect("encode full-featured to CBOR");
    let ext_cbor = encode_cbor(&ext).expect("encode extension-types to CBOR");

    let minimal_zstd = compress_zstd(&minimal_json).expect("compress minimal JSON");
    let full_zstd = compress_zstd(&full_json).expect("compress full-featured JSON");
    let full_cbor_zstd = compress_zstd(&full_cbor).expect("compress full-featured CBOR");

    write_fixture("minimal.cbor.omts", &minimal_cbor);
    write_fixture("full-featured.cbor.omts", &full_cbor);
    write_fixture("extension-types.cbor.omts", &ext_cbor);
    write_fixture("minimal.zstd.omts", &minimal_zstd);
    write_fixture("full-featured.zstd.omts", &full_zstd);
    write_fixture("full-featured.cbor.zstd.omts", &full_cbor_zstd);

    println!("Generated fixtures:");
    println!("  minimal.cbor.omts ({} bytes)", minimal_cbor.len());
    println!("  full-featured.cbor.omts ({} bytes)", full_cbor.len());
    println!("  extension-types.cbor.omts ({} bytes)", ext_cbor.len());
    println!("  minimal.zstd.omts ({} bytes)", minimal_zstd.len());
    println!("  full-featured.zstd.omts ({} bytes)", full_zstd.len());
    println!(
        "  full-featured.cbor.zstd.omts ({} bytes)",
        full_cbor_zstd.len()
    );
}
