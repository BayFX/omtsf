//! Verification tests for committed binary (CBOR and zstd-compressed) fixture files.
//!
//! Each test reads a binary fixture from `tests/fixtures/` via `include_bytes!`,
//! parses it with [`omtsf_core::parse_omts`], and asserts that the result is
//! logically equivalent to the corresponding JSON fixture.
//!
//! These tests cover SPEC-007 Sections 2, 4, 5, and 6 (encoding detection, CBOR
//! decoding, cross-encoding equivalence, and zstd decompression).
#![allow(clippy::expect_used)]

use omtsf_core::{Encoding, OmtsFile, parse_omts};

const MAX_DECOMPRESSED: usize = 16 * 1024 * 1024;

fn parse_json_fixture(bytes: &[u8]) -> OmtsFile {
    serde_json::from_slice(bytes).expect("JSON fixture should parse as OmtsFile")
}

// ── CBOR fixtures ────────────────────────────────────────────────────────────

/// `minimal.cbor.omts` decodes to the same model as `minimal.omts`.
#[test]
fn cbor_minimal_matches_json() {
    let json_bytes = include_bytes!("../../../tests/fixtures/minimal.omts");
    let cbor_bytes = include_bytes!("../../../tests/fixtures/minimal.cbor.omts");

    let json_file = parse_json_fixture(json_bytes);
    let (cbor_file, encoding) = parse_omts(cbor_bytes, MAX_DECOMPRESSED).expect("parse CBOR");

    assert_eq!(encoding, Encoding::Cbor, "fixture must be detected as CBOR");
    assert_eq!(
        json_file, cbor_file,
        "CBOR fixture must be logically equivalent to JSON fixture"
    );
}

/// `full-featured.cbor.omts` decodes to the same model as `full-featured.omts`.
#[test]
fn cbor_full_featured_matches_json() {
    let json_bytes = include_bytes!("../../../tests/fixtures/full-featured.omts");
    let cbor_bytes = include_bytes!("../../../tests/fixtures/full-featured.cbor.omts");

    let json_file = parse_json_fixture(json_bytes);
    let (cbor_file, encoding) = parse_omts(cbor_bytes, MAX_DECOMPRESSED).expect("parse CBOR");

    assert_eq!(encoding, Encoding::Cbor, "fixture must be detected as CBOR");
    assert_eq!(
        json_file, cbor_file,
        "CBOR fixture must be logically equivalent to JSON fixture"
    );
}

/// `extension-types.cbor.omts` decodes to the same model as `extension-types.omts`.
#[test]
fn cbor_extension_types_matches_json() {
    let json_bytes = include_bytes!("../../../tests/fixtures/extension-types.omts");
    let cbor_bytes = include_bytes!("../../../tests/fixtures/extension-types.cbor.omts");

    let json_file = parse_json_fixture(json_bytes);
    let (cbor_file, encoding) = parse_omts(cbor_bytes, MAX_DECOMPRESSED).expect("parse CBOR");

    assert_eq!(encoding, Encoding::Cbor, "fixture must be detected as CBOR");
    assert_eq!(
        json_file, cbor_file,
        "CBOR fixture must be logically equivalent to JSON fixture"
    );
}

// ── zstd-compressed JSON fixtures ───────────────────────────────────────────

/// `minimal.zstd.omts` decompresses and parses to the same model as `minimal.omts`.
#[test]
fn zstd_json_minimal_matches_json() {
    let json_bytes = include_bytes!("../../../tests/fixtures/minimal.omts");
    let zstd_bytes = include_bytes!("../../../tests/fixtures/minimal.zstd.omts");

    let json_file = parse_json_fixture(json_bytes);
    let (zstd_file, encoding) = parse_omts(zstd_bytes, MAX_DECOMPRESSED).expect("parse zstd");

    assert_eq!(
        encoding,
        Encoding::Json,
        "decompressed payload must be detected as JSON"
    );
    assert_eq!(
        json_file, zstd_file,
        "zstd+JSON fixture must be logically equivalent to plain JSON fixture"
    );
}

/// `full-featured.zstd.omts` decompresses and parses to the same model as `full-featured.omts`.
#[test]
fn zstd_json_full_featured_matches_json() {
    let json_bytes = include_bytes!("../../../tests/fixtures/full-featured.omts");
    let zstd_bytes = include_bytes!("../../../tests/fixtures/full-featured.zstd.omts");

    let json_file = parse_json_fixture(json_bytes);
    let (zstd_file, encoding) = parse_omts(zstd_bytes, MAX_DECOMPRESSED).expect("parse zstd");

    assert_eq!(
        encoding,
        Encoding::Json,
        "decompressed payload must be detected as JSON"
    );
    assert_eq!(
        json_file, zstd_file,
        "zstd+JSON fixture must be logically equivalent to plain JSON fixture"
    );
}

// ── zstd-compressed CBOR fixture ────────────────────────────────────────────

/// `full-featured.cbor.zstd.omts` decompresses and parses to the same model as `full-featured.omts`.
#[test]
fn zstd_cbor_full_featured_matches_json() {
    let json_bytes = include_bytes!("../../../tests/fixtures/full-featured.omts");
    let zstd_cbor_bytes = include_bytes!("../../../tests/fixtures/full-featured.cbor.zstd.omts");

    let json_file = parse_json_fixture(json_bytes);
    let (zstd_cbor_file, encoding) =
        parse_omts(zstd_cbor_bytes, MAX_DECOMPRESSED).expect("parse zstd+CBOR");

    assert_eq!(
        encoding,
        Encoding::Cbor,
        "decompressed payload must be detected as CBOR"
    );
    assert_eq!(
        json_file, zstd_cbor_file,
        "zstd+CBOR fixture must be logically equivalent to plain JSON fixture"
    );
}

// ── structural checks ────────────────────────────────────────────────────────

/// CBOR `minimal` fixture starts with the CBOR self-describing tag (SPEC-007 §4.1).
#[test]
fn cbor_minimal_starts_with_self_describing_tag() {
    let cbor_bytes = include_bytes!("../../../tests/fixtures/minimal.cbor.omts");
    assert_eq!(
        &cbor_bytes[..3],
        &[0xD9, 0xD9, 0xF7],
        "CBOR fixture must start with self-describing tag 55799"
    );
}

/// zstd fixtures start with the zstd magic bytes (SPEC-007 §6.3).
#[test]
fn zstd_fixtures_start_with_zstd_magic() {
    let minimal_zstd = include_bytes!("../../../tests/fixtures/minimal.zstd.omts");
    let full_zstd = include_bytes!("../../../tests/fixtures/full-featured.zstd.omts");
    let full_cbor_zstd = include_bytes!("../../../tests/fixtures/full-featured.cbor.zstd.omts");

    for (name, bytes) in [
        ("minimal.zstd.omts", minimal_zstd.as_ref()),
        ("full-featured.zstd.omts", full_zstd.as_ref()),
        ("full-featured.cbor.zstd.omts", full_cbor_zstd.as_ref()),
    ] {
        assert_eq!(
            &bytes[..4],
            &[0x28, 0xB5, 0x2F, 0xFD],
            "{name}: zstd fixture must start with zstd magic bytes"
        );
    }
}

/// CBOR full-featured fixture preserves the node and edge counts from the JSON fixture.
#[test]
fn cbor_full_featured_preserves_counts() {
    let json_bytes = include_bytes!("../../../tests/fixtures/full-featured.omts");
    let cbor_bytes = include_bytes!("../../../tests/fixtures/full-featured.cbor.omts");

    let json_file = parse_json_fixture(json_bytes);
    let (cbor_file, _) = parse_omts(cbor_bytes, MAX_DECOMPRESSED).expect("parse CBOR");

    assert_eq!(
        json_file.nodes.len(),
        cbor_file.nodes.len(),
        "node count must match"
    );
    assert_eq!(
        json_file.edges.len(),
        cbor_file.edges.len(),
        "edge count must match"
    );
}

/// CBOR extension-types fixture preserves extension node and edge types.
#[test]
fn cbor_extension_types_preserves_extension_type_tags() {
    use omtsf_core::enums::{EdgeTypeTag, NodeTypeTag};

    let cbor_bytes = include_bytes!("../../../tests/fixtures/extension-types.cbor.omts");
    let (cbor_file, _) = parse_omts(cbor_bytes, MAX_DECOMPRESSED).expect("parse CBOR");

    let ext_node_count = cbor_file
        .nodes
        .iter()
        .filter(|n| matches!(&n.node_type, NodeTypeTag::Extension(_)))
        .count();
    assert!(
        ext_node_count >= 2,
        "CBOR extension-types fixture must contain at least 2 extension-typed nodes, got {ext_node_count}"
    );

    let ext_edge_count = cbor_file
        .edges
        .iter()
        .filter(|e| matches!(&e.edge_type, EdgeTypeTag::Extension(_)))
        .count();
    assert!(
        ext_edge_count >= 2,
        "CBOR extension-types fixture must contain at least 2 extension-typed edges, got {ext_edge_count}"
    );
}
