# omtsf-core Technical Specification: Graph Engine

**Status:** Draft
**Date:** 2026-02-20

---

## 1. Purpose

This document specifies the graph construction, representation, and query subsystem within `omtsf-core`. The graph engine converts a deserialized `OmtsFile` into a directed labeled property multigraph backed by `petgraph`, then provides traversal and query algorithms that implement the `reach`, `path`, `subgraph`, and `inspect` CLI commands. It also provides cycle detection used by the validation engine to enforce structural constraints (SPEC-001 Section 9.3, L3-MRG-02).

The engine is a library component. It accepts in-memory data structures and returns in-memory results. It has no I/O dependencies and compiles to `wasm32-unknown-unknown`.

**Spec requirement coverage:**

| Requirement | Source | Section |
|---|---|---|
| Directed labeled property multigraph | SPEC-001 Section 1 | 2 |
| 7 node types, 16 edge types (15 core + `same_as`) | SPEC-001 Sections 4-7, SPEC-003 Section 7 | 2.2 |
| Multiple edges of same type between same nodes | SPEC-001 Section 3.2 | 2.1 |
| Graph type constraints (permitted source/target) | SPEC-001 Section 9.5 | 2.4 |
| Cycle rules per edge type | SPEC-001 Section 9.3 | 6 |
| `legal_parentage` forest constraint | L3-MRG-02 | 6 |
| Advisory size limits (1M nodes, 5M edges) | SPEC-001 Section 9.4 | 8.1 |
| Node/edge ID uniqueness | SPEC-001 Section 9.1 | 2.4 |
| Post-merge invariants | SPEC-003 Section 5.1 | 7 |

---

## 2. petgraph Wrapper

### 2.1 Graph Type Selection: `StableDiGraph`

The engine uses `petgraph::stable_graph::StableDiGraph<NodeWeight, EdgeWeight>`.

**Why `StableDiGraph` over `DiGraph`:** The standard `DiGraph` invalidates node and edge indices on removal. OMTSF operations that mutate the graph after construction -- redaction (SPEC-004) removes nodes and edges, subgraph extraction produces a filtered copy -- require stable indices. When a node is removed from a `StableDiGraph`, other indices remain valid. This avoids a class of subtle bugs where a stored `NodeIndex` silently refers to the wrong node after a removal operation. The cost is a small per-slot overhead (a tombstone marker), which is acceptable given that the graph is constructed once and queried many times.

**Why not `GraphMap`:** `GraphMap` provides adjacency-map-backed lookup but does not support multigraphs (multiple edges of the same type between the same node pair), which is a hard requirement of the OMTSF data model (SPEC-001 Section 3.2).

### 2.2 Weight Types

**Node weight.** Each `petgraph` node carries a `NodeWeight` struct:

```rust
#[derive(Debug, Clone)]
pub struct NodeWeight {
    /// Graph-local ID from the .omts file (e.g., "org-acme").
    pub local_id: String,
    /// Parsed node type: known built-in or extension string.
    pub node_type: NodeTypeTag,
    /// Index into the OmtsFile::nodes Vec for the full deserialized node.
    pub data_index: usize,
}
```

**Edge weight.** Each `petgraph` edge carries an `EdgeWeight` struct:

```rust
#[derive(Debug, Clone)]
pub struct EdgeWeight {
    /// Graph-local ID from the .omts file (e.g., "edge-001").
    pub local_id: String,
    /// Parsed edge type: known built-in or extension string.
    pub edge_type: EdgeTypeTag,
    /// Index into the OmtsFile::edges Vec for the full deserialized edge.
    pub data_index: usize,
}
```

The indirection via `data_index` is deliberate. `petgraph` stores weights inline in its node and edge slab arrays. Keeping the weight small (three fields, roughly 56 bytes on 64-bit) means that BFS and DFS traversals -- which iterate over node/edge slabs sequentially -- stay cache-friendly. The full property data lives in a parallel `Vec` and is accessed only when needed for output serialization or property inspection.

`NodeTypeTag` and `EdgeTypeTag` are the serde-facing type tags that handle both the 7 known node types (`organization`, `facility`, `good`, `person`, `attestation`, `consignment`, `boundary_ref`) and the 16 edge types (`ownership`, `operational_control`, `legal_parentage`, `former_identity`, `beneficial_ownership`, `supplies`, `subcontracts`, `tolls`, `distributes`, `brokers`, `operates`, `produces`, `composed_of`, `sells_to`, `attested_by`, `same_as`) as well as arbitrary extension types via reverse-domain notation strings.

### 2.3 Index Mapping

During construction the engine builds a bidirectional lookup:

- `HashMap<String, NodeIndex>` -- maps graph-local ID strings to petgraph `NodeIndex` values. Used to resolve edge `source`/`target` references during construction and to translate user-supplied node IDs in CLI commands.
- The reverse mapping (index to local ID) is stored in the `NodeWeight` itself (`local_id` field).

This map is constructed once during `build_graph` and is immutable thereafter.

### 2.4 Construction from `OmtsFile`

```rust
pub fn build_graph(file: &OmtsFile) -> Result<OmtsGraph, GraphBuildError>
```

Construction is a two-pass process:

1. **Node pass.** Iterate `file.nodes`. For each node, insert into the `StableDiGraph` with a `NodeWeight`, and record the `local_id -> NodeIndex` mapping. Fail with `GraphBuildError::DuplicateNodeId` if a local ID appears twice. This is also an L1-GDM-01 validation error, but the graph engine does not assume pre-validation.

2. **Edge pass.** Iterate `file.edges`. For each edge, look up `source` and `target` in the ID map. Fail with `GraphBuildError::DanglingEdgeRef` if either is missing (corresponding to L1-GDM-03). Insert the edge into the `StableDiGraph` with an `EdgeWeight`.

Construction is O(N + E) where N is node count and E is edge count. The `HashMap` lookup is amortized O(1). Both the `StableDiGraph` and the `HashMap` are pre-allocated with `with_capacity` to avoid incremental reallocation. For the advisory upper bound of 1,000,000 nodes and 5,000,000 edges (SPEC-001 Section 9.4), construction allocates roughly 56 MB for node weights plus 56 MB for edge weights, plus the `HashMap` overhead.

The returned `OmtsGraph` struct bundles the `StableDiGraph`, the ID-to-index map, and accessor methods:

```rust
#[derive(Debug)]
pub struct OmtsGraph {
    graph: StableDiGraph<NodeWeight, EdgeWeight>,
    id_to_index: HashMap<String, NodeIndex>,
}

impl OmtsGraph {
    pub fn node_count(&self) -> usize;
    pub fn edge_count(&self) -> usize;
    pub fn node_index(&self, id: &str) -> Option<&NodeIndex>;
    pub fn node_weight(&self, idx: NodeIndex) -> Option<&NodeWeight>;
    pub fn edge_weight(&self, idx: EdgeIndex) -> Option<&EdgeWeight>;
    pub fn graph(&self) -> &StableDiGraph<NodeWeight, EdgeWeight>;
}
```

---

## 3. Reachability

Implements the `omtsf reach <file> <node-id>` command.

### 3.1 Algorithm: BFS

The engine uses breadth-first search (BFS) for reachability. Justification:

- BFS visits nodes in order of increasing hop distance, which is a natural and useful ordering for supply chain analysis (tier 1 suppliers before tier 2).
- BFS uses O(V) memory for the visited set and the queue, same as DFS. There is no stack overflow risk from deep recursion, which matters for pathological chain-shaped graphs at the advisory limit of 1M nodes.
- DFS would be equally correct for the boolean reachability question but provides no distance ordering.

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
- `direction` controls traversal:
  - `Direction::Forward` -- follow outgoing edges (downstream from the start node).
  - `Direction::Backward` -- follow incoming edges (upstream from the start node).
  - `Direction::Both` -- follow edges in either direction (treating the graph as undirected). Useful for connected-component queries.
- The returned set includes all reachable nodes but excludes the start node itself.

The `Direction` enum is defined in the `queries` module:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    Forward,
    Backward,
    Both,
}
```

### 3.3 Edge-Type Filtering

The optional `edge_filter: Option<&HashSet<EdgeTypeTag>>` parameter restricts traversal to edges whose type is in the given set. When `None`, all edge types are traversed. This supports queries such as "all organizations reachable via supply relationship edges only" by passing `{Supplies, Subcontracts, Tolls, Distributes, Brokers, SellsTo}`.

Filtering is evaluated per-edge during traversal rather than by pre-building per-edge-type subgraphs, which would multiply memory usage by the number of edge types.

---

## 4. Path Finding

Implements the `omtsf path <file> <from> <to>` command.

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

Returns `None` if no path exists. Returns `Some(vec)` where the vector is the sequence of node indices from `from` to `to` inclusive. When `from == to`, returns `Some(vec![from_idx])`.

### 4.2 All Paths with Depth Limit

Enumerating all simple paths between two nodes is NP-hard in general (it is equivalent to counting Hamiltonian paths). The engine provides an all-paths query with a mandatory depth limit to keep runtime bounded.

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

The implementation uses iterative-deepening DFS (IDDFS) with an explicit stack to avoid recursive function calls and any stack-overflow risk on large graphs. Each stack frame holds the current node, the depth consumed, the current path, and a visited-on-path set that enforces simple paths (no node revisited within a single path). Paths are collected into a `HashSet<Vec<NodeIndex>>` to deduplicate across depth iterations.

Time complexity is bounded by O(V^d) where d is `max_depth`, which is acceptable for the small depths (typically 3-8 hops) meaningful in supply chain analysis. The default depth limit of 20 hops is large enough to traverse any realistic supply chain while preventing combinatorial explosion on dense subgraphs.

### 4.3 Edge-Type Filtering

Both `shortest_path` and `all_paths` accept the same optional `edge_filter` parameter as `reachable_from`, constraining traversal to specific relationship types.

---

## 5. Subgraph Extraction

Implements the `omtsf subgraph <file> <node-id>...` command.

### 5.1 Induced Subgraph

Given a set of node IDs, the engine extracts the induced subgraph: the specified nodes and every edge in the original graph whose source and target are both in the node set.

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
3. Build a `HashSet<usize>` of included `data_index` values for efficient filtering of the `file.nodes` vec.
4. Iterate all edges in the graph via `edge_references()`. For each edge where both endpoints are in the index set, record its `data_index`.
5. Assemble the result as a valid `OmtsFile` with the original header fields (version, snapshot date, file salt, disclosure scope, previous snapshot ref, snapshot sequence, extra) and the filtered node and edge arrays. Nodes are emitted in their original file order (by `data_index`) for deterministic output.

The edge iteration is O(E) which dominates construction. For the advisory limit of 5,000,000 edges this completes in tens of milliseconds.

### 5.2 Ego-Graph (Node + Radius)

The ego-graph is a convenience built on top of bounded BFS and induced subgraph:

```rust
pub fn ego_graph(
    graph: &OmtsGraph,
    file: &OmtsFile,
    center: &str,
    radius: usize,
    direction: Direction,
) -> Result<OmtsFile, QueryError>
```

Algorithm: run a bounded BFS from the center node using a `VecDeque<(NodeIndex, usize)>` queue where each entry tracks hops consumed. Collect all nodes within `radius` hops (inclusive of the center). Then extract the induced subgraph of the collected set via the shared `assemble_subgraph` internal function.

Radius 0 returns only the center node. Radius 1 returns the center plus its direct neighbours in the specified direction. The BFS respects the `direction` parameter (forward, backward, or both) when expanding each frontier.

### 5.3 Output Validity

The output of both extraction functions is a valid `OmtsFile`. Graph-local IDs are preserved from the source file, so edge `source`/`target` references remain correct. The `reporting_entity` header field is retained only if the referenced node is present in the subgraph; otherwise it is set to `None`. This prevents a dangling `reporting_entity` reference that would violate L1-GDM-05.

The output round-trips cleanly through serde: `serde_json::to_string` followed by `serde_json::from_str` produces an identical `OmtsFile`.

### 5.4 Selector-Based Subgraph Extraction

The selector-based extraction extends the induced subgraph and ego-graph functions to support property-based node/edge selection. The full specification is in `query.md`; this section covers the algorithm and its integration with the existing extraction infrastructure.

#### 5.4.1 Algorithm

```rust
pub fn selector_subgraph(
    graph: &OmtsGraph,
    file: &OmtsFile,
    selectors: &SelectorSet,
    expand: usize,
) -> Result<OmtsFile, QueryError>
```

The algorithm has four phases:

1. **Seed scan.** Linear scan of `file.nodes` and `file.edges`, evaluating `SelectorSet::matches_node` and `SelectorSet::matches_edge` per element. Produces two sets: `seed_nodes: HashSet<NodeIndex>` and `seed_edges: HashSet<EdgeIndex>`. Complexity: O((N + E) * S) where S is the selector count.

2. **Seed edge resolution.** For each seed edge, add its source and target to `seed_nodes`. This ensures that directly matched edges contribute their endpoints to the expansion frontier. Complexity: O(|seed_edges|).

3. **BFS expansion.** Starting from `seed_nodes`, perform bounded BFS for `expand` hops using the same traversal logic as `ego_graph` (Section 5.2). Edges are traversed in both directions (treating the graph as undirected) to capture both upstream and downstream neighbors. Complexity: O(V + E) per hop, bounded by graph size.

4. **Induced subgraph assembly.** Delegate to the shared `assemble_subgraph` internal function (Section 5.1, step 5). The expanded node set defines the induced subgraph. Complexity: O(E).

The phases are sequential; the total complexity is O((N + E) * S + expand * (V + E)).

#### 5.4.2 Shared Infrastructure

The `assemble_subgraph` function used by `induced_subgraph`, `ego_graph`, and `selector_subgraph` is the same internal function. It accepts a `HashSet<NodeIndex>` and the source `OmtsFile`, and produces a filtered `OmtsFile`. This ensures consistent output validity (Section 5.3) across all extraction paths.

#### 5.4.3 `selector_match` (Scan Only)

For the `query` CLI command, which displays matches without producing a subgraph:

```rust
pub fn selector_match(
    file: &OmtsFile,
    selectors: &SelectorSet,
) -> SelectorMatchResult
```

This function performs only phase 1 (seed scan) and returns indices into the `file.nodes` and `file.edges` vectors. It does not build the graph, making it faster for display-only queries. Complexity: O((N + E) * S).

---

## 6. Cycle Detection

### 6.1 Structural Constraints (SPEC-001 Section 9.3)

SPEC-001 Section 9.3 defines per-edge-type cycle rules:

| Edge-type subgraph | Cycles permitted? | Rationale |
|---|---|---|
| Supply relationships (`supplies`, `subcontracts`, `tolls`, `distributes`, `brokers`, `sells_to`) | Yes | Circular supply chains exist (e.g., recycling loops) |
| `ownership` | Yes | Cross-holdings are common in corporate structures |
| `legal_parentage` | No -- must form a forest (L3-MRG-02) | A subsidiary cannot be its own parent |
| `attested_by`, `operates`, `produces`, `composed_of` | Not specified; structurally unlikely | |
| `former_identity` | Not specified; semantically acyclic | |

The validation engine calls into the graph engine to check the `legal_parentage` constraint and, optionally, to report cycles in other subgraphs as informational findings.

### 6.2 Algorithm: Kahn's Topological Sort

For `legal_parentage` acyclicity, the engine extracts the edge-type-filtered subgraph and runs Kahn's algorithm (iterative BFS-based topological sort). The choice of Kahn's over DFS-based cycle detection is deliberate: Kahn's provides a natural byproduct -- when the algorithm stalls (no zero-in-degree nodes remain), the remaining nodes form the strongly connected components containing the cycles, making it straightforward to identify participants.

Algorithm:

1. Build an in-degree map (`HashMap<NodeIndex, usize>`) for every node with respect to the filtered edge set. All graph nodes start at in-degree 0; only filtered edges increment the count.
2. Seed a BFS queue with all zero-in-degree nodes.
3. Repeatedly dequeue a node, decrement the in-degrees of its successors (via filtered edges). When a successor's in-degree reaches 0, enqueue it.
4. If all nodes are consumed, the subgraph is acyclic. If nodes remain, they participate in cycles.
5. Extract individual cycles from the remaining nodes via DFS, restricted to the cyclic-node set and the filtered edge types.

```rust
pub fn detect_cycles(
    graph: &OmtsGraph,
    edge_types: &HashSet<EdgeTypeTag>,
) -> Vec<Vec<NodeIndex>>
```

Returns an empty vector if acyclic. Otherwise returns one or more cycles as node sequences. Each cycle is a closed representation: the first and last node are the same (e.g., `[A, B, C, A]`).

### 6.3 Individual Cycle Extraction

When Kahn's algorithm leaves unvisited nodes, the engine partitions them into individual cycles using iterative DFS with an explicit stack. The DFS is restricted to nodes in the cyclic set and edges matching the filter. When a back-edge is detected (a successor that is already on the current DFS path), the path segment from that successor to the current position is extracted as a cycle.

The extraction uses `globally_visited` tracking to avoid reporting the same cycle from multiple starting nodes.

### 6.4 Reporting

When a cycle is detected in a forbidden subgraph, the validation engine translates the `Vec<NodeIndex>` sequences into diagnostics containing:

- The rule identifier (`L3-MRG-02` for legal parentage).
- The cycle as a sequence of node local IDs connected by edge local IDs.
- The edge type(s) involved.

When cycles are detected in permitted subgraphs (supply relationships, ownership), the engine can optionally report them as informational findings for graph exploration, but they do not constitute validation failures.

---

## 7. Relation to Merge

The graph engine is used during merge for one specific computation after the merge procedure completes: post-merge validation. The merge engine itself uses union-find (disjoint-set with path compression and union by rank) for the transitive closure of merge candidates (SPEC-003 Section 4, step 3), not the petgraph-backed graph.

Union-find is preferred for identity resolution because:

- The identifier overlap graph is dense and short-lived. Building a full `StableDiGraph` for it adds unnecessary overhead.
- Union-find provides amortized near-O(1) per operation and answers "are these two nodes in the same group?" without materializing the full graph.
- After merge groups are determined, the merged graph is constructed from scratch using `build_graph` on the merged `OmtsFile`.

Post-merge, the graph engine is invoked for:

- **L3-MRG-02:** Cycle detection on the `legal_parentage` subgraph of the merged output.
- **L3-MRG-01:** Ownership percentage sum checks, which iterate inbound `ownership` edges per node.
- **General L1 invariants:** All graph invariants (node ID uniqueness, edge ID uniqueness, edge endpoints reference existing nodes) must hold on the merged output.

---

## 8. Performance Considerations

### 8.1 Allocation Strategy

For graphs approaching the advisory limits (1M nodes, 5M edges), allocation patterns matter. The engine:

- Pre-allocates `StableDiGraph` capacity using `with_capacity(node_count, edge_count)` to avoid incremental reallocation during construction.
- Pre-allocates the ID-to-index `HashMap` with `HashMap::with_capacity(node_count)`.
- Uses `data_index` indirection (Section 2.2) to keep the petgraph slab entries small and cache-line-friendly during traversal.

### 8.2 Traversal Performance

BFS and DFS traversals access the node and edge slab arrays in roughly sequential order (modulo graph topology). The small weight structs (roughly 56 bytes each) mean that several weights fit in a single cache line (64 bytes), reducing cache misses during neighbor iteration.

Edge-type filtering during traversal adds a branch per edge. This is preferable to pre-building per-edge-type subgraphs, which would multiply memory usage by the number of edge types (up to 16 core types plus extensions).

### 8.3 WASM Compatibility

All algorithms use stack-allocated or heap-allocated Rust structures. No OS-level threading, no filesystem access, no system calls. The `petgraph` crate compiles cleanly to `wasm32-unknown-unknown`. The `HashMap` uses the standard library's `RandomState` hasher; in WASM builds this falls back to a fixed seed, which is acceptable since the hash map is not exposed to adversarial input (keys are graph-local ID strings controlled by the file author, and the advisory size limits bound the number of entries).

### 8.4 Complexity Summary

| Operation | Time | Space |
|---|---|---|
| `build_graph` | O(N + E) | O(N + E) |
| `reachable_from` | O(V + E) | O(V) |
| `shortest_path` | O(V + E) | O(V) |
| `all_paths` (depth d) | O(V^d) | O(V * d) |
| `induced_subgraph` | O(E) | O(N + E) |
| `ego_graph` (radius r) | O(V + E) | O(V + E) |
| `selector_match` | O((N + E) * S) | O(N + E) |
| `selector_subgraph` | O((N + E) * S + expand * (V + E)) | O(V + E) |
| `detect_cycles` | O(V + E) | O(V + E) |
