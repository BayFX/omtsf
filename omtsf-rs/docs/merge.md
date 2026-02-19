# omtsf-cli Technical Specification: Merge Engine

**Status:** Draft
**Date:** 2026-02-19

---

## 1. Purpose

This document specifies the design of the merge engine in `omtsf-core`. The merge engine implements SPEC-003: given two or more `.omts` files describing overlapping supply chain subgraphs, it produces a single graph that is the deduplicated union. The engine must guarantee commutativity, associativity, and idempotency so that independent parties merging overlapping files in any order converge on an identical result.

The merge engine is the most algorithmically dense component in `omtsf-core`. It combines union-find for identity resolution, predicate-based candidate detection with indexed lookups, deterministic property conflict recording, and post-merge validation. This specification covers data structures, algorithms, ordering guarantees, and testing strategy.

---

## 2. Union-Find for Identity Resolution

### 2.1 Data Structure

Node identity resolution uses a union-find (disjoint set) structure. Each node from the concatenated input receives a slot in a flat `Vec<usize>` parent array, indexed by a dense node ordinal assigned during concatenation. The structure uses both path compression (path halving variant) and union-by-rank to achieve near-constant amortized time per operation.

```rust
struct UnionFind {
    parent: Vec<usize>,
    rank: Vec<u8>,
}

impl UnionFind {
    fn find(&mut self, mut x: usize) -> usize { ... }
    fn union(&mut self, a: usize, b: usize) { ... }
}
```

Path compression uses the iterative path-halving technique (each node points to its grandparent during `find`) rather than full recursive compression. This avoids stack depth concerns on large graphs while still achieving the inverse-Ackermann amortized bound. Union-by-rank breaks ties deterministically: when ranks are equal, the lower ordinal becomes the root. This ensures that `find` returns the same representative regardless of operation ordering, which is critical for commutativity.

### 2.2 Feeding Identity Predicates into Union-Find

Before union-find operations begin, the engine builds an **identifier index**: a `HashMap<CanonicalId, Vec<usize>>` mapping each canonical identifier string to the list of node ordinals carrying that identifier. Construction is O(total identifiers) with a single pass over all nodes.

For each canonical identifier key that maps to two or more nodes, the engine evaluates the full identity predicate (scheme match, value match, authority match, temporal compatibility) for each pair. Pairs that satisfy the predicate are unioned. The `internal` scheme is excluded from the index entirely -- `internal` identifiers are never inserted into the map.

Temporal compatibility is evaluated pairwise: for two identifier records sharing the same `(scheme, value, authority)` tuple, the engine checks whether their validity intervals overlap. If both carry `valid_to` and the earlier `valid_to` precedes the later `valid_from`, the pair is rejected. If either record omits temporal fields entirely, compatibility is assumed.

ANNULLED LEIs are excluded at index-construction time. When the engine encounters an LEI identifier whose `verification_status` is known to correspond to ANNULLED (detectable via L2 enrichment data if available), it skips index insertion for that record.

After all pairwise unions complete, the union-find implicitly represents the transitive closure: if node X matches Y via one identifier and Y matches Z via another, `find(X) == find(Z)`.

---

## 3. Identity Predicates

### 3.1 Node Identity

The node identity predicate is a pure function over two identifier records:

```rust
fn identifiers_match(a: &IdentifierRecord, b: &IdentifierRecord) -> bool {
    if a.scheme == "internal" || b.scheme == "internal" {
        return false;
    }
    if a.scheme != b.scheme {
        return false;
    }
    if a.value.trim() != b.value.trim() {
        return false;
    }
    if a.authority.is_some() || b.authority.is_some() {
        match (&a.authority, &b.authority) {
            (Some(aa), Some(ba)) => {
                if !aa.eq_ignore_ascii_case(ba) { return false; }
            }
            _ => return false,
        }
    }
    temporal_compatible(&a, &b)
}
```

The predicate is symmetric by construction: every comparison is symmetric (string equality, case-insensitive equality, interval overlap). This symmetry is the foundation of the commutativity guarantee.

### 3.2 Edge Identity

Edge identity is evaluated after node merge groups are resolved. Two edges are merge candidates when:

1. Their source nodes belong to the same union-find group
2. Their target nodes belong to the same union-find group
3. Their `type` fields are equal
4. They share an external identifier (same predicate as nodes), OR they lack external identifiers and their merge-identity properties match per the type-specific table in SPEC-003 Section 3.1

`same_as` edges are excluded: they are never merged with other edges.

### 3.3 Performance: Indexing and Hashing

The canonical identifier string serves as the hash key for the identifier index. The `CanonicalId` type is a newtype over `String` that enforces percent-encoding of colons (`%3A`), percent signs (`%25`), newlines (`%0A`), and carriage returns (`%0D`) at construction time. Hashing uses the default `SipHash-1-3` provided by the standard library `HashMap`, which is sufficient for non-adversarial inputs. For adversarial-input resilience (fuzz testing), the engine should not assume uniform hash distribution.

Edge candidate detection uses a composite key: `(find(source), find(target), type)` mapped to a `Vec<usize>` of edge ordinals. This groups edges by resolved endpoints and type in O(total edges), after which pairwise comparison within each bucket handles identifier and property matching.

---

## 4. Property Merge Strategy

### 4.1 Conflict Resolution

When a merge group contains nodes (or edges) with differing values for the same property, the engine does not pick a winner. Instead it records all distinct values with provenance:

```rust
struct Conflict {
    field: String,
    values: Vec<ConflictEntry>,
}

struct ConflictEntry {
    value: serde_json::Value,
    source_file: String,
}
```

The `_conflicts` array is appended to the merged node at the top level (for nodes) or inside `properties` (for edges). Conflict entries are sorted by `source_file` lexicographically, then by the JSON-serialized form of `value`, ensuring deterministic output.

### 4.2 Per-Property-Type Merge Functions

- **Scalar properties (name, country, etc.):** If all source values are equal, retain the value. If they differ, omit the property from the merged output and record a conflict.
- **Identifiers array:** Set union. Deduplicate by canonical string. Sort the merged array by canonical string in lexicographic UTF-8 byte order.
- **Labels array:** Set union of `{key, value}` pairs. Sort by `key` ascending, then `value` ascending, with absent values (key present, value null) sorting before present values.
- **Graph-local `id`:** Assigned by the engine. The engine uses the canonical identifier of the group representative (lowest canonical identifier by sort order) as a deterministic seed, but the `id` value itself is opaque and file-local.

### 4.3 Provenance Tracking

The merged file header includes a `merge_metadata` object:

```rust
struct MergeMetadata {
    source_files: Vec<String>,
    reporting_entities: Vec<String>,
    timestamp: String,           // ISO 8601
    merged_node_count: usize,
    merged_edge_count: usize,
    conflict_count: usize,
}
```

When source files declare different `reporting_entity` values, the merged header omits `reporting_entity` and records both values in `merge_metadata.reporting_entities`.

---

## 5. Determinism Guarantees

Every merge output must be byte-identical for the same set of inputs regardless of argument order or grouping. The following invariants enforce this:

1. **Identifier sort order.** After merge, each node's `identifiers` array is sorted by canonical string in lexicographic UTF-8 byte order. This is the primary ordering mechanism.

2. **Node output order.** Merged nodes are emitted sorted by their lowest canonical identifier string. Nodes with no external identifiers are sorted by their assigned `id`.

3. **Edge output order.** Edges are sorted by `(find(source) canonical, find(target) canonical, type, lowest edge canonical identifier)`.

4. **Conflict array order.** Conflicts within a node are sorted by `field` name. Values within a conflict are sorted by `(source_file, json_value)`.

5. **Label sort order.** Labels sorted by `(key, value)` with absent values before present.

6. **`source_files` in `merge_metadata`.** Sorted lexicographically.

These orderings collectively guarantee that `merge(A, B)` and `merge(B, A)` produce byte-identical JSON output. Implementations can verify determinism with a SHA-256 hash of the serialized output.

---

## 6. Algebraic Property Tests

The three algebraic guarantees -- commutativity, associativity, idempotency -- are the highest-priority test targets. Property-based testing with `proptest` is the primary verification strategy.

### 6.1 Graph Generation Strategy

The `proptest` strategy generates small `.omts` graphs (1-30 nodes, 0-50 edges) with controlled identifier overlap:

```rust
fn arb_omts_file() -> impl Strategy<Value = OmtsFile> {
    // Generate nodes with random subsets of identifiers
    // drawn from a shared pool, ensuring some overlap
    // between generated files.
    // Edge types drawn from the 16 core types.
    // Properties populated with small random values.
}
```

The identifier pool is shared across generated files to ensure merge candidates exist. The pool contains identifiers from multiple schemes (`lei`, `duns`, `gln`, `nat-reg`) with occasional temporal bounds.

### 6.2 Test Properties

**Commutativity:**
```rust
proptest! {
    fn merge_is_commutative(a in arb_omts_file(), b in arb_omts_file()) {
        let ab = merge(&a, &b);
        let ba = merge(&b, &a);
        assert_eq!(sha256(&serialize(&ab)), sha256(&serialize(&ba)));
    }
}
```

**Associativity:**
```rust
proptest! {
    fn merge_is_associative(
        a in arb_omts_file(),
        b in arb_omts_file(),
        c in arb_omts_file(),
    ) {
        let ab_c = merge(&merge(&a, &b), &c);
        let a_bc = merge(&a, &merge(&b, &c));
        assert_eq!(sha256(&serialize(&ab_c)), sha256(&serialize(&a_bc)));
    }
}
```

**Idempotency:**
```rust
proptest! {
    fn merge_is_idempotent(a in arb_omts_file()) {
        let aa = merge(&a, &a);
        // Structural equivalence: same nodes, edges,
        // identifiers, properties. Graph-local IDs may differ.
        assert_structurally_equal(&a, &aa);
    }
}
```

Idempotency uses structural comparison rather than byte equality because the merge engine reassigns graph-local `id` values. Structural equality checks that the set of canonical identifiers, properties, and edge connectivity are identical.

### 6.3 Regression Fixtures

In addition to property tests, the `tests/` directory contains hand-crafted `.omts` fixture pairs covering:
- Disjoint graphs (no merge candidates)
- Full overlap (identical files)
- Partial overlap with conflicting properties
- Transitive merge chains (A-B via LEI, B-C via DUNS)
- `same_as` edges at each confidence level
- Temporal incompatibility preventing merge
- ANNULLED LEI exclusion
- Large merge groups triggering safety warnings

---

## 7. `same_as` Handling

### 7.1 Integration with Union-Find

`same_as` edges are processed after the identifier-based union-find pass. The engine scans all edges of type `same_as` and evaluates each against the configured confidence threshold:

- `definite`: always honored (SHOULD per spec)
- `probable`: honored if threshold is `probable` or lower
- `possible`: honored only if threshold is `possible`

The default threshold is `definite`. When a `same_as` edge is honored, the engine calls `union(find(source), find(target))` on the existing union-find structure. Because union-find inherently computes transitive closure, no separate closure step is needed: if A `same_as` B and B `same_as` C, all three end up in the same group after two union operations.

### 7.2 Cycle Considerations

`same_as` is semantically symmetric and forms an undirected equivalence relation. Cycles in `same_as` edges (A->B, B->C, C->A) are not problematic -- they are redundant unions that union-find handles idempotently. However, `same_as` edges themselves are never merged or deduplicated. They are retained in the output as advisory provenance records, with their `source` and `target` references rewritten to the merged node IDs.

### 7.3 Interaction with Identifier-Based Merge

`same_as` unions are applied after identifier-based unions. This ordering is logically irrelevant (union-find is order-independent) but allows the engine to report which merge groups were formed by identifiers alone versus which were extended by `same_as` edges. This distinction is recorded in merge metadata for auditability.

---

## 8. Post-Merge Validation and Graph Invariants

After the eight-step merge procedure completes, the engine runs L1 validation on the merged output. The implementation must not emit a file that violates L1 rules. Key checks:

- **No duplicate node IDs.** The engine assigns IDs, so this is enforced by construction.
- **All edge references resolve.** Edge source/target rewriting in step 5 uses the union-find representative, guaranteeing resolution.
- **Identifier uniqueness per node.** The deduplication in step 4 (union by canonical string) prevents duplicate identifier records.

L2 and L3 rules are re-evaluated and reported as warnings. Two L3 rules are merge-specific:

- **L3-MRG-01:** For each node, sum inbound `ownership` edge `percentage` values (considering temporal overlap). Warn if the sum exceeds 100.
- **L3-MRG-02:** Extract the `legal_parentage` subgraph and verify it forms a forest (no directed cycles). The engine runs a topological sort on this subgraph; if it fails, it reports the cycle.

### 8.1 Merge-Group Safety Limits

After transitive closure, the engine computes the size of each merge group. If any group exceeds a configurable threshold (default: 50 nodes), the engine emits a warning identifying the group, its member nodes, and the bridging identifiers that caused the transitive chain. This helps operators detect false-positive cascades where a single erroneous identifier match pulls unrelated entities into a single group.
