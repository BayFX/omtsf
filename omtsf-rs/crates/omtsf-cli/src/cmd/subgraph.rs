//! Implementation of `omtsf subgraph <file> <node-id>...`.
//!
//! Parses an `.omts` file, builds the directed graph, extracts the induced
//! subgraph for the specified nodes (optionally expanded by `--expand` hops),
//! and writes a valid `.omts` file to stdout.
//!
//! Flags:
//! - `--expand <n>` (default 0): include neighbours up to `n` hops from the
//!   specified nodes before computing the induced subgraph.
//! - `--to <encoding>` (default json): output encoding (`json` or `cbor`).
//! - `--compress`: wrap serialized output in a zstd frame.
//!
//! Output: a valid `.omts` file written to stdout in the requested encoding.
//! The `--format` flag does not affect this command (the spec requires `.omts`
//! output regardless of `--format`).
//!
//! Exit codes: 0 = success, 1 = one or more node IDs not found,
//! 2 = parse/build failure.
use std::io::Write as _;

use omtsf_core::OmtsFile;
use omtsf_core::graph::queries::Direction as CoreDirection;
use omtsf_core::graph::{QueryError, build_graph, ego_graph, induced_subgraph};
use omtsf_core::newtypes::CalendarDate;

use crate::TargetEncoding;
use crate::cmd::init::today_string;
use crate::error::CliError;

/// Runs the `subgraph` command.
///
/// Builds the graph from the pre-parsed `file` and extracts the induced
/// subgraph for `node_ids`.  When `expand > 0`, the neighbourhood of each
/// listed node (within `expand` hops in both directions) is added to the node
/// set before the induced subgraph is computed.
///
/// The resulting `.omts` file is serialized to stdout using `to` and,
/// optionally, compressed with zstd when `compress` is `true`.
///
/// # Errors
///
/// - [`CliError`] exit code 2 if the graph cannot be built or serialization fails.
/// - [`CliError`] exit code 1 if any node ID is not found in the graph.
pub fn run(
    file: &OmtsFile,
    node_ids: &[String],
    expand: u32,
    to: &TargetEncoding,
    compress: bool,
) -> Result<(), CliError> {
    let graph = build_graph(file).map_err(|e| CliError::GraphBuildError {
        detail: e.to_string(),
    })?;

    let mut subgraph_file = if expand == 0 {
        let id_refs: Vec<&str> = node_ids.iter().map(String::as_str).collect();
        induced_subgraph(&graph, file, &id_refs).map_err(query_error_to_cli)?
    } else {
        compute_expanded_subgraph(&graph, file, node_ids, expand)?
    };

    let today = today_string().map_err(|e| CliError::IoError {
        source: "system clock".to_owned(),
        detail: e,
    })?;
    subgraph_file.snapshot_date =
        CalendarDate::try_from(today.as_str()).map_err(|e| CliError::IoError {
            source: "system clock".to_owned(),
            detail: format!("generated date is invalid: {e}"),
        })?;

    let bytes = serialize(&subgraph_file, to, compress)?;

    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    out.write_all(&bytes).map_err(|e| CliError::IoError {
        source: "stdout".to_owned(),
        detail: e.to_string(),
    })?;

    // Append a trailing newline for uncompressed JSON so the shell prompt
    // appears on a new line.  Binary outputs (CBOR, any compressed payload)
    // must not have an appended newline because that would corrupt the stream.
    let is_text_output = matches!(to, TargetEncoding::Json) && !compress;
    if is_text_output {
        out.write_all(b"\n").map_err(|e| CliError::IoError {
            source: "stdout".to_owned(),
            detail: e.to_string(),
        })?;
    }

    Ok(())
}

/// Serializes `file` to bytes using the requested encoding and optional
/// compression.
///
/// - `--to json` (default): pretty-printed JSON.
/// - `--to cbor`: CBOR with self-describing tag 55799.
/// - `--compress`: wraps the serialized bytes in a zstd frame.
fn serialize(file: &OmtsFile, to: &TargetEncoding, compress: bool) -> Result<Vec<u8>, CliError> {
    match to {
        TargetEncoding::Cbor => omtsf_core::convert(file, omtsf_core::Encoding::Cbor, compress)
            .map_err(|e| CliError::InternalError {
                detail: e.to_string(),
            }),
        TargetEncoding::Json => {
            let json_bytes =
                serde_json::to_vec_pretty(file).map_err(|e| CliError::InternalError {
                    detail: format!("JSON pretty-print failed: {e}"),
                })?;
            if compress {
                omtsf_core::compress_zstd(&json_bytes).map_err(|e| CliError::InternalError {
                    detail: format!("zstd compression failed: {e}"),
                })
            } else {
                Ok(json_bytes)
            }
        }
    }
}

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

    for id in node_ids {
        if graph.node_index(id).is_none() {
            return Err(CliError::NodeNotFound {
                node_id: id.clone(),
            });
        }
    }

    let mut expanded_ids: HashSet<String> = HashSet::new();

    for id in node_ids {
        let ego = ego_graph(graph, file, id, expand as usize, CoreDirection::Both)
            .map_err(query_error_to_cli)?;
        for node in &ego.nodes {
            expanded_ids.insert(node.id.to_string());
        }
    }

    let id_refs: Vec<&str> = expanded_ids.iter().map(String::as_str).collect();
    induced_subgraph(graph, file, &id_refs).map_err(query_error_to_cli)
}

/// Converts a [`QueryError`] to the appropriate [`CliError`].
fn query_error_to_cli(e: QueryError) -> CliError {
    match e {
        QueryError::NodeNotFound(id) => CliError::NodeNotFound { node_id: id },
        QueryError::EmptyResult => CliError::NoResults {
            detail: "no elements matched the given selectors".to_owned(),
        },
    }
}
