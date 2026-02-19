# omtsf-core Technical Specification: Graph Engine

**Status:** Draft
**Date:** 2026-02-19

---

## 1. Purpose

This document specifies the graph construction, representation, and query subsystem within `omtsf-core`. The graph engine converts a deserialized `OmtsFile` into a directed labeled property multigraph backed by `petgraph`, then provides traversal and query algorithms that implement the `reach`, `path`, and `subgraph` CLI commands. It also provides cycle detection used by the validation engine to enforce structural constraints (SPEC-001 Section 9.3, L3-MRG-02).

The engine is a library component. It accepts in-memory data structures and returns in-memory results. It has no I/O dependencies and compiles to `wasm32-unknown-unknown`.

---

## 2. petgraph Wrapper

### 2.1 Graph Type Selection: `StableDiGraph`

The engine uses `petgraph::stable_graph::StableDiGraph<NodeWeight, EdgeWeight>`.

**Why `StableDiGraph` over `DiGraph`:** The standard `DiGraph` invalidates node and edge indices on removal. OMTSF operations that mutate the graph after construction -- redaction (SPEC-004) removes nodes and edges, subgraph extraction produces a filtered copy -- require stable indices. When a node is removed from a `StableDiGraph`, other indices remain valid. This avoids a class of subtle bugs where a stored `NodeIndex` silently refers to the wrong node after a removal operation. The cost is a small per-slot overhead (a generation counter or tombstone marker), which is acceptable given that the graph is constructed once and queried many times.

`GraphMap` was considered and rejected. It provides adjacency-map-backed lookup but does not support multigraphs (multiple edges of the same type between the same node pair), which is a hard requirement of the OMTSF data model.

### 2.2 Weight Types

**Node weight.** Each `petgraph` node carries a `NodeWeight` struct:

```rust
struct NodeWeight {
    /// Graph-local ID from the .omts file (e.g., "org-acme").
    local_id: String,
    /// Parsed node type enum.
    node_type: NodeType,
    /// Index into an external Vec<Node> holding the full deserialized node
    /// with all properties, identifiers, and labels. Keeps the graph struct
    /// cache-friendly by avoiding large inline payloads.
    data_index: usize,
}
```

**Edge weight.** Each `petgraph` edge carries an `EdgeWeight` struct:

```rust
struct EdgeWeight {
    /// Graph-local ID from the .omts file (e.g., "edge-001").
    local_id: String,
    /// Parsed edge type enum.
    edge_type: EdgeType,
    /// Index into an external Vec<Edge> holding the full deserialized edge
    /// with all properties.
    data_index: usize,
}
```

The indirection via `data_index` is deliberate. `petgraph` stores weights inline in its node and edge slab arrays. Keeping the weight small (three fields, roughly 56 bytes on 64-bit) means that BFS and DFS traversals -- which iterate over node/edge slabs sequentially -- stay cache-friendly. The full property data lives in a parallel `Vec` and is accessed only when needed for output serialization or property inspection.

### 2.3 Index Mapping

During construction the engine builds a bidirectional lookup:

- `HashMap<String, NodeIndex>` — maps graph-local ID strings to petgraph `NodeIndex` values. Used to resolve edge `source`/`target` references during construction and to translate user-supplied node IDs in CLI commands.
- The reverse mapping (index to local ID) is stored in the `NodeWeight` itself (`local_id` field).

This map is constructed once during `build_graph` and is immutable thereafter.

### 2.4 Construction from `OmtsFile`

```rust
fn build_graph(file: &OmtsFile) -> Result<OmtsGraph, GraphBuildError>
```

Construction is a two-pass process:

1. **Node pass.** Iterate `file.nodes`. For each node, insert into the `StableDiGraph` with a `NodeWeight`, and record the `local_id -> NodeIndex` mapping. Fail with `GraphBuildError::DuplicateNodeId` if a local ID appears twice (this is also an L1 validation error, but the graph engine must not assume pre-validation).

2. **Edge pass.** Iterate `file.edges`. For each edge, look up `source` and `target` in the ID map. Fail with `GraphBuildError::DanglingEdgeRef` if either is missing. Insert the edge into the `StableDiGraph` with an `EdgeWeight`.

Construction is O(N + E) where N is node count and E is edge count. The `HashMap` lookup is amortized O(1). For the advisory upper bound of 1,000,000 nodes and 5,000,000 edges, construction allocates roughly 56 MB for node weights plus 56 MB for edge weights, plus the `HashMap` overhead. This fits comfortably in memory.

The returned `OmtsGraph` struct bundles the `StableDiGraph`, the ID-to-index map, and references or owned copies of the full node and edge data vectors.

---

## 3. Reachability

Implements the `omtsf reach <file> <node-id>` command.

### 3.1 Algorithm: BFS

The engine uses breadth-first search (BFS) for reachability. Justification:

- BFS visits nodes in order of increasing hop distance, which is a natural and useful ordering for supply chain analysis (tier 1 suppliers before tier 2).
- BFS uses O(V) memory for the visited set and the queue, same as DFS. There is no stack overflow risk from deep recursion, which matters for pathological chain-shaped graphs.
- DFS would be equally correct for the boolean reachability question but provides no distance ordering.

### 3.2 API

```rust
fn reachable_from(
    graph: &OmtsGraph,
    start: &str,
    direction: Direction,
) -> Result<HashSet<NodeIndex>, QueryError>
```

- `start` is a graph-local node ID string. Returns `QueryError::NodeNotFound` if absent.
- `direction` controls traversal:
  - `Direction::Forward` — follow outgoing edges (downstream from the start node).
  - `Direction::Backward` — follow incoming edges (upstream from the start node).
  - `Direction::Both` — follow edges in either direction (treating the graph as undirected). Useful for connected-component queries.
- The returned set includes all reachable nodes but excludes the start node itself.

### 3.3 Edge-Type Filtering

An optional `edge_filter: Option<&HashSet<EdgeType>>` parameter restricts traversal to edges whose type is in the given set. When `None`, all edge types are traversed. This supports queries such as "all organizations reachable via supply relationship edges only" by passing `{Supplies, Subcontracts, Tolls, Distributes, Brokers, SellsTo}`.

---

## 4. Path Finding

Implements the `omtsf path <file> <from> <to>` command.

### 4.1 Shortest Path: BFS

For unweighted shortest path, the engine uses BFS from the source node, terminating when the target is first reached. This yields a shortest path in O(V + E) time. The BFS predecessor map is used to reconstruct the path.

```rust
fn shortest_path(
    graph: &OmtsGraph,
    from: &str,
    to: &str,
    direction: Direction,
) -> Result<Option<Vec<NodeIndex>>, QueryError>
```

Returns `None` if no path exists. Returns `Some(vec)` where the vector is the sequence of node indices from `from` to `to` inclusive.

### 4.2 All Paths with Depth Limit

Enumerating all simple paths between two nodes is NP-hard in general (it is equivalent to counting Hamiltonian paths). The engine provides an all-paths query with a mandatory depth limit to keep runtime bounded.

```rust
fn all_paths(
    graph: &OmtsGraph,
    from: &str,
    to: &str,
    max_depth: usize,
    direction: Direction,
) -> Result<Vec<Vec<NodeIndex>>, QueryError>
```

The implementation uses iterative-deepening DFS with a visited set on the current path (to avoid revisiting nodes within a single path, enforcing simple paths). Time complexity is bounded by O(V^d) where d is `max_depth`, which is acceptable for the small depths (typically 3-8 hops) meaningful in supply chain analysis.

The default depth limit, when not specified by the user, is 20 hops. This is large enough to traverse any realistic supply chain while preventing combinatorial explosion on dense subgraphs.

### 4.3 Edge-Type Filtering

Both `shortest_path` and `all_paths` accept the same optional `edge_filter` parameter as `reachable_from`, constraining traversal to specific relationship types.

---

## 5. Subgraph Extraction

Implements the `omtsf subgraph <file> <node-id>...` command.

### 5.1 Induced Subgraph

Given a set of node IDs, the engine extracts the induced subgraph: the specified nodes and every edge in the original graph whose source and target are both in the node set.

```rust
fn induced_subgraph(
    graph: &OmtsGraph,
    node_ids: &[&str],
) -> Result<OmtsFile, QueryError>
```

Algorithm:

1. Resolve each string ID to a `NodeIndex`. Fail on any unknown ID.
2. Collect the `NodeIndex` values into a `HashSet` for O(1) membership testing.
3. Iterate all edges in the graph. For each edge where both endpoints are in the set, include it in the output.
4. Assemble the result as a valid `OmtsFile` with the original header fields (version, snapshot date, file salt) and the filtered node and edge arrays.

The edge iteration is O(E) which dominates construction. For the advisory limit of 5,000,000 edges this completes in tens of milliseconds.

### 5.2 Ego-Graph (Node + Radius)

The ego-graph is a convenience built on top of reachability and induced subgraph:

```rust
fn ego_graph(
    graph: &OmtsGraph,
    center: &str,
    radius: usize,
    direction: Direction,
) -> Result<OmtsFile, QueryError>
```

Algorithm: run a bounded BFS from the center node, collecting all nodes within `radius` hops. Then extract the induced subgraph of the collected set. This is equivalent to an induced subgraph over the `radius`-neighborhood of the center node.

### 5.3 Output Validity

The output of both extraction functions is a valid `OmtsFile`. Graph-local IDs are preserved from the source file, so edge `source`/`target` references remain correct. The `reporting_entity` header field is retained only if the referenced node is present in the subgraph; otherwise it is omitted.

---

## 6. Cycle Detection

### 6.1 Structural Constraints

SPEC-001 Section 9.3 defines per-edge-type cycle rules:

| Edge-type subgraph | Cycles permitted? |
|---|---|
| Supply relationships (`supplies`, `subcontracts`, `tolls`, `distributes`, `brokers`, `sells_to`) | Yes |
| `ownership` | Yes (cross-holdings) |
| `legal_parentage` | No (must form a forest, L3-MRG-02) |
| `attested_by`, `operates`, `produces`, `composed_of` | Not specified; structurally unlikely |
| `former_identity` | Not specified; semantically acyclic |

The validation engine calls into the graph engine to check the `legal_parentage` constraint and, optionally, to report cycles in other subgraphs as informational findings.

### 6.2 Algorithm

For `legal_parentage` acyclicity, the engine extracts the edge-type-filtered subgraph and runs a topological sort. If the sort fails to consume all nodes (i.e., it produces fewer nodes than the subgraph contains), a cycle exists.

The implementation uses Kahn's algorithm (iterative BFS-based topological sort) rather than DFS-based cycle detection. Kahn's algorithm provides a natural byproduct: the set of nodes with zero in-degree at each iteration, which makes it straightforward to identify exactly which nodes participate in the cycle. When the algorithm stalls (no zero-in-degree nodes remain), the remaining nodes form the strongly connected component(s) containing the cycle.

```rust
fn detect_cycles(
    graph: &OmtsGraph,
    edge_types: &HashSet<EdgeType>,
) -> Vec<Vec<NodeIndex>>
```

Returns an empty vector if acyclic. Otherwise returns one or more cycles as node sequences. The validation engine translates these into diagnostic findings with graph-local IDs and edge types for human-readable reporting.

### 6.3 Reporting

When a cycle is detected in a forbidden subgraph, the diagnostic includes:

- The rule identifier (`L3-MRG-02` for legal parentage).
- The cycle as a sequence of node local IDs connected by edge local IDs.
- The edge type(s) involved.

When cycles are detected in permitted subgraphs (supply relationships, ownership), the engine can optionally report them as informational findings for graph exploration, but they do not constitute validation failures.

---

## 7. Relation to Merge

The graph engine is used during merge for one specific computation: the transitive closure of merge candidates (SPEC-003 Section 4, step 3). The merge engine constructs an ephemeral undirected graph where each node represents an `OmtsFile` node and edges connect merge candidate pairs identified by the identity predicate. Connected components of this graph are the merge groups.

This computation uses union-find (disjoint-set with path compression and union by rank) rather than the petgraph-backed graph, because:

- The identifier overlap graph is dense and short-lived. Building a full `StableDiGraph` for it adds unnecessary overhead.
- Union-find provides amortized near-O(1) per operation and answers "are these two nodes in the same group?" without materializing the full graph.
- After merge groups are determined, the merged graph is constructed from scratch using `build_graph` on the merged `OmtsFile`.

The graph engine's role in merge is therefore limited to post-merge validation (cycle detection on the merged output) rather than the merge computation itself.

---

## 8. Performance Considerations

### 8.1 Allocation Strategy

For graphs approaching the advisory limits (1M nodes, 5M edges), allocation patterns matter. The engine:

- Pre-allocates `StableDiGraph` capacity using `with_capacity(node_count, edge_count)` to avoid incremental reallocation during construction.
- Pre-allocates the ID-to-index `HashMap` with `HashMap::with_capacity(node_count)`.
- Uses `data_index` indirection (Section 2.2) to keep the petgraph slab entries small and cache-line-friendly during traversal.

### 8.2 Traversal Performance

BFS and DFS traversals access the node and edge slab arrays in roughly sequential order (modulo graph topology). The small weight structs (roughly 56 bytes each) mean that several weights fit in a single cache line (64 bytes), reducing cache misses during neighbor iteration.

Edge-type filtering during traversal adds a branch per edge. This is preferable to pre-building per-edge-type subgraphs, which would multiply memory usage by the number of edge types.

### 8.3 WASM Compatibility

All algorithms use stack-allocated or heap-allocated Rust structures. No OS-level threading, no filesystem access, no system calls. The `petgraph` crate compiles cleanly to `wasm32-unknown-unknown`. The `HashMap` uses the standard library's `RandomState` hasher; in WASM builds this falls back to a fixed seed, which is acceptable since the hash map is not exposed to adversarial input (keys are graph-local ID strings controlled by the file author, and file size limits bound the number of entries).
