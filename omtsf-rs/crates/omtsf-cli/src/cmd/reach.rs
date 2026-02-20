//! Implementation of `omtsf reach <file> <node-id>`.
//!
//! Parses an `.omts` file, builds the directed graph, performs a BFS from the
//! given source node, and writes the set of reachable node IDs to stdout.
//!
//! Flags:
//! - `--depth <n>` (optional): maximum traversal depth.
//! - `--direction <d>`: traversal direction (`outgoing` (default), `incoming`,
//!   or `both`).
//!
//! Output (human mode): one node ID per line, sorted for determinism.
//! Output (JSON mode): a JSON object `{"node_ids": [...], "count": N}`.
//!
//! Exit codes: 0 = success, 1 = source node not found, 2 = parse/build failure.
use omtsf_core::OmtsFile;
use omtsf_core::graph::queries::Direction as CoreDirection;
use omtsf_core::graph::{QueryError, build_graph, reachable_from};

use crate::Direction;
use crate::OutputFormat;
use crate::error::CliError;

// ---------------------------------------------------------------------------
// run
// ---------------------------------------------------------------------------

/// Runs the `reach` command.
///
/// Parses `content` as an OMTSF file, builds the graph, finds all nodes
/// reachable from `node_id`, and writes them to stdout in the requested
/// format.
///
/// `depth` is an optional maximum traversal depth (in hops).  When `None`,
/// the traversal is unbounded (limited only by graph size).
///
/// # Errors
///
/// - [`CliError`] exit code 2 if the content cannot be parsed or the graph
///   cannot be built.
/// - [`CliError`] exit code 1 if `node_id` is not found in the graph.
pub fn run(
    content: &str,
    node_id: &str,
    depth: Option<u32>,
    direction: &Direction,
    format: &OutputFormat,
) -> Result<(), CliError> {
    let file: OmtsFile = serde_json::from_str(content).map_err(|e| CliError::IoError {
        source: "<input>".to_owned(),
        detail: format!("JSON parse error: {e}"),
    })?;

    let graph = build_graph(&file).map_err(|e| CliError::GraphBuildError {
        detail: e.to_string(),
    })?;

    let core_direction = to_core_direction(direction);

    let reachable = if let Some(max_depth) = depth {
        reachable_from_bounded(&graph, node_id, max_depth, core_direction)?
    } else {
        reachable_from(&graph, node_id, core_direction, None).map_err(query_error_to_cli)?
    };

    // Collect node IDs from the reachable set, sorted for determinism.
    let mut node_ids: Vec<String> = reachable
        .into_iter()
        .filter_map(|idx| graph.node_weight(idx).map(|w| w.local_id.clone()))
        .collect();
    node_ids.sort();

    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    match format {
        OutputFormat::Human => print_human(&mut out, &node_ids),
        OutputFormat::Json => print_json(&mut out, &node_ids),
    }
    .map_err(|e| CliError::IoError {
        source: "stdout".to_owned(),
        detail: e.to_string(),
    })
}

// ---------------------------------------------------------------------------
// Depth-bounded reachability
// ---------------------------------------------------------------------------

/// Performs a depth-bounded BFS from `start`, returning all reachable
/// [`petgraph::stable_graph::NodeIndex`] values within `max_depth` hops.
///
/// The start node itself is excluded from the result, consistent with the
/// unbounded [`reachable_from`] behaviour.
fn reachable_from_bounded(
    graph: &omtsf_core::graph::OmtsGraph,
    start: &str,
    max_depth: u32,
    direction: CoreDirection,
) -> Result<std::collections::HashSet<petgraph::stable_graph::NodeIndex>, CliError> {
    use std::collections::{HashSet, VecDeque};

    let start_idx = *graph
        .node_index(start)
        .ok_or_else(|| CliError::NodeNotFound {
            node_id: start.to_owned(),
        })?;

    let mut visited: HashSet<petgraph::stable_graph::NodeIndex> = HashSet::new();
    // Queue entries: (node_index, depth_so_far).
    let mut queue: VecDeque<(petgraph::stable_graph::NodeIndex, u32)> = VecDeque::new();

    visited.insert(start_idx);
    queue.push_back((start_idx, 0));

    while let Some((current, depth)) = queue.pop_front() {
        if depth >= max_depth {
            continue;
        }
        let next_depth = depth + 1;
        for neighbour in neighbours(graph, current, direction) {
            if !visited.contains(&neighbour) {
                visited.insert(neighbour);
                queue.push_back((neighbour, next_depth));
            }
        }
    }

    visited.remove(&start_idx);
    Ok(visited)
}

/// Returns the immediate neighbours of `node` in `direction`.
fn neighbours(
    graph: &omtsf_core::graph::OmtsGraph,
    node: petgraph::stable_graph::NodeIndex,
    direction: CoreDirection,
) -> Vec<petgraph::stable_graph::NodeIndex> {
    use petgraph::visit::EdgeRef as _;

    let g = graph.graph();
    let mut result = Vec::new();

    match direction {
        CoreDirection::Forward => {
            for edge_ref in g.edges(node) {
                result.push(edge_ref.target());
            }
        }
        CoreDirection::Backward => {
            for edge_ref in g.edges_directed(node, petgraph::Direction::Incoming) {
                result.push(edge_ref.source());
            }
        }
        CoreDirection::Both => {
            for edge_ref in g.edges(node) {
                result.push(edge_ref.target());
            }
            for edge_ref in g.edges_directed(node, petgraph::Direction::Incoming) {
                result.push(edge_ref.source());
            }
        }
    }

    result
}

// ---------------------------------------------------------------------------
// Output
// ---------------------------------------------------------------------------

/// Writes reachable node IDs in human-readable format (one per line).
fn print_human<W: std::io::Write>(w: &mut W, node_ids: &[String]) -> std::io::Result<()> {
    for id in node_ids {
        writeln!(w, "{id}")?;
    }
    Ok(())
}

/// Writes reachable node IDs as a JSON object.
fn print_json<W: std::io::Write>(w: &mut W, node_ids: &[String]) -> std::io::Result<()> {
    let ids_array: Vec<serde_json::Value> = node_ids
        .iter()
        .map(|s| serde_json::Value::String(s.clone()))
        .collect();

    let mut obj = serde_json::Map::new();
    obj.insert("node_ids".to_owned(), serde_json::Value::Array(ids_array));
    obj.insert(
        "count".to_owned(),
        serde_json::Value::Number(node_ids.len().into()),
    );

    let json = serde_json::to_string_pretty(&serde_json::Value::Object(obj))
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    writeln!(w, "{json}")
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Converts a CLI [`Direction`] to the core [`CoreDirection`].
fn to_core_direction(d: &Direction) -> CoreDirection {
    match d {
        Direction::Outgoing => CoreDirection::Forward,
        Direction::Incoming => CoreDirection::Backward,
        Direction::Both => CoreDirection::Both,
    }
}

/// Converts a [`QueryError`] to the appropriate [`CliError`].
fn query_error_to_cli(e: QueryError) -> CliError {
    match e {
        QueryError::NodeNotFound(id) => CliError::NodeNotFound { node_id: id },
    }
}
