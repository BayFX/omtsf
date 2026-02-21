# omtsf-core Technical Specification: Diff Engine

**Status:** Draft
**Date:** 2026-02-21

---

## 1. Purpose

This document specifies the structural diff engine in `omtsf-core`. The engine compares two parsed `.omts` files and produces a typed description of what changed: which nodes and edges were added, removed, or modified, and which properties within matched elements differ.

The diff engine reuses the merge identity predicates defined in SPEC-003 to determine correspondence between elements across files. Two nodes that would be merge candidates are treated as "the same entity observed differently." Everything that fails to match is an addition or deletion.

The engine lives in `omtsf-core` and operates on deserialized `&OmtsFile` values. It has no filesystem dependency and is WASM-compatible. The CLI's `omtsf diff <a> <b>` command handles file I/O, calls the library, and formats the output.

---

## 2. Node and Edge Matching

### 2.1 Node Matching

Node matching applies the SPEC-003 Section 2 identity predicate. Two nodes from file A and file B are a **matched pair** if they share at least one external identifier where:

1. `scheme` values are equal (case-sensitive), excluding `internal`
2. `value` values are equal (case-sensitive, whitespace-trimmed)
3. `authority` values are equal (case-insensitive) when present in either record
4. Temporal validity periods overlap (or at least one is open-ended)

These rules are implemented by `identifiers_match()` in `identity.rs`, which the diff engine calls directly rather than reimplementing.

**Algorithm.** Node matching proceeds in five steps:

1. **Index construction.** Build a `HashMap<CanonicalId, Vec<usize>>` for each file, mapping canonical identifier strings (`{scheme}:{value}` or `{scheme}:{authority}:{value}`) to node ordinals. Identifiers with `scheme: "internal"` are excluded. This index is produced by `build_identifier_index()` from `canonical.rs`.

2. **Candidate pair discovery.** For each canonical key present in both indices, iterate all `(node_a, node_b)` combinations. For each pair, verify the match via `identifiers_match()` to enforce authority case rules and temporal compatibility that the string index alone cannot check.

3. **Transitive closure via union-find.** Assign each node a slot in a unified ordinal space: A-nodes occupy `[0, len_a)`, B-nodes occupy `[len_a, len_a + len_b)`. For each confirmed match pair, call `uf.union(a_idx, len_a + b_idx)`. This reuses the same `UnionFind` structure described in `merge.md` Section 2, with path-halving compression and union-by-rank with lower-ordinal-wins tie-breaking. Transitive closure handles the case where entity X carries a DUNS in file A and an LEI in file B, while a third identifier on a different node links the two.

4. **Group extraction and ambiguity detection.** After union-find settles, group all active nodes by their representative. Each group contains zero or more A-nodes and zero or more B-nodes. If a group contains multiple nodes from the same file (e.g., two A-nodes map to the same B-node), the engine emits a diagnostic warning naming the ambiguous group. All nodes in the group are still reported as matched -- the ambiguity is the caller's concern, not a blocking error.

5. **Classification.** Groups with only A-members produce **deletions**. Groups with only B-members produce **additions**. Groups with both produce **matched pairs**. Within a multi-member group, each A-node is paired with each B-node (Cartesian product), which is conservative -- it surfaces all possible diffs rather than silently choosing one pairing.

When a `DiffFilter` restricts `node_types`, only nodes passing the type filter participate in matching. Nodes of excluded types are invisible to the algorithm.

### 2.2 Edge Matching

Edge matching applies the SPEC-003 Section 3 identity predicate, evaluated after node matching is complete. Two edges are a **matched pair** if:

1. Their source nodes belong to the same union-find match group
2. Their target nodes belong to the same match group
3. Their `type` values are equal
4. They share an external identifier, OR they lack external identifiers and their merge-identity properties (per the SPEC-003 Section 3.1 table) are equal

The per-type identity property table, reused directly from SPEC-003:

| Edge Type | Identity Properties (beyond type + endpoints) |
|-----------|-----------------------------------------------|
| `ownership` | `percentage`, `direct` |
| `operational_control` | `control_type` |
| `legal_parentage` | `consolidation_basis` |
| `former_identity` | `event_type`, `effective_date` |
| `beneficial_ownership` | `control_type`, `percentage` |
| `supplies` | `commodity`, `contract_ref` |
| `subcontracts` | `commodity`, `contract_ref` |
| `tolls` | `commodity` |
| `distributes` | `service_type` |
| `brokers` | `commodity` |
| `operates` | *(type + endpoints suffice)* |
| `produces` | *(type + endpoints suffice)* |
| `composed_of` | *(type + endpoints suffice)* |
| `sells_to` | `commodity`, `contract_ref` |
| `attested_by` | `scope` |
| `same_as` | *(never matched -- always unique)* |

**Algorithm.** Edge matching builds a `HashMap<String, usize>` mapping each `NodeId` to its union-find representative, then groups A-edges into buckets keyed by `(src_rep, tgt_rep, edge_type)`. For each B-edge, the engine looks up the corresponding bucket and finds the first unconsumed A-edge that satisfies `edges_match()` from `identity.rs`. Consumed A-edges are tracked in a `HashSet<usize>`. Excess A-edges are deletions; unmatched B-edges are additions.

Extension edge types (reverse-domain notation) have no entry in the identity property table. They fall back to identifier-only matching. Two extension edges with the same endpoints and type but no identifiers are treated as unmatched (one deletion, one addition) rather than silently paired.

`same_as` edges are never matched across files. Every `same_as` edge in A appears as a deletion and every `same_as` edge in B appears as an addition. This is intentional: `same_as` edges are intra-file assertions with no cross-file identity.

---

## 3. Property Comparison

For each matched pair of nodes or edges, the engine compares properties field by field. Three categories of comparison apply: scalar fields, identifier arrays, and label arrays.

### 3.1 Scalar Properties

Scalar fields (`name`, `jurisdiction`, `status`, `percentage`, `direct`, etc.) are compared by value equality. A `PropertyChange` is recorded when the value in A differs from the value in B, or when a field is present in one but absent in the other.

**Semantic equivalence for dates.** Date fields are compared as calendar dates, not as raw strings. The engine normalizes non-conformant representations before comparison (e.g., `"2026-2-9"` is normalized to `"2026-02-09"`) to avoid false positives on formatting variation. This normalization is applied internally via `normalise_date()` and does not alter the reported values.

**Numeric comparison.** Numeric fields (`percentage`, `quantity`, `volume`, `annual_value`, `share_of_buyer_demand`, emissions fields) are compared by value using a floating-point epsilon of `1e-9`. This avoids false positives from JSON serialization round-trips where `51.0` and `51` encode the same value.

**Semantic equality dispatch.** The internal `values_equal()` function dispatches by JSON value type: numbers use epsilon comparison, strings containing hyphens are date-normalized before comparison, and all other types (booleans, nulls, arrays, objects) use structural equality.

### 3.2 Identifier Array Comparison

The `identifiers` array is compared as a set, keyed by canonical string form (`{scheme}:{value}` or `{scheme}:{authority}:{value}`). `internal` scheme identifiers are excluded from the set, consistent with their exclusion from identity matching.

- Identifiers present in B but not A are **additions**.
- Identifiers present in A but not B are **deletions**.
- Identifiers present in both are compared field by field for changes to `valid_from`, `valid_to`, `sensitivity`, `verification_status`, `verification_date`, `authority`, and any unknown extra fields. Only fields that actually differ produce `PropertyChange` entries.

### 3.3 Label Array Comparison

The `labels` array is compared as a set of `{key, value}` pairs. The atomic identity unit is the full tuple, not just the key.

- Labels present in B but not A are **additions**.
- Labels present in A but not B are **deletions**.
- A change in value for a given key appears as a deletion of the old `{key, old_value}` pair and an addition of the new `{key, new_value}` pair. There is no "modified label" concept.

### 3.4 Nested Objects

**`data_quality`.** Compared field by field (`confidence`, `source`, `last_verified`) plus any unknown extra fields. When one side has a `data_quality` object and the other does not, the entire object is reported as a single addition or deletion.

**`geo`.** Compared as raw `serde_json::Value` using structural equality. A point `{lat: 53.38, lon: -1.47}` and GeoJSON geometry are compared as JSON objects, not parsed into typed `Geo` values.

**Edge `properties` wrapper.** The wrapper is transparent to the diff engine. The engine compares logical edge properties, not the JSON nesting structure. All edge property fields are compared through `compare_edge_props()` which operates on `EdgeProperties` directly.

**Unknown fields.** Both `Node` and `EdgeProperties` carry a `#[serde(flatten)] extra: Map<String, Value>` field. The engine compares extra fields key-by-key using `values_equal()`, ensuring that unknown fields added by newer spec versions are surfaced in diffs rather than silently ignored.

---

## 4. Diff API

### 4.1 Core Types

```rust
/// A lightweight reference to a node, carrying just enough for readable output.
pub struct NodeRef {
    pub id: NodeId,
    pub node_type: String,
    pub name: Option<String>,
}

/// A lightweight reference to an edge.
pub struct EdgeRef {
    pub id: EdgeId,
    pub edge_type: String,
    pub source: NodeId,
    pub target: NodeId,
}

/// A change to a single scalar property field.
pub struct PropertyChange {
    pub field: String,
    pub old_value: Option<serde_json::Value>,   // None = absent in A
    pub new_value: Option<serde_json::Value>,   // None = absent in B
}

/// Set diff of identifiers between two matched elements.
pub struct IdentifierSetDiff {
    pub added: Vec<Identifier>,
    pub removed: Vec<Identifier>,
    pub modified: Vec<IdentifierFieldDiff>,     // same canonical key, field-level changes
}

/// Field-level changes on a single identifier present in both A and B.
pub struct IdentifierFieldDiff {
    pub canonical_key: CanonicalId,
    pub field_changes: Vec<PropertyChange>,
}

/// Set diff of labels between two matched elements.
pub struct LabelSetDiff {
    pub added: Vec<Label>,
    pub removed: Vec<Label>,
}

/// Differences found between a matched pair of nodes.
pub struct NodeDiff {
    pub id_a: String,                           // graph-local ID in file A
    pub id_b: String,                           // graph-local ID in file B
    pub node_type: String,
    pub matched_by: Vec<String>,                // canonical ID strings that caused the match
    pub property_changes: Vec<PropertyChange>,
    pub identifier_changes: IdentifierSetDiff,
    pub label_changes: LabelSetDiff,
}

/// Differences found between a matched pair of edges.
pub struct EdgeDiff {
    pub id_a: String,
    pub id_b: String,
    pub edge_type: String,
    pub property_changes: Vec<PropertyChange>,
    pub identifier_changes: IdentifierSetDiff,
    pub label_changes: LabelSetDiff,
}
```

`NodeRef` and `EdgeRef` are lightweight references carrying just enough information for readable output without cloning entire elements. `Value` is `serde_json::Value` -- the engine does not interpret property values beyond the comparison rules in Section 3.

### 4.2 Result Types

```rust
/// Classification of node differences between two files.
pub struct NodesDiff {
    pub added: Vec<NodeRef>,
    pub removed: Vec<NodeRef>,
    pub modified: Vec<NodeDiff>,
    pub unchanged: Vec<NodeDiff>,
}

/// Classification of edge differences between two files.
pub struct EdgesDiff {
    pub added: Vec<EdgeRef>,
    pub removed: Vec<EdgeRef>,
    pub modified: Vec<EdgeDiff>,
    pub unchanged: Vec<EdgeDiff>,
}

/// The complete result of a structural diff between two OMTSF files.
pub struct DiffResult {
    pub nodes: NodesDiff,
    pub edges: EdgesDiff,
    pub warnings: Vec<String>,
}

/// Summary statistics.
pub struct DiffSummary {
    pub nodes_added: usize,
    pub nodes_removed: usize,
    pub nodes_modified: usize,
    pub nodes_unchanged: usize,
    pub edges_added: usize,
    pub edges_removed: usize,
    pub edges_modified: usize,
    pub edges_unchanged: usize,
}

impl DiffResult {
    /// Computes summary counts directly from the classified vectors.
    pub fn summary(&self) -> DiffSummary;

    /// Returns `true` iff no additions, removals, or modifications exist.
    /// The CLI uses this to choose between exit code 0 and 1.
    pub fn is_empty(&self) -> bool;
}
```

Matched pairs with zero property changes, zero identifier changes, and zero label changes are classified as `unchanged` rather than `modified`. This distinction allows the summary to report "3 unchanged" without the caller having to re-inspect each `NodeDiff`.

### 4.3 Library Entry Points

```rust
/// Compares two parsed OMTSF files and returns a full diff.
/// File A is the baseline ("before"); file B is the target ("after").
pub fn diff(a: &OmtsFile, b: &OmtsFile) -> DiffResult;

/// Compares two parsed OMTSF files with an optional filter.
pub fn diff_filtered(a: &OmtsFile, b: &OmtsFile, filter: Option<&DiffFilter>) -> DiffResult;
```

`diff()` delegates to `diff_filtered()` with `None`. The filter struct:

```rust
pub struct DiffFilter {
    pub node_types: Option<HashSet<String>>,   // restrict to these node types; None = all
    pub edge_types: Option<HashSet<String>>,   // restrict to these edge types; None = all
    pub ignore_fields: HashSet<String>,        // skip these property names during comparison
}
```

Filtering by node type cascades to edges: when `node_types` is set, edges whose source or target has a filtered-out type are excluded. This ensures that filtering to `organization` nodes also excludes `operates` edges pointing at `facility` nodes. The `ignore_fields` set applies to both node and edge property comparison, using the field name string (e.g., `"valid_from"`, `"data_quality.confidence"`).

---

## 5. Output Formats

### 5.1 Human-Readable (Default)

The default output is a text format inspired by unified diff, adapted for graph structures. Each changed element gets a block. Header lines identify the element; body lines show property changes.

```
--- a/node org-bolt (organization) "Bolt Supplies Ltd"
+++ b/node org-bolt-v2 (organization) "Bolt Supplies Ltd"
  matched by: duns:234567890
  jurisdiction: GB  (unchanged)
~ name: "Bolt Supplies Ltd" -> "Bolt Supplies Limited"
+ identifier: gln:5060012340001
- label: {com.acme.risk-tier: low}
+ label: {com.acme.risk-tier: medium}

+ node org-newco (organization) "NewCo Holdings AG"

- edge edge-004 (ownership) org-acme -> org-bolt
  percentage: 51.0, direct: true

~ edge edge-001 (supplies) org-bolt -> org-acme
  ~ commodity: "7318.15" -> "7318.15.90"
  + tier: 1

=== Summary ===
Nodes:  1 added, 0 removed, 1 modified, 3 unchanged
Edges:  0 added, 1 removed, 1 modified, 2 unchanged
```

Prefixes: `+` = added, `-` = removed, `~` = modified, no prefix = context. Modified elements show both graph-local IDs when they differ across files.

### 5.2 JSON (Machine-Readable)

With `--format json`, the CLI serializes the `DiffResult` directly as JSON to stdout. The structure mirrors the Rust types in Section 4. Property values are serialized as their JSON types. The output is a single JSON object suitable for programmatic consumption.

```json
{
  "summary": {
    "nodes_added": 1,
    "nodes_removed": 0,
    "nodes_modified": 1,
    "nodes_unchanged": 3,
    "edges_added": 0,
    "edges_removed": 1,
    "edges_modified": 1,
    "edges_unchanged": 2
  },
  "nodes": {
    "added": [],
    "removed": [],
    "modified": []
  },
  "edges": {
    "added": [],
    "removed": [],
    "modified": []
  },
  "warnings": []
}
```

---

## 6. CLI Integration

The `omtsf diff <a> <b>` command reads two `.omts` files, parses both into `OmtsFile` values, calls `diff` (or `diff_filtered` if filters are provided), and writes the result to stdout.

| Flag | Effect |
|------|--------|
| `--format text` | Human-readable output (default) |
| `--format json` | JSON output |
| `--summary-only` | Print only the summary line, no per-element details |
| `--node-type <type>` | Restrict diff to nodes of this type (repeatable) |
| `--edge-type <type>` | Restrict diff to edges of this type (repeatable) |
| `--ignore-field <field>` | Exclude this property from comparison (repeatable) |

**Direction convention.** File A is the baseline ("before"), file B is the target ("after"). Additions are elements present in B but not A. Deletions are elements present in A but not B. This matches `git diff <old> <new>` semantics.

**Exit codes:**

| Code | Meaning |
|------|---------|
| 0 | Diff computed successfully, files are identical |
| 1 | Diff computed successfully, files differ |
| 2 | One or both files failed to parse |

This follows the `diff(1)` convention: exit code 1 means "differences found," not "error."

---

## 7. Edge Cases

**Version mismatch.** If the two files declare different `omtsf_version` values, the engine emits a warning but proceeds. Property comparison still works -- unknown fields in either file are captured as `Value` and compared structurally.

**Empty files.** A file with zero nodes and zero edges is valid. Diffing it against a populated file produces all nodes and edges as additions (or deletions, depending on argument order).

**Boundary refs.** `boundary_ref` nodes are matched by their `opaque` scheme identifier, like any other node. Property comparison applies to whatever fields are present on the stub.

**`same_as` edges.** Per the merge-identity table, `same_as` edges are never matched across files. Every `same_as` edge in A appears as a deletion, and every `same_as` edge in B appears as an addition.

**Header fields.** The diff engine compares graph elements (nodes and edges), not file header fields. Changes to `snapshot_date`, `file_salt`, `reporting_entity`, or other header fields are outside the scope of the structural diff. The CLI MAY report header differences as a separate informational section in the human-readable output, but this is a formatting concern, not an engine concern.

**Extension types.** Nodes and edges with extension types (reverse-domain notation) participate in matching and comparison like any core type. Extension edge types have no entry in the merge-identity property table, so they fall back to identifier-only matching.
