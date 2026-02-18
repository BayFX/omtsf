# Expert Panel Report: OMTSF Entity Identification Specification (Revision 2)

**Spec Reviewed:** OMTSF-SPEC-001 — Entity Identification (Draft, Revision 2, 2026-02-17)
**Panel Date:** 2026-02-18
**Panel Chair:** Automated synthesis of 9 independent expert reviews

---

## Panel Chair Summary

Revision 2 of the Entity Identification Specification represents a decisive maturation from its predecessor. All nine panelists confirmed that their P0/Critical findings from the initial review have been resolved — the spec now includes supply relationship edge types, an attestation model, beneficial ownership support, boundary reference collision fixes, licensing clarity, and governance scaffolding. No panelist identified new Critical-severity design flaws. The specification has moved from "theoretically sound" to "practically implementable," with the consensus assessment that it is ready for draft finalization pending resolution of the issues documented below.

The strongest consensus finding — flagged independently by three panelists from three different perspectives (Varga/graph theory, Lindgren/procurement, Okafor/governance) — is that the `same_as` edge type introduced in Section 14.1 is informally specified and would be rejected by conformant validators under L1-ID-16. This is elevated to P0 because it creates a direct contradiction within the specification. The second strongest consensus, supported by three panelists (Osei, Engström, Moreau), is the need for a confidence/verification field on identifier records to distinguish verified from self-reported data — essential for risk-weighted merge and regulatory reporting.

Areas of productive disagreement exist between the security perspective (Tanaka advocates fresh salt per file for maximum anti-enumeration protection) and the supply chain perspective (Osei notes this makes redacted subgraphs un-mergeable). Between the standards perspective (Nakamura wants normative ISO 6523 ICD mapping tables) and the adoption perspective (Lindgren wants a one-page quick-start guide), a healthy tension exists about where to invest specification effort. The panel recommends both, recognizing they serve different audiences. The ERP integration expert (Krishnamurthy) and regulatory expert (Moreau) independently identified the same structural gap from different angles: the absence of a BOM/`composed_of` edge type (Krishnamurthy) and the absence of consignment-level attestation linkage (Moreau) are two facets of the same material traceability gap that EUDR enforcement demands.

## Panel Composition

| Panelist | Role | Key Focus Area |
|----------|------|----------------|
| Patricia Engström | Entity Identification & Corporate Hierarchy Specialist | DUNS/LEI lifecycle, merge identity, corporate structure |
| Dr. Kenji Nakamura | Standards Development & Interoperability Specialist | ISO/GS1 alignment, conformance clauses, governance |
| Prof. Elena Varga | Graph Data Modeling & Algorithm Specialist | Merge algebra, graph model formalism, edge identity |
| Rajesh Krishnamurthy | Enterprise Systems Architect | ERP mapping, SAP/Oracle/D365 integration, delta updates |
| Dr. Isabelle Moreau | Supply Chain Regulatory Compliance Advisor | CSDDD, EUDR, LkSG, attestation, beneficial ownership |
| Dr. Yuki Tanaka | Data Security & Privacy Architect | Sensitivity, boundary references, GDPR, cryptographic integrity |
| Dr. Amara Osei | Supply Chain Visibility & Risk Analyst | Multi-tier visibility, disruption modeling, data quality |
| Marcus Lindgren | Chief Procurement Officer | Operational usability, adoption, supplier burden |
| Danielle Okafor | Open Source Strategy & Governance Lead | TSC charter, licensing, adoption flywheel, contributor process |

---

## Consensus Findings

These issues were independently identified by multiple panelists, giving them the highest confidence weight.

### 1. `same_as` edge type needs formal specification (3 panelists)

**Flagged by:** Varga (graph algebra), Lindgren (procurement usability), Okafor (governance)

Section 14.1 introduces a `same_as` edge type for intra-file deduplication, but it is absent from the formal edge type taxonomies (Sections 6, 7, 8) and the recognized type list in L1-ID-16. A conformant validator would reject files containing `same_as` edges. This is a direct contradiction within the specification. Varga additionally flags that `same_as` transitivity (Open Question #4) must be resolved because it affects merge algebra. Okafor notes this sets a governance precedent for how edge types are introduced.

### 2. Confidence/verification field on identifier records (3 panelists)

**Flagged by:** Osei (supply chain risk), Engström (entity resolution quality), Moreau (regulatory reporting)

No mechanism exists to distinguish a DUNS number verified against D&B's API from one self-reported on a supplier questionnaire. Risk scoring, merge quality assessment, and regulatory reporting all require this signal. Recommended enum: `verified`, `reported`, `inferred`, `unverified`.

### 3. Temporal overlap consideration in merge identity predicate (2 panelists)

**Flagged by:** Engström (DUNS reassignment risk), Varga (merge algebra correctness)

The identity predicate (Section 9.1) checks scheme, value, and authority equality but does not consider `valid_from`/`valid_to`. An identifier reassigned to a different entity (common with DUNS and GLN) could cause false merges. Both panelists recommend requiring temporal overlap or emitting warnings when ranges don't overlap.

### 4. TSC organizational infrastructure missing (2 panelists)

**Flagged by:** Okafor (governance), Nakamura (standards process)

Section 4.3 normatively depends on a TSC that does not exist. No charter, membership criteria, quorum, or formation process is defined. Until constituted, the governance process is unexecutable.

### 5. Material traceability gap: BOM + consignment-level attestation (2 panelists)

**Flagged by:** Krishnamurthy (BOM/`composed_of` edge type), Moreau (consignment-level attestation linkage)

The spec cannot model what a product is made of (no BOM decomposition) or link attestations to specific consignments (attestation only attaches to entities/facilities). Both are required for EUDR Article 4(2) raw-material-to-finished-product traceability and CBAM embedded emissions calculations.

---

## Critical Issues

| ID | Issue | Panelist(s) | Section |
|----|-------|-------------|---------|
| C1 | `same_as` edge type contradicts L1-ID-16 — informally introduced in Section 14.1, absent from recognized edge types, would be rejected by conformant validators | Varga, Lindgren, Okafor | 14.1, 11.1 |
| C2 | TSC charter undefined — governance process in Section 4.3 is unexecutable without organizational infrastructure | Okafor, Nakamura | 4.3 |

---

## Major Issues

| ID | Issue | Panelist(s) | Section |
|----|-------|-------------|---------|
| M1 | Edge merge fallback on "property equality" is underspecified — "non-temporal properties" not formally defined per edge type | Varga | 9.2 |
| M2 | No confidence/verification field on identifier records | Osei, Engström, Moreau | 3.2 |
| M3 | No ISO 6523 ICD mapping table — blocks interop with PEPPOL/EN 16931 | Nakamura | 12 |
| M4 | No conformance clauses for Producer, Consumer, Validator roles | Nakamura | (missing) |
| M5 | No BOM/`composed_of` edge type for material decomposition | Krishnamurthy | 7 |
| M6 | No delta/patch update pattern for incremental ERP exports | Krishnamurthy | (missing) |
| M7 | SAP Business Partner model (`BUT000`/`BUT0ID`) not mapped | Krishnamurthy | 13.1 |
| M8 | Oracle SCM Cloud and D365 mappings remain too shallow | Krishnamurthy | 13.2, 13.3 |
| M9 | Attestation lacks chain-of-custody linkage to specific consignments | Moreau | 8 |
| M10 | No attestation revocation or supersession model | Moreau | 8 |
| M11 | CSDDD downstream "chain of activities" not modeled | Moreau | 7 |
| M12 | `nat-reg` default sensitivity of `public` incorrect for sole proprietorships (GDPR) | Tanaka | 10.1 |
| M13 | Boundary reference stability across re-exports unaddressed | Tanaka | 10.3 |
| M14 | No guidance for conflicting DUNS-to-LEI mappings during transitive closure merge | Engström | 9.3 |
| M15 | Joint ventures and split-identity entities unmodeled | Engström | 5, 6 |
| M16 | Merge conflict record structure undefined — disagreements are human-readable only | Osei | 9.3 |
| M17 | No quantitative properties on supply edges (volume, capacity, spend share) | Osei | 7 |
| M18 | No supplier-facing data collection workflow guidance | Lindgren | 14 |
| M19 | No procurement-specific relationship context (approved/blocked status, contract terms) | Lindgren | 7 |
| M20 | No contributor process (CONTRIBUTING.md, DCO/CLA) | Okafor | (missing) |
| M21 | ISO 6523 relationship understated as "informed by" vs "aligns with" | Nakamura | 12.2 |

---

## Minor Issues

| ID | Issue | Panelist(s) | Section |
|----|-------|-------------|---------|
| m1 | No canonical ordering for merged identifier arrays — weakens idempotency | Varga | 9.3 |
| m2 | `boundary_ref` not listed in Section 5 node type taxonomy | Varga | 5, 10.3 |
| m3 | Hyperedge/n-ary relationship pattern undocumented | Varga | 7 |
| m4 | Edge merge commutativity depends on unspecified property comparison semantics | Varga | 9.4 |
| m5 | Scheme vocabulary lacks versioning semantics | Nakamura | 4 |
| m6 | GS1 EPC URI conversion rules incomplete | Nakamura | 12.2 |
| m7 | UNTDID 3055 reference is incorrect/misleading | Nakamura | 12.2 |
| m8 | No EDI coexistence positioning | Krishnamurthy | (missing) |
| m9 | `authority` naming convention for `internal` scheme not formalized | Krishnamurthy, Lindgren | 4.1 |
| m10 | `beneficial_ownership` percentage optional with no band alternative | Moreau | 6.5 |
| m11 | No `regulatory_jurisdiction` field on attestation nodes | Moreau | 8 |
| m12 | UFLPA entity list screening not addressed | Moreau | 12.3 |
| m13 | `former_identity` does not capture successor liability | Moreau | 6.4 |
| m14 | Disclosure scope lacks cryptographic binding | Tanaka | 10.2 |
| m15 | Merge provenance lacks trust domain attribution | Tanaka | 9.5 |
| m16 | No guidance on file-level encryption or transport security | Tanaka | (missing) |
| m17 | Opaque identifier values not length/format constrained in validation | Tanaka | 11.1 |
| m18 | OpenCorporates not referenced as enrichment source | Engström | 14.2 |
| m19 | DUNS reassignment risk not documented | Engström | 4.1 |
| m20 | `tolls` edge direction may confuse bidirectional material flows | Osei | 7.3 |
| m21 | No sub-tier mapping completeness metadata | Osei | (missing) |
| m22 | `good` node lacks batch/lot-level granularity for EUDR | Osei, Moreau | 5.3 |
| m23 | Cost-of-identifier-acquisition unacknowledged | Lindgren | 14 |
| m24 | No guidance on handling M&A in operational procurement context | Lindgren | 6.4, 14 |
| m25 | Multi-ERP landscape deduplication under-specified | Lindgren | 14 |
| m26 | No conformance test suite plan | Okafor | (missing) |
| m27 | Adoption complexity for small suppliers — no quick-start guide | Okafor | (missing) |
| m28 | Extension scheme registry governance implicit | Okafor | 4.2 |
| m29 | Governance scope limited to identifier schemes only | Okafor | 4.3 |
| m30 | No state-owned enterprise modeling guidance | Moreau | 14 |

---

## Consolidated Recommendations

### P0 — Immediate (before draft finalization)

| # | Recommendation | Originator(s) |
|---|---------------|---------------|
| P0-1 | **Formally specify `same_as` as a core edge type.** Add to edge type taxonomy, include in L1-ID-16 recognized types, define transitivity (resolve Open Question #4), define properties (`confidence`, `basis`), and specify interaction with merge semantics (include in union-find computation). | Varga, Lindgren, Okafor |
| P0-2 | **Draft a TSC charter.** Define membership criteria, quorum, voting, conflict-of-interest, bootstrap process. Scope authority over all normative registries (schemes, edge types, node types, validation rules, merge semantics). | Okafor, Nakamura |

### P1 — Before v1.0

| # | Recommendation | Originator(s) |
|---|---------------|---------------|
| P1-1 | **Add confidence/verification field to identifier records.** Enum: `verified`, `reported`, `inferred`, `unverified`. Optional `verification_date` and `verification_source`. | Osei, Engström, Moreau |
| P1-2 | **Add temporal overlap consideration to merge identity predicate.** Require overlap or emit L2 warning when identifier temporal ranges don't overlap. | Engström, Varga |
| P1-3 | **Add normative ISO 6523 ICD mapping table.** Map core schemes to ICD codes (lei→0199, gln→0088, duns→0060). Provide bidirectional conversion algorithm. | Nakamura |
| P1-4 | **Define conformance clauses for Producer, Consumer, Validator roles.** Anchor RFC 2119 language to specific role requirements. | Nakamura |
| P1-5 | **Resolve Open Question #3: edge merge identity.** Define edge identity as composite key (resolved source, resolved target, type) when no external identifiers present. Retain both as parallel edges rather than property-based dedup. | Varga |
| P1-6 | **Mandate canonical ordering of identifier arrays after merge.** Sort by canonical string form (Section 3.4) in lexicographic UTF-8 byte order. | Varga |
| P1-7 | **Add `composed_of` edge type for BOM decomposition.** Properties: `quantity`, `unit`, `valid_from`, `valid_to`. Map to SAP STPO/STKO, Oracle BOM_STRUCTURES_B. | Krishnamurthy, Moreau |
| P1-8 | **Define delta/patch envelope for incremental updates.** File-level `update_type` (snapshot/delta), operations array (add/modify/remove). | Krishnamurthy |
| P1-9 | **Add SAP Business Partner model mapping.** Map BUT000, BUT0ID, CDS views alongside LFA1 fields. | Krishnamurthy |
| P1-10 | **Expand Oracle SCM Cloud and D365 mappings to API-level detail.** Reference REST endpoints, OData entities, key fields. | Krishnamurthy |
| P1-11 | **Illustrate consignment-level attestation linkage.** Formalize `attested_by` edges from `good` nodes. Consider lot/consignment construct for EUDR Article 4(2). | Moreau, Osei |
| P1-12 | **Add attestation lifecycle status.** `status` field: `active`, `revoked`, `superseded`, `withdrawn`. Optional `superseded_by` reference. | Moreau |
| P1-13 | **Add `percentage_band` alternative on `beneficial_ownership` edges.** Enum: `below_25`, `25_to_50`, `50_to_75`, `above_75`. | Moreau |
| P1-14 | **Add downstream supply chain edge guidance for CSDDD.** Guidance or `sells_to` edge type for CSDDD Article 8(2)(b) downstream due diligence. | Moreau |
| P1-15 | **Add sole-proprietorship GDPR guidance for `nat-reg` sensitivity.** Require producers to assess personal data status; recommend `restricted` for sole proprietors. | Tanaka |
| P1-16 | **Document boundary reference stability trade-off.** State fresh salt is default; document salt reuse option for cross-file correlation with anti-enumeration warnings. | Tanaka |
| P1-17 | **Add opaque identifier format validation.** L1 rule requiring 64-character lowercase hex string for `opaque` scheme values. | Tanaka |
| P1-18 | **Add joint venture representation guidance.** `governance_structure` property on `organization` nodes or dedicated edge type. | Engström |
| P1-19 | **Define confidence hierarchy for identifier scheme matches.** Document that LEI=authoritative, DUNS=high-confidence, nat-reg=high within jurisdiction, etc. | Engström |
| P1-20 | **Define structured merge conflict records.** `conflicts` array with `property`, `values` (with provenance), `resolution`. | Osei |
| P1-21 | **Define recommended quantitative properties for supply edges.** `volume`, `annual_value`, `lead_time_days`, `criticality`. Optional, not required for validity. | Osei |
| P1-22 | **Publish CONTRIBUTING.md and adopt DCO.** Define contribution process for spec and code. | Okafor |
| P1-23 | **Publish conformance test suite seed.** 30-40 .omts fragments covering L1 rules with expected outcomes. | Okafor |
| P1-24 | **Create minimum viable file quick-start guide.** One-page guide for non-technical producers. | Okafor, Lindgren |
| P1-25 | **Add recommended enrichment priority path.** Cost-ordered: free registries → GLEIF bulk → ERP VAT → DUNS (if subscription exists). | Lindgren |
| P1-26 | **Add worked multi-ERP deduplication example.** Show same supplier across SAP/Oracle/D365 with three `internal` identifiers. | Lindgren |
| P1-27 | **Upgrade ISO 6523 relationship from "informed by" to "aligns with."** | Nakamura |
| P1-28 | **Clarify or correct UNTDID 3055 reference.** | Nakamura |

### P2 — Future

| # | Recommendation | Originator(s) |
|---|---------------|---------------|
| P2-1 | Introduce batch/lot-level support for `good` nodes (EUDR Article 9) | Osei, Moreau |
| P2-2 | Add `mapping_completeness` metadata to file headers | Osei |
| P2-3 | Add `regulatory_jurisdiction` field to attestation nodes | Moreau |
| P2-4 | Add sanctions/restricted-party list screening guidance | Moreau |
| P2-5 | Introduce `trust_domain` field in merge provenance | Tanaka |
| P2-6 | Define file integrity extension point (detached signatures) | Tanaka |
| P2-7 | Add transport security guidance | Tanaka |
| P2-8 | Add `boundary_ref` to Section 5 node type taxonomy | Varga |
| P2-9 | Document n-ary relationship decomposition pattern | Varga |
| P2-10 | Define property comparison semantics for merge (JSON value equality) | Varga |
| P2-11 | Add `scheme_vocabulary_version` field to file header | Nakamura |
| P2-12 | State GS1 EPC URI conversion requires Company Prefix data | Nakamura |
| P2-13 | Publish EDI coexistence guidance | Krishnamurthy |
| P2-14 | Formalize `authority` naming convention for `internal` scheme | Krishnamurthy, Lindgren |
| P2-15 | Define extension point guidance for procurement-specific edge properties | Lindgren |
| P2-16 | Acknowledge cost barriers to identifier enrichment | Lindgren |
| P2-17 | Add supplier data collection guidance | Lindgren |
| P2-18 | Establish community extension scheme registry | Okafor |
| P2-19 | Define governance lifecycle for non-scheme vocabularies | Okafor |
| P2-20 | Reference OpenCorporates as enrichment source | Engström |
| P2-21 | Document DUNS reassignment risk | Engström |
| P2-22 | Add state-owned enterprise modeling guidance | Moreau |
| P2-23 | Acknowledge successor liability gap in `former_identity` | Moreau |

---

## Cross-Domain Interactions

These are points where one expert's recommendations directly affect another's domain — often the most valuable insights from a multi-expert review.

### 1. Merge Algebra × Entity Resolution: Temporal Identity Predicate
**Varga + Engström** — The transitive closure requirement (Section 9.3) that Varga advocated creates a specific false-merge risk that Engström identified: identifier reassignment (common with DUNS) can transitively link unrelated entities across files. Both recommend extending the identity predicate with temporal overlap checking. They offer to co-author a formal "temporally compatible identity predicate" definition.

### 2. BOM Decomposition × Consignment Attestation: Material Traceability
**Krishnamurthy + Moreau + Osei** — Three experts converge on the same gap from different angles. Krishnamurthy needs `composed_of` edges for ERP BOM structures. Moreau needs consignment-level attestation for EUDR. Osei needs lot-level `good` nodes for disruption analysis. Together these define the full material traceability chain: raw material (lot) → components (BOM) → finished product, with attestations at each level.

### 3. Confidence Field × Merge Quality × Regulatory Reporting
**Osei + Engström + Moreau** — The confidence/verification field recommended by all three serves different purposes: Osei needs it for risk-weighted analysis, Engström for merge quality assessment, Moreau for regulatory evidence strength. A unified confidence metadata model applicable to identifiers, edges, and nodes would satisfy all three.

### 4. Governance Scope × Merge Stability
**Okafor + Varga** — Okafor notes governance authority should extend beyond the scheme registry to cover merge semantics. Varga reinforces this: if a future spec version changes the edge identity predicate or transitive closure behavior, previously merged datasets become inconsistent. The TSC charter should treat Section 9 as a stability-critical component requiring major version increments for changes.

### 5. Privacy × Graph Topology Inference
**Tanaka + Moreau** — Tanaka and Moreau both identify that ownership edge chains in public-scope files can reveal UBO-adjacent information even when `person` nodes are stripped. Chains terminating at high-percentage ownership nodes strongly imply redacted natural persons. This is an inherent limitation of graph-based selective disclosure that should be documented.

### 6. Boundary References × Mergeability Trade-off
**Tanaka + Engström** — Tanaka's anti-enumeration design (fresh salt per file) makes boundary references un-correlatable across files. Engström confirms this means redacted subgraphs are inherently un-mergeable for entities lacking public identifiers. Both agree this is by design but should be explicitly documented.

### 7. ERP Delta Updates × Security Sensitivity
**Krishnamurthy + Tanaka** — Krishnamurthy's delta/patch recommendation raises a security concern flagged by Tanaka: delta operations reveal which suppliers were recently added or removed, which is more sensitive than a static snapshot. Any delta specification must inherit `disclosure_scope` constraints.

### 8. Downstream Supply Chains × ERP Sales Modules
**Moreau + Krishnamurthy** — CSDDD's downstream due diligence obligation means the spec will eventually need mappings from ERP sales/distribution modules (SAP SD, Oracle Order Management), not just procurement modules. Currently the spec is upstream-only.

### 9. SAP Tax Fields × Identifier Scheme Disambiguation
**Nakamura + Krishnamurthy** — SAP's `STCD1`/`STCD2` fields store various tax identifiers (VAT, EIN, CNPJ), not exclusively VAT numbers. The ERP mapping should note that scheme assignment (`vat` vs `nat-reg`) depends on the identifier type, not the field name. The `BUT0ID` table in the Business Partner model handles this more cleanly via typed `ID_TYPE` keys.

### 10. Adoption Strategy × Reference Implementation
**Lindgren + Okafor** — Lindgren identifies that the single most impactful ecosystem deliverable would be a reference SAP S/4HANA extractor producing valid `.omts` files. Okafor's adoption flywheel depends on concrete first-mover tooling. Both recommend prioritizing this in the open source roadmap.

---

## Individual Expert Reports

### Patricia Engström — Entity Identification & Corporate Hierarchy Specialist

**Overall Verdict:** P0 remediation solid. DUNS branch/HQ disambiguation and LEI lifecycle handling are operationally accurate. No new Critical issues.

**Key Concerns:**
- **[Major]** No guidance for conflicting DUNS-to-LEI mappings during transitive closure merge — identifier reassignment can link unrelated entities
- **[Major]** Joint ventures unmodeled — 50/50 ownership entities appear under two parent trees with no shared-governance indicator
- **[Moderate]** No confidence scoring on cross-scheme identity assertions
- **[Moderate]** No temporal overlap requirement in identity predicate

**Top Recommendations:** P1: temporal overlap in identity predicate, joint venture representation, confidence hierarchy for scheme matches. P2: OpenCorporates enrichment reference, DUNS reassignment documentation.

---

### Dr. Kenji Nakamura — ISO & Standards Expert

**Overall Verdict:** Standards alignment is credible. Check digit promotion to L1, canonical format, and governance process are well-implemented. Ready for formal review period.

**Key Concerns:**
- **[Major]** No normative ISO 6523 ICD mapping table — blocks PEPPOL/EN 16931 interoperability
- **[Major]** No conformance clauses for Producer/Consumer/Validator roles
- **[Major]** ISO 6523 relationship understated as "informed by" vs "aligns with"
- **[Minor]** Scheme vocabulary lacks versioning semantics
- **[Minor]** GS1 EPC URI conversion incomplete; UNTDID 3055 reference incorrect

**Top Recommendations:** P1: ISO 6523 ICD mapping table, conformance clauses, TSC co-charter with Okafor, upgrade ISO 6523 relationship language.

---

### Prof. Elena Varga — Graph Theory & Data Modeling Expert

**Overall Verdict:** Merge algebra is formally correct (commutativity, associativity, idempotency). Union-find recommendation is optimal. Multigraph model with independent edge identity is essential and well-designed.

**Key Concerns:**
- **[Major]** Edge merge fallback on "property equality" underspecified (Open Question #3)
- **[Major]** `same_as` edge not formally specified — absent from taxonomy, transitivity unresolved
- **[Medium]** No canonical ordering for merged identifier arrays — weakens idempotency
- **[Medium]** `boundary_ref` not in Section 5 taxonomy
- **[Medium]** N-ary relationships undocumented

**Top Recommendations:** P1: resolve edge merge identity, formally specify `same_as` (transitive, in union-find), mandate identifier array ordering. P2: add `boundary_ref` to taxonomy, document n-ary decomposition pattern.

---

### Rajesh Krishnamurthy — Enterprise Systems Architect

**Overall Verdict:** P0 remediation solid. SAP field mappings are domain-accurate. Spec is implementable for pilot SAP integration.

**Key Concerns:**
- **[Major]** Oracle SCM Cloud and D365 mappings remain too shallow — field names without API endpoints
- **[Major]** No BOM/`composed_of` edge type — cannot model material composition
- **[Major]** No delta/patch pattern — full re-export is infeasible for large vendor masters
- **[Major]** SAP Business Partner model (`BUT000`/`BUT0ID`) not mapped — incomplete for greenfield S/4HANA

**Top Recommendations:** P1: API-level Oracle/D365 mappings, `composed_of` edge type, delta/patch envelope, SAP BP model mapping. P2: EDI coexistence note, `authority` naming convention.

---

### Dr. Isabelle Moreau — Regulatory Compliance Expert

**Overall Verdict:** Beneficial ownership and attestation models are well-designed. GDPR/AMLD privacy tension handled correctly. Regulatory alignment table is credible.

**Key Concerns:**
- **[Major]** Attestation lacks consignment-level linkage — EUDR Article 4(2) requires product-to-plot traceability
- **[Major]** No attestation revocation/supersession model — revoked SA8000 differs from expired
- **[Major]** CSDDD downstream "chain of activities" not modeled — edge taxonomy is upstream-only
- **[Moderate]** `beneficial_ownership` percentage optional with no band alternative
- **[Moderate]** No `regulatory_jurisdiction` on attestation nodes

**Top Recommendations:** P1: consignment-level attestation, attestation lifecycle status, percentage band, downstream edge guidance. P2: regulatory jurisdiction field, sanctions screening, SOE guidance.

---

### Dr. Yuki Tanaka — Security & Privacy Expert

**Overall Verdict:** Significant improvement. Boundary reference hashing is cryptographically sound. Disclosure scope enforcement at L1 is correct. Person node privacy design is layered and GDPR-aware.

**Key Concerns:**
- **[Major]** `nat-reg` default `public` sensitivity incorrect for sole proprietorships (GDPR Article 4(1))
- **[Major]** Boundary reference stability across re-exports unaddressed
- **[Moderate]** Disclosure scope lacks cryptographic binding
- **[Moderate]** Merge provenance lacks trust domain attribution

**Top Recommendations:** P1: sole-proprietorship GDPR guidance for `nat-reg`, boundary reference stability documentation, opaque identifier format validation. P2: trust domain in merge provenance, file integrity extension, transport security guidance.

---

### Dr. Amara Osei — Supply Chain Visibility & Risk Analyst

**Overall Verdict:** All P0 findings resolved. Supply edge taxonomy with regulatory annotations is the strongest addition in R2. No new Critical issues.

**Key Concerns:**
- **[Major]** No confidence/verification field on identifiers — cannot distinguish verified vs self-reported
- **[Major]** No quantitative properties on supply edges — blocks disruption modeling
- **[Major]** Merge conflict record structure undefined — disagreements not machine-processable
- **[Minor]** `tolls` edge direction may confuse bidirectional flows
- **[Minor]** `good` node lacks batch/lot granularity for EUDR

**Top Recommendations:** P1: confidence field, structured conflict records, quantitative supply edge properties. P2: batch/lot support, mapping completeness metadata.

---

### Marcus Lindgren — Chief Procurement Officer

**Overall Verdict:** Internal identifiers as first-class and tiered validation are the right design for procurement adoption. ERP mappings are actionable. The spec is excellent as a data model but not yet a practical implementation guide.

**Key Concerns:**
- **[Major]** No supplier-facing data collection workflow guidance
- **[Major]** `same_as` edge referenced but not formally specified
- **[Major]** No procurement-specific relationship context (approved/blocked, contract terms)
- **[Minor]** Cost barriers to identifier enrichment unacknowledged
- **[Minor]** Multi-ERP deduplication under-specified

**Top Recommendations:** P1: formalize `same_as`, enrichment priority path, multi-ERP dedup example. P2: procurement extension points, cost barrier acknowledgment, supplier data collection guide.

---

### Danielle Okafor — Open Source Strategy & Governance Lead

**Overall Verdict:** Licensing clarity and governance scaffolding are materially better than R1. The spec has moved from "no governance" to "credible scaffolding." The scaffolding now needs to become load-bearing.

**Key Concerns:**
- **[High]** TSC is referenced but undefined — governance is unexecutable
- **[High]** No contributor process (CONTRIBUTING.md, DCO)
- **[High]** `same_as` edge informally introduced — sets governance precedent
- **[Medium]** No conformance test suite plan
- **[Medium]** Adoption complexity for small suppliers — no quick-start guide

**Top Recommendations:** P0: formalize `same_as`, draft TSC charter. P1: CONTRIBUTING.md + DCO, conformance test suite, quick-start guide. P2: extension scheme registry, governance lifecycle for non-scheme vocabularies.
