# omtsf-core Technical Specification: Graph Engine

**Status:** Draft
**Date:** 2026-02-21

---

## 1. Purpose

This document specifies the graph construction, query, and cycle-detection subsystem within `omtsf-core`. The engine converts a deserialized `OmtsFile` into a directed labeled property multigraph backed by `petgraph`, then provides traversal algorithms that implement the `reach`, `path`, `subgraph`, `query`, and `extract-subchain` CLI commands and the cycle detection used by validation rule L3-MRG-02.

The engine is a pure library component with no I/O dependencies. It compiles to `wasm32-unknown-unknown`.

---

## 2. petgraph Wrapper

### 2.1 Graph Type Selection: `StableDiGraph`

The engine uses `petgraph::stable_graph::StableDiGraph<NodeWeight, EdgeWeight>`.

**Why `StableDiGraph` over `DiGraph`:** `DiGraph` invalidates node and edge indices on removal. OMTSF operations that mutate the graph after construction -- redaction removes nodes, subgraph extraction filters copies -- require stable indices. `StableDiGraph` uses tombstones to preserve index validity at the cost of a small per-slot overhead, acceptable since the graph is constructed once and queried many times.

**Why not `GraphMap`:** `GraphMap` does not support multigraphs (multiple edges of the same type between the same node pair), which is a hard requirement (SPEC-001 Section 3.2).

### 2.2 Weight Types

```rust
#[derive(Debug, Clone)]
pub struct NodeWeight {
    pub local_id: String,
    pub node_type: NodeTypeTag,
    pub data_index: usize,
}

#[derive(Debug, Clone)]
pub struct EdgeWeight {
    pub local_id: String,
    pub edge_type: EdgeTypeTag,
    pub data_index: usize,
}
```

The `data_index` indirection keeps weights small (~56 bytes on 64-bit) for cache-friendly BFS/DFS traversal. Full property data lives in the parallel `OmtsFile::nodes`/`OmtsFile::edges` vectors and is accessed only for output serialization.

`NodeTypeTag` covers the 7 known node types (`organization`, `facility`, `good`, `person`, `attestation`, `consignment`, `boundary_ref`) plus extension strings. `EdgeTypeTag` covers the 16 known edge types (`ownership`, `operational_control`, `legal_parentage`, `former_identity`, `beneficial_ownership`, `supplies`, `subcontracts`, `tolls`, `distributes`, `brokers`, `operates`, `produces`, `composed_of`, `sells_to`, `attested_by`, `same_as`) plus extensions.

### 2.3 Index Mapping and Type Indexes

Construction builds four lookup structures:

- `id_to_index: HashMap<String, NodeIndex>` -- resolves graph-local ID strings to petgraph indices. Used for edge endpoint resolution and CLI node-ID lookups.
- `nodes_by_type: HashMap<NodeTypeTag, Vec<NodeIndex>>` -- O(1) access to all nodes of a given type. Enables fast-path selector-based extraction.
- `edges_by_type: HashMap<EdgeTypeTag, Vec<EdgeIndex>>` -- O(1) access to all edges of a given type. Used by selector extraction and cycle detection.
- The reverse mapping (index to ID) lives in `NodeWeight::local_id`.

All maps are immutable after construction.

### 2.4 Construction from `OmtsFile`

```rust
pub fn build_graph(file: &OmtsFile) -> Result<OmtsGraph, GraphBuildError>
```

Two-pass O(N + E) construction:

1. **Node pass.** Insert each node with a `NodeWeight`, record in `id_to_index` and `nodes_by_type`. Fail on duplicate IDs (`GraphBuildError::DuplicateNodeId`).
2. **Edge pass.** Resolve `source`/`target` via `id_to_index`, insert edge with `EdgeWeight`, record in `edges_by_type`. Fail on dangling references (`GraphBuildError::DanglingEdgeRef`).

Both `StableDiGraph` and `HashMap` are pre-allocated with `with_capacity` to avoid reallocation.

```rust
#[derive(Debug)]
pub struct OmtsGraph {
    graph: StableDiGraph<NodeWeight, EdgeWeight>,
    id_to_index: HashMap<String, NodeIndex>,
    nodes_by_type: HashMap<NodeTypeTag, Vec<NodeIndex>>,
    edges_by_type: HashMap<EdgeTypeTag, Vec<EdgeIndex>>,
}

impl OmtsGraph {
    pub fn node_count(&self) -> usize;
    pub fn edge_count(&self) -> usize;
    pub fn node_index(&self, id: &str) -> Option<&NodeIndex>;
    pub fn node_weight(&self, idx: NodeIndex) -> Option<&NodeWeight>;
    pub fn edge_weight(&self, idx: EdgeIndex) -> Option<&EdgeWeight>;
    pub fn graph(&self) -> &StableDiGraph<NodeWeight, EdgeWeight>;
    pub fn nodes_of_type(&self, t: &NodeTypeTag) -> &[NodeIndex];
    pub fn edges_of_type(&self, t: &EdgeTypeTag) -> &[EdgeIndex];
}
```

---

## 3. Reachability

### 3.1 Algorithm Choice: BFS

The engine uses breadth-first search for reachability. BFS visits nodes in order of increasing hop distance, which is the natural ordering for supply chain analysis (tier 1 suppliers before tier 2). BFS uses O(V) memory for the visited set and the queue, identical to DFS, but avoids stack overflow risk from deep recursion on pathological chain-shaped graphs at the advisory limit of 1M nodes. DFS would be equally correct for the boolean reachability question but provides no distance ordering.

### 3.2 API

```rust
pub fn reachable_from(
    graph: &OmtsGraph,
    start: &str,
    direction: Direction,
    edge_filter: Option<&HashSet<EdgeTypeTag>>,
) -> Result<HashSet<NodeIndex>, QueryError>
```

- `start` is a graph-local node ID string. Returns `QueryError::NodeNotFound` if absent.
- `direction` controls which edges are followed during traversal:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    Forward,   // outgoing edges (downstream)
    Backward,  // incoming edges (upstream)
    Both,      // undirected view, useful for connected-component queries
}
```

The returned set includes all reachable nodes but excludes the start node itself. The optional `edge_filter` restricts traversal to edges whose type is in the given set; `None` traverses all types. This supports queries such as "all organizations reachable via supply relationship edges only." Filtering is evaluated per-edge during traversal rather than by materializing per-type subgraphs, which would multiply memory by the number of edge types.

---

## 4. Path Finding

### 4.1 Shortest Path: BFS

For unweighted shortest path, the engine uses BFS from the source node, terminating when the target is first reached. This yields a shortest path in O(V + E) time. A predecessor map (`HashMap<NodeIndex, NodeIndex>`) is maintained during BFS and used to reconstruct the path by walking backwards from `to` to `from`.

```rust
pub fn shortest_path(
    graph: &OmtsGraph,
    from: &str,
    to: &str,
    direction: Direction,
    edge_filter: Option<&HashSet<EdgeTypeTag>>,
) -> Result<Option<Vec<NodeIndex>>, QueryError>
```

Returns `None` if no path exists. `Some(vec![from_idx])` when `from == to`.

### 4.2 All Paths with Depth Limit

Iterative-deepening DFS with explicit stack (no recursion). Each frame carries the current node, depth, path, and visited-on-path set for simple-path enforcement. Paths are deduplicated via `HashSet<Vec<NodeIndex>>`.

```rust
pub const DEFAULT_MAX_DEPTH: usize = 20;

pub fn all_paths(
    graph: &OmtsGraph,
    from: &str,
    to: &str,
    max_depth: usize,
    direction: Direction,
    edge_filter: Option<&HashSet<EdgeTypeTag>>,
) -> Result<Vec<Vec<NodeIndex>>, QueryError>
```

Complexity is O(V^d) bounded by `max_depth`. The default of 20 hops covers any realistic supply chain.

---

## 5. Subgraph Extraction

### 5.1 Induced Subgraph

```rust
pub fn induced_subgraph(
    graph: &OmtsGraph,
    file: &OmtsFile,
    node_ids: &[&str],
) -> Result<OmtsFile, QueryError>
```

Algorithm:

1. Resolve each string ID to a `NodeIndex`. Fail with `QueryError::NodeNotFound` on any unknown ID.
2. Collect the `NodeIndex` values into a `HashSet` for O(1) membership testing.
3. Iterate outgoing edges of each included node via `g.edges(node_idx)`. For each edge where the target is also in the set, record its `data_index`. This is O(K * D) where K is included nodes and D is average out-degree, rather than O(E_total) -- a significant improvement when extracting small subgraphs from large graphs.
4. Assemble a valid `OmtsFile` with original header fields preserved and nodes emitted in file order for deterministic output. The `reporting_entity` header is set to `None` if the referenced node is absent (preventing L1-GDM-05 violation).

The output round-trips cleanly through serde: `serde_json::to_string` followed by `serde_json::from_str` produces an identical `OmtsFile`.

### 5.2 Ego-Graph

```rust
pub fn ego_graph(
    graph: &OmtsGraph,
    file: &OmtsFile,
    center: &str,
    radius: usize,
    direction: Direction,
) -> Result<OmtsFile, QueryError>
```

Bounded BFS from center with `VecDeque<(NodeIndex, usize)>` tracking hop count. Collects all nodes within `radius` hops, then delegates to the shared `assemble_subgraph` function.

### 5.3 Selector-Based Extraction

```rust
pub fn selector_subgraph(
    graph: &OmtsGraph,
    file: &OmtsFile,
    selectors: &SelectorSet,
    expand: usize,
) -> Result<OmtsFile, QueryError>
```

The algorithm runs four sequential phases:

1. **Seed scan.** Identify matching nodes and edges. When the only active node selectors are type filters, the `nodes_by_type` index is used (O(matched) instead of O(N)). Otherwise, a linear scan evaluates `SelectorSet::matches_node` per element. Edge matching follows the same pattern with `edges_by_type`. Complexity: O((N + E) * S) worst case, where S is the selector count.

2. **Seed edge resolution.** For each seed edge, add its source and target to the seed node set. This ensures matched edges contribute their endpoints to the BFS frontier.

3. **BFS expansion.** Starting from seed nodes, perform bounded BFS for `expand` hops treating the graph as undirected (`Direction::Both`). This captures both upstream and downstream neighbors of seed elements.

4. **Induced subgraph assembly.** Delegate to the shared `assemble_subgraph` function. The expanded node set defines the induced subgraph.

For display-only queries (`omtsf query`), `selector_match` performs only the seed scan and returns file-vector indices without building the graph, making it faster when no subgraph output is needed.

---

## 6. Cycle Detection

### 6.1 Constraints

| Edge-type subgraph | Cycles? | Rationale |
|---|---|---|
| `supplies`, `subcontracts`, `tolls`, `distributes`, `brokers`, `sells_to` | Yes | Recycling loops, circular trade |
| `ownership`, `beneficial_ownership` | Yes | Cross-holdings |
| `legal_parentage` | **No** (L3-MRG-02) | A subsidiary cannot be its own parent |
| Other types | Not specified | |

### 6.2 Algorithm: Kahn's Topological Sort

```rust
pub fn detect_cycles(
    graph: &OmtsGraph,
    edge_types: &HashSet<EdgeTypeTag>,
) -> Vec<Vec<NodeIndex>>
```

Kahn's algorithm computes in-degree per node for the filtered edge set, seeds a queue with zero-in-degree nodes, and peels them off while decrementing successor in-degrees. If unvisited nodes remain, they participate in cycles.

Individual cycles are extracted from the cyclic-node set via iterative DFS with `globally_visited` tracking. The `filtered_successors` helper restricts neighbor enumeration to cyclic nodes and matching edge types. Each returned cycle is closed: first and last node are identical (e.g., `[A, B, C, A]`).

Kahn's was chosen over DFS-based detection because the residual node set (those not consumed by topological sort) naturally identifies all cycle participants, simplifying extraction.

### 6.3 Reporting

The validation engine translates `Vec<NodeIndex>` cycles into diagnostics with rule ID (`L3-MRG-02`), node local IDs, and edge types. Cycles in permitted subgraphs are optionally reported as informational findings.

---

## 7. Relation to Merge

The merge engine uses union-find (disjoint-set with path-halving and union-by-rank) for transitive closure of merge candidates, not the petgraph graph. Union-find is preferred because the identifier overlap graph is dense and short-lived, and amortized near-O(1) operations suffice. The `UnionFind` implementation uses deterministic tie-breaking (lower ordinal wins) to ensure commutativity.

Post-merge, the graph engine validates the merged output:
- **L3-MRG-02:** `legal_parentage` cycle detection.
- **L3-MRG-01:** Ownership percentage sums via `edges_by_type` index.
- **L1 invariants:** Node/edge ID uniqueness, referential integrity.

---

## 8. Performance

### 8.1 Advisory Limits and Allocation

The advisory limits are 1M nodes and 5M edges (SPEC-001 Section 9.4). Construction allocates roughly 56 MB for node weights and 56 MB for edge weights in the petgraph slab arrays, plus HashMap overhead for the ID and type indexes. Pre-allocation via `with_capacity(node_count, edge_count)` prevents incremental reallocation during construction.

The small weight structs (~56 bytes each) keep several weights within a single 64-byte cache line, reducing cache misses during BFS/DFS neighbor iteration. Edge-type filtering adds one branch per edge during traversal, which is cheaper than pre-building per-edge-type subgraphs that would multiply memory by the number of edge types (up to 16 core types plus extensions).

### 8.2 WASM Compatibility

All algorithms use stack-allocated or heap-allocated Rust structures. No OS-level threading, no filesystem access, no system calls. `petgraph` compiles cleanly to `wasm32-unknown-unknown`. The `HashMap` hasher falls back to a fixed seed in WASM builds, which is acceptable since keys are graph-local ID strings controlled by the file author and bounded by the advisory size limits.

### 8.3 Complexity Summary

| Operation | Time | Space |
|---|---|---|
| `build_graph` | O(N + E) | O(N + E) |
| `reachable_from` | O(V + E) | O(V) |
| `shortest_path` | O(V + E) | O(V) |
| `all_paths` (depth d) | O(V^d) | O(V * d) |
| `induced_subgraph` (K nodes) | O(K * D) | O(K + included edges) |
| `ego_graph` (radius r) | O(V + E) | O(V + E) |
| `selector_match` | O((N + E) * S) | O(N + E) |
| `selector_subgraph` | O((N + E) * S + expand * (V + E)) | O(V + E) |
| `detect_cycles` | O(V + E) | O(V + E) |
