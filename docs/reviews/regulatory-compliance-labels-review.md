# Expert Review: Labels Mechanism & Reporting Entity Implementation

**Reviewer:** Regulatory Compliance Expert (Supply Chain Regulatory Compliance Advisor)
**Date:** 2026-02-18
**Topic:** Review of Section 8.4 Labels and `reporting_entity` field (P0 recommendations R1, R2)
**Specs Reviewed:** OMTS-SPEC-001 (Graph Data Model) Section 8.4, Section 2 (file header), Section 9; OMTS-SPEC-003 (Merge Semantics) Section 4, Section 6

---

## Assessment

The implementation of the labels mechanism in Section 8.4 and the `reporting_entity` file header field addresses the two P0 recommendations from the original panel review in a structurally sound manner. From a regulatory compliance standpoint, the `{key, value}` model with reverse-domain namespacing provides sufficient expressiveness to carry the regulatory classifications I identified as missing in my original review -- UFLPA entity list status, LkSG risk priority tiers, EUDR commodity scope flags, CSDDD chain-of-activities scope markers, and CBAM installation classification. The `reporting_entity` field directly resolves the `tier` ambiguity I flagged: a file declaring `"reporting_entity": "org-acme"` now anchors perspective-dependent properties like `tier` to a specific obligated entity, which is essential for multi-entity corporate group compliance under both LkSG and the forthcoming CSDDD transposition.

My principal concern is the omission of temporal validity fields (`valid_from`, `valid_to`) from label entries. In my original panel recommendation, I specifically proposed `{key, value, valid_from, valid_to}` because regulatory classifications are inherently temporal: EUDR country risk classifications are reviewed periodically (first review scheduled for 2026 based on updated FAO FRA data), LkSG mandates annual risk analysis with ad-hoc reassessment upon substantiated knowledge of violations, and the UFLPA Entity List is updated continuously (growing from 66 entities in 2024 to 144 as of 2025). A label `{"key": "gov.dhs.uflpa-entity-list"}` that was valid in Q1 2026 may not be valid in Q3 2026 after entity list revision. Without temporal bounds on the label itself, the only way to determine currency is to cross-reference `snapshot_date` or `data_quality.last_verified` -- workable but indirect.

That said, I recognize this is a design tradeoff favoring simplicity, and the existing `snapshot_date` and `data_quality.last_verified` fields provide a reasonable, if coarser, temporal signal. The implementation is adequate for regulatory classification use cases today, with the temporal limitation being a friction point rather than a blocker. The snapshot-based model means each new `.omts` export reflects the classification state at that point in time, and `previous_snapshot_ref` enables diffing across snapshots to detect classification changes.

---

## Strengths

- **Reverse-domain namespacing directly supports multi-jurisdictional compliance.** A company subject to LkSG, CSDDD, EUDR, and UFLPA simultaneously can use `de.bafa.lksg-risk-priority`, `eu.csddd.scope`, `eu.eudr.commodity-scope`, and `gov.dhs.uflpa-entity-list` labels on the same node without collision. This is exactly what cross-jurisdictional harmonization requires.

- **Boolean flag semantics map cleanly to entity list screening.** UFLPA entity list membership, EUDR high-risk country sourcing flags, and Conflict Minerals Regulation smelter identification are all binary classifications. A label `{"key": "gov.dhs.uflpa-entity-list"}` (no value, presence = true) is the correct semantic representation.

- **Labels on edges enable relationship-level regulatory classification.** LkSG Section 2(7) and CSDDD Article 3(g) define due diligence obligations across the chain of activities, not just at the entity level. Being able to label a `supplies` edge as `{"key": "de.bafa.lksg-prioritized"}` correctly models BAFA's guidance that companies should prioritize specific supplier relationships, not just supplier entities.

- **Set-union merge semantics are correct for regulatory labels.** When two files independently classify the same entity -- one as EUDR-in-scope, the other as LkSG-prioritized -- the merged node should carry both labels. Set union produces this result without conflicts.

- **`reporting_entity` with L1-GDM-05 validation provides the anchor for perspective-dependent compliance data.** The requirement that `reporting_entity` must reference an existing `organization` node prevents dangling references and ensures the obligated entity is fully modeled in the graph.

- **L2-GDM-04 linking `tier` to `reporting_entity` is a well-designed completeness warning.** This catches the exact scenario I flagged in the original review without making `reporting_entity` mandatory for files that do not use `tier`.

---

## Concerns

- **[Major] No temporal validity on labels.** EUDR country risk classifications change through periodic Commission review (Article 29(2)). LkSG requires annual risk analysis updates (Section 5(4)). UFLPA entity list additions occur on a rolling basis. A label asserting `{"key": "eu.eudr.country-risk", "value": "low"}` without `valid_from`/`valid_to` cannot express that this classification was valid as of April 2025 but may be revised in the 2026 review cycle. Workaround exists via `snapshot_date` and `data_quality.last_verified`, but these are file-level and node-level signals respectively, not label-level. When a node carries 5 labels from 3 different regulatory regimes, each with its own review cycle, a single `last_verified` date is insufficient.

- **[Minor] No guidance on OMTS-reserved key prefixes for regulatory classifications.** The spec reserves dotless keys for "future OMTS-defined vocabularies" but does not signal intent to standardize regulatory classification keys. Absent guidance, early adopters will use inconsistent keys (`eu.eudr.scope` vs. `eu.eudr.commodity-scope` vs. `eudr.in-scope`). This is addressed by panel recommendation R3 (P1), but a brief forward reference in Section 8.4 would reduce early fragmentation.

- **[Minor] `value` field is string-only.** EUDR country risk uses a three-level enum (`low`, `standard`, `high`). LkSG risk analysis uses severity and likelihood matrices. These are representable as strings, but without type guidance, producers may use `"high"`, `"HIGH"`, `"3"`, or `"High Risk"` interchangeably. The spec's recommendation of lowercase kebab-case for keys does not extend to values.

- **[Minor] `reporting_entity` in merge provenance could be stronger.** SPEC-003 Section 6 says merged files SHOULD record source `reporting_entity` values and SHOULD omit `reporting_entity` from the header when perspectives differ. This is correct but purely advisory (SHOULD). For regulatory audit trails under LkSG Section 10(2) (documentation and reporting obligation), the provenance of perspective-dependent data is not optional -- it is required evidence. Elevating this to MUST for conformant merge implementations would strengthen audit defensibility.

---

## Recommendations

1. **(P1) Add an informative note in Section 8.4 acknowledging temporal classification use cases and recommending `data_quality.last_verified` as the interim signal for label currency.** This does not change the data model but gives regulatory implementers explicit guidance. A future minor version could add optional `valid_from`/`valid_to` to label entries without breaking backward compatibility.

2. **(P1) Add a forward reference in Section 8.4 to the planned recommended key vocabulary (R3 from the panel report).** Even a single sentence -- "An informative appendix defining recommended keys for common regulatory, procurement, and risk classification taxonomies is planned" -- would signal to early adopters that standardization is coming and discourage premature key proliferation.

3. **(P2) Recommend lowercase kebab-case for label values, not just keys.** Add to the key naming conventions: "Producers SHOULD use lowercase kebab-case for both keys and values to maximize interoperability." This reduces the `"high"` vs. `"High"` vs. `"HIGH"` fragmentation risk.

4. **(P2) Strengthen merge provenance for `reporting_entity` from SHOULD to MUST in SPEC-003 Section 6.** Regulatory audit trails under LkSG, CSDDD, and UK Modern Slavery Act require clear provenance of who reported what. Advisory provenance is insufficient for audit-grade evidence.

---

## Cross-Expert Notes

- **For the Supply Chain Expert:** The labels mechanism directly supports the Kraljic quadrant classification and regulatory risk prioritization workflows you identified. However, the absence of temporal validity means that tracking classification changes over time (e.g., a supplier moving from "leverage" to "strategic" in the Kraljic matrix) requires comparing successive snapshots rather than reading label history on a single node. Evaluate whether the snapshot-diff model is sufficient for your risk trend analysis use cases.

- **For the Procurement Expert:** The `{key, value}` model is simpler than the `{taxonomy, code, label}` model you proposed, but it achieves the same outcome -- `{"key": "com.acme.kraljic", "value": "strategic"}` vs. `{"taxonomy": "kraljic", "code": "strategic", "label": "Strategic Supplier"}`. The tradeoff is that display labels must be resolved by tooling rather than carried in the data. For ERP integration, this means the SPEC-005 mapping (R4) becomes more important.

- **For the Graph Modeling Expert:** The implementation chose a unified `{key, value}` model over your proposed separation of labels (boolean flags) and annotations (key-value pairs). From a regulatory perspective, both boolean flags (`gov.dhs.uflpa-entity-list` with no value) and key-value classifications (`eu.eudr.country-risk: high`) are needed in equal measure. The unified model handles both cases. Evaluate whether the set-union merge semantics interact correctly with your identity predicate exclusion -- particularly whether labels added post-merge could create semantic inconsistencies.

- **For the Enterprise Integration Expert:** The reverse-domain namespacing maps directly to your ERP field mapping concern. SAP vendor group `EKORG` can be carried as `{"key": "com.sap.ekorg", "value": "1000"}`, Oracle DFFs as `{"key": "com.oracle.supplier-type", "value": "preferred"}`, D365 `VendorGroupId` as `{"key": "com.microsoft.d365.vendor-group", "value": "30"}`. Ensure SPEC-005 Section 4.1 is updated (per R4) with these concrete mappings so that ERP export tooling has unambiguous guidance.

---

## Sources

- [EU Corporate Sustainability Due Diligence Directive - European Commission](https://commission.europa.eu/business-economy-euro/doing-business-eu/sustainability-due-diligence-responsible-business/corporate-sustainability-due-diligence_en)
- [CSDDD Timeline Explained - Ecobio Manager](https://ecobiomanager.com/csddd-timeline-explained/)
- [BAFA Risk Analysis Guidance](https://www.bafa.de/EN/Supply_Chain_Act/Risk_Analysis/risk_analysis_node.html)
- [BAFA Identifying, Weighting and Prioritizing Risks](https://www.bafa.de/SharedDocs/Downloads/EN/Supply_Chain_Act/guidance_risk_analysis.pdf?__blob=publicationFile&v=3)
- [UFLPA Entity List - DHS](https://www.dhs.gov/uflpa-entity-list)
- [DHS 2025 UFLPA Update](https://www.cmtradelaw.com/2025/08/dhs-2025-uflpa-update-targets-new-industries-and-expands-entity-list/)
- [EUDR Country Risk Classifications - Ropes & Gray](https://www.ropesgray.com/en/insights/viewpoints/102kcln/european-commission-sets-country-risk-classifications-for-eu-deforestation-regula)
- [EUDR Benchmarking Guide - LiveEO](https://www.live-eo.com/blog/article-eudr-country-risk-benchmarking-guide)
- [EU Deforestation Regulation Updates 2026 - QIMA](https://blog.qima.com/esg/eu-deforestation-update-2026)
- [LkSG Risk Analysis - Envoria](https://envoria.com/insights-news/german-supply-chain-act-lksg-the-importance-of-risk-analysis-incl-practical-tips)
