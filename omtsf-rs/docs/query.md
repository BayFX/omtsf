# omtsf-core Technical Specification: Query & Subchain Extraction

**Status:** Draft
**Date:** 2026-02-21

---

## 1. Purpose

This document specifies the selector-based query and subchain extraction subsystem within `omtsf-core` and its CLI surface in `omtsf-cli`. The system enables users to filter supply chain graphs by node/edge properties (type, labels, identifiers, jurisdiction, name) and extract the matching subgraph with connecting neighbors.

The existing `subgraph` command requires users to know exact node IDs. The `query` and `extract-subchain` commands extend this to property-based selection, enabling queries such as "all organizations in Germany with an ISO 14001 attestation" without prior knowledge of specific node identifiers.

**Spec requirement coverage:**

| Requirement | Source | Section |
|---|---|---|
| Label-based filtering | SPEC-001 Section 8.4 | 2, 3 |
| Identifier scheme matching | SPEC-002 Section 3 | 2, 3 |
| Node type filtering | SPEC-001 Section 4 | 2, 3 |
| Edge type filtering | SPEC-001 Section 5 | 2, 3 |
| Jurisdiction filtering | SPEC-001 Section 4.1 | 2, 3 |
| Subgraph extraction with expansion | graph-engine.md Section 5 | 4 |

---

## 2. Selectors

A **selector** is a predicate that matches nodes, edges, or both. Selectors are the building blocks for property-based queries.

### 2.1 Selector Types

| Selector | CLI Flag | Targets | Match Semantics |
|---|---|---|---|
| Node type | `--node-type <type>` | Nodes | Matches nodes whose `node_type` equals the given `NodeTypeTag` |
| Edge type | `--edge-type <type>` | Edges | Matches edges whose `edge_type` equals the given `EdgeTypeTag` |
| Label (key) | `--label <key>` | Nodes, Edges | Matches elements that have a label with the given key (any value or no value) |
| Label (key=value) | `--label <key>=<value>` | Nodes, Edges | Matches elements that have a label with the given key and exact value |
| Identifier (scheme) | `--identifier <scheme>` | Nodes | Matches nodes that have at least one identifier with the given scheme |
| Identifier (scheme:value) | `--identifier <scheme>:<value>` | Nodes | Matches nodes that have an identifier with the given scheme and exact value |
| Jurisdiction | `--jurisdiction <CC>` | Nodes | Matches nodes whose `jurisdiction` is the given ISO 3166-1 alpha-2 country code |
| Name | `--name <pattern>` | Nodes | Case-insensitive substring match on the node `name` field |

### 2.2 Composition Rules

All selector flags are repeatable. Composition follows two rules:

1. **OR within the same selector type.** Multiple `--node-type organization --node-type facility` matches nodes that are organizations **or** facilities.
2. **AND across different selector types.** `--node-type organization --jurisdiction DE` matches nodes that are organizations **and** have jurisdiction `DE`.

This is the standard "disjunctive normal form within conjunctive groups" pattern used by tools like `kubectl` label selectors.

### 2.3 Parsed Representation

```rust
/// A single selector predicate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selector {
    /// Match nodes by type.
    NodeType(NodeTypeTag),
    /// Match edges by type.
    EdgeType(EdgeTypeTag),
    /// Match by label key (any value).
    LabelKey(String),
    /// Match by label key and exact value.
    LabelKeyValue(String, String),
    /// Match by identifier scheme (any value).
    IdentifierScheme(String),
    /// Match by identifier scheme and exact value.
    IdentifierSchemeValue(String, String),
    /// Match by jurisdiction (ISO 3166-1 alpha-2).
    Jurisdiction(CountryCode),
    /// Case-insensitive substring match on node name.
    Name(String),
}
```

### 2.4 `SelectorSet`

A `SelectorSet` groups selectors by type for efficient evaluation:

```rust
/// Grouped selectors with AND/OR composition.
#[derive(Debug, Clone, Default)]
pub struct SelectorSet {
    pub node_types: Vec<NodeTypeTag>,
    pub edge_types: Vec<EdgeTypeTag>,
    pub label_keys: Vec<String>,
    pub label_key_values: Vec<(String, String)>,
    pub identifier_schemes: Vec<String>,
    pub identifier_scheme_values: Vec<(String, String)>,
    pub jurisdictions: Vec<CountryCode>,
    pub names: Vec<String>,
}

impl SelectorSet {
    /// Returns true if no selectors are specified.
    pub fn is_empty(&self) -> bool;

    /// Returns true if the given node matches all selector groups.
    pub fn matches_node(&self, node: &Node) -> bool;

    /// Returns true if the given edge matches all selector groups.
    pub fn matches_edge(&self, edge: &Edge) -> bool;
}
```

**Evaluation of `matches_node`:**

1. If `node_types` is non-empty, the node's type must match at least one entry (OR).
2. If `label_keys` is non-empty, the node must have at least one label whose key matches any entry (OR).
3. If `label_key_values` is non-empty, the node must have at least one label matching any (key, value) pair (OR).
4. If `identifier_schemes` is non-empty, the node must have at least one identifier whose scheme matches any entry (OR).
5. If `identifier_scheme_values` is non-empty, the node must have at least one identifier matching any (scheme, value) pair (OR).
6. If `jurisdictions` is non-empty, the node's `jurisdiction` must match at least one entry (OR).
7. If `names` is non-empty, the node's `name` must contain at least one entry as a case-insensitive substring (OR).
8. All non-empty groups must pass (AND across groups).

Node-only selectors (`--node-type`, `--identifier`, `--jurisdiction`, `--name`) are skipped when evaluating edges. Edge-only selectors (`--edge-type`) are skipped when evaluating nodes.

**Evaluation of `matches_edge`:**

1. If `edge_types` is non-empty, the edge's type must match at least one entry (OR).
2. Label selectors are evaluated against the edge's `labels` field (same logic as nodes).
3. Node-only selectors (`node_types`, `identifier_schemes`, `identifier_scheme_values`, `jurisdictions`, `names`) are ignored for edge matching.

### 2.5 Parsing from CLI Arguments

The `--label` flag uses the presence of `=` to distinguish key-only from key=value:
- `--label certified` -> `Selector::LabelKey("certified")`
- `--label tier=1` -> `Selector::LabelKeyValue("tier", "1")`

The `--identifier` flag uses `:` as the delimiter:
- `--identifier lei` -> `Selector::IdentifierScheme("lei")`
- `--identifier duns:123456789` -> `Selector::IdentifierSchemeValue("duns", "123456789")`

The `--jurisdiction` flag validates the value as an ISO 3166-1 alpha-2 code using the existing `CountryCode` newtype.

The `--name` flag accepts arbitrary strings. The match is case-insensitive and uses substring containment, not glob or regex. This is deliberate: name fields are human-readable display names, and substring search is the most intuitive matching behavior.

---

## 3. Selector Evaluation Algorithm

### 3.1 Seed Set Construction

Given a `SelectorSet` and an `OmtsFile`:

```
seed_nodes = { n in file.nodes | selector_set.matches_node(n) }
seed_edges = { e in file.edges | selector_set.matches_edge(e) }
```

The scan is a single linear pass over all nodes and all edges: O(N + E) where N is node count and E is edge count. Selector evaluation per element is O(S) where S is the total number of selector values across all groups.

### 3.2 Neighbor Expansion

After the seed set is computed, the query engine expands to include connecting elements:

1. **For each seed node:** include all incident edges (both incoming and outgoing) and their opposite endpoints. This ensures that a matched node is shown in context -- you always see what connects to it.
2. **For each seed edge:** include its source and target nodes.

This produces the **base result set**: seed nodes + seed edges + incident edges of seed nodes + endpoints of seed edges.

### 3.3 Hop Expansion (BFS)

The optional `--expand N` flag (default 1) controls how many additional hops from the seed set are included. The expansion uses the same BFS algorithm as `ego_graph` (graph-engine.md Section 5.2):

1. Initialize the BFS frontier with all seed nodes.
2. For each node in the frontier, traverse all edges (both directions) and add the opposite endpoint to the next frontier if not already visited.
3. Repeat for `N` hops.
4. The final node set is the union of all visited nodes across all hops.

When `--expand 0` is specified, the result contains only the seed nodes/edges and their immediate incident neighbors (step 3.2). When `--expand 1` (default), one additional hop of BFS is performed from the seed set.

### 3.4 Induced Subgraph Assembly

After expansion, compute the induced subgraph of the final node set using the shared `assemble_subgraph` function (graph-engine.md Section 5.1):

1. Collect the final node set as a `HashSet<NodeIndex>`.
2. Include every edge where both source and target are in the node set.
3. Assemble the output `OmtsFile` with the original header and filtered nodes/edges.

### 3.5 Complexity

| Phase | Time | Space |
|---|---|---|
| Seed scan | O((N + E) * S) | O(N + E) for seed sets |
| Neighbor expansion (step 3.2) | O(degree_sum(seed_nodes) + seed_edges) | O(result set size) |
| BFS hop expansion (step 3.3) | O(V + E) per hop, bounded by graph size | O(V) visited set |
| Induced subgraph assembly | O(E) | O(N + E) output |

Where S is the total selector count. For typical queries (S < 10), the selector evaluation cost per element is negligible, and the overall complexity is dominated by the graph traversal: O(V + E) for the full pipeline.

---

## 4. Library API

### 4.1 Module Location

The selector types and query functions are placed in `omtsf-core::graph::extraction`, alongside the existing `induced_subgraph` and `ego_graph` functions.

```
omtsf-core/src/graph/
├── mod.rs          // OmtsGraph, build_graph
├── queries.rs      // reachable_from, shortest_path, all_paths
├── extraction.rs   // induced_subgraph, ego_graph, selector_subgraph (NEW)
└── selectors.rs    // Selector, SelectorSet (NEW)
```

### 4.2 `selector_subgraph`

```rust
/// Extracts a subgraph based on selector predicates.
///
/// 1. Scans all nodes and edges to build the seed set.
/// 2. Expands by `expand` hops using BFS from seed nodes.
/// 3. Returns the induced subgraph of the expanded node set.
///
/// Returns `QueryError::EmptyResult` if no nodes or edges match the selectors.
pub fn selector_subgraph(
    graph: &OmtsGraph,
    file: &OmtsFile,
    selectors: &SelectorSet,
    expand: usize,
) -> Result<OmtsFile, QueryError>
```

### 4.3 `selector_match`

For the `query` command (display-only, no subgraph extraction), a lighter function returns matching node and edge indices without assembling an `OmtsFile`:

```rust
/// Returns the indices of all nodes and edges matching the given selectors.
///
/// Does not perform neighbor expansion or subgraph assembly.
pub fn selector_match(
    file: &OmtsFile,
    selectors: &SelectorSet,
) -> SelectorMatchResult

/// Result of a selector match scan.
#[derive(Debug, Default)]
pub struct SelectorMatchResult {
    /// Indices into `file.nodes` for matching nodes.
    pub node_indices: Vec<usize>,
    /// Indices into `file.edges` for matching edges.
    pub edge_indices: Vec<usize>,
}
```

### 4.4 Error Types

The existing `QueryError` enum gains one new variant:

```rust
pub enum QueryError {
    NodeNotFound(String),
    EmptyResult,  // NEW: no elements matched the selectors
}
```

`EmptyResult` is distinct from a successful query that happens to match zero elements after expansion. It signals that the selector scan itself found no matches, which the CLI maps to exit code 1.

---

## 5. CLI Commands

### 5.1 `omtsf query <file> [selectors]`

Displays nodes and edges matching the given selectors. This is a read-only inspection command -- it does not produce a subgraph file.

**Arguments:**
- `<file>` (required) -- Path to an `.omts` file, or `-` for stdin.

**Selector flags:** (all repeatable)
- `--node-type <type>` -- Filter by node type.
- `--edge-type <type>` -- Filter by edge type.
- `--label <key>` or `--label <key>=<value>` -- Filter by label.
- `--identifier <scheme>` or `--identifier <scheme>:<value>` -- Filter by identifier.
- `--jurisdiction <CC>` -- Filter by jurisdiction (ISO 3166-1 alpha-2).
- `--name <pattern>` -- Case-insensitive substring match on node name.

**Additional flags:**
- `--count` -- Print only the count of matching nodes and edges, not the full listing.

**Behavior:** Parses the file, evaluates selectors against all nodes and edges, and displays matching elements to stdout. In human mode, output is a table with columns for ID, type, name/source/target, and matched selector. In JSON mode, emits a JSON object with `nodes` and `edges` arrays containing the matched elements. Reports match counts to stderr.

At least one selector flag must be provided. If no selectors are given, the command exits with a usage error.

**Exit codes:** 0 = at least one match found, 1 = no matches found, 2 = parse/input failure.

**Examples:**
```
omtsf query supply-chain.omts --node-type organization --jurisdiction DE
omtsf query supply-chain.omts --label certified --name "Acme"
omtsf query -f json graph.omts --identifier lei --count
omtsf query graph.omts --edge-type supplies --label tier=1
```

### 5.2 `omtsf extract-subchain <file> [selectors]`

Extracts the subgraph matching the given selectors and writes a valid `.omts` file to stdout. This is the property-based equivalent of `omtsf subgraph`.

**Arguments:**
- `<file>` (required) -- Path to an `.omts` file, or `-` for stdin.

**Selector flags:** Same as `omtsf query` (Section 5.1).

**Additional flags:**
- `--expand <n>` -- Include neighbors up to `n` hops from the seed set (default: 1). Setting `--expand 0` returns only the seed nodes/edges and their immediate incident neighbors.

**Behavior:** Parses the file, evaluates selectors to build the seed set, expands by `--expand` hops, computes the induced subgraph, and writes the result to stdout as a valid `.omts` file. The output header is copied from the input with an updated `snapshot_date`. The `reporting_entity` is retained only if the referenced node is present in the output subgraph. Reports extraction statistics (seed count, expanded count, output node/edge count) to stderr in verbose mode.

At least one selector flag must be provided. If no selectors are given, the command exits with a usage error.

**Exit codes:** 0 = subgraph extracted, 1 = no matches found for the given selectors, 2 = parse/input failure.

**Examples:**
```
omtsf extract-subchain supply-chain.omts --node-type organization --jurisdiction DE > german-orgs.omts
omtsf extract-subchain supply-chain.omts --identifier lei --expand 2 > lei-neighborhood.omts
omtsf extract-subchain graph.omts --label tier=1 --expand 0 > tier1-only.omts
cat graph.omts | omtsf extract-subchain - --name "Acme" > acme-subchain.omts
```

---

## 6. Benchmark Specification

This section specifies benchmarks for the selector query engine, following the patterns established in the existing benchmark suite (see `omtsf-bench/benches/subgraph_extraction.rs` and `omtsf-bench/benches/graph_queries.rs`).

### 6.1 Benchmark File

**New file:** `omtsf-bench/benches/selector_query.rs`

**Cargo.toml entry:**
```toml
[[bench]]
name = "selector_query"
harness = false
```

**justfile update:** Add `selector_query` to the `bench-quick` recipe.

### 6.2 Setup Pattern

Following the existing `Setup` struct pattern:

```rust
struct Setup {
    file: OmtsFile,
    graph: OmtsGraph,
}

fn setup(tier: SizeTier) -> Setup {
    let file = generate_supply_chain(&tier.config(42));
    let graph = build_graph(&file).expect("builds");
    Setup { file, graph }
}
```

### 6.3 Benchmark Groups

#### Group A: Selector Scan (`selector_match`)

Measures the cost of scanning all nodes/edges to identify matches, without subgraph assembly.

| Benchmark | Description | Selectors | Size Tiers |
|---|---|---|---|
| `selector_match/label/S,M,L,XL` | Label-only selector scan | `--label certified` (key-only) | S, M, L, XL |
| `selector_match/node_type/S,M,L,XL` | Node-type selector scan | `--node-type organization` | S, M, L, XL |
| `selector_match/multi_selector/S,M,L,XL` | Combined selectors | `--node-type organization --label tier=1 --jurisdiction DE` | S, M, L, XL |

**Throughput metric:** `Throughput::Elements(node_count + edge_count)` -- measures how many elements per second the selector engine can scan.

#### Group B: Subgraph Extraction (`selector_subgraph`)

Measures the full pipeline: selector scan + neighbor expansion + induced subgraph assembly.

| Benchmark | Description | Seed Match Rate | Size Tiers |
|---|---|---|---|
| `selector_subgraph/narrow/S,M,L` | ~5% seed match rate | `--node-type attestation` (attestations are ~10% of nodes) | S, M, L |
| `selector_subgraph/broad/S,M,L` | ~50% seed match rate | `--node-type organization` (orgs are ~45% of nodes) | S, M, L |
| `selector_subgraph/expand_1/S,M,L` | Seed + 1-hop expansion | `--node-type attestation --expand 1` | S, M, L |
| `selector_subgraph/expand_3/S,M,L` | Seed + 3-hop expansion | `--node-type attestation --expand 3` | S, M, L |

**Throughput metric:** `Throughput::Elements(output_node_count)` -- measures output nodes per second to normalize across different expansion levels.

### 6.4 Measurement Strategy

- All benchmarks use the default Criterion sample size (100 iterations) except `selector_subgraph/expand_3` which uses `sample_size(20)` due to higher per-iteration cost.
- Each benchmark pre-computes the `Setup` struct outside the measurement loop. Only the selector evaluation and subgraph extraction are timed.
- For throughput comparisons, results are presented alongside existing `induced_subgraph` and `ego_graph` baselines from Group 4 (subgraph_extraction.rs).

### 6.5 Expected Scaling

| Phase | Expected Complexity | Validation Criterion |
|---|---|---|
| Selector scan | O(N + E) linear | S-to-M ratio should be ~10x (matching element ratio) |
| Narrow subgraph extraction | O(E) dominated by edge scan | Should be comparable to `induced_subgraph` at similar output sizes |
| Hop expansion | O(V + E) per hop | Should match `ego_graph` performance at equivalent radius |

### 6.6 Post-Implementation Evaluation

After the selector engine is implemented:

1. Run `just bench-group selector_query` across all tiers.
2. Compare `selector_match` scan time against raw iteration over `file.nodes` / `file.edges` to measure selector evaluation overhead.
3. Compare `selector_subgraph/narrow` against `induced_subgraph` at 10% node fraction to verify extraction cost parity.
4. Compare `selector_subgraph/expand_1` against `ego_graph` root radius 1 to verify BFS expansion cost parity.
5. Verify linear scaling: `selector_match` S-to-M ratio should be approximately 10x (proportional to element count ratio).
6. Document results in `BENCHMARK_RESULTS.md` Section 10.

---

## 7. Generator Updates

The existing generator (`omtsf-bench/src/generator/`) must produce fixtures with sufficient label and identifier coverage for meaningful selector benchmarks.

### 7.1 New Configuration Field

```rust
pub struct GeneratorConfig {
    // ... existing fields ...
    /// Average labels per node (default 1.5).
    /// Labels include both key-only flags and key=value pairs.
    pub label_density: f64,
}
```

Default values per tier:

| Tier | `label_density` |
|---|---|
| S | 1.5 |
| M | 1.5 |
| L | 2.0 |
| XL | 2.0 |

### 7.2 Label Generation

A new function `gen_labels` in the generator's `nodes` module:

```rust
pub fn gen_labels(rng: &mut StdRng, label_density: f64) -> Vec<Label> {
    // ...
}
```

**Label vocabulary:**

| Key | Values | Distribution |
|---|---|---|
| `certified` | (key-only flag) | 30% of nodes |
| `tier` | `1`, `2`, `3`, `4`, `5` | All organization nodes |
| `risk-level` | `low`, `medium`, `high` | 50% of nodes |
| `eu-regulated` | (key-only flag) | Nodes with jurisdiction in EU countries |
| `audit-year` | `2024`, `2025`, `2026` | Attestation nodes |
| `sector` | `mining`, `manufacturing`, `logistics`, `agriculture`, `chemicals` | Organization nodes |

Labels include both key-only flags (boolean markers) and key=value pairs to exercise both selector forms.

### 7.3 Jurisdiction Distribution

The existing generator already assigns jurisdictions to organization and facility nodes from the `COUNTRIES` constant (15 countries). This provides sufficient diversity for `--jurisdiction` selector benchmarks. No changes required.

Country distribution across generated nodes:

| Country | Approximate % |
|---|---|
| US, GB, DE, FR, NL, JP, CN, BR, IN, AU, KR, SG, CH, SE, CA | ~6.7% each (uniform random) |

### 7.4 Backwards Compatibility

The `label_density` field is added to `GeneratorConfig` with a default value. Existing code that constructs `GeneratorConfig` manually will need to add the new field. The `SizeTier::config()` method sets the default. Fixture regeneration is backwards-compatible -- the JSON format does not change, only the data content gains label entries.

---

## 8. Interaction with Existing Commands

### 8.1 Relationship to `subgraph`

`extract-subchain` is a superset of `subgraph`. The `subgraph` command selects nodes by explicit ID; `extract-subchain` selects nodes by property predicates. Both use the same underlying `assemble_subgraph` function for output assembly.

The two commands remain separate for clarity and backward compatibility:
- `subgraph` is precise: "give me exactly these nodes."
- `extract-subchain` is exploratory: "give me all nodes matching these criteria."

### 8.2 Relationship to `query`

`query` is a read-only inspection of what matches. `extract-subchain` is a write operation (produces an `.omts` file). The `query --count` variant is useful for previewing the size of an extraction before running it.

### 8.3 Composability

The output of `extract-subchain` is a valid `.omts` file, so it can be piped into any other command:

```
omtsf extract-subchain graph.omts --jurisdiction DE | omtsf validate -
omtsf extract-subchain graph.omts --node-type organization | omtsf inspect -
omtsf extract-subchain a.omts --label certified | omtsf diff - <(omtsf extract-subchain b.omts --label certified)
```
