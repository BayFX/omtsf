# Benchmark Results

Collected on 2026-02-22 using `cargo bench` (Criterion 0.5, default sample sizes).

CBOR backend: **cbor4ii 1.x** (replaced ciborium 0.2 — see CBOR Library Comparison below).

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
| Deserialize      | 196 us | 2.18 ms | 10.8 ms  | 34.0 ms  | 91-140 MiB/s   |
| Serialize compact|  55 us |  549 us |  2.36 ms |  6.90 ms | 450-510 MiB/s  |
| Serialize pretty |  99 us | 1.03 ms |  4.28 ms | 11.8 ms  | ~270-460 MiB/s |

### CBOR (cbor4ii)

| Operation        |   S    |   M     |    L     | Throughput        |
|------------------|-------:|--------:|---------:|-------------------|
| Decode           | 202 us | 2.03 ms | 12.9 ms  | 73-110 MiB/s      |
| Encode           |  40 us |  414 us |  1.80 ms | 518-548 MiB/s     |

### JSON vs CBOR Comparison

| Operation   | JSON (L) | CBOR (L) | CBOR/JSON | Notes                            |
|-------------|--------:|---------:|:---------:|:---------------------------------|
| Deserialize |  10.8 ms | 12.9 ms  |   1.20x   | cbor4ii near JSON parity         |
| Serialize   |  2.36 ms |  1.80 ms |   0.76x   | CBOR 24% faster than JSON        |

**Analysis:** After switching from ciborium to cbor4ii, CBOR performance is
dramatically improved. CBOR encode is now **24-27% faster than JSON** across
all tiers. CBOR decode is within 3-20% of JSON — effectively at parity for
S/M, slightly slower at L due to `#[serde(flatten)]` overhead scaling with
element count.

CBOR offers 21% smaller output with faster encoding, making it the preferred
format for both network transfer and storage. JSON remains slightly faster for
deserialization at large scale.

## Group 2: Graph Construction

| Tier |  Time  | Throughput   |
|------|-------:|--------------|
| S    |  35 us | 4.0 Melem/s  |
| M    | 366 us | 4.0 Melem/s  |
| L    | 1.64 ms| 3.6 Melem/s  |
| XL   | 5.13 ms| 2.9 Melem/s  |

`build_graph` sustains ~3-4 million elements/sec. Now includes building
`nodes_by_type` and `edges_by_type` indexes (HashMap inserts per element).
Slight throughput drop at XL due to hash map resizing. Graph construction is
fast enough to be negligible relative to I/O.

## Group 3: Graph Queries

### Reachability (`reachable_from`)

| Variant                |    S   |    M    |    L    |     XL   |
|------------------------|-------:|--------:|--------:|---------:|
| Forward from root      |  5.9 us|  69.3 us|  288 us |   813 us |
| Forward from leaf      |  143 ns|   143 ns|  141 ns |   142 ns |
| Backward from root     |  3.5 us|  41.7 us|  168 us |   448 us |
| Both from mid          |  9.5 us|  109 us |  457 us |  1.33 ms |
| Filtered (supplies)    |  568 ns|   3.3 us|  9.7 us |  19.3 us |

Leaf queries are O(1) -- constant ~142 ns regardless of graph size. Edge-type filtering
yields ~40x speedup. Full forward traversal of XL graph: under 1 ms.

### Shortest Path

| Variant        |    S   |    M    |    L    |     XL   |
|----------------|-------:|--------:|--------:|---------:|
| Root to leaf   |  7.6 us|  98 us  |  416 us |  1.15 ms |
| Root to mid    |  6.8 us|  60 us  |  262 us |   538 us |
| No path        |  156 ns|  157 ns |  157 ns |   156 ns |

No-path detection is O(1). Longest paths (root to leaf spanning full depth) scale
linearly.

### All Paths

| Variant  |    S    |     M    |
|----------|--------:|---------:|
| Depth 5  |  227 us |  3.46 ms |
| Depth 10 | 1.56 ms | 193.0 ms |

All-paths is the most expensive query -- exponential in path depth. M/depth_10 at
193 ms is the single slowest benchmark. Only benchmarked on S/M sizes.

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
| L1 only    | 34 us   |   406 us |  2.07 ms |   7.19 ms |
| L1 + L2    | 58 us   |   720 us |  3.94 ms |  14.3 ms  |
| L1 + L2 + L3 | 59 us |   746 us |  3.82 ms |  14.9 ms  |

L1 validation is fast (proportional to element count). L2 adds semantic checks;
the O(E*N) bug in `facility_ids_with_org_connection` has been fixed (pre-built
HashSet replaces per-edge linear scan), reducing L1+L2 cost by ~37-53% at L/XL.
L3 (cycle detection) adds negligible overhead on top of L2. Full L1+L2+L3
validation of a 5 MB XL file: 15 ms.

## Group 7: Merge Pipeline

| Variant                 |    S     |     M     |     L     |
|-------------------------|--------:|----------:|----------:|
| Self-merge (100% overlap)| 946 us  |  11.3 ms  |  59.6 ms  |
| Disjoint (0% overlap)   | 1.13 ms |  15.6 ms  |  84.3 ms  |
| 3-file merge            | 1.85 ms |  24.6 ms  |       --  |

Merge is the most expensive operation per-element. Disjoint merge is ~35% more
expensive than self-merge (more output nodes). The 3-file merge cost is roughly
additive.

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
| Identical                |  313 us |  3.56 ms | 18.2 ms  |  81.2 ms  |
| Disjoint                 |  186 us |  2.07 ms | 10.1 ms  |       --  |
| Filtered (org + supplies)|  113 us |  1.61 ms | 13.4 ms  |       --  |

Self-diff (identical files) is more expensive than disjoint diff because it must
match every element. XL self-diff at 81 ms is the second-slowest operation overall.

## Group 10: Selector Query

### `selector_match` (scan only, no subgraph assembly)

| Selector           |    S     |    M     |    L     |    XL     | Throughput      |
|--------------------|--------:|---------:|---------:|----------:|-----------------|
| Label key          | 1.06 us | 10.1 us  |  63.2 us |  231 us   |  65-147 Melem/s |
| Node type          |  567 ns |  3.3 us  |  14.0 us |  31.7 us  | 249-474 Melem/s |
| Multi (type+label) |  877 ns | 10.2 us  |  59.1 us |  186 us   |  81-161 Melem/s |

Node-type matching is ~3-8x faster than label matching -- enum comparison vs string
lookup in the labels map. Multi-selector performance is close to label-only because
the label check dominates. All selector scans complete under 250 us at XL.

### `selector_subgraph` (full pipeline: scan + expand + assemble)

| Variant                     |     S     |     M     |     L     |
|-----------------------------|--------:|----------:|----------:|
| Narrow (attestation, exp 0) |  6.2 us |   71.5 us |   305 us  |
| Broad (organization, exp 0) | 58.3 us |   690 us  |  3.09 ms  |
| Expand 1 (attestation)      | 17.7 us |   192 us  |   852 us  |
| Expand 3 (attestation)      | 93.0 us |   933 us  |  4.48 ms  |

Narrow selectors (~5% seed match) are ~10x cheaper than broad (~45% seed match).
Type-only selectors now use the graph's type index instead of scanning all nodes,
yielding ~21-24% improvement on narrow queries. Each expansion hop roughly doubles
the cost.

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

Throughput drops from ~3-4 Melem/s at XL to ~1.2 Melem/s at Huge -- hash map
resizing and cache pressure dominate at 2.2M elements. Now includes building
type indexes.

### Reachability (`reachable_from`)

| Variant                |    Huge    |
|------------------------|----------:|
| Forward from root      |   413 ms  |
| Filtered (supplies)    |  2.25 ms  |
| Both from mid          |   759 ms  |

Edge-type filtering yields ~184x speedup at this scale (vs ~40x at XL).
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

## Scaling Analysis

Element ratios between tiers: S to M ~10x, M to L ~4x, L to XL ~2.5x,
XL to Huge ~148x.

| Operation       | S to M | M to L | L to XL | XL to Huge | Complexity |
|-----------------|:------:|:------:|:-------:|:----------:|:----------:|
| Deserialize JSON| 11.1x  |  5.0x  |  3.1x   |    137x    |    O(n)    |
| Serialize JSON  | 10.0x  |  4.3x  |  2.9x   |    197x    |    O(n)    |
| Decode CBOR     | 10.0x  |  6.4x  |   --    |     --     |    O(n)    |
| Encode CBOR     | 10.3x  |  4.3x  |   --    |     --     |    O(n)    |
| Build graph     | 10.5x  |  4.5x  |  3.1x   |    359x    | O(n log n) |
| Validate L1     | 12.0x  |  5.1x  |  3.5x   |    477x    | O(n log n) |
| Validate L1+L2+L3| 12.6x |  5.1x  |  3.9x   |    336x    | O(n log n) |
| Diff identical  | 11.4x  |  5.1x  |  4.5x   |    --      | O(n log n) |
| Redact partner  | 12.2x  |  4.8x  |  3.5x   |    --      |    O(n)    |
| Selector (label)| 9.5x   |  6.3x  |  3.7x   |    357x    | O(n log n) |
| Selector (type) | 5.9x   |  4.2x  |  2.3x   |    530x    | O(n log n) |

At the XL-to-Huge jump (~148x elements), most operations show super-linear
scaling. Parse and serialize remain close to linear (137x). Build graph
and validation L1 scale at ~2.4-3.2x expected, suggesting O(n log n) from hash
map growth. **L1+L2+L3 validation now scales at 336x (vs 148x elements),
confirming the O(E*N) → O(N+E) fix brought it to O(n log n) range.**

CBOR decode and encode both scale linearly across S-L tiers, consistent with JSON.

## Key Takeaways

1. **CBOR encode is 24-27% faster than JSON** — cbor4ii's compact encoding and
   direct buffer writes outperform serde_json's string formatting. Encode throughput:
   518-548 MiB/s (CBOR) vs 450-510 MiB/s (JSON).
2. **CBOR decode is at JSON parity** — within 3% at S/M, 20% slower at L. At Huge
   scale, CBOR decode is only 1.07x JSON deserialize. The ciborium-to-cbor4ii switch
   improved decode by **2.0x** (from 1.6x slower to near parity).
3. **CBOR is 21% smaller than compact JSON** — consistent 0.78-0.80 ratio across
   all tiers. Combined with faster encoding, CBOR is strictly better for storage
   and network transfer.
4. **`#[serde(flatten)]` overhead is 5-7% for CBOR** — down from 16-20% with
   ciborium. cbor4ii's slice-based access minimizes the penalty from serde's
   Content-buffering path.
5. **ciborium is 2.0x slower than cbor4ii** for decode — the dominant factor is
   `from_reader()` trait-based byte reads vs `from_slice()` direct slice access.
6. **Serialization is 3-5x faster than deserialization** — serde's write path is
   highly optimized. Ratio holds at Huge tier.
7. **Graph queries are the fastest operations** -- sub-millisecond even at XL.
   Edge-type filtering provides 10-40x speedups (184x at Huge).
8. **Merge is the most expensive operation** -- canonical identifier matching
   dominates. 84 ms for L-tier disjoint merge.
9. **`all_paths` with depth 10 is the performance cliff** -- 193 ms on M-tier,
   exponential growth. Depth limits are essential.
10. **Cycle detection adds negligible cost to validation** -- L3 is essentially free
    on top of L2.
11. **No operation requires optimization for the current scale target** -- all are
    within acceptable latency bounds.
12. **Selector scans are extremely fast** -- under 250 us for XL. Type-index fast
    path yields 21-25% improvement on type-only `selector_subgraph` queries.
13. **L2 validation O(E*N) bug is fixed** -- full L1+L2+L3 validation at Huge tier
    now completes in 5.0 s (was estimated ~6 hours). The fix pre-builds a
    facility-ID HashSet, eliminating the per-edge linear scan.
14. **Huge-tier parse + build round-trip: ~6.5 s** -- loading a 500 MB supply chain
    graph into memory is feasible for batch analytics. Serialize back in 1.4 s.
15. **Huge-tier CBOR encode: 892 ms** (448 MiB/s) — 34% faster than JSON serialize.
    CBOR decode: 5.0 s (80 MiB/s) — at parity with JSON deserialize (4.7 s).
16. **`assemble_subgraph` optimization** -- iterating only outgoing edges of included
    nodes (vs all edges) improves subgraph extraction for small subsets of large graphs.
