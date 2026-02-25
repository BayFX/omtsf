# Expert Review: Labels Mechanism & Reporting Entity (Procurement Expert)

**Reviewer:** Procurement Expert (Chief Procurement Officer)
**Date:** 2026-02-18
**Topic:** Review of labels mechanism (Section 8.4) and `reporting_entity` field additions to SPEC-001 and SPEC-003
**Specs Reviewed:** OMTS-SPEC-001 (Graph Data Model), OMTS-SPEC-003 (Merge Semantics)

---

## Assessment

As someone who manages 4,000+ direct suppliers across multiple ERPs and procurement platforms, the addition of both the labels mechanism and `reporting_entity` field directly addresses the two most operationally significant gaps I identified in the previous review. Without a classification mechanism, every OMTS export from our SAP, Coupa, and Jaggaer instances would have been incompatible with one another at the segmentation layer -- the layer where procurement teams actually spend their time filtering, prioritizing, and making decisions. The implementation is pragmatic and shows restraint: it provides a single `labels` array with `{key, value}` semantics rather than the more complex two-array (`labels` + `annotations`) or three-field (`taxonomy`, `code`, `label`) models proposed by other panelists. For adoption, simpler is better.

The `reporting_entity` field in the file header resolves the `tier` ambiguity that made merged files unreliable for tier-based reporting. The design choice to make it a graph-local reference to an existing `organization` node is correct -- it forces producers to actually include the reporting entity as a first-class graph participant rather than declaring it out-of-band. The L2-GDM-04 validation rule ("if any `supplies` edge carries a `tier` property, the file SHOULD declare `reporting_entity`") is a well-calibrated nudge: it warns without blocking, which is the right posture for incremental adoption. The merge provenance handling in SPEC-003 Section 6 -- recording source `reporting_entity` values and omitting the field when sources disagree -- is clean and avoids false precision.

My remaining concerns are operational, not architectural. The implementation is a sound foundation, but without recommended key vocabularies and ERP mapping guidance, adoption will fragment on key naming conventions. Every SAP shop will independently decide how to encode vendor groups, and we will be right back to incompatible custom properties -- just inside a standard container.

---

## Strengths

- **Single unified mechanism.** The `{key, value}` pair model covers both boolean flags (e.g., `com.acme.strategic-supplier`) and key-value classifications (e.g., `com.acme.risk-tier: high`) without requiring implementers to choose between separate label and annotation systems. This matches how SAP S/4HANA and Coupa both model supplier classification internally -- as key-value attribute pairs, not as separate type systems.
- **Reverse-domain namespacing for custom keys.** Reserving dot-free keys for future OMTS vocabularies while directing custom keys to `com.acme.*` notation is a proven pattern (Java packages, Android permissions, Apple UTIs). It prevents the collision risk flagged in finding M7 of the original panel report.
- **Labels do not participate in identity predicates.** This is critical. If labels were merge-identity-significant, adding a classification in one system would prevent merge with the same entity in another system that lacks that classification. The set-union merge behavior means classifications are purely additive, which matches procurement reality -- different stakeholders classify the same supplier differently and all perspectives are valid.
- **Merge by set union with deterministic sort.** The sorted-by-key-then-value output ensures that `merge(A, B) = merge(B, A)`, preserving the algebraic properties in SPEC-003 Section 5. No conflict recording needed for labels, which reduces merge complexity for implementers.
- **`reporting_entity` validation chain.** L1-GDM-05 ensures the reference is structurally valid; L2-GDM-04 warns when `tier` is used without it. This two-level approach lets simple files omit `reporting_entity` while flagging the inconsistency in files that actually use perspective-dependent properties.
- **Advisory 100-label limit.** Realistic ceiling. Our SAP instance has roughly 15 classification dimensions per vendor. Even with multiple systems contributing, 100 is generous without being dangerous.

---

## Concerns

- **[Major] No recommended key vocabulary.** The mechanism is a container without contents. Without at least an informative appendix listing recommended keys for common procurement classifications -- Kraljic quadrant, approval status, supplier diversity category, UNSPSC commodity -- every adopter will independently invent key names. `com.acme.risk-tier`, `com.example.risk_level`, `org.company.riskrating` will all mean the same thing. This was R3 from the original panel and remains unaddressed.

- **[Major] No guidance for mapping ERP segmentation fields.** SPEC-005 acknowledges SAP vendor groups, Oracle DFFs, and D365 `VendorGroupId` but has no mapping to the new labels mechanism. Without concrete examples (e.g., "SAP vendor group `STRA` maps to `{"key": "com.sap.vendor-group", "value": "STRA"}`"), ERP integration developers will make inconsistent choices. This was R4 from the original panel.

- **[Minor] `value` is string-only.** Procurement classifications sometimes carry numeric values (e.g., a Kraljic score of 7.5, a risk rating of 3). Encoding these as strings (`"value": "7.5"`) works but loses type information for downstream analytics. This is tolerable for v1 but should be noted as a known limitation.

- **[Minor] No temporal validity on labels.** A supplier classified as `com.acme.risk-tier: high` in 2024 may have been reclassified to `low` in 2026. The current model has no way to express when a classification was valid. During merge, both values would coexist as separate `{key, value}` pairs (`high` and `low`), which is technically correct (both were true at different times) but potentially confusing. The `snapshot_date` on the file partially mitigates this.

- **[Minor] `tier` still only on `supplies` edges.** The original panel (R5) recommended extending `tier` to `subcontracts`, `tolls`, `distributes`, and `brokers` because LkSG Section 2(7) and CSDDD apply to all supply chain relationship types. This was not addressed in the current changes. The labels mechanism provides a workaround (e.g., `{"key": "com.acme.tier", "value": "2"}` on a `subcontracts` edge) but a workaround is not the same as first-class support.

---

## Recommendations

1. **(P1) Publish an informative appendix or SPEC-006 section defining recommended label keys for common procurement classifications.** At minimum: Kraljic quadrant, approval/lifecycle status, supplier diversity classification, regulatory scope (LkSG, CSDDD, EUDR, UFLPA), UNSPSC commodity code, and business unit / purchasing organization. Use OMTS-reserved (dot-free) keys for these standard vocabularies. This directly addresses the original panel R3.

2. **(P1) Add concrete ERP-to-labels mapping examples in SPEC-005.** Show how SAP vendor groups, Oracle supplier classification DFFs, D365 `VendorGroupId`, Coupa supplier tags, and Jaggaer classification attributes map to the `labels` array. Without this, ERP integration developers are flying blind. This directly addresses the original panel R4.

3. **(P1) Extend `tier` as a named property to `subcontracts`, `tolls`, `distributes`, and `brokers` edge types.** Regulatory frameworks do not distinguish between relationship types when counting tiers. Relying on labels as a workaround for a property the spec already defines on `supplies` is inconsistent. This directly addresses the original panel R5.

4. **(P2) Consider adding an optional `valid_from` / `valid_to` to label entries** for time-sensitive classifications. This would let merged files carry historical classification context and avoid the ambiguity of two conflicting values for the same key coexisting after set union.

5. **(P2) Document the string-only `value` limitation** and provide guidance for encoding numeric classifications (e.g., "encode numeric values as decimal strings; consumers SHOULD parse numeric label values when the key's vocabulary defines a numeric domain").

---

## Cross-Expert Notes

- **Enterprise Integration Expert:** The labels mechanism directly unblocks the ERP field mapping work (R4). I recommend we coordinate on a shared set of mapping examples for SPEC-005. The `com.sap.*`, `com.oracle.*`, `com.microsoft.*` namespace convention should be established before multiple implementations diverge.

- **Graph Modeling Expert:** The single `labels` array (rather than separate labels + annotations) means graph databases loading OMTS data will need to decide at import time which labels to promote to native graph labels for indexing. The spec should note that labels with keys from OMTS-reserved vocabularies are candidates for native label promotion in graph databases.

- **Regulatory Compliance Expert:** The `tier` gap on non-`supplies` edges remains relevant for LkSG/CSDDD compliance. Labels provide an interim workaround, but the Regulatory Compliance Expert should confirm whether the workaround is acceptable for regulatory reporting or whether first-class `tier` support is required on all supply relationship edge types.

- **Supply Chain Expert:** The `reporting_entity` implementation matches your original R2 proposal closely. The merge provenance handling in SPEC-003 Section 6 should satisfy the concern about `tier` ambiguity in merged files. The remaining gap is the recommended key vocabulary (R3), which we should define collaboratively.

---

## Sources

- [SAP S/4HANA Supplier Classification and Segmentation (Scope Item 19E)](https://www.scribd.com/document/415199324/19E-S4CLD1902-Supplier-Classification-and-Segmentation)
- [Coupa AI-Driven Commodity Classification](https://www.coupa.com/blog/using-ai-driven-commodity-classification-improve-business-spend/)
- [SAP S/4HANA Supplier Classification Overview](https://www.accio.com/supplier/supplier-classification-in-sap-s4hana)
- [GS1 Global Traceability Standard](https://www.gs1.org/standards/gs1-global-traceability-standard/current-standard)
