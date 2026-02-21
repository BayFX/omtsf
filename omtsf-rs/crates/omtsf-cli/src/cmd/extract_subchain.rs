//! Implementation of `omtsf extract-subchain <file> [selector flags]`.
//!
//! Parses an `.omts` file, applies selector predicates to identify seed nodes
//! and edges, optionally expands the result by `--expand` hops, then writes
//! the resulting induced subgraph as a valid `.omts` file (pretty-printed JSON)
//! to stdout.
//!
//! Flags:
//! - `--node-type`, `--edge-type`, `--label`, `--identifier`, `--jurisdiction`,
//!   `--name` (repeatable, see [`crate::cmd::selectors`] for parsing rules)
//! - `--expand <N>` (default 1): BFS expansion hops from seed elements.
//!
//! Output: a valid `.omts` JSON file (pretty-printed).  The `--format` flag
//! is ignored; `.omts` output is always produced regardless.
//!
//! Exit codes: 0 = success, 1 = no matches, 2 = parse/build error.
use omtsf_core::OmtsFile;
use omtsf_core::graph::{QueryError, build_graph, selector_subgraph};

use crate::cmd::selectors::build_selector_set;
use crate::error::CliError;

/// Runs the `extract-subchain` command.
///
/// Parses `content` as an OMTSF file, builds a [`SelectorSet`] from the
/// supplied flag vectors, then calls [`selector_subgraph`] to produce the
/// induced subgraph after `expand` BFS hops.
///
/// The resulting `.omts` file is written as pretty-printed JSON to stdout.
///
/// # Errors
///
/// - [`CliError`] exit code 2 if `content` cannot be parsed or the graph
///   cannot be built, or if no selector flags were provided.
/// - [`CliError`] exit code 1 if no nodes or edges match the selectors.
#[allow(clippy::too_many_arguments)]
pub fn run(
    content: &str,
    node_types: &[String],
    edge_types: &[String],
    labels: &[String],
    identifiers: &[String],
    jurisdictions: &[String],
    names: &[String],
    expand: u32,
) -> Result<(), CliError> {
    let file: OmtsFile = serde_json::from_str(content).map_err(|e| CliError::ParseFailed {
        detail: format!("line {}, column {}: {e}", e.line(), e.column()),
    })?;

    let graph = build_graph(&file).map_err(|e| CliError::GraphBuildError {
        detail: e.to_string(),
    })?;

    let selector_set = build_selector_set(
        node_types,
        edge_types,
        labels,
        identifiers,
        jurisdictions,
        names,
    )?;

    let subgraph_file =
        selector_subgraph(&graph, &file, &selector_set, expand as usize).map_err(|e| match e {
            QueryError::EmptyResult => CliError::NoResults {
                detail: "no nodes or edges matched the given selectors".to_owned(),
            },
            QueryError::NodeNotFound(id) => CliError::NodeNotFound { node_id: id },
        })?;

    let output = serde_json::to_string_pretty(&subgraph_file).map_err(|e| CliError::IoError {
        source: "<output>".to_owned(),
        detail: format!("JSON serialize error: {e}"),
    })?;

    println!("{output}");
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    #![allow(clippy::panic)]

    use super::*;
    const SAMPLE_FILE: &str = r#"{
        "omtsf_version": "1.0.0",
        "snapshot_date": "2026-02-19",
        "file_salt": "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef",
        "nodes": [
            {"id": "org-1", "type": "organization", "name": "Acme Corp"},
            {"id": "fac-1", "type": "facility"},
            {"id": "org-2", "type": "organization", "name": "Beta Ltd"}
        ],
        "edges": [
            {"id": "e-1", "type": "supplies", "source": "org-1", "target": "fac-1"},
            {"id": "e-2", "type": "supplies", "source": "fac-1", "target": "org-2"}
        ]
    }"#;

    fn empty() -> Vec<String> {
        vec![]
    }

    fn strs(v: &[&str]) -> Vec<String> {
        v.iter().map(std::string::ToString::to_string).collect()
    }

    /// Selecting organizations with expand=0 produces a valid .omts file.
    #[test]
    fn test_extract_subchain_org_expand_0() {
        let result = run(
            SAMPLE_FILE,
            &strs(&["organization"]),
            &empty(),
            &empty(),
            &empty(),
            &empty(),
            &empty(),
            0,
        );
        assert!(result.is_ok(), "should succeed: {result:?}");
    }

    /// Selecting facilities with expand=1 includes adjacent org nodes.
    #[test]
    fn test_extract_subchain_facility_expand_1() {
        let result = run(
            SAMPLE_FILE,
            &strs(&["facility"]),
            &empty(),
            &empty(),
            &empty(),
            &empty(),
            &empty(),
            1,
        );
        assert!(result.is_ok(), "should succeed with expand=1: {result:?}");
    }

    /// Selecting by edge type succeeds when matching edges exist.
    #[test]
    fn test_extract_subchain_edge_type_supplies() {
        let result = run(
            SAMPLE_FILE,
            &empty(),
            &strs(&["supplies"]),
            &empty(),
            &empty(),
            &empty(),
            &empty(),
            0,
        );
        assert!(
            result.is_ok(),
            "should succeed for supplies edges: {result:?}"
        );
    }

    /// Selecting a type with no matches returns `NoResults` (exit code 1).
    #[test]
    fn test_extract_subchain_no_match_returns_exit_1() {
        let result = run(
            SAMPLE_FILE,
            &strs(&["good"]), // no Good nodes in the fixture
            &empty(),
            &empty(),
            &empty(),
            &empty(),
            &empty(),
            0,
        );
        let err = result.expect_err("no good nodes → NoResults");
        assert_eq!(err.exit_code(), 1);
    }

    /// No selector flags returns `InvalidArgument` (exit code 2).
    #[test]
    fn test_extract_subchain_no_selectors_returns_exit_2() {
        let result = run(
            SAMPLE_FILE,
            &empty(),
            &empty(),
            &empty(),
            &empty(),
            &empty(),
            &empty(),
            1,
        );
        let err = result.expect_err("no selectors → error");
        assert_eq!(err.exit_code(), 2);
    }

    /// Invalid JSON returns `ParseFailed` (exit code 2).
    #[test]
    fn test_extract_subchain_invalid_json_returns_exit_2() {
        let result = run(
            "not valid json",
            &strs(&["organization"]),
            &empty(),
            &empty(),
            &empty(),
            &empty(),
            &empty(),
            1,
        );
        let err = result.expect_err("bad JSON → ParseFailed");
        assert_eq!(err.exit_code(), 2);
    }

    /// The subgraph produced from selector extraction round-trips through serde.
    #[test]
    fn test_extract_subchain_produces_valid_omts() {
        let file: OmtsFile = serde_json::from_str(SAMPLE_FILE).expect("parse");
        let graph = omtsf_core::build_graph(&file).expect("build");

        let selector_set = crate::cmd::selectors::build_selector_set(
            &strs(&["organization"]),
            &[],
            &[],
            &[],
            &[],
            &[],
        )
        .expect("build selector set");

        let subgraph = omtsf_core::graph::selector_subgraph(&graph, &file, &selector_set, 0)
            .expect("extract subgraph");

        let json = serde_json::to_string_pretty(&subgraph).expect("serialize");
        let back: OmtsFile = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.nodes.len(), subgraph.nodes.len());
        assert_eq!(back.edges.len(), subgraph.edges.len());
    }

    /// expand=0 with org selector produces only org nodes (no facility),
    /// since facility is not in the seed and there is no expansion.
    #[test]
    fn test_extract_subchain_expand_0_excludes_non_seeds() {
        let file: OmtsFile = serde_json::from_str(SAMPLE_FILE).expect("parse");
        let graph = omtsf_core::build_graph(&file).expect("build");

        let selector_set = crate::cmd::selectors::build_selector_set(
            &strs(&["organization"]),
            &[],
            &[],
            &[],
            &[],
            &[],
        )
        .expect("build selector set");

        let subgraph = omtsf_core::graph::selector_subgraph(&graph, &file, &selector_set, 0)
            .expect("extract subgraph");

        let ids: Vec<String> = subgraph.nodes.iter().map(|n| n.id.to_string()).collect();
        assert!(ids.contains(&"org-1".to_owned()), "org-1 must be present");
        assert!(ids.contains(&"org-2".to_owned()), "org-2 must be present");
        assert!(
            !ids.contains(&"fac-1".to_owned()),
            "fac-1 must not be in seed-only result"
        );
    }

    /// expand=1 with org selector includes facility (1 hop from orgs).
    #[test]
    fn test_extract_subchain_expand_1_includes_adjacent() {
        let file: OmtsFile = serde_json::from_str(SAMPLE_FILE).expect("parse");
        let graph = omtsf_core::build_graph(&file).expect("build");

        let selector_set = crate::cmd::selectors::build_selector_set(
            &strs(&["organization"]),
            &[],
            &[],
            &[],
            &[],
            &[],
        )
        .expect("build selector set");

        let subgraph = omtsf_core::graph::selector_subgraph(&graph, &file, &selector_set, 1)
            .expect("extract subgraph");

        let ids: Vec<String> = subgraph.nodes.iter().map(|n| n.id.to_string()).collect();
        assert!(
            ids.contains(&"fac-1".to_owned()),
            "fac-1 must be included with expand=1"
        );
        assert_eq!(subgraph.nodes.len(), 3, "all 3 nodes with expand=1");
    }
}
