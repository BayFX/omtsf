# OMTSF Specification: Merge Semantics

**Spec:** OMTSF-SPEC-003
**Status:** Draft
**Date:** 2026-02-18
**Revision:** 1
**License:** [CC-BY-4.0](LICENSE)
---

## Related Specifications

| Spec | Relationship |
|------|-------------|
| OMTSF-SPEC-001 (Graph Data Model) | **Prerequisite.** Defines the graph structure (nodes, edges, types) that this spec operates on. |
| OMTSF-SPEC-002 (Entity Identification) | **Prerequisite.** Defines the external identifiers used by the merge identity predicates in this spec. The canonical string format (OMTSF-SPEC-002, Section 4) is used for deterministic comparison. |

---

## 1. Overview

Merge is the operation of combining two or more `.omts` files that describe overlapping portions of a supply network into a single coherent graph. The vision describes this as "concatenating and deduplicating lists." This specification defines what deduplication means, the formal identity predicates, and the algebraic properties that ensure deterministic results.

---

## 2. Identity Predicate for Nodes

Two nodes from different files are **merge candidates** if and only if they share at least one external identifier record (as defined in OMTSF-SPEC-002, Section 3) where all of the following hold:

1. `scheme` values are equal (case-sensitive string comparison)
2. `value` values are equal (case-sensitive string comparison after normalization: leading/trailing whitespace trimmed, for numeric-only schemes leading zeros are significant)
3. If `authority` is present in **either** record, `authority` values MUST be equal (case-insensitive string comparison)
4. **Temporal compatibility:** If both identifier records carry `valid_from` and/or `valid_to` fields, their validity periods MUST overlap or at least one period MUST be open-ended (no `valid_to`). Specifically:
   - If both records have `valid_to` and the earlier `valid_to` is before the later `valid_from`, the identifiers are NOT temporally compatible and do NOT satisfy the identity predicate.
   - If either record lacks `valid_from` and `valid_to`, temporal compatibility is assumed (backward-compatible with files that omit temporal fields).
   - **Rationale:** DUNS and GLN numbers are reassigned after entity dissolution. Without temporal checking, a DUNS number valid 2010-2015 for Entity A and the same DUNS reassigned 2020-2025 to Entity B would produce a false merge.

The `internal` scheme is explicitly excluded: `internal` identifiers NEVER satisfy the identity predicate across files, because they are scoped to their issuing system.

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
   - After merge, the `identifiers` array MUST be sorted by the canonical string form (OMTSF-SPEC-002, Section 4) in lexicographic UTF-8 byte order. This ensures deterministic output regardless of merge order, supporting the commutativity property.
   - For each property present in multiple source nodes:
     - If values are equal: retain the value.
     - If values differ: the merger MUST record both values with their provenance (source file, reporting entity). Conflict resolution is a tooling concern.
   - **Labels** (OMTSF-SPEC-001, Section 8.4): compute the set union of `{key, value}` pairs from all source nodes. After merge, sort the `labels` array by `key` (lexicographic), then by `value` (lexicographic, absent values sort before present values). Labels do not produce conflicts — they are purely additive.
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

Conflict records are informational. Validators MUST NOT reject files containing `_conflicts`. Tooling SHOULD present conflicts to users for resolution.

### 4.1 Merge-Group Safety Limits

Transitive closure (step 3) can amplify false-positive matches: a single erroneous identifier match cascades through the entire connected component. To mitigate this risk:

When transitive closure produces a merge group exceeding **50 nodes**, implementations SHOULD emit a warning identifying the group and its bridging identifiers.

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

- All L1 rules from OMTSF-SPEC-001 and OMTSF-SPEC-002 MUST hold on the merged output.
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

Enrichment (adding external identifiers to nodes, as described in OMTSF-SPEC-005, Section 5) is not purely additive with respect to the merge graph. Adding an external identifier to a node may:

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

