# Graph Modeling Expert Review: Labels Mechanism and Reporting Entity

**Reviewer:** Graph Modeling Expert (Graph Data Modeling & Algorithm Specialist)
**Date:** 2026-02-18
**Topic:** Review of the labels mechanism (Section 8.4) and `reporting_entity` field additions to SPEC-001 and SPEC-003, implementing P0 recommendations R1 and R2 from the supply chain segmentation panel review.

---

## Assessment

The implementation addresses the unanimous panel finding (R1) and the reporting-entity gap (R2) with a design that deliberately departs from my original proposal of separate `labels` (boolean flags) and `annotations` (key-value pairs). Having reviewed the result, I consider the unified `labels` array with optional `value` field a defensible simplification. In the GQL/ISO 39075 property graph model, labels are identifiers that are either present or not present -- pure set-membership flags with no associated value. Properties are key-value pairs. The two concepts are architecturally distinct because they serve different query patterns: labels support O(1) set-membership filtering (e.g., `MATCH (n:HighRisk)`), while properties support O(log n) key-value lookup. My original proposal preserved this distinction. The implemented design collapses both into a single array where `value`-less entries act as boolean flags and `value`-bearing entries act as key-value classifications. This is a pragmatic choice for a file serialization format (as opposed to a query engine), and I accept the tradeoff: OMTS is not a graph database, and the serialization format does not need to mirror the internal indexing structures of Neo4j or TigerGraph. What matters is that a conformant consumer can load the `labels` array into either representation without loss -- and it can.

The `reporting_entity` field is well-designed. It correctly references a graph-local node ID rather than embedding a separate entity description, avoiding data duplication and leveraging the existing identity predicate infrastructure for cross-file resolution. The L1-GDM-05 validation rule ensures referential integrity, and the L2-GDM-04 warning for orphaned `tier` values is a practical completeness check. The merge provenance treatment -- recording each source file's `reporting_entity` and omitting it from the merged header when sources disagree -- is the correct algebraic choice. It preserves commutativity (merge(A,B) = merge(B,A)) and avoids arbitrarily privileging one perspective.

The merge semantics for labels -- set union with deterministic sort order, no conflicts -- are clean and preserve the algebraic properties defined in SPEC-003 Section 5. Set union is commutative, associative, and idempotent, which means labels cannot violate the merge algebra. The sort specification (key lexicographic, then value lexicographic, absent before present) ensures byte-identical output regardless of merge order. This is exactly right.

---

## Strengths

- **Round-trip fidelity preserved.** The unified `{key, value}` structure serializes losslessly to JSON and back. No information is lost during serialization or deserialization, satisfying a core property graph serialization requirement.
- **Merge algebra intact.** Set union of `{key, value}` pairs is commutative, associative, and idempotent. The deterministic sort order ensures `merge(A, B)` produces byte-identical output to `merge(B, A)`. Labels cannot produce conflicts, which simplifies merge implementations.
- **Namespace collision prevention.** Reserving dotless keys for future OMTS vocabularies and recommending reverse-domain notation for custom keys mirrors the extension mechanism for node/edge types (Sections 8.1-8.2), creating a consistent namespacing pattern across the specification.
- **Placement consistency.** Placing `labels` at top-level for nodes and inside `properties` for edges follows the same serialization pattern as `data_quality` (Section 8.3), maintaining structural predictability.
- **Identity predicate exclusion.** Explicitly excluding labels from merge candidate detection (SPEC-003, Section 2) is essential. Labels are classification metadata, not identity assertions. Including them would create false negatives (two files describing the same entity with different labels would fail to merge).
- **Advisory size limit.** The 100-label cap in Section 9.4 is reasonable for preventing abuse without constraining legitimate use. Most real-world entities will carry 5-20 labels.
- **`reporting_entity` as graph-local reference.** Referencing a node ID rather than embedding entity metadata avoids the duplication and staleness problems that would arise from a separate `{entity_id, name}` object.

---

## Concerns

- **[Minor] No uniqueness constraint on `{key, value}` pairs within a single node/edge.** The spec does not state whether duplicate entries (same key and same value) are prohibited, permitted, or collapsed during validation. In Neo4j, a node either has or does not have a label -- duplicates are meaningless. A producer could emit `[{"key": "com.acme.critical"}, {"key": "com.acme.critical"}]`. Merge (set union) will deduplicate, but pre-merge validation has no rule to flag this. This is a minor gap since merge corrects it, but an L2 warning would be hygienic.

- **[Minor] No guidance on multi-valued classification semantics.** The spec does not address whether `[{"key": "com.acme.risk-tier", "value": "high"}, {"key": "com.acme.risk-tier", "value": "medium"}]` represents two independent classifications or a conflict. In GQL, labels are unary (no value), so this ambiguity does not arise. The unified design inherits this edge case. After merge from two sources that assign different risk tiers, the result will carry both values with no indication of disagreement. This is consistent with the "labels do not produce conflicts" rule but may surprise consumers expecting single-valued semantics for certain keys.

- **[Minor] `value` type restricted to string.** Numeric classifications (e.g., risk score 0.75, priority rank 3) must be string-encoded. This is defensible for a serialization format (JSON strings are universally portable), but it means consumers must parse `"0.75"` back to a float. A brief note acknowledging this tradeoff and advising consistent string formatting (e.g., "producers SHOULD use consistent string formatting for numeric classification values") would help interoperability.

- **[Minor] Sort order for absent `value` is specified but edge cases are not.** The spec says "absent values sort before present values," which is clear. However, it does not specify behavior for empty strings (`"value": ""`). Is `""` equivalent to absent? I assume not (it is a present string that happens to be empty), but explicit clarification would prevent implementation divergence.

---

## Recommendations

1. **(P1)** Add an L2 validation rule: "The `labels` array on a single node or edge SHOULD NOT contain duplicate `{key, value}` pairs." This catches producer bugs early without blocking files.

2. **(P1)** Add a brief note in Section 8.4: "When a key admits only one classification value at a time (e.g., a risk tier), producers SHOULD emit only the current value. After merge, consumers encountering multiple values for the same key SHOULD treat them as observations from different sources rather than a conflict." This sets expectations without over-constraining the model.

3. **(P2)** Clarify that `"value": ""` (empty string) is distinct from absent `value` and sorts as a present value (lexicographically before any non-empty string). One sentence suffices.

4. **(P2)** When OMTS defines its first reserved (dotless) vocabulary keys (per the R3 recommendation from the original panel), consider whether any keys should be formally single-valued (e.g., `risk-tier` admits exactly one value per entity). This would be defined in the vocabulary specification, not in Section 8.4 itself, preserving the simplicity of the current design.

---

## Cross-Expert Notes

- **Supply Chain Expert / Procurement Expert:** The unified `labels` array directly addresses the Kraljic quadrant, regulatory scope, and supplier diversity classification use cases raised by both experts. A supplier carrying `[{"key": "com.acme.kraljic", "value": "strategic"}, {"key": "org.lksg.risk-priority", "value": "high"}]` is exactly the multi-dimensional segmentation pattern they requested. However, the multi-valued ambiguity I note above (Concern 2) is particularly relevant to procurement workflows where a single supplier should have exactly one Kraljic quadrant. The recommended vocabulary work (original R3) should address single-valued semantics for specific keys.

- **Enterprise Integration Expert:** The `labels` array provides the missing mapping target for SAP vendor groups, Oracle DFFs, and D365 VendorGroupId. SPEC-005 should be updated to show concrete mapping examples: `{"key": "com.sap.vendor-group", "value": "ZSTR"}`, `{"key": "com.oracle.procurement-bu", "value": "US_PROC_BU"}`. This directly unblocks the original R4 recommendation.

- **Regulatory Compliance Expert:** The `reporting_entity` field resolves the tier ambiguity for LkSG/CSDDD due diligence scoping. The merge provenance treatment (SPEC-003, Section 6) correctly handles the multi-perspective problem: when two companies merge their supply chain graphs, the merged file drops `reporting_entity` and records both perspectives in `merge_metadata`. This is exactly the behavior needed for consortium-level compliance reporting. The labels mechanism supports lightweight regulatory tagging (e.g., `{"key": "eu.csddd.in-scope"}`) without misusing attestation nodes.

---

Sources:
- [ISO/IEC 39075:2024 - GQL](https://www.iso.org/standard/76120.html)
- [GQL Standards](https://www.gqlstandards.org/)
- [Labels vs. Indexed Properties in Neo4j Data Modelling](https://graphaware.com/blog/neo4j-graph-model-design-labels-versus-indexed-properties/)
- [Graph Modeling: Labels - Neo4j Developer Blog](https://medium.com/neo4j/graph-modeling-labels-71775ff7d121)
- [Property Graph - Wikipedia](https://en.wikipedia.org/wiki/Property_graph)
- [PG-Schema: Schemas for Property Graphs](https://arxiv.org/html/2211.10962)
- [JSON-Graph Specification](https://github.com/jsongraph/json-graph-specification)
- [Microsoft Fabric GQL Language Guide](https://learn.microsoft.com/en-us/fabric/graph/gql-language-guide)
