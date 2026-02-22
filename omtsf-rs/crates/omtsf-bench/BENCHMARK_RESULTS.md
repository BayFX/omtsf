# Benchmark Results

Collected on 2026-02-22 using `cargo bench` (Criterion 0.5, default sample sizes).

CBOR backend: **cbor4ii 1.x** (replaced ciborium 0.2 — see CBOR Library Comparison below).

Phase 11 performance optimizations applied (T-059 through T-069).

## Test Data Profiles

| Tier | Nodes   | Edges     | Total Elements | JSON Size | CBOR Size | CBOR/JSON |
|------|--------:|----------:|---------------:|----------:|----------:|:---------:|
| S    |      50 |        91 |            141 |    28 KB  |    22 KB  |   0.79    |
| M    |     500 |       982 |          1,482 |   285 KB  |   225 KB  |   0.79    |
| L    |   2,000 |     3,948 |          5,948 | 1,201 KB  |   943 KB  |   0.79    |
| XL   |   5,000 |    10,007 |         15,007 | 3,194 KB  | 2,507 KB  |   0.78    |
| Huge | 736,550 | 1,489,886 |      2,226,436 |   500 MB  |   400 MB  |   0.80    |

All generation is deterministic (seed=42). XL hits the ~5 MB target.
Huge tier is a 20-tier supply chain generated once to disk (`just gen-huge`)
and loaded by the benchmark harness.

CBOR is consistently ~21% smaller than compact JSON across all tiers, because
CBOR encodes map keys and short strings more efficiently (no quoting, varint
lengths).

---

## Group 1: Parse & Serialize

### JSON

| Operation        |   S    |   M     |    L     |    XL    | Throughput     |
|------------------|-------:|--------:|---------:|---------:|----------------|
| Deserialize      | 162 us | 1.84 ms | 11.4 ms  | 32.8 ms  | 97-173 MiB/s   |
| Serialize compact|  52 us |  547 us |  2.36 ms |  6.64 ms | 480-540 MiB/s  |
| Serialize pretty |  95 us |  980 us |  4.10 ms | 11.6 ms  | ~442-474 MiB/s |

### CBOR (cbor4ii)

| Operation        |   S    |   M     |    L     |    XL    | Throughput        |
|------------------|-------:|--------:|---------:|---------:|-------------------|
| Decode           | 163 us | 1.82 ms |  8.49 ms | 27.3 ms  | 92-135 MiB/s      |
| Encode           |  34 us |  354 us |  1.52 ms |  4.39 ms | 571-646 MiB/s     |

### JSON vs CBOR Comparison

| Operation   | JSON (L) | CBOR (L) | CBOR/JSON | Notes                            |
|-------------|--------:|---------:|:---------:|:---------------------------------|
| Deserialize | 11.4 ms  |  8.49 ms |   0.74x   | CBOR 26% faster than JSON        |
| Serialize   |  2.36 ms |  1.52 ms |   0.64x   | CBOR 36% faster than JSON        |

**Analysis:** After Phase 11 optimizations (visitor-based deserialization for enum tags,
`TryFrom<String>` for newtypes to avoid double allocation), both JSON and CBOR deserialization
are significantly faster. CBOR decode is now **26% faster than JSON** at L tier (was 20%
slower pre-optimization). CBOR encode remains 36% faster than JSON.

The visitor-based deserialization (T-063) eliminates intermediate String allocations for
known enum variants, benefiting CBOR more than JSON since CBOR's Content-buffering path
previously amplified allocation overhead.

## Group 2: Graph Construction

| Tier |  Time  | Throughput   |
|------|-------:|--------------|
| S    |  29 us | 4.9 Melem/s  |
| M    | 293 us | 5.1 Melem/s  |
| L    | 1.40 ms| 4.3 Melem/s  |
| XL   | 4.43 ms| 3.4 Melem/s  |

`build_graph` sustains ~3.4-5.1 million elements/sec. T-065 eliminated ~3M String
allocations per XL build by using `&str` borrows for HashMap lookups and deferring
`to_string()` calls until after duplicate checks pass. Throughput improved ~20% at XL
(was 2.9 Melem/s). Now includes building `nodes_by_type` and `edges_by_type` indexes.

## Group 3: Graph Queries

### Reachability (`reachable_from`)

| Variant                |    S   |    M    |    L    |     XL   |
|------------------------|-------:|--------:|--------:|---------:|
| Forward from root      |  4.5 us|  55.3 us|  234 us |   701 us |
| Forward from leaf      |  140 ns|   136 ns|  136 ns |   138 ns |
| Backward from root     |  2.6 us|  31.9 us|  134 us |   376 us |
| Both from mid          |  6.1 us|  77.0 us|  343 us |  1.02 ms |
| Filtered (supplies)    |  571 ns|   3.3 us|  9.6 us |  19.4 us |

T-067 introduced a reusable neighbour buffer, eliminating per-call Vec allocations
in BFS traversals. Leaf queries remain O(1) ~138 ns. Forward root improved ~14% at XL.
Edge-type filtering yields ~36x speedup.

### Shortest Path

| Variant        |    S   |    M    |    L    |     XL   |
|----------------|-------:|--------:|--------:|---------:|
| Root to leaf   |  6.8 us|  88 us  |  365 us |  1.01 ms |
| Root to mid    |  6.0 us|  53 us  |  229 us |   483 us |
| No path        |  159 ns|  154 ns |  154 ns |   156 ns |

No-path detection is O(1). Modest improvements from neighbour buffer reuse (T-067).

### All Paths

| Variant  |    S    |     M    |
|----------|--------:|---------:|
| Depth 5  |  15.4 us|   259 us |
| Depth 10 |  47.4 us|  11.6 ms |

**Massive improvement from T-059**: replaced exponential-cloning IDDFS with push/pop
backtracking DFS using `Vec<bool>` bitset for cycle detection. M/depth_10 dropped from
193 ms to 11.6 ms — a **16.6x speedup**. S/depth_10 dropped from 1.56 ms to 47 us —
a **33x speedup**. The algorithm is now practical for interactive use at M scale.

## Group 4: Subgraph Extraction

### Induced Subgraph

| % Nodes |    S    |    M     |    L     |
|---------|--------:|---------:|---------:|
| 10%     | 12.7 us |   168 us |   756 us |
| 25%     | 34.0 us |   393 us |  1.78 ms |
| 50%     | 68.8 us |   753 us |  3.57 ms |
| 100%    | 110 us  |  1.25 ms |  6.27 ms |

Near-perfect linear scaling with fraction extracted. Full L extraction in ~6 ms.
These numbers include the optimized `assemble_subgraph` which now iterates only
outgoing edges of included nodes instead of all edges in the graph.

### Ego Graph

| Variant      |    S    |    M     |    L     |
|--------------|--------:|---------:|---------:|
| Root radius 1| 32.4 us |   252 us |   828 us |
| Root radius 2| 73.9 us |   681 us |  2.05 ms |
| Root radius 3| 102 us  |  1.07 ms |  3.65 ms |
| Mid radius 2 | 11.7 us |  40.4 us |   112 us |

Mid-node ego graphs are much cheaper than root ego graphs (fewer neighbors). Each
additional radius roughly doubles the cost.

## Group 5: Cycle Detection

| Variant                       |    S    |    M    |    L     |     XL   |
|-------------------------------|--------:|--------:|---------:|---------:|
| Acyclic, all types            | 27.4 us |  308 us |  1.37 ms |  4.06 ms |
| Acyclic, `legal_parentage`    |  8.7 us |   96 us |   376 us |  1.07 ms |
| Cyclic, all types             | 25.8 us |  291 us |  1.31 ms |      --  |
| Cyclic, `legal_parentage`     |  8.7 us |   91 us |   377 us |      --  |

Edge-type filtering yields ~3.5x speedup. Cyclic vs. acyclic performance is nearly
identical -- the algorithm does not short-circuit on first cycle. XL cycle detection
in 4 ms.

## Group 6: Validation

| Level      |    S    |    M     |    L     |     XL    |
|------------|--------:|---------:|---------:|----------:|
| L1 only    | 34 us   |   414 us |  2.08 ms |   7.86 ms |
| L1 + L2    | 58 us   |   742 us |  3.82 ms |  14.8 ms  |
| L1 + L2 + L3 | 59 us |   747 us |  3.80 ms |  14.7 ms  |

L1 validation is fast (proportional to element count). T-064 pre-built a HashMap for
ownership edge lookup in L3-MRG-01, eliminating the O(N*E) scan. L2 adds semantic
checks with the pre-built HashSet for facility-ID lookups (fixed earlier). L3 (cycle
detection) adds negligible overhead on top of L2. Full L1+L2+L3 validation of a
5 MB XL file: 14.7 ms (was 14.9 ms).

## Group 7: Merge Pipeline

| Variant                 |    S     |     M     |     L     |
|-------------------------|--------:|----------:|----------:|
| Self-merge (100% overlap)| 914 us  |  10.9 ms  |  60.1 ms  |
| Disjoint (0% overlap)   | 1.12 ms |  15.5 ms  |  82.6 ms  |
| 3-file merge            | 1.82 ms |  24.9 ms  |       --  |

T-068 eliminated redundant `CanonicalId` recomputation by reusing the existing
identifier index. Marginal improvement (~3%) since merge is dominated by node
matching and output construction.

## Group 8: Redaction

### By scope (retain all nodes)

| Scope   |    S     |    M     |    L     |     XL    |
|---------|--------:|---------:|---------:|----------:|
| Partner |  153 us |  1.87 ms |  8.99 ms |  31.8 ms  |
| Public  |  140 us |  1.68 ms |  8.22 ms |  25.6 ms  |

### Varying retain % (M tier)

| Retain % | Partner  | Public   |
|----------|--------:|---------:|
| 10%      | 1.27 ms | 1.24 ms  |
| 50%      | 1.79 ms | 1.63 ms  |
| 90%      | 1.86 ms | 1.64 ms  |

Public redaction is slightly faster than partner (person nodes are removed entirely,
reducing output). Retain fraction has modest impact -- the bulk of the cost is graph
traversal, not output construction.

## Group 9: Diff

| Variant                  |    S     |    M     |    L     |     XL    |
|--------------------------|--------:|---------:|---------:|----------:|
| Identical                |  316 us |  3.60 ms | 17.4 ms  |  70.3 ms  |
| Disjoint                 |  194 us |  2.20 ms | 10.8 ms  |       --  |
| Filtered (org + supplies)|  110 us |  1.22 ms |  6.13 ms |       --  |

T-060 pre-built HashMap for O(1) node lookups during edge matching (was O(N) per edge).
T-061 replaced `Vec.contains()` with HashSet for O(1) containment checks in active node
sets. T-066 replaced `serde_json::to_value()` tag conversion with direct `as_str()`.
Combined improvement: ~13% at XL for identical diff (was 81.2 ms).

## Group 10: Selector Query

### `selector_match` (scan only, no subgraph assembly)

| Selector           |    S     |    M     |    L     |    XL     | Throughput      |
|--------------------|--------:|---------:|---------:|----------:|-----------------|
| Label key          |  991 ns | 10.1 us  |  68.1 us |  239 us   |  63-142 Melem/s |
| Node type          |  619 ns |  3.6 us  |  12.7 us |  31.3 us  | 228-480 Melem/s |
| Multi (type+label) |  909 ns | 10.4 us  |  56.6 us |  188 us   |  80-155 Melem/s |

T-069 pre-computed lowercased selector patterns, eliminating per-pattern `to_lowercase()`
calls during matching. Label matching improved ~4x (was 4.1 µs at S, now 991 ns). Node-type
matching improved ~2.6x at S. Multi-selector improved ~3x at S. All selector scans complete
under 240 µs at XL.

### `selector_subgraph` (full pipeline: scan + expand + assemble)

| Variant                     |     S     |     M     |     L     |
|-----------------------------|--------:|----------:|----------:|
| Narrow (attestation, exp 0) |  7.1 us |   80.9 us |   343 us  |
| Broad (organization, exp 0) | 66.5 us |   797 us  |  3.38 ms  |
| Expand 1 (attestation)      | 20.2 us |   217 us  |   910 us  |
| Expand 3 (attestation)      | 105 us  |  1.09 ms  |  4.75 ms  |

Narrow selectors (~5% seed match) are ~10x cheaper than broad (~45% seed match).
Type-only selectors use the graph's type index instead of scanning all nodes.
Each expansion hop roughly doubles the cost.

---

## Group 11: Huge Tier (737K nodes, 1.5M edges, 500 MB)

Fixture pre-generated to disk via `just gen-huge`; benchmarks load from
`target/bench-fixtures/huge.omts.json`. Run via `just bench-huge`.

### Parse & Serialize

| Operation        |   Time   | Throughput   |
|------------------|--------:|--------------|
| Deserialize JSON |  4.68 s  | 107 MiB/s    |
| Serialize JSON   |  1.36 s  | 367 MiB/s    |
| Decode CBOR      |  5.01 s  |  80 MiB/s    |
| Encode CBOR      |  892 ms  | 448 MiB/s    |

JSON serialize/deserialize ratio holds at ~3.4x, consistent with smaller tiers.
CBOR decode is 1.07x JSON deserialize — near parity at Huge scale. CBOR encode
is **34% faster** than JSON serialize.

CBOR benchmarks run in a separate binary (`huge_cbor`) to avoid OOM on
memory-constrained machines.

### Graph Construction

| Time   | Throughput    |
|-------:|---------------|
| 1.84 s | 1.21 Melem/s  |

Throughput drops from ~3.4-5.1 Melem/s at XL to ~1.2 Melem/s at Huge -- hash map
resizing and cache pressure dominate at 2.2M elements. Now includes building
type indexes.

### Reachability (`reachable_from`)

| Variant                |    Huge    |
|------------------------|----------:|
| Forward from root      |   413 ms  |
| Filtered (supplies)    |  2.25 ms  |
| Both from mid          |   759 ms  |

Edge-type filtering yields ~184x speedup at this scale (vs ~36x at XL).
Full bidirectional traversal from mid-graph: 759 ms.

### Shortest Path

| Variant        |    Huge    |
|----------------|----------:|
| Root to leaf   |   527 ms  |
| Root to mid    |  64.7 ms  |
| No path        |   158 ns  |

No-path remains O(1) at 158 ns, identical to all smaller tiers.
Root-to-leaf spans 20 tiers in 527 ms.

### Selector Query

#### `selector_match`

| Selector     |   Huge   | Throughput      |
|--------------|--------:|-----------------:|
| Label key    | 82.5 ms  |  27.0 Melem/s   |
| Node type    | 16.8 ms  | 132.5 Melem/s   |
| Multi        | 59.5 ms  |  37.4 Melem/s   |

Label matching drops to ~27 Melem/s at Huge -- cache misses on the
larger label maps dominate.

#### `selector_subgraph`

| Variant                     |   Huge    |
|----------------------------|---------:|
| Narrow (attestation, exp 0) |  237 ms  |
| Narrow (attestation, exp 1) |  708 ms  |
| Narrow (attestation, exp 3) | 4.32 s   |
| Broad (organization, exp 0) | 2.73 s   |
| Broad (organization, exp 1) | 4.24 s   |

Type-index fast path yields improvement on narrow exp 0 (type-only selector
skips the 737K-node linear scan). Broader expansions are dominated by BFS and
subgraph assembly, so the scan optimization is less visible.

### Validation

| Level      |   Huge   | Throughput     |
|------------|--------:|----------------|
| L1 only    |  3.43 s  | 649 Kelem/s    |
| L1+L2+L3   |  5.00 s  | 445 Kelem/s    |

**L1+L2+L3 is now tractable at Huge tier.** The previous O(E*N) bug in
`facility_ids_with_org_connection` caused L2 validation alone to be estimated at
~21,500 s (~6 hours). After fixing it to O(N+E) with a pre-built HashSet,
full L1+L2+L3 completes in 5.0 s -- a >4000x improvement. L2 adds only ~1.6 s
on top of L1.

---

## Group 12: CBOR Library Comparison

Compares decode and encode throughput of **serde_json** (JSON), **ciborium 0.2**
(CBOR, reader-based), and **cbor4ii 1.x** (CBOR, slice-based). `omtsf-core` now
uses cbor4ii in production; ciborium is retained as a dev-dependency for ongoing
regression comparison.

### Decode

| Size | serde_json | ciborium  | cbor4ii  | cbor4ii vs JSON |
|------|----------:|---------:|---------:|:---------------:|
| S    |   196 us  |   402 us |   202 us |      1.03x      |
| M    |  2.18 ms  |  4.14 ms |  2.03 ms |      0.93x      |
| L    |  10.8 ms  |  20.5 ms |  12.9 ms |      1.20x      |

cbor4ii decode is **2.0x faster** than ciborium across all tiers. At S/M, cbor4ii
is at parity with JSON (within 3-7%). At L, cbor4ii is 20% slower than JSON — the
`#[serde(flatten)]` Content-buffering overhead grows with element count.

ciborium's `from_reader()` trait-based byte reads are the dominant bottleneck;
cbor4ii's `from_slice()` direct slice access eliminates per-byte virtual dispatch.

### Encode

| Size | serde_json | ciborium  | cbor4ii  | cbor4ii vs JSON |
|------|----------:|---------:|---------:|:---------------:|
| S    |  55.5 us  |  68.8 us |  40.3 us |      0.73x      |
| M    |   549 us  |   682 us |   414 us |      0.75x      |
| L    |  2.36 ms  |  2.94 ms |  1.80 ms |      0.76x      |

cbor4ii encode is **1.7x faster** than ciborium and **24-27% faster than JSON**
across all tiers. CBOR's more compact encoding (no quoting, varint lengths) means
less data to write, and cbor4ii's direct buffer writes are more efficient than
serde_json's string formatting.

---

## Group 13: `#[serde(flatten)]` Overhead

Measures the deserialization overhead of `#[serde(flatten)]` (which activates
serde's Content-buffering path) vs `#[serde(deny_unknown_fields)]` (direct
deserialization). Both struct variants are structurally identical; the `extra`
field in the `flat` variant is always empty, so the wire payload is bit-for-bit
identical.

### cbor4ii (CBOR)

| Size | flat     | strict   | overhead |
|------|--------:|---------:|:--------:|
| S    | 36.4 us | 34.5 us  |   5.6%   |
| M    |  348 us |  332 us  |   5.0%   |
| L    | 1.60 ms | 1.49 ms  |   7.1%   |

### serde_json (JSON)

| Size | flat     | strict   | overhead |
|------|--------:|---------:|:--------:|
| S    | 40.5 us | 41.4 us  |  -2.3%   |
| M    |  423 us |  411 us  |   2.9%   |
| L    | 1.89 ms | 1.79 ms  |   5.3%   |

CBOR flatten overhead is **5-7%** with cbor4ii — down from 16-20% with ciborium.
JSON flatten overhead is negligible (0-5%). The Content-buffering machinery has
minimal impact with efficient format backends.

---

## Phase 11 Performance Impact Summary

Comparison of pre- and post-Phase 11 numbers for the most impacted benchmarks:

| Benchmark                    | Before   | After    | Change  | Task   |
|------------------------------|--------:|---------:|:-------:|--------|
| all_paths/depth_10/M         | 193 ms   | 11.6 ms  | **-94%** | T-059 |
| all_paths/depth_10/S         | 1.56 ms  | 47.4 us  | **-97%** | T-059 |
| all_paths/depth_5/M          | 3.46 ms  | 259 us   | **-93%** | T-059 |
| all_paths/depth_5/S          | 227 us   | 15.4 us  | **-93%** | T-059 |
| selector_match_label/S       | 4.1 us   | 991 ns   | **-76%** | T-069 |
| selector_match_label/XL      | 909 us   | 239 us   | **-74%** | T-069 |
| selector_match_node_type/L   | 46.6 us  | 12.7 us  | **-73%** | T-069 |
| selector_match_multi/XL      | 224 us   | 188 us   | **-16%** | T-069 |
| build_graph/elements/XL      | 5.13 ms  | 4.43 ms  | **-14%** | T-065 |
| diff_identical/self/XL       | 81.2 ms  | 70.3 ms  | **-13%** | T-060/T-061/T-066 |
| deserialize/json/M           | 2.18 ms  | 1.84 ms  | **-16%** | T-062/T-063 |
| deserialize/cbor/L           | 12.9 ms  | 8.49 ms  | **-34%** | T-062/T-063 |
| reachable_from/forward_root/XL| 813 us  | 701 us   | **-14%** | T-067 |

## Scaling Analysis

Element ratios between tiers: S to M ~10x, M to L ~4x, L to XL ~2.5x,
XL to Huge ~148x.

| Operation       | S to M | M to L | L to XL | XL to Huge | Complexity |
|-----------------|:------:|:------:|:-------:|:----------:|:----------:|
| Deserialize JSON| 11.3x  |  6.2x  |  2.9x   |    143x    |    O(n)    |
| Serialize JSON  | 10.6x  |  4.3x  |  2.8x   |    205x    |    O(n)    |
| Decode CBOR     | 11.2x  |  4.7x  |  3.2x   |     --     |    O(n)    |
| Encode CBOR     | 10.4x  |  4.3x  |  2.9x   |     --     |    O(n)    |
| Build graph     | 10.2x  |  4.8x  |  3.2x   |    415x    | O(n log n) |
| Validate L1     | 12.2x  |  5.0x  |  3.8x   |    436x    | O(n log n) |
| Validate L1+L2+L3| 12.7x |  5.1x  |  3.9x   |    340x    | O(n log n) |
| Diff identical  | 11.4x  |  4.8x  |  4.0x   |    --      | O(n log n) |
| Redact partner  | 12.2x  |  4.8x  |  3.5x   |    --      |    O(n)    |
| Selector (label)| 10.2x  |  6.7x  |  3.5x   |    345x    | O(n log n) |
| Selector (type) | 5.8x   |  3.5x  |  2.5x   |    537x    | O(n log n) |

At the XL-to-Huge jump (~148x elements), most operations show super-linear
scaling. Parse and serialize remain close to linear (143x). Build graph
and validation L1 scale at ~2.9-3.0x expected, suggesting O(n log n) from hash
map growth. **L1+L2+L3 validation now scales at 340x (vs 148x elements),
confirming the O(E*N) → O(N+E) fix brought it to O(n log n) range.**

CBOR decode and encode both scale linearly across S-L tiers, consistent with JSON.

## Key Takeaways

1. **all_paths query: 16.6x speedup** (T-059) — backtracking DFS with `Vec<bool>` bitset
   replaces exponential-cloning IDDFS. M/depth_10: 193 ms → 11.6 ms.
2. **Selector matching: 3-4x speedup** (T-069) — pre-computed lowercased patterns
   eliminate per-match `to_lowercase()` calls. Label match at S: 4.1 µs → 991 ns.
3. **CBOR decode: 34% faster at L** (T-062/T-063) — visitor-based deserialization
   and `TryFrom<String>` eliminate intermediate allocations. CBOR is now 26% faster
   than JSON for deserialization at L tier.
4. **CBOR encode is 36% faster than JSON** — cbor4ii's compact encoding and direct buffer
   writes outperform serde_json. Encode throughput: 571-646 MiB/s (CBOR) vs 480-540 MiB/s
   (JSON).
5. **CBOR is 21% smaller than compact JSON** — consistent 0.78-0.80 ratio across all tiers.
6. **`#[serde(flatten)]` overhead is 5-7% for CBOR** — down from 16-20% with ciborium.
7. **Diff: 13% faster at XL** (T-060/T-061/T-066) — HashMap pre-indexing, HashSet
   containment, and direct tag-to-string conversion.
8. **Graph construction: 14% faster at XL** (T-065) — `&str` borrows eliminate ~3M
   String allocations per build.
9. **Graph queries: ~14% faster** (T-067) — reusable neighbour buffer eliminates
   per-call Vec allocations.
10. **L2 validation O(E*N) bug is fixed** — full L1+L2+L3 validation at Huge tier
    now completes in 5.0 s (was estimated ~6 hours).
11. **Huge-tier parse + build round-trip: ~6.5 s** — loading a 500 MB supply chain
    graph into memory is feasible for batch analytics.
12. **No operation requires optimization for the current scale target** — all are
    within acceptable latency bounds.
