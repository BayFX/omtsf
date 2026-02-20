//! Implementation of `omtsf subgraph <file> <node-id>...`.
//!
//! Parses an `.omts` file, builds the directed graph, extracts the induced
//! subgraph for the specified nodes (optionally expanded by `--expand` hops),
//! and writes a valid `.omts` file to stdout.
//!
//! Flags:
//! - `--expand <n>` (default 0): include neighbours up to `n` hops from the
//!   specified nodes before computing the induced subgraph.
//!
//! Output: a valid `.omts` JSON file.  The `--format` flag does not change
//! the output format here (the spec requires `.omts` output regardless of
//! `--format`).
//!
//! Exit codes: 0 = success, 1 = one or more node IDs not found,
//! 2 = parse/build failure.
use omtsf_core::OmtsFile;
use omtsf_core::graph::queries::Direction as CoreDirection;
use omtsf_core::graph::{QueryError, build_graph, ego_graph, induced_subgraph};

use crate::error::CliError;

// ---------------------------------------------------------------------------
// run
// ---------------------------------------------------------------------------

/// Runs the `subgraph` command.
///
/// Parses `content` as an OMTSF file, builds the graph, and extracts the
/// induced subgraph for `node_ids`.  When `expand > 0`, the neighbourhood of
/// each listed node (within `expand` hops in both directions) is added to the
/// node set before the induced subgraph is computed.
///
/// The resulting `.omts` file is written as pretty-printed JSON to stdout.
///
/// # Errors
///
/// - [`CliError`] exit code 2 if the content cannot be parsed or the graph
///   cannot be built.
/// - [`CliError`] exit code 1 if any node ID is not found in the graph.
pub fn run(content: &str, node_ids: &[String], expand: u32) -> Result<(), CliError> {
    let file: OmtsFile = serde_json::from_str(content).map_err(|e| CliError::IoError {
        source: "<input>".to_owned(),
        detail: format!("JSON parse error: {e}"),
    })?;

    let graph = build_graph(&file).map_err(|e| CliError::GraphBuildError {
        detail: e.to_string(),
    })?;

    let subgraph_file = if expand == 0 {
        // No expansion: plain induced subgraph.
        let id_refs: Vec<&str> = node_ids.iter().map(String::as_str).collect();
        induced_subgraph(&graph, &file, &id_refs).map_err(query_error_to_cli)?
    } else {
        // Expansion: compute the union of ego-graphs for each listed node,
        // then take the induced subgraph of that union.
        compute_expanded_subgraph(&graph, &file, node_ids, expand)?
    };

    let output = serde_json::to_string_pretty(&subgraph_file).map_err(|e| CliError::IoError {
        source: "<output>".to_owned(),
        detail: format!("JSON serialize error: {e}"),
    })?;

    println!("{output}");
    Ok(())
}

// ---------------------------------------------------------------------------
// Expanded subgraph helper
// ---------------------------------------------------------------------------

/// Computes the induced subgraph after expanding each node in `node_ids` by
/// `expand` hops in both directions.
///
/// Algorithm:
/// 1. For each node in `node_ids`, compute the ego-graph with radius `expand`
///    and direction `Both`.
/// 2. Union all ego-graph node sets.
/// 3. Extract the induced subgraph of the union.
fn compute_expanded_subgraph(
    graph: &omtsf_core::graph::OmtsGraph,
    file: &OmtsFile,
    node_ids: &[String],
    expand: u32,
) -> Result<OmtsFile, CliError> {
    use std::collections::HashSet;

    // Validate that all requested nodes exist before beginning expansion.
    for id in node_ids {
        if graph.node_index(id).is_none() {
            return Err(CliError::NodeNotFound {
                node_id: id.clone(),
            });
        }
    }

    // Collect the union of all expanded node IDs.
    let mut expanded_ids: HashSet<String> = HashSet::new();

    for id in node_ids {
        let ego = ego_graph(graph, file, id, expand as usize, CoreDirection::Both)
            .map_err(query_error_to_cli)?;
        for node in &ego.nodes {
            expanded_ids.insert(node.id.to_string());
        }
    }

    // Extract the induced subgraph of the union.
    let id_refs: Vec<&str> = expanded_ids.iter().map(String::as_str).collect();
    induced_subgraph(graph, file, &id_refs).map_err(query_error_to_cli)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Converts a [`QueryError`] to the appropriate [`CliError`].
fn query_error_to_cli(e: QueryError) -> CliError {
    match e {
        QueryError::NodeNotFound(id) => CliError::NodeNotFound { node_id: id },
    }
}
