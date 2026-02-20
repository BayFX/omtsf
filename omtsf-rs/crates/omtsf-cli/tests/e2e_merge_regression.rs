//! Merge regression tests covering edge-case scenarios from the merge spec.
//!
//! Each test pair is a fixture designed to exercise a specific merge behaviour:
//! disjoint graphs, full overlap, partial overlap, transitive chains, and
//! ANNULLED LEI handling.
#![allow(clippy::expect_used)]

use std::io::Write as _;
use std::path::PathBuf;
use std::process::Command;

/// Path to the compiled `omtsf` binary.
fn omtsf_bin() -> PathBuf {
    let mut path = std::env::current_exe().expect("current exe");
    path.pop();
    if path.ends_with("deps") {
        path.pop();
    }
    path.push("omtsf");
    path
}

/// Path to a shared fixture file.
fn fixture(name: &str) -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("../../tests/fixtures");
    path.push(name);
    path
}

/// Run `omtsf merge` on two fixtures and return the parsed JSON value.
fn run_merge(file_a: &str, file_b: &str) -> serde_json::Value {
    let out = Command::new(omtsf_bin())
        .args([
            "merge",
            fixture(file_a).to_str().expect("path"),
            fixture(file_b).to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf merge");
    assert_eq!(
        out.status.code(),
        Some(0),
        "merge must exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    serde_json::from_str(stdout.trim()).expect("merge output must be valid JSON")
}

/// Write a `serde_json::Value` to a temporary file and validate it with
/// `omtsf validate --level 1`. Returns true if validation exits 0.
fn validate_value(value: &serde_json::Value) -> bool {
    let json_str = serde_json::to_string(value).expect("serialize to JSON");
    let mut tmp = tempfile::NamedTempFile::new().expect("temp file");
    tmp.write_all(json_str.as_bytes()).expect("write JSON");

    let out = Command::new(omtsf_bin())
        .args([
            "validate",
            "--level",
            "1",
            tmp.path().to_str().expect("path"),
        ])
        .output()
        .expect("run omtsf validate");
    out.status.code() == Some(0)
}

// ---------------------------------------------------------------------------
// Regression 1: Disjoint graphs
// ---------------------------------------------------------------------------

/// Merging two files with no overlapping nodes (no shared LEI/DUNS etc.)
/// preserves all nodes from both files in the output.
#[test]
fn merge_disjoint_preserves_all_nodes() {
    let value = run_merge("merge-disjoint-a.omts", "merge-disjoint-b.omts");
    let nodes = value["nodes"].as_array().expect("nodes array");
    // disjoint-a has 2 nodes (org + facility).
    // disjoint-b has 3 nodes (org + facility + good), no shared identifiers.
    assert_eq!(
        nodes.len(),
        5,
        "disjoint merge must retain all 5 nodes; nodes: {nodes:?}"
    );
}

/// Disjoint merge must preserve all edges from both inputs.
#[test]
fn merge_disjoint_preserves_all_edges() {
    let value = run_merge("merge-disjoint-a.omts", "merge-disjoint-b.omts");
    let edges = value["edges"].as_array().expect("edges array");
    // disjoint-a has 1 edge, disjoint-b has 2 edges.
    assert_eq!(
        edges.len(),
        3,
        "disjoint merge must retain all 3 edges; edges: {edges:?}"
    );
}

/// Disjoint merge output passes L1 validation.
#[test]
fn merge_disjoint_output_passes_validate() {
    let value = run_merge("merge-disjoint-a.omts", "merge-disjoint-b.omts");
    assert!(
        validate_value(&value),
        "disjoint merge output must pass L1 validation"
    );
}

// ---------------------------------------------------------------------------
// Regression 2: Full overlap (same LEIs in both files)
// ---------------------------------------------------------------------------

/// Merging two files where all nodes share the same LEIs produces a merged
/// file where each entity group is a single canonical node.
#[test]
fn merge_full_overlap_deduplicates_nodes() {
    let value = run_merge("merge-full-overlap-a.omts", "merge-full-overlap-b.omts");
    let nodes = value["nodes"].as_array().expect("nodes array");
    // Both files have 2 nodes sharing the same 2 LEIs → should reduce to 2 nodes.
    assert_eq!(
        nodes.len(),
        2,
        "full-overlap merge must deduplicate to 2 nodes; nodes: {nodes:?}"
    );
}

/// Full-overlap merged output passes L1 validation.
#[test]
fn merge_full_overlap_output_passes_validate() {
    let value = run_merge("merge-full-overlap-a.omts", "merge-full-overlap-b.omts");
    assert!(
        validate_value(&value),
        "full-overlap merge output must pass L1 validation"
    );
}

/// Merging from two sources, the merged node accumulates all unique identifiers
/// from both inputs (identifier set-union).
#[test]
fn merge_full_overlap_accumulates_identifiers() {
    let value = run_merge("merge-full-overlap-a.omts", "merge-full-overlap-b.omts");
    let nodes = value["nodes"].as_array().expect("nodes array");

    // Find the Epsilon node: in file-a it had only LEI 5493006MHB84DD0ZWV18,
    // in file-b it gained DUNS 555666777. After merge, both should be present.
    let epsilon = nodes
        .iter()
        .find(|n| {
            n["identifiers"]
                .as_array()
                .map(|ids| {
                    ids.iter().any(|id| {
                        id["scheme"].as_str() == Some("lei")
                            && id["value"].as_str() == Some("5493006MHB84DD0ZWV18")
                    })
                })
                .unwrap_or(false)
        })
        .expect("epsilon node (with BIS LEI) must be present after merge");

    let ids = epsilon["identifiers"]
        .as_array()
        .expect("identifiers array");
    let has_lei = ids.iter().any(|id| id["scheme"].as_str() == Some("lei"));
    let has_duns = ids.iter().any(|id| id["scheme"].as_str() == Some("duns"));

    assert!(has_lei, "merged epsilon node must have LEI identifier");
    assert!(
        has_duns,
        "merged epsilon node must have DUNS identifier from file-b"
    );
}

// ---------------------------------------------------------------------------
// Regression 3: Partial overlap
// ---------------------------------------------------------------------------

/// Merging files with partial overlap: some nodes share identifiers, some are
/// unique. The merged output contains one canonical node for each entity group.
#[test]
fn merge_partial_overlap_correct_node_count() {
    let value = run_merge("merge-partial-a.omts", "merge-partial-b.omts");
    let nodes = value["nodes"].as_array().expect("nodes array");
    // partial-a: org-hub (LEI TESTLEIHUBCORPTEST001) + org-spoke-a (DUNS only)
    // partial-b: org-hub-2 (same LEI) + org-spoke-b (different LEI)
    // Hub is shared → 1 merged hub + spoke-a + spoke-b = 3 unique entities.
    assert_eq!(
        nodes.len(),
        3,
        "partial overlap must yield 3 distinct entities; nodes: {nodes:?}"
    );
}

/// Partial-overlap merged output passes L1 validation.
#[test]
fn merge_partial_overlap_output_passes_validate() {
    let value = run_merge("merge-partial-a.omts", "merge-partial-b.omts");
    assert!(
        validate_value(&value),
        "partial-overlap merge output must pass L1 validation"
    );
}

// ---------------------------------------------------------------------------
// Regression 4: Transitive chain
// ---------------------------------------------------------------------------

/// Merging files that form a transitive supply chain produces a connected graph
/// covering all tiers. The shared tier-2 node (by DUNS) is merged.
#[test]
fn merge_transitive_chain_merges_shared_tier() {
    let value = run_merge("merge-transitive-a.omts", "merge-transitive-b.omts");
    let nodes = value["nodes"].as_array().expect("nodes array");
    // transitive-a: org-t1 (LEI) + org-t2 (DUNS 111222333)
    // transitive-b: org-t2-mirror (same DUNS) + org-t3 (different DUNS)
    // org-t2 and org-t2-mirror share DUNS → merged; total = t1 + t2 + t3 = 3.
    assert_eq!(
        nodes.len(),
        3,
        "transitive chain must merge shared tier-2 node; nodes: {nodes:?}"
    );
}

/// Transitive chain merged output passes L1 validation.
#[test]
fn merge_transitive_chain_output_passes_validate() {
    let value = run_merge("merge-transitive-a.omts", "merge-transitive-b.omts");
    assert!(
        validate_value(&value),
        "transitive chain merge output must pass L1 validation"
    );
}

/// The merged transitive chain has edges spanning all three tiers.
#[test]
fn merge_transitive_chain_has_full_supply_chain() {
    let value = run_merge("merge-transitive-a.omts", "merge-transitive-b.omts");
    let edges = value["edges"].as_array().expect("edges array");
    // Each file has one supply edge; after merge, both edges survive.
    assert_eq!(
        edges.len(),
        2,
        "merged transitive chain must have 2 supply edges; edges: {edges:?}"
    );
    // Both edges must be of type "supplies".
    for edge in edges {
        assert_eq!(
            edge["type"].as_str(),
            Some("supplies"),
            "all transitive chain edges must be 'supplies'"
        );
    }
}

// ---------------------------------------------------------------------------
// Regression 5: ANNULLED LEI
// ---------------------------------------------------------------------------

/// Merging files where some nodes carry an ANNULLED LEI. Annulled LEI nodes
/// are not used for identity matching, so each keeps its own identity group.
#[test]
fn merge_annulled_lei_nodes_not_merged_by_lei() {
    let value = run_merge("merge-annulled-lei-a.omts", "merge-annulled-lei-b.omts");
    let nodes = value["nodes"].as_array().expect("nodes array");

    // annulled-a: org-live (LEI 5493006MHB84DD0ZWV18) + org-annulled (annulled LEI)
    // annulled-b: org-live-2 (same LEI + DUNS) + org-annulled-2 (same annulled LEI)
    //
    // org-live and org-live-2 share a valid LEI → merged into 1 node.
    // org-annulled and org-annulled-2 share an ANNULLED LEI.
    // Annulled LEIs are excluded from the identity index. Without other matching
    // identifiers, they form 2 separate groups.
    // Total: 1 (live merged) + 2 (annulled, not merged) = 3 groups.
    assert_eq!(
        nodes.len(),
        3,
        "annulled LEI nodes must not be merged by their annulled LEI; nodes: {nodes:?}"
    );
}

/// ANNULLED LEI merged output passes L1 validation.
#[test]
fn merge_annulled_lei_output_passes_validate() {
    let value = run_merge("merge-annulled-lei-a.omts", "merge-annulled-lei-b.omts");
    assert!(
        validate_value(&value),
        "annulled-LEI merge output must pass L1 validation"
    );
}

/// The live entity (valid LEI) is properly deduplicated and accumulates
/// identifiers from both files (LEI from file-a, DUNS from file-b).
#[test]
fn merge_annulled_lei_live_entity_merged_correctly() {
    let value = run_merge("merge-annulled-lei-a.omts", "merge-annulled-lei-b.omts");
    let nodes = value["nodes"].as_array().expect("nodes array");

    // Find the live entity: it has a valid (non-annulled) LEI and should also
    // carry the DUNS from file-b after merging.
    let live_node = nodes.iter().find(|n| {
        n["identifiers"]
            .as_array()
            .map(|ids| {
                ids.iter().any(|id| {
                    id["scheme"].as_str() == Some("lei")
                        && id["value"].as_str() == Some("5493006MHB84DD0ZWV18")
                        && id.get("entity_status").is_none()
                })
            })
            .unwrap_or(false)
    });

    assert!(
        live_node.is_some(),
        "live entity with valid LEI must exist in merged output"
    );

    let live_ids = live_node
        .expect("live node")
        .get("identifiers")
        .and_then(|v| v.as_array())
        .expect("live node has identifiers");

    let has_duns = live_ids
        .iter()
        .any(|id| id["scheme"].as_str() == Some("duns"));
    assert!(
        has_duns,
        "live entity must carry DUNS from file-b after merge"
    );
}

// ---------------------------------------------------------------------------
// Regression 6: merge output has required merge metadata
// ---------------------------------------------------------------------------

/// Any merged output must carry `merge_metadata` per the spec.
#[test]
fn merge_output_contains_merge_metadata() {
    let value = run_merge("merge-a.omts", "merge-b.omts");
    assert!(
        value.get("merge_metadata").is_some(),
        "merged output must contain merge_metadata"
    );
    let meta = &value["merge_metadata"];
    assert!(
        meta.get("merged_at").is_some() || meta.get("source_files").is_some(),
        "merge_metadata must have at least one provenance field; meta: {meta:?}"
    );
}
