//! Implementation of `omtsf path <file> <from> <to>`.
//!
//! Parses an `.omts` file, builds the directed graph, finds paths from `from`
//! to `to`, and writes them to stdout.
//!
//! Flags:
//! - `--max-paths <n>` (default 10): maximum number of paths to report.
//! - `--max-depth <n>` (default 20): maximum path length in edges.
//!
//! Output (human mode): each path on one line with node IDs separated by
//! ` -> `, shortest first.
//! Output (JSON mode): a JSON object `{"paths": [[...], ...], "count": N}`,
//! with paths ordered shortest-first.
//!
//! Exit codes: 0 = at least one path found, 1 = no path / node not found,
//! 2 = parse/build failure.
use omtsf_core::OmtsFile;
use omtsf_core::graph::queries::Direction as CoreDirection;
use omtsf_core::graph::{QueryError, all_paths, build_graph};

use crate::OutputFormat;
use crate::error::CliError;

/// Runs the `path` command.
///
/// Builds the graph from the pre-parsed `file` and finds up to `max_paths`
/// paths from `from` to `to` with a maximum length of `max_depth` edges.
/// Paths are ordered shortest-first.
///
/// # Errors
///
/// - [`CliError`] exit code 2 if the graph cannot be built.
/// - [`CliError`] exit code 1 if either node ID is not found, or no path
///   exists.
pub fn run(
    file: &OmtsFile,
    from: &str,
    to: &str,
    max_paths: usize,
    max_depth: u32,
    format: &OutputFormat,
) -> Result<(), CliError> {
    let graph = build_graph(file).map_err(|e| CliError::GraphBuildError {
        detail: e.to_string(),
    })?;

    let mut raw_paths = all_paths(
        &graph,
        from,
        to,
        max_depth as usize,
        CoreDirection::Forward,
        None,
    )
    .map_err(query_error_to_cli)?;

    if raw_paths.is_empty() {
        return Err(CliError::NoResults {
            detail: format!("no path from {from:?} to {to:?}"),
        });
    }

    raw_paths.sort_by(|a, b| {
        a.len().cmp(&b.len()).then_with(|| {
            let ids_a: Vec<&str> = a
                .iter()
                .filter_map(|&idx| graph.node_weight(idx).map(|w| w.local_id.as_str()))
                .collect();
            let ids_b: Vec<&str> = b
                .iter()
                .filter_map(|&idx| graph.node_weight(idx).map(|w| w.local_id.as_str()))
                .collect();
            ids_a.cmp(&ids_b)
        })
    });

    let paths: Vec<Vec<String>> = raw_paths
        .iter()
        .take(max_paths)
        .map(|path| {
            path.iter()
                .filter_map(|&idx| graph.node_weight(idx).map(|w| w.local_id.clone()))
                .collect()
        })
        .collect();

    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    match format {
        OutputFormat::Human => print_human(&mut out, &paths),
        OutputFormat::Json => print_json(&mut out, &paths),
    }
    .map_err(|e| CliError::IoError {
        source: "stdout".to_owned(),
        detail: e.to_string(),
    })
}

/// Writes paths in human-readable format: each path on one line, node IDs
/// separated by ` -> `.
fn print_human<W: std::io::Write>(w: &mut W, paths: &[Vec<String>]) -> std::io::Result<()> {
    for path in paths {
        writeln!(w, "{}", path.join(" -> "))?;
    }
    Ok(())
}

/// Writes paths as a JSON object `{"paths": [[...], ...], "count": N}`.
fn print_json<W: std::io::Write>(w: &mut W, paths: &[Vec<String>]) -> std::io::Result<()> {
    let paths_array: Vec<serde_json::Value> = paths
        .iter()
        .map(|path| {
            serde_json::Value::Array(
                path.iter()
                    .map(|id| serde_json::Value::String(id.clone()))
                    .collect(),
            )
        })
        .collect();

    let mut obj = serde_json::Map::new();
    obj.insert("paths".to_owned(), serde_json::Value::Array(paths_array));
    obj.insert(
        "count".to_owned(),
        serde_json::Value::Number(paths.len().into()),
    );

    let json = serde_json::to_string_pretty(&serde_json::Value::Object(obj))
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    writeln!(w, "{json}")
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
