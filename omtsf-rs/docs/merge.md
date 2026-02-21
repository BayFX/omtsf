# omtsf-core Technical Specification: Merge Engine

**Status:** Draft
**Date:** 2026-02-21

---

## 1. Purpose

This document specifies the merge engine in `omtsf-core`, implementing SPEC-003: given two or more `.omts` files describing overlapping supply chain subgraphs, produce a single deduplicated graph. The engine guarantees commutativity, associativity, and idempotency (SPEC-003 S5) so that independent parties merging overlapping files in any order converge on an identical result.

---

## 2. Union-Find for Identity Resolution

### 2.1 Data Structure

Node identity resolution uses a union-find (disjoint set) with path halving and union-by-rank. Each node from the concatenated input receives a slot in a flat `Vec<usize>` parent array, indexed by a dense ordinal assigned during concatenation.

```rust
pub struct UnionFind {
    parent: Vec<usize>,
    rank: Vec<u8>,
}

impl UnionFind {
    pub fn new(n: usize) -> Self { ... }
    pub fn find(&mut self, mut x: usize) -> usize { ... }
    pub fn union(&mut self, a: usize, b: usize) { ... }
}
```

Path halving links each visited node to its grandparent during `find`, achieving the inverse-Ackermann amortized bound without recursion. Union-by-rank breaks ties deterministically: when ranks are equal, the **lower ordinal** becomes root. This ensures `find` returns the same representative regardless of operation ordering -- critical for commutativity.

**Why union-find.** BFS/DFS connected-component algorithms require materializing the full match graph before computing components. Union-find processes each match pair incrementally in O(alpha(n)) amortized time. The deterministic tie-breaking rule eliminates the post-hoc canonicalization step that BFS/DFS would require.

### 2.2 Feeding Identity Predicates into Union-Find

The engine builds an **identifier index**: `HashMap<CanonicalId, Vec<usize>>` mapping each canonical identifier string to node ordinals carrying that identifier. Construction is O(total identifiers) with a single pass.

For each key mapping to 2+ nodes, the engine evaluates the full identity predicate pairwise and unions matching pairs. The `internal` scheme is excluded from the index entirely. ANNULLED LEIs are excluded at construction time via `is_lei_annulled`, which checks `extra["entity_status"] == "ANNULLED"`.

After all unions complete, the union-find implicitly represents the transitive closure (SPEC-003 S4 step 3): if X matches Y via one identifier and Y matches Z via another, `find(X) == find(Z)`.

**File-scoped node IDs.** Node IDs are file-local. The pipeline resolves each edge's source/target through the per-file ID map of the owning file, not a global map that would clobber entries from later files.

---

## 3. Identity Predicates

### 3.1 Node Identity Predicate

```rust
pub fn identifiers_match(a: &Identifier, b: &Identifier) -> bool {
    if a.scheme == "internal" || b.scheme == "internal" { return false; }
    if a.scheme != b.scheme { return false; }
    if a.value.trim() != b.value.trim() { return false; }
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

The predicate is symmetric by construction: every comparison is symmetric (string equality, case-insensitive equality, interval overlap). Temporal compatibility rejects pairs only when both carry `valid_to` and one ends strictly before the other begins. Missing temporal fields assume compatibility.

### 3.2 Edge Identity Predicate

Two edges are merge candidates when: (1) source nodes share a union-find group, (2) target nodes share a group, (3) types are equal, (4) they share an external identifier OR lack external identifiers and their type-specific identity properties match per SPEC-003 S3.1.

The `edge_identity_properties_match` function encodes the per-type table as an exhaustive match. Floating-point properties (`percentage`) use `to_bits()` comparison rather than IEEE 754 equality, avoiding `NaN == NaN` and `-0.0 == +0.0` traps. `same_as` edges are always excluded.

### 3.3 Performance

The `CanonicalId` newtype (percent-encoding colons, percent signs, newlines, carriage returns) serves as the hash key. Edge candidate detection uses `EdgeCompositeKey`:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EdgeCompositeKey {
    pub source_rep: usize,
    pub target_rep: usize,
    pub edge_type: EdgeTypeTag,
}
```

This groups edges by resolved endpoints and type in O(E). For N nodes, I identifiers, E edges: total merge cost is O(I * B + E) amortized, where B is the average index bucket size (typically 2).

---

## 4. Property Merge Strategy

### 4.1 Conflict Resolution

Differing values are not resolved -- all distinct values are recorded with provenance:

```rust
pub struct Conflict { pub field: String, pub values: Vec<ConflictEntry> }
pub struct ConflictEntry { pub value: serde_json::Value, pub source_file: String }

pub fn merge_scalars<T: Serialize + Clone>(
    inputs: &[(Option<T>, &str)],
) -> ScalarMergeResult<T> { ... }
```

Entries are sorted by `(source_file, json_value_as_string)` and deduplicated. When a property is present in some sources and absent in others, the present value wins without conflict.

### 4.2 Per-Property-Type Merge

- **Scalars:** `resolve_scalar_merge` returns `(Option<T>, Option<Conflict>)`.
- **Identifiers:** Set union via `merge_identifiers`, deduplicated by `CanonicalId`, sorted lexicographically.
- **Labels:** Set union of `{key, value}` pairs, sorted by key then value (`None` before `Some`).
- **Graph-local ID:** Assigned sequentially (`n-0`, `n-1`, ...) after sorting groups by lowest canonical identifier.

### 4.3 Provenance Tracking

```rust
pub struct MergeMetadata {
    pub source_files: Vec<String>,
    pub reporting_entities: Vec<String>,
    pub timestamp: String,
    pub merged_node_count: usize,
    pub merged_edge_count: usize,
    pub conflict_count: usize,
}
```

When source files declare different `reporting_entity` values, the merged header omits `reporting_entity` and records all values in `reporting_entities`. Lists are sorted and deduplicated.

---

## 5. Determinism Guarantees

Every merge output must be byte-identical for the same inputs regardless of argument order. Seven invariants enforce this:

1. **Identifier sort:** canonical string, lexicographic UTF-8 byte order.
2. **Node output order:** `(min_canonical_id, representative_ordinal)`.
3. **Edge output order:** `(source_canonical, target_canonical, type, edge_canonical, rep_ordinal)`.
4. **Conflict order:** sorted by `field`, then `(source_file, json_value)`.
5. **Label sort:** `(key, value)`, absent values before present.
6. **`source_files`:** sorted and deduplicated.
7. **Union-find stability:** lower-ordinal-wins tie-breaking.

### 5.1 Canonical Ordering

The node ordering deserves elaboration. After computing merge groups, each group is assigned a sort key consisting of the lexicographically smallest canonical identifier string across all nodes in the group. Groups with no external identifiers use their representative ordinal as a tiebreaker. This key is input-order-independent because canonical strings are properties of the identifiers themselves, not of the position they appeared in the input. The same group, discovered via any ordering of input files, produces the same minimum canonical string.

Edge ordering follows the same principle: edges are sorted by the canonical keys of their resolved source and target node groups, then by type string, then by the lowest canonical identifier on the edge itself.

### 5.2 Hash-Based Verification

The `stable_hash` function provides a verification mechanism:

1. Serialize the merged `OmtsFile` to canonical JSON.
2. Zero non-deterministic fields: `file_salt` (randomly generated) and `merge_metadata.timestamp` (wall-clock dependent).
3. Compute SHA-256 of the resulting byte string.

Two outputs are equivalent when their stable hashes match. For idempotency, byte comparison is insufficient because the engine reassigns graph-local IDs and generates fresh salt. Instead, `assert_structurally_equal` compares:

- **Node partition:** the set of canonical-identifier-sets across nodes. Each node in the original matches exactly one node in the merged output by its identifier set.
- **Edge connectivity:** the set of `(src_cid_set, tgt_cid_set, type)` triples, resolving endpoints through canonical identifiers rather than graph-local IDs.

---

## 6. Algebraic Property Tests

Tests use `proptest` with a strategy generating small graphs (1-6 nodes, 0-20 edges) with DUNS identifiers from a shared pool of 6 values. DUNS avoids check-digit complexity of LEI/GLN.

```rust
proptest! {
    fn merge_is_commutative(a in arb_omts_file(), b in arb_omts_file()) {
        let ab = merge(&[a.clone(), b.clone()])?;
        let ba = merge(&[b, a])?;
        prop_assert_eq!(stable_hash(&ab.file), stable_hash(&ba.file));
    }

    fn merge_is_associative(a in arb_omts_file(), b in arb_omts_file(), c in arb_omts_file()) {
        let ab_c = merge(&[merge(&[a.clone(), b.clone()])?.file, c.clone()])?;
        let a_bc = merge(&[a, merge(&[b, c])?.file])?;
        prop_assert_eq!(stable_hash(&ab_c.file), stable_hash(&a_bc.file));
    }

    fn merge_is_idempotent(a in arb_omts_file()) {
        let aa = merge(&[a.clone(), a.clone()])?;
        assert_structurally_equal(&a, &aa.file);
    }
}
```

### 6.1 Why the Properties Hold

**Commutativity** holds because: (a) the identifier index is a set operation -- the same pairs appear regardless of file order; (b) union-find representative selection uses the deterministic lower-ordinal rule, and the node-ordering step (Section 5.1) produces the same ordinal assignment once groups are sorted by canonical key; (c) all output arrays use canonical sort orders independent of input order.

**Associativity** holds because: (a) transitive closure over identifiers is determined by the full set of overlap relationships, not discovery order; (b) merging a previously-merged file re-discovers the same overlaps (merged files retain the union of source identifiers); (c) conflict entries carry `source_file` provenance with sorted deduplication producing identical sets regardless of grouping.

**Idempotency** holds because: (a) merging a file with itself creates node pairs with identical identifiers -- every pair satisfies the predicate; (b) the merged group retains the same identifier set (union of identical sets); (c) scalar properties agree, producing no conflicts.

### 6.2 Regression Fixtures

Hand-crafted fixture pairs in `omtsf-cli/tests/` cover: disjoint graphs, full overlap, conflicting properties, transitive chains (A-B via LEI, B-C via DUNS), `same_as` at each confidence level, temporal incompatibility, ANNULLED LEI exclusion, oversized groups, and colliding file-local node IDs.

---

## 7. `same_as` Handling

### 7.1 Transitivity Closure via Union-Find

`same_as` edges are processed after the identifier-based pass via `SameAsThreshold`:

```rust
pub enum SameAsThreshold {
    Definite,   // only "definite" (default)
    Probable,   // "definite" and "probable"
    Possible,   // all three levels
}
```

Absent confidence is treated as `"possible"`. When honoured, the engine calls `uf.union(src_ord, tgt_ord)` on the existing structure. Transitive closure is inherent: `same_as(A,B)` + `same_as(B,C)` yields `find(A) == find(C)` after two unions. This also composes with identifier-based merges: if A and B share an LEI and B and C are linked by `same_as`, all three unify.

`same_as` edges are semantically symmetric and never merged with other edges. Cycles (A->B, B->C, C->A) are redundant unions handled idempotently. Honoured edges are retained in output with rewritten endpoints as provenance records.

The `apply_same_as_edges` function extracts confidence from `properties.extra["confidence"]`, falling back to `edge.extra["confidence"]`, to accommodate both the properties-wrapper and top-level serialization patterns:

```rust
pub fn apply_same_as_edges<'a, F>(
    edges: &'a [Edge],
    node_id_to_ordinal: F,
    uf: &mut UnionFind,
    threshold: SameAsThreshold,
) -> Vec<&'a Edge>
where
    F: Fn(&str) -> Option<usize>,
```

Honoured edges are collected and returned so the caller can record which groups were extended by `same_as`, supporting merge provenance auditing (SPEC-003 S6). The ordering -- `same_as` after identifier-based unions -- is logically irrelevant (union-find is order-independent) but enables this reporting distinction.

---

## 8. The Eight-Step Merge Procedure

The `merge_with_config` function orchestrates the full SPEC-003 S4 procedure:

1. **Concatenate** all nodes into a flat `Vec<Node>`, tracking origins and building per-file `HashMap<&str, usize>` ID maps.
2. **Build identifier index**, filtering out `internal` and ANNULLED LEIs. Evaluate pairwise predicates and union matching pairs.
3. **Apply `same_as` edges** to the union-find, gated by `MergeConfig::same_as_threshold`.
4. **Check merge-group safety limits.** Emit `MergeWarning::OversizedMergeGroup` for any group exceeding `MergeConfig::group_size_limit` (default: 50).
5. **Merge each node group.** Union identifiers, union labels, merge scalars (agree or conflict), assign deterministic new node ID.
6. **Rewrite edge references** through per-file ID maps to global ordinals, then to union-find representatives, then to new merged node IDs. Build the edge candidate index.
7. **Deduplicate edges.** Pairwise `edges_match` within each bucket; second union-find for edge groups. Merge identifiers, labels; retain representative's scalar properties.
8. **Emit output file** with merged nodes, merged edges, fresh `file_salt`, latest `snapshot_date`, `merge_metadata`. Run L1 validation; return `MergeError::PostMergeValidationFailed` on failure.

---

## 9. Post-Merge Validation

The merged output must pass L1 validation (SPEC-003 S5.1). Key invariants enforced:

- **No duplicate node IDs** -- sequential assignment (`n-0`, `n-1`, ...) by construction.
- **All edge references resolve** -- rewriting through `rep_to_new_id`; dangling edges dropped.
- **Identifier uniqueness per node** -- deduplication by canonical string in step 5.
- **Graph type constraints** (SPEC-001 S9.5) preserved -- merge does not change node/edge types.

L2 and L3 rules are re-evaluated as warnings. Two L3 rules are merge-specific:

- **L3-MRG-01:** Sum inbound `ownership` edge `percentage` values per node (considering temporal overlap). Warn if > 100.
- **L3-MRG-02:** Extract the `legal_parentage` subgraph and verify it forms a forest (no directed cycles).

Merge-group safety limits (SPEC-003 S4.1) emit `MergeWarning::OversizedMergeGroup` when any group exceeds the configured limit, helping operators detect false-positive cascades from erroneous identifier matches.
