# OMTS Specification: Merge Semantics

**Spec:** OMTS-SPEC-003
**Status:** Draft
**Date:** 2026-02-25
**Revision:** 2
**License:** [CC-BY-4.0](LICENSE)
---

## Related Specifications

| Spec | Relationship |
|------|-------------|
| OMTS-SPEC-001 (Graph Data Model) | **Prerequisite.** Defines the graph structure (nodes, edges, types) that this spec operates on. |
| OMTS-SPEC-002 (Entity Identification) | **Prerequisite.** Defines the external identifiers used by the merge identity predicates in this spec. The canonical string format (OMTS-SPEC-002, Section 4) is used for deterministic comparison. |

---

## 1. Overview

This specification defines two distinct operations for combining `.omts` files:

1. **Merge** (Sections 2--8): Combines files from potentially different origins using external identifiers. Designed for the multi-party case where two organizations contribute overlapping portions of a supply network. Merge is commutative, associative, and idempotent.

2. **Same-Origin Update** (Section 11): Reconciles a new version of a file against its own prior version from the same source system, using `internal` identifiers scoped to a shared `authority` value. Designed for the re-import case where one party updates their own data. Same-origin update is directional (not commutative) — the new file takes precedence.

Applying general merge to two versions of the same internal-only file will produce duplicate nodes. For that use case, use same-origin update instead.

---

## 2. Identity Predicate for Nodes

Two nodes from different files are **merge candidates** if and only if they share at least one external identifier record (as defined in OMTS-SPEC-002, Section 3) where all of the following hold:

1. `scheme` values are equal (case-sensitive string comparison)
2. `value` values are equal (case-sensitive string comparison after normalization: leading/trailing whitespace trimmed, for numeric-only schemes leading zeros are significant)
3. If `authority` is present in **either** record, `authority` values MUST be equal (case-insensitive string comparison)
4. **Temporal compatibility:** If both identifier records carry `valid_from` and/or `valid_to` fields, their validity periods MUST overlap or at least one period MUST be open-ended (no `valid_to`). Specifically:
   - If both records have `valid_to` and the earlier `valid_to` is before the later `valid_from`, the identifiers are NOT temporally compatible and do NOT satisfy the identity predicate.
   - If either record lacks `valid_from` and `valid_to`, temporal compatibility is assumed (backward-compatible with files that omit temporal fields).

The `internal` scheme is explicitly excluded: `internal` identifiers NEVER satisfy the merge identity predicate across files, because they are scoped to their issuing system. For reconciling successive exports from the same source system (where `internal` identifiers are semantically stable), use the same-origin update operation (Section 11) instead.

---

## 3. Identity Predicate for Edges

Two edges from different files are **merge candidates** if all of the following hold:

1. Their resolved source nodes are merge candidates (or the same node post-merge)
2. Their resolved target nodes are merge candidates (or the same node post-merge)
3. Their `type` values are equal
4. They share at least one external identifier (if identifiers are present on edges), OR they have no external identifiers and their core properties are equal (same `type`, same resolved endpoints, same non-temporal properties)

This definition supports the multigraph model: two edges with the same type and endpoints but different properties (e.g., two distinct supply contracts) are NOT merge candidates unless they share an explicit external identifier.

### 3.1 Edge Merge-Identity Properties by Type

When edges lack explicit external identifiers (condition 4 in Section 3), merge identity falls back to property comparison. The following non-temporal properties form the merge-identity key for each edge type:

| Edge Type | Merge-Identity Properties (beyond type + endpoints) |
|-----------|---------------------------------------------------|
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
| `operates` | *(none — type + endpoints suffice)* |
| `produces` | *(none — type + endpoints suffice)* |
| `composed_of` | *(none — type + endpoints suffice)* |
| `sells_to` | `commodity`, `contract_ref` |
| `attested_by` | `scope` |
| `same_as` | *(always unique — never merged)* |

Temporal properties (`valid_from`, `valid_to`) are excluded from merge-identity comparison. Two edges with the same type, endpoints, and merge-identity properties but different validity dates represent the same relationship observed at different times and SHOULD be merged.

---

## 4. Merge Procedure

Given files A and B:

1. **Concatenate** all nodes from A and B into a single list.
2. **Identify** merge candidate pairs using the identity predicate (Section 2).
3. **Compute transitive closure** of merge candidates. If node X is a merge candidate with node Y (via identifier I1), and node Y is a merge candidate with node Z (via identifier I2), then X, Y, and Z are all merged into a single node. This is required because the same real-world entity may carry different identifiers in different files (e.g., LEI in file A, DUNS in file B, both LEI and DUNS in file C).
4. **Merge** each candidate group:
   - The merged node retains the **union** of all identifier records from all sources.
   - After merge, the `identifiers` array MUST be sorted by the canonical string form (OMTS-SPEC-002, Section 4) in lexicographic UTF-8 byte order. This ensures deterministic output regardless of merge order, supporting the commutativity property.
   - For each property present in multiple source nodes:
     - If values are equal: retain the value.
     - If values differ: the merger MUST record both values with their provenance (source file, reporting entity). Conflict resolution is a tooling concern.
   - **Labels** (OMTS-SPEC-001, Section 8.4): compute the set union of `{key, value}` pairs from all source nodes. After merge, sort the `labels` array by `key` (lexicographic), then by `value` (lexicographic, absent values sort before present values). Labels do not produce conflicts — they are purely additive.
   - The merged node's graph-local `id` is assigned by the merger (it is an arbitrary file-local string).

**Conflict record structure.** When property values differ across source nodes, the merger SHOULD record conflicts in a `_conflicts` array on the merged node (serialized at the top level for nodes, inside `properties` for edges):

```json
{
  "_conflicts": [
    {
      "field": "name",
      "values": [
        { "value": "Acme GmbH", "source_file": "export-sap.omts" },
        { "value": "ACME Manufacturing GmbH", "source_file": "export-ariba.omts" }
      ]
    }
  ]
}
```

The `_conflicts` structure above is shown in JSON. In CBOR encoding, the same logical map structure applies; see OMTS-SPEC-007 for serialization rules.

Conflict records are informational. Validators MUST NOT reject files containing `_conflicts`. Tooling SHOULD present conflicts to users for resolution.

### 4.1 Merge-Group Safety Limits

Transitive closure (step 3) can amplify false-positive matches: a single erroneous identifier match cascades through the entire connected component. To mitigate this risk, implementations MUST apply the following merge-group size thresholds:

| Group Size (excluding `same_as`-linked nodes) | Required Behavior |
|-----------------------------------------------|-------------------|
| 1--3 nodes | Normal merge; no warning required |
| 4--9 nodes | Implementations MUST emit a warning identifying the group, its member nodes, and the bridging identifiers that caused each union. Merge proceeds. |
| 10+ nodes | Implementations MUST emit a prominent warning. A merge group of 10 or more distinct nodes almost certainly indicates a data quality problem (e.g., a reassigned DUNS number, an erroneous identifier, or a "garbage" tax ID shared by many unrelated entities). Implementations SHOULD provide an option to split or reject oversized groups. |

Nodes linked solely by `same_as` edges (OMTS-SPEC-003, Section 7) are counted as group members for the purpose of this threshold but are excluded from the "excluding `same_as`" count used to determine the warning tier. This prevents advisory `same_as` edges from inflating the threshold.

The thresholds above are defaults. Implementations MAY allow users to configure custom thresholds for specific use cases (e.g., conglomerate files where large groups are expected).

### 4.2 `tier` Property Reconciliation

The `tier` property on `supplies` edges (OMTS-SPEC-001, Section 6.1) is perspective-dependent: it is defined relative to the file's `reporting_entity`. When merging files from different reporting entities, `tier` conflicts are structurally expected and MUST NOT be treated as data quality errors.

When merging files with different `reporting_entity` values:

1. If the merged file retains a `reporting_entity` (because one source's perspective is designated as primary), `tier` values from that source are retained. Conflicting `tier` values from other sources are recorded in `_conflicts` (Section 4) with their source file and reporting entity for context.
2. If the merged file omits `reporting_entity` (because no single perspective is primary), all `tier` values are recorded in `_conflicts`. Consumers MUST NOT interpret `tier` values without knowing the reporting entity they are relative to.

Implementations SHOULD include the `reporting_entity` from each source file in conflict records to enable downstream recomputation of perspective-relative tiers.

5. **Rewrite** all edge source/target references to use the merged node IDs.
6. **Identify** merge candidate edge pairs using the edge identity predicate (Section 3).
7. **Deduplicate** edges that are merge candidates, merging their properties as with nodes.
8. **Retain** all non-duplicate edges.

---

## 5. Algebraic Properties

For the decentralized merge model to work -- where different parties independently merge overlapping files without coordination -- the merge operation MUST satisfy the following algebraic properties:

**Commutativity:** `merge(A, B) = merge(B, A)`. The order in which two files are provided to a merge operation MUST NOT affect the result. This is satisfied by the identity predicate (symmetric) and the union-based merge procedure.

**Associativity:** `merge(merge(A, B), C) = merge(A, merge(B, C))`. Three-file merge MUST produce the same result regardless of grouping. This is satisfied by the transitive closure computation in step 3: the final merge graph is determined by the full set of identifier overlap relationships, not by the order in which they are discovered.

**Idempotency:** `merge(A, A) = A`. Merging a file with itself MUST produce an equivalent graph (same nodes, edges, identifiers, and properties; graph-local IDs may differ).

### 5.1 Post-Merge Validation

After merge completes, the merged file MUST satisfy the same structural validation rules as any other `.omts` file:

- All L1 rules from OMTS-SPEC-001 and OMTS-SPEC-002 MUST hold on the merged output.
- If any L1 rule fails after merge (e.g., duplicate node IDs from ID assignment, broken edge references), the merge implementation MUST correct the violation or report a merge failure. Implementations MUST NOT produce output that fails L1 validation.
- L2 and L3 rules SHOULD be re-evaluated on the merged output. Merge may resolve some L2 warnings (e.g., a node that lacked external identifiers in one file may gain them from the other).

---

## 6. Merge Provenance

To support post-merge auditability, the merged file SHOULD include a `merge_metadata` section in the file header recording:

- Source file identifiers (file hash or filename)
- `reporting_entity` values from each source file (if present). When source files declare different reporting entities, the merged file SHOULD omit `reporting_entity` from the file header (the merged graph is no longer from a single perspective) and record the source values here instead.
- Merge timestamp
- Number of nodes and edges merged
- Number of property conflicts detected

---

## 7. `same_as` Edge Type

The `same_as` edge type declares that two nodes in the same file are believed to represent the same real-world entity, but the producer was unable to merge them with sufficient confidence.

### 7.1 Definition

| Property | Required | Type | Description |
|----------|----------|------|-------------|
| `confidence` | No | enum | `definite`, `probable`, `possible`. Default: `probable`. |
| `basis` | No | string | Justification for the equivalence assertion (e.g., `name_match`, `address_match`, `manual_review`) |

**Direction convention:** `same_as` is symmetric in semantics. The choice of `source` and `target` is arbitrary. Merge engines MUST treat `same_as` as an undirected relationship.

### 7.2 Merge Engine Behavior

The `same_as` edge type is **advisory**: merge engines MAY use it to combine nodes but are not required to. Specifically:

- When `confidence` is `definite`: merge engines SHOULD treat the two nodes as merge candidates and include them in the transitive closure computation (Section 4, step 3).
- When `confidence` is `probable` or `possible`: merge engines MAY treat the two nodes as merge candidates, depending on their confidence threshold configuration.

When a merge engine honors `same_as` edges, it MUST apply transitive closure: if A `same_as` B and B `same_as` C, then A, B, and C are all merged into a single node.

### 7.3 When to Use

Producers SHOULD use `same_as` when deduplication is not feasible during export -- for example, when two ERP vendor records likely represent the same legal entity but the producer cannot determine this with sufficient confidence. Example:

```json
{
  "id": "edge-sa-001",
  "type": "same_as",
  "source": "org-acme-v100",
  "target": "org-acme-v200",
  "properties": {
    "confidence": "probable",
    "basis": "name_match"
  }
}
```

---

## 8. Intra-File Deduplication

ERP systems frequently contain duplicate records for the same real-world entity. Producers MUST address this to avoid polluting the graph with duplicate nodes.

**Recommended approach:**

1. **Before export**, identify vendor records that represent the same legal entity. Two records are candidates for deduplication if they share any external identifier (`duns`, `lei`, `nat-reg`, `vat`) or if fuzzy name matching with address comparison produces high confidence.
2. **Produce one `organization` node per distinct legal entity**, carrying all `internal` identifiers from each source record. For example, if vendor `V-100` and `V-200` in SAP both represent Acme GmbH, produce a single node with two `internal` identifiers:
   ```json
   {
     "id": "org-acme",
     "type": "organization",
     "name": "Acme GmbH",
     "identifiers": [
       { "scheme": "internal", "value": "V-100", "authority": "sap-prod-100" },
       { "scheme": "internal", "value": "V-200", "authority": "sap-prod-200" },
       { "scheme": "duns", "value": "081466849" }
     ]
   }
   ```
3. **If deduplication is not feasible**, produce separate nodes and declare equivalence using a `same_as` edge (Section 7).

---

## 9. Enrichment and Merge Interaction

Enrichment (adding external identifiers to nodes, as described in OMTS-SPEC-005, Section 5) is not purely additive with respect to the merge graph. Adding an external identifier to a node may:

1. **Create new merge candidates.** If enrichment adds a DUNS number to node A, and node B in another file already carries that DUNS, nodes A and B become merge candidates that were not previously linkable.

2. **Reveal prior merge errors.** If two nodes were merged via a shared DUNS number, and subsequent enrichment reveals they have different LEIs, the merge may have been based on a reassigned identifier (see temporal compatibility in Section 2, condition 4).

**Recommendations for enrichment tooling:**

- After adding external identifiers, re-evaluate merge groups using the updated identity predicate.
- Record in `merge_metadata` (Section 6) whether the merge was performed pre- or post-enrichment.
- When enrichment creates new merge candidates, emit `same_as` edges with `confidence: "probable"` and `basis: "enrichment_match"` rather than performing automatic merge. This allows human review before graph topology changes.

---

## 10. Validation Rules

### 10.1 Level 3 -- Enrichment

These rules require external data or cross-file context and are intended for enrichment tooling.

| Rule | Description |
|------|-------------|
| L3-MRG-01 | The sum of inbound `ownership` `percentage` values to any single node (for overlapping validity periods) SHOULD NOT exceed 100 |
| L3-MRG-02 | `legal_parentage` edges SHOULD form a forest (no cycles in the parentage subgraph) |

---

## 11. Same-Origin Update

The merge operation (Sections 2--8) combines files from different origins using external identifiers. It is not designed for reconciling successive exports from the same source system. Applying merge to two versions of the same internal-only file produces duplicate nodes because `internal` identifiers do not satisfy the merge identity predicate.

Same-origin update is a separate operation that reconciles a **new file** against a **base file** when both originate from the same source. It uses `internal` identifiers scoped to a shared `authority` value as the matching key.

### 11.1 Preconditions

Same-origin update applies when:

1. Both files contain nodes with `internal` identifiers sharing the same `authority` value.
2. The shared `authority` value is stable and unique to the producing data source (see OMTS-SPEC-002, Section 5 and OMTS-SPEC-005, Section 1.1 for naming conventions).

Implementations MUST require explicit invocation of same-origin update. It MUST NOT be applied automatically during a general merge operation.

**Authority stability requirement.** The `authority` value used for same-origin matching MUST be stable across exports from the same source and unique to that source. Generic values (e.g., `"supplier-list"`) are insufficient — they could collide across unrelated data sources. Recommended patterns:

- ERP exports: `{system_type}-{instance_id}[-{client}]` (e.g., `sap-prod-100`)
- Excel imports: `{organization_id}:{list_scope}` (e.g., `acme-corp:approved-suppliers`)
- Procurement platforms: `{platform}-{tenant}` (e.g., `ariba-acme-network`)

### 11.2 Node Identity Predicate

A node in the new file and a node in the base file are **update candidates** if they share at least one `internal` identifier record where all of the following hold:

1. `scheme` values are both `"internal"`
2. `authority` values are equal (case-insensitive string comparison)
3. `value` values are equal (case-sensitive string comparison)

This predicate does NOT apply transitively. Each node in the new file matches at most one node in the base file. If multiple base-file nodes match (due to duplicate `internal` identifiers), the implementation MUST emit a warning and skip the ambiguous match.

### 11.3 Update Procedure

Given a base file B and a new file N:

1. **Match** each node in N against nodes in B using the identity predicate (Section 11.2).
2. **For matched nodes** (update):
   - **Properties:** Values from N replace values from B (last-write-wins). The replaced value from B is recorded in `_conflicts` (Section 4) with source provenance.
   - **Identifiers:** Compute the union of identifier arrays from B and N. Identifiers present in B but not in N are preserved — this ensures that external identifiers added by enrichment (OMTS-SPEC-005, Section 6) survive re-import. After union, sort by canonical string form (OMTS-SPEC-002, Section 4).
   - **Labels:** Compute the set union of `{key, value}` pairs from B and N (same as merge, Section 4).
   - The updated node retains the graph-local `id` from B.
3. **For unmatched nodes in N** (insert): Add to the output as new nodes.
4. **For unmatched nodes in B** (unmatched base nodes): Apply the configured `unmatched_node_policy`:
   - `retain` (default): Preserve the node and its edges unchanged.
   - `flag`: Preserve the node and add a label `{ "key": "omts.update.unmatched" }` for review.
   - `expire`: Set `valid_to` on the node (and its outbound edges that lack `valid_to`) to the new file's `snapshot_date`.
5. **Rewrite** edge source/target references: matched nodes keep their base-file IDs; new nodes receive IDs assigned by the implementation.
6. **Deduplicate edges** using the same logic as merge (Sections 3 and 4, steps 5--8), applied to the combined edge set.

### 11.4 Algebraic Properties

Same-origin update has different algebraic properties than merge:

- **NOT commutative:** `update(B, N) ≠ update(N, B)`. The new file takes precedence over the base file. This is by design — same-origin update is directional.
- **Idempotent:** `update(B, B) = B`. Applying the same file as both base and new produces an equivalent graph.
- **Sequential composition:** `update(update(B, N1), N2)` produces the same result as `update(B, N2)` when N2 is a complete re-export (all nodes present). When N2 is a partial export, sequential composition is meaningful: only nodes present in N2 are updated.

### 11.5 Provenance

The output file SHOULD include `merge_metadata` (Section 6) recording:

- Operation type: `"same_origin_update"`
- The `authority` value matched on
- Base file identifier (hash or filename)
- New file identifier (hash or filename)
- Count of nodes: updated, inserted, retained (unmatched base)
- Update timestamp

### 11.6 Interaction with Enrichment

Same-origin update and enrichment (OMTS-SPEC-005, Section 6) form a complementary pipeline:

1. **Export** from source system → `.omts` file with `internal` identifiers only.
2. **Enrich** → add external identifiers (`lei`, `duns`, `nat-reg`, `vat`) to nodes.
3. **Re-export** from same source system → new `.omts` file with updated properties.
4. **Same-origin update** → reconcile re-export against enriched base. External identifiers added in step 2 are preserved (Section 11.3, step 2).
5. The updated file now carries both current properties from the source system and external identifiers from enrichment. It can participate in cross-file merge (Sections 2--8).

**Invariant:** Same-origin update MUST NOT discard identifiers present in the base file. The identifier union in step 2 of Section 11.3 is always additive.

