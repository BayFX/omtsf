# Expert Panel Report: Labels Mechanism and Reporting Entity Implementation

**Date:** 2026-02-18
**Panel Chair:** Expert Panel Review Process
**Topic:** Review of the labels mechanism (Section 8.4) and `reporting_entity` field additions to SPEC-001 and SPEC-003, implementing the P0 recommendations (R1 and R2) from the supply chain segmentation panel review.

---

## Panel Chair Summary

All five experts agree that the implementation of both P0 recommendations -- the `labels` array (R1) and the `reporting_entity` file header field (R2) -- is architecturally sound and directly addresses the concerns raised by the original segmentation panel. No expert rated any concern as Critical, and the Graph Modeling Expert (whose original proposal of separate `labels` + `annotations` arrays was simplified to a unified design) explicitly accepted the tradeoff, noting that OMTS is a serialization format and need not mirror graph database indexing structures. The unified `{key, value}` model with optional `value` for boolean flags was praised across the panel for its simplicity and adoption-friendliness.

The strongest consensus finding is that the labels mechanism, while structurally complete, needs **recommended key vocabularies** to achieve its interoperability goals. The Procurement Expert and Regulatory Compliance Expert both rated this as a Major concern: without standard key names for common classifications (Kraljic quadrant, regulatory scope, approval status, supplier diversity), every adopter will invent their own conventions, fragmenting the very interoperability the mechanism is designed to enable. This was original panel recommendation R3 and remains the most important follow-up work.

A secondary consensus emerged around **temporal validity on labels**. Four of five experts noted that classifications change over time (LkSG mandates annual risk review, EUDR country risk classifications are periodically revised, supplier Kraljic quadrants shift with market conditions). The Regulatory Compliance Expert rated this Major; the other three rated it Minor. All acknowledged that the snapshot-based temporal model (`snapshot_date` + `previous_snapshot_ref`) provides a workable workaround, but label-level temporality would strengthen regulatory audit trail support. The panel recommends this as a P2 enhancement that can be added in a future minor version without breaking backward compatibility.

There was no disagreement between panelists on the core design. The variation was limited to severity ratings for the temporal validity concern (Major vs Minor) and the priority of follow-up work items. The implementation successfully resolves the two P0 findings that motivated it.

---

## Panel Composition

| Panelist | Role | Key Focus Area |
|----------|------|---------------|
| Supply Chain Expert | Supply Chain Visibility & Risk Analyst | Real-world segmentation workflows, regulatory risk analysis |
| Procurement Expert | Chief Procurement Officer | ERP usability, adoption cost, supplier classification |
| Graph Modeling Expert | Graph Data Modeling & Algorithm Specialist | GQL alignment, merge algebra, serialization fidelity |
| Enterprise Integration Expert | Enterprise Systems Architect | ERP export mapping, SAP/Oracle/D365 field alignment |
| Regulatory Compliance Expert | Supply Chain Regulatory Compliance Advisor | LkSG/CSDDD/EUDR/UFLPA classification requirements |

---

## Consensus Findings

### 1. The implementation adequately addresses both P0 recommendations (5/5 experts)

All five experts confirmed that the `labels` mechanism (R1) and `reporting_entity` field (R2) resolve the gaps identified in the original panel review. The `{key, value}` model is expressive enough for all identified use cases (Kraljic quadrants, regulatory scope, ERP vendor groups, diversity classifications, entity list flags). The `reporting_entity` field anchors perspective-dependent properties like `tier`.

### 2. Recommended key vocabularies are needed before adoption (4/5 experts)

The Procurement Expert, Supply Chain Expert, Regulatory Compliance Expert, and Enterprise Integration Expert all flagged that without recommended key names, adopters will independently invent incompatible conventions. This was original panel recommendation R3 (P1) and remains the highest-priority follow-up.

### 3. Temporal validity on labels is a friction point for regulatory compliance (4/5 experts)

The Supply Chain Expert, Procurement Expert, Enterprise Integration Expert, and Regulatory Compliance Expert all noted that classifications change over time and the current model cannot express when a label was valid. The snapshot-based workaround is adequate but coarse. The Regulatory Compliance Expert rated this Major; others rated it Minor.

### 4. ERP mapping guidance is needed in SPEC-005 (3/5 experts)

The Procurement Expert, Enterprise Integration Expert, and Regulatory Compliance Expert converged on the need for concrete ERP-to-labels mapping examples in SPEC-005, showing how SAP vendor groups, Oracle business classifications, and D365 vendor groups map to the `labels` array.

### 5. `tier` property scope on non-`supplies` edges remains unaddressed (3/5 experts)

The Supply Chain Expert, Procurement Expert, and Regulatory Compliance Expert noted that the original panel recommendation R5 (extend `tier` to `subcontracts`, `tolls`, etc.) was not addressed. Labels provide a workaround but not first-class support.

---

## Critical Issues

No expert rated any concern as **[Critical]**. The implementation is considered viable as-is.

---

## Major Issues

### M1. No recommended key vocabulary (Procurement Expert, Supply Chain Expert)
**Severity:** Major (Procurement), P1 (Supply Chain)

The labels mechanism is a container without standardized contents. Without recommended keys for common procurement and regulatory classifications, every adopter will independently invent key names (`com.acme.risk-tier` vs `com.example.risk_level` vs `org.company.riskrating`). This directly undermines the interoperability goal. Original panel R3 remains unaddressed.

### M2. No ERP field mapping guidance (Procurement Expert, Enterprise Integration Expert)
**Severity:** Major

SPEC-005 acknowledges SAP vendor groups, Oracle DFFs, and D365 `VendorGroupId` but still has no mapping to the new labels mechanism. Without concrete examples, ERP integration developers will make inconsistent choices. Original panel R4 is now unblocked but not yet delivered.

### M3. No temporal validity on labels (Regulatory Compliance Expert)
**Severity:** Major (Regulatory), Minor (3 others)

EUDR country risk classifications change through periodic review, LkSG mandates annual risk analysis updates, and the UFLPA Entity List is updated continuously. A label without `valid_from`/`valid_to` cannot express when a classification was effective. The snapshot-based workaround is functional but indirect, especially when a node carries labels from multiple regulatory regimes with different review cycles.

### M4. No guidance on multi-org classification differences (Enterprise Integration Expert)
**Severity:** Major

SAP purchasing organizations and Oracle procurement BUs scope classifications to specific buying entities. Two purchasing orgs may classify the same vendor differently. The label model can handle this but there is no documented pattern, risking inconsistent encoding.

---

## Minor Issues

| # | Issue | Flagged By |
|---|-------|-----------|
| m1 | No uniqueness constraint on duplicate `{key, value}` pairs within a single node/edge | Graph Modeling Expert |
| m2 | No guidance on multi-valued classification semantics (same key, different values) | Graph Modeling Expert, Supply Chain Expert |
| m3 | `value` is string-only, losing type information for numeric classifications | Procurement Expert, Graph Modeling Expert, Enterprise Integration Expert, Regulatory Compliance Expert |
| m4 | Empty string vs absent `value` edge case unspecified | Graph Modeling Expert |
| m5 | `reporting_entity` merge semantics could lose perspective data for `tier` interpretation | Supply Chain Expert |
| m6 | `tier` still limited to `supplies` edges only (panel R5 unaddressed) | Supply Chain Expert, Procurement Expert, Regulatory Compliance Expert |
| m7 | No temporal validity on individual labels | Supply Chain Expert, Procurement Expert, Enterprise Integration Expert |
| m8 | Merge provenance for `reporting_entity` is SHOULD not MUST | Regulatory Compliance Expert |
| m9 | No `buying_org` label convention documented | Enterprise Integration Expert |
| m10 | Kebab-case recommendation for keys does not extend to values | Regulatory Compliance Expert |

---

## Consolidated Recommendations

### P0 -- Immediate

No P0 recommendations. The implementation is sound as delivered.

### P1 -- Before v1

| # | Recommendation | Originating Expert(s) |
|---|---------------|----------------------|
| R1 | **Publish recommended label keys** in an informative appendix or SPEC-006 for common classifications: Kraljic quadrant, approval status, supplier diversity, regulatory scope (LkSG, CSDDD, EUDR, UFLPA, CBAM), UNSPSC commodity code, business unit. Use OMTS-reserved (dotless) keys. | Procurement Expert, Supply Chain Expert, Regulatory Compliance Expert |
| R2 | **Add concrete ERP-to-labels mapping examples in SPEC-005** for SAP vendor groups, Oracle business classifications, D365 `VendorGroupId`, Coupa supplier tags. | Procurement Expert, Enterprise Integration Expert, Regulatory Compliance Expert |
| R3 | **Extend `tier` property to `subcontracts`, `tolls`, `distributes`, `brokers` edge types.** Regulatory frameworks do not distinguish relationship types when counting tiers. | Supply Chain Expert, Procurement Expert, Regulatory Compliance Expert |
| R4 | **Add L2 validation rule against duplicate `{key, value}` pairs** within a single node/edge. Catches producer bugs without blocking files. | Graph Modeling Expert |
| R5 | **Add guidance note on multi-valued key semantics.** Clarify that `{key, value}` is the atomic identity unit (multiple values for the same key are permitted), and that after merge, consumers encountering multiple values for the same key should treat them as observations from different sources. | Graph Modeling Expert, Supply Chain Expert |
| R6 | **Add informative note on tier interpretation in multi-perspective merged graphs.** When a merged file omits `reporting_entity`, `tier` values should be interpreted via `merge_metadata` provenance. | Supply Chain Expert |
| R7 | **Add guidance for encoding organizational-scope-dependent classifications.** Document recommended pattern for SAP `EKORG`-scoped classifications (e.g., embed scope in key: `com.acme.ekorg-1000.vendor-group`). | Enterprise Integration Expert |
| R8 | **Add forward reference in Section 8.4 to planned key vocabulary.** Signal to early adopters that standardization is coming to discourage premature key proliferation. | Regulatory Compliance Expert |

### P2 -- Future

| # | Recommendation | Originating Expert(s) |
|---|---------------|----------------------|
| R9 | **Consider optional `valid_from`/`valid_to` on label entries** for temporal classification tracking. Design so it does not break set-union merge model. | All 5 experts |
| R10 | **Clarify empty string vs absent `value` distinction.** State that `"value": ""` is distinct from absent and sorts as a present value. | Graph Modeling Expert |
| R11 | **Recommend lowercase kebab-case for label values**, not just keys. Reduces `"high"` vs `"HIGH"` fragmentation. | Regulatory Compliance Expert |
| R12 | **Document string-only `value` limitation** with guidance for encoding numeric classifications as decimal strings. | Procurement Expert, Enterprise Integration Expert |
| R13 | **Strengthen merge provenance for `reporting_entity` from SHOULD to MUST.** Regulatory audit trails require clear provenance of perspective-dependent data. | Regulatory Compliance Expert |

---

## Cross-Domain Interactions

### Classification vocabulary is the critical path for interoperability (Procurement <-> Regulatory <-> ERP)
Three experts converged on the same conclusion from different angles: without recommended key names, the labels mechanism provides a standard container with non-standard contents. The Procurement Expert needs Kraljic keys, the Regulatory Compliance Expert needs LkSG/CSDDD scope keys, and the Enterprise Integration Expert needs ERP field mapping conventions. R1 (recommended vocabulary) and R2 (ERP mappings) are tightly coupled and should be developed together.

### Temporal validity affects regulatory audit defensibility (Regulatory <-> Supply Chain <-> ERP)
The Regulatory Compliance Expert's Major rating for temporal validity is grounded in specific statutory requirements (LkSG Section 5(4) annual risk review, EUDR Article 29(2) periodic country risk reclassification). The Supply Chain Expert and Enterprise Integration Expert acknowledged the same gap but accepted the snapshot-based workaround. The design allows adding temporal fields in a future minor version without breaking compatibility.

### Multi-org classification requires a documented pattern (ERP <-> Procurement)
The Enterprise Integration Expert flagged that SAP `EKORG`-scoped and Oracle `ProcurementBUId`-scoped classifications need a consistent encoding convention. The Procurement Expert's concern about ERP mapping guidance is directly related -- without a multi-org pattern, the SAP-to-OMTS mapping in SPEC-005 will be incomplete.

### GQL alignment is sufficient for a serialization format (Graph <-> All)
The Graph Modeling Expert explicitly acknowledged that the unified `labels` array (vs separate labels + annotations) is a pragmatic choice for a file format. Conformant consumers can load the data into graph databases using either label-promotion or property-indexing strategies. This removes the GQL alignment concern from the original panel review.

---

## Individual Expert Reports

### Supply Chain Expert

**Verdict:** Solid implementation. Both P0 concerns addressed.

**Strengths:** `reporting_entity` resolves tier ambiguity; set-union merge is conflict-free; namespaced keys prevent collisions; boolean flags map to binary classifications naturally.

**Concerns:** All Minor -- no temporal validity on labels; `reporting_entity` merge could lose perspective data; no guidance on same-key multi-value cardinality; `tier` still only on `supplies` edges.

**Top Recommendations:** (P1) Clarify multi-value cardinality; (P1) Add tier interpretation guidance for merged graphs; (P1) Extend `tier` to other edge types; (P1) Define recommended vocabulary; (P2) Consider temporal fields on labels.

---

### Procurement Expert

**Verdict:** Sound foundation, but adoption will fragment without recommended vocabularies.

**Strengths:** Single unified mechanism matching ERP key-value patterns; reverse-domain namespacing; labels excluded from identity predicates; deterministic merge; well-calibrated `reporting_entity` validation chain.

**Concerns:** [Major] No recommended key vocabulary (R3 unaddressed); [Major] No ERP mapping guidance (R4 unaddressed); [Minor] string-only values; [Minor] no temporal validity; [Minor] `tier` scope limited.

**Top Recommendations:** (P1) Publish recommended keys; (P1) Add ERP mapping examples in SPEC-005; (P1) Extend `tier`; (P2) Consider temporal fields; (P2) Document string-only limitation.

---

### Graph Modeling Expert

**Verdict:** Defensible simplification of the GQL-inspired separate labels/annotations proposal. Merge algebra fully intact.

**Strengths:** Round-trip fidelity; commutative/associative/idempotent merge; consistent namespacing; placement consistency with `data_quality`; identity predicate exclusion; graph-local `reporting_entity` reference.

**Concerns:** All Minor -- no uniqueness constraint on duplicate pairs; no multi-valued semantics guidance; string-only values; empty string vs absent edge case.

**Top Recommendations:** (P1) L2 rule against duplicates; (P1) Guidance on multi-valued keys; (P2) Clarify empty string; (P2) Reserve single-valued semantics for future vocabulary keys.

---

### Enterprise Integration Expert

**Verdict:** Directly unblocks ERP mapping work. Design maps cleanly to SAP/Oracle/D365 classification structures.

**Strengths:** Direct ERP field mapping; reverse-domain prevents cross-company collisions; boolean flags cover indicator fields; set-union merge correct for classification data; `reporting_entity` anchors perspective.

**Concerns:** [Major] No temporal validity on labels; [Major] No multi-org classification encoding guidance; [Minor] string-only values; [Minor] no `buying_org` convention documented.

**Top Recommendations:** (P1) Document org-scope-dependent classification pattern; (P1) Update SPEC-005 with label mapping examples; (P1) Consider temporal fields; (P2) Document `buying_org` label convention.

---

### Regulatory Compliance Expert

**Verdict:** Adequate for regulatory classification today, with temporal limitation as friction rather than blocker.

**Strengths:** Reverse-domain supports multi-jurisdictional compliance; boolean flags map to entity list screening; edge labels enable relationship-level regulatory classification; set-union correct for regulatory labels; L1/L2 validation chain well-designed.

**Concerns:** [Major] No temporal validity on labels (EUDR, LkSG, UFLPA all require temporal classification awareness); [Minor] no guidance on regulatory key prefixes; [Minor] string-only values; [Minor] merge provenance SHOULD vs MUST.

**Top Recommendations:** (P1) Informative note on temporal workaround; (P1) Forward reference to planned vocabulary; (P2) Kebab-case for values; (P2) Strengthen merge provenance to MUST.
