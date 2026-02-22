/// Graph query algorithms: reachability, shortest path, and all-paths enumeration.
///
/// Implements Sections 3 and 4 of the graph-engine technical specification.
/// All functions operate on an [`OmtsGraph`] and return results as sequences
/// of [`NodeIndex`] values.
///
/// # Direction
///
/// Every query accepts a [`Direction`] parameter controlling which edges are
/// followed:
/// - [`Direction::Forward`] — outgoing edges only (downstream traversal).
/// - [`Direction::Backward`] — incoming edges only (upstream traversal).
/// - [`Direction::Both`] — edges in either direction (undirected view).
///
/// # Edge-Type Filtering
///
/// All three query functions accept an optional `edge_filter: Option<&HashSet<EdgeTypeTag>>`.
/// When `Some`, only edges whose `edge_type` is in the set are traversed.
/// When `None`, all edge types are traversed.
use std::collections::{HashMap, HashSet, VecDeque};

use petgraph::stable_graph::NodeIndex;
use petgraph::visit::EdgeRef;

use crate::enums::EdgeTypeTag;
use crate::graph::OmtsGraph;

#[cfg(test)]
mod tests;

/// Controls which edges are followed during graph traversal.
///
/// Used by [`reachable_from`], [`shortest_path`], and [`all_paths`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    /// Follow outgoing edges only — traverse downstream from the start node.
    Forward,
    /// Follow incoming edges only — traverse upstream from the start node.
    Backward,
    /// Follow edges in either direction, treating the graph as undirected.
    Both,
}

/// Errors that can occur during graph queries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryError {
    /// A node ID supplied to a query function does not exist in the graph.
    ///
    /// The contained string is the unknown ID.
    NodeNotFound(String),
    /// A selector-based query matched no nodes or edges in the file.
    ///
    /// Distinct from a query that matches elements but produces an empty
    /// subgraph after expansion. This variant signals that the selector scan
    /// itself found zero matches, which the CLI maps to exit code 1.
    EmptyResult,
}

impl std::fmt::Display for QueryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QueryError::NodeNotFound(id) => write!(f, "node not found: {id:?}"),
            QueryError::EmptyResult => write!(f, "no elements matched the given selectors"),
        }
    }
}

impl std::error::Error for QueryError {}

/// Returns `true` if the edge should be traversed given the optional filter.
///
/// When `filter` is `None`, all edges pass. When `Some`, only edges whose
/// `edge_type` is in the set pass.
fn edge_passes(edge_type: &EdgeTypeTag, filter: Option<&HashSet<EdgeTypeTag>>) -> bool {
    match filter {
        None => true,
        Some(allowed) => allowed.contains(edge_type),
    }
}

/// Fills `buf` with the neighbour [`NodeIndex`] values reachable from `node`
/// in one step, respecting `direction` and `edge_filter`.
///
/// The buffer is cleared before being populated, so callers can reuse a single
/// allocation across many iterations rather than allocating a fresh `Vec` per
/// call.
fn neighbours_into(
    graph: &OmtsGraph,
    node: NodeIndex,
    direction: Direction,
    edge_filter: Option<&HashSet<EdgeTypeTag>>,
    buf: &mut Vec<NodeIndex>,
) {
    buf.clear();
    let g = graph.graph();

    match direction {
        Direction::Forward => {
            for edge_ref in g.edges(node) {
                if edge_passes(&edge_ref.weight().edge_type, edge_filter) {
                    buf.push(edge_ref.target());
                }
            }
        }
        Direction::Backward => {
            for edge_ref in g.edges_directed(node, petgraph::Direction::Incoming) {
                if edge_passes(&edge_ref.weight().edge_type, edge_filter) {
                    buf.push(edge_ref.source());
                }
            }
        }
        Direction::Both => {
            for edge_ref in g.edges(node) {
                if edge_passes(&edge_ref.weight().edge_type, edge_filter) {
                    buf.push(edge_ref.target());
                }
            }
            for edge_ref in g.edges_directed(node, petgraph::Direction::Incoming) {
                if edge_passes(&edge_ref.weight().edge_type, edge_filter) {
                    buf.push(edge_ref.source());
                }
            }
        }
    }
}

/// Returns the set of all nodes reachable from `start` via BFS.
///
/// The start node itself is excluded from the result.
///
/// # Parameters
///
/// - `graph` — the graph to query.
/// - `start` — graph-local node ID of the starting node.
/// - `direction` — which edges to follow (see [`Direction`]).
/// - `edge_filter` — optional set of allowed edge types; `None` traverses all.
///
/// # Errors
///
/// Returns [`QueryError::NodeNotFound`] if `start` does not exist in the graph.
pub fn reachable_from(
    graph: &OmtsGraph,
    start: &str,
    direction: Direction,
    edge_filter: Option<&HashSet<EdgeTypeTag>>,
) -> Result<HashSet<NodeIndex>, QueryError> {
    let start_idx = *graph
        .node_index(start)
        .ok_or_else(|| QueryError::NodeNotFound(start.to_owned()))?;

    let mut visited: HashSet<NodeIndex> = HashSet::new();
    let mut queue: VecDeque<NodeIndex> = VecDeque::new();
    let mut nbuf: Vec<NodeIndex> = Vec::new();

    visited.insert(start_idx);
    queue.push_back(start_idx);

    while let Some(current) = queue.pop_front() {
        neighbours_into(graph, current, direction, edge_filter, &mut nbuf);
        for &neighbour in &nbuf {
            if !visited.contains(&neighbour) {
                visited.insert(neighbour);
                queue.push_back(neighbour);
            }
        }
    }

    visited.remove(&start_idx);

    Ok(visited)
}

/// Returns the shortest path from `from` to `to` as a sequence of node indices.
///
/// Uses BFS, terminating as soon as `to` is first reached. The returned
/// vector is ordered from `from` to `to` inclusive.
///
/// Returns `None` if no path exists between the two nodes.
///
/// # Parameters
///
/// - `graph` — the graph to query.
/// - `from` — graph-local ID of the source node.
/// - `to` — graph-local ID of the destination node.
/// - `direction` — which edges to follow (see [`Direction`]).
/// - `edge_filter` — optional set of allowed edge types; `None` traverses all.
///
/// # Errors
///
/// Returns [`QueryError::NodeNotFound`] if either `from` or `to` does not
/// exist in the graph.
pub fn shortest_path(
    graph: &OmtsGraph,
    from: &str,
    to: &str,
    direction: Direction,
    edge_filter: Option<&HashSet<EdgeTypeTag>>,
) -> Result<Option<Vec<NodeIndex>>, QueryError> {
    let from_idx = *graph
        .node_index(from)
        .ok_or_else(|| QueryError::NodeNotFound(from.to_owned()))?;
    let to_idx = *graph
        .node_index(to)
        .ok_or_else(|| QueryError::NodeNotFound(to.to_owned()))?;

    if from_idx == to_idx {
        return Ok(Some(vec![from_idx]));
    }

    let mut visited: HashSet<NodeIndex> = HashSet::new();
    let mut predecessor: HashMap<NodeIndex, NodeIndex> = HashMap::new();
    let mut queue: VecDeque<NodeIndex> = VecDeque::new();
    let mut nbuf: Vec<NodeIndex> = Vec::new();

    visited.insert(from_idx);
    queue.push_back(from_idx);

    'bfs: while let Some(current) = queue.pop_front() {
        neighbours_into(graph, current, direction, edge_filter, &mut nbuf);
        for &neighbour in &nbuf {
            if !visited.contains(&neighbour) {
                visited.insert(neighbour);
                predecessor.insert(neighbour, current);

                if neighbour == to_idx {
                    break 'bfs;
                }

                queue.push_back(neighbour);
            }
        }
    }

    if !visited.contains(&to_idx) {
        return Ok(None);
    }

    let mut path = Vec::new();
    let mut current = to_idx;
    loop {
        path.push(current);
        if current == from_idx {
            break;
        }
        match predecessor.get(&current) {
            Some(&prev) => {
                current = prev;
            }
            None => {
                // Should never happen: `to_idx` was reached via BFS so there
                // must be an unbroken predecessor chain back to `from_idx`.
                break;
            }
        }
    }
    path.reverse();

    Ok(Some(path))
}

/// Default maximum depth for [`all_paths`] when not otherwise specified.
pub const DEFAULT_MAX_DEPTH: usize = 20;

/// Returns all simple paths from `from` to `to` up to `max_depth` hops.
///
/// Uses recursive backtracking DFS with a single shared path vector and
/// on-path set, avoiding per-neighbour cloning. A "simple path" visits each
/// node at most once. The depth limit bounds the search to prevent
/// combinatorial explosion on dense subgraphs.
///
/// The default depth limit is [`DEFAULT_MAX_DEPTH`] (20 hops).
///
/// # Parameters
///
/// - `graph` — the graph to query.
/// - `from` — graph-local ID of the source node.
/// - `to` — graph-local ID of the destination node.
/// - `max_depth` — maximum number of hops (edges) in any returned path.
/// - `direction` — which edges to follow (see [`Direction`]).
/// - `edge_filter` — optional set of allowed edge types; `None` traverses all.
///
/// # Errors
///
/// Returns [`QueryError::NodeNotFound`] if either `from` or `to` does not
/// exist in the graph.
pub fn all_paths(
    graph: &OmtsGraph,
    from: &str,
    to: &str,
    max_depth: usize,
    direction: Direction,
    edge_filter: Option<&HashSet<EdgeTypeTag>>,
) -> Result<Vec<Vec<NodeIndex>>, QueryError> {
    let from_idx = *graph
        .node_index(from)
        .ok_or_else(|| QueryError::NodeNotFound(from.to_owned()))?;
    let to_idx = *graph
        .node_index(to)
        .ok_or_else(|| QueryError::NodeNotFound(to.to_owned()))?;

    let mut results: Vec<Vec<NodeIndex>> = Vec::new();

    if from_idx == to_idx {
        results.push(vec![from_idx]);
        return Ok(results);
    }

    dfs_paths(
        graph,
        from_idx,
        to_idx,
        max_depth,
        direction,
        edge_filter,
        &mut results,
    );

    Ok(results)
}

/// Immutable traversal context shared across all recursive DFS frames.
struct DfsCtx<'a> {
    graph: &'a OmtsGraph,
    target: NodeIndex,
    depth_limit: usize,
    direction: Direction,
    edge_filter: Option<&'a HashSet<EdgeTypeTag>>,
}

/// Initialises shared state and launches the backtracking DFS from `from` to
/// `target` with a depth ceiling of `depth_limit`.
fn dfs_paths(
    graph: &OmtsGraph,
    from: NodeIndex,
    target: NodeIndex,
    depth_limit: usize,
    direction: Direction,
    edge_filter: Option<&HashSet<EdgeTypeTag>>,
    results: &mut Vec<Vec<NodeIndex>>,
) {
    let ctx = DfsCtx {
        graph,
        target,
        depth_limit,
        direction,
        edge_filter,
    };
    let mut path: Vec<NodeIndex> = vec![from];
    let mut on_path: Vec<bool> = vec![false; graph.graph().node_count()];
    on_path[from.index()] = true;
    let mut nbuf: Vec<NodeIndex> = Vec::new();

    dfs_recurse(&ctx, from, 0, &mut path, &mut on_path, &mut nbuf, results);
}

/// Recursive backtracking DFS step.
///
/// `path` and `on_path` are shared across all recursive calls and restored
/// via push/pop so no allocations occur per neighbour. `nbuf` is a reusable
/// scratch buffer for neighbour lists, eliminating per-call `Vec` allocations.
/// A result clone is made only when a complete path to `target` is found.
fn dfs_recurse(
    ctx: &DfsCtx<'_>,
    current: NodeIndex,
    depth: usize,
    path: &mut Vec<NodeIndex>,
    on_path: &mut Vec<bool>,
    nbuf: &mut Vec<NodeIndex>,
    results: &mut Vec<Vec<NodeIndex>>,
) {
    if current == ctx.target && depth > 0 {
        results.push(path.clone());
        return;
    }

    if depth >= ctx.depth_limit {
        return;
    }

    neighbours_into(ctx.graph, current, ctx.direction, ctx.edge_filter, nbuf);

    // Copy indices out so `nbuf` is free to be reused by the recursive call.
    let neighbours: Vec<NodeIndex> = nbuf.clone();

    for neighbour in neighbours {
        if !on_path[neighbour.index()] {
            path.push(neighbour);
            on_path[neighbour.index()] = true;

            dfs_recurse(ctx, neighbour, depth + 1, path, on_path, nbuf, results);

            path.pop();
            on_path[neighbour.index()] = false;
        }
    }
}
