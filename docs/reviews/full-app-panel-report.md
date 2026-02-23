# Expert Panel Report: Full Application Review

**Date:** 2026-02-23
**Scope:** Complete OMTSF project — 7 specification documents, Rust reference implementation (5 crates, ~52K lines), JSON Schema, Excel templates, governance, and project architecture

---

## Panel Chair Summary

This panel convened all eleven domain experts to review the complete OMTSF project at a point of significant maturity: seven specification documents (four normative, two informative, one serialization binding), a production-quality Rust implementation with 1,035+ tests and benchmarks scaling to 2.2M elements, Excel import/export templates, and governance infrastructure. The project has advanced substantially from earlier reviews — the reference implementation is no longer a roadmap item but a working system with 13 CLI commands, three serialization formats (JSON, CBOR, zstd-compressed), and a validated graph engine.

The panel reaches broad consensus on four points. First, OMTSF addresses a genuine gap: no open, vendor-neutral file format exists for exchanging supply chain graph data across organizational boundaries, and regulatory pressure (CSDDD, EUDR, LkSG, CBAM) makes this gap urgent. Second, the core design decisions are sound — the composite identifier model, the flat adjacency list serialization, the three-level validation architecture, and the commutative/associative/idempotent merge algebra are all well-grounded in their respective domains. Third, the Rust implementation demonstrates disciplined engineering: workspace-wide safety lints, newtype-driven domain modeling, comprehensive test coverage, and benchmarked performance. Fourth, the project is ready for external adoption pilots but not yet for production regulatory use, primarily due to gaps in attestation integrity, merge safety thresholds, and WASM bindings.

The panel identifies areas of disagreement primarily around severity: whether the empty WASM crate is a Critical blocker (Systems Engineering Expert) or a Major gap that can be addressed post-pilot (Open Source Strategy Expert), and whether the `tier` property should be a graph-computed value (Supply Chain Expert, Graph Modeling Expert) or remain a producer-supplied field with merge conflict handling (Entity Identification Expert). The regulatory experts converge on the need for cryptographically bound attestations before EUDR/CSDDD compliance use, while the Data Format Expert notes the underlying content hash specification is itself ambiguous.

---

## Panel Composition

| Panelist | Role | Key Focus Area |
|----------|------|----------------|
| Supply Chain Expert | Supply Chain Visibility & Risk Analyst | Multi-tier mapping, disruption analysis, regulatory alignment |
| Procurement Expert | Chief Procurement Officer | Operational usability, ERP integration, supplier burden |
| Standards Expert | Standards Development & Interoperability Specialist | ISO/GS1/UN alignment, specification rigor, governance |
| Systems Engineering Expert | Senior Systems Engineer (Rust) | Implementation quality, WASM, performance, safety |
| Graph Modeling Expert | Graph Data Modeling & Algorithm Specialist | Data model expressiveness, merge semantics, algorithms |
| Enterprise Integration Expert | Enterprise Systems Architect | ERP export feasibility, master data, EDI coexistence |
| Regulatory Compliance Expert | Supply Chain Regulatory Compliance Advisor | CSDDD, EUDR, UFLPA, CBAM, audit trails |
| Data Format Expert | Data Format Architect | Serialization, schema evolution, integrity, compression |
| Open Source Strategy Expert | Open Source Strategy & Governance Lead | Governance, licensing, adoption, ecosystem |
| Security & Privacy Expert | Data Security & Privacy Architect | Selective disclosure, integrity, local processing |
| Entity Identification Expert | Entity Identification & Corporate Hierarchy Specialist | Entity resolution, corporate structures, identifier coverage |

---

## Consensus Findings

**1. The composite identifier model is the correct design.** All experts who reviewed entity identification (Entity ID Expert, Standards Expert, Enterprise Integration Expert, Procurement Expert) independently validate the decision to support multiple identifier schemes without mandating any single one. The Entity ID Expert confirms from 17 years at D&B that no single identifier covers the global commercial universe, and the Standards Expert notes the ISO 6523 ICD alignment is correct.

**2. The merge algebra is formally sound.** The Graph Modeling Expert and Entity ID Expert both confirm the commutativity, associativity, and idempotency properties are correctly specified and implemented. The union-find with deterministic tie-breaking satisfies the mathematical requirements.

**3. The "data stays local" architecture is credible.** The Security & Privacy Expert, Systems Engineering Expert, and Procurement Expert all affirm that the CLI/WASM architecture genuinely avoids remote data transmission. The `omtsf-core` crate's compiler-enforced I/O prohibition provides structural enforcement.

**4. The regulatory alignment is well-timed but incomplete.** The Regulatory Compliance Expert, Supply Chain Expert, and Standards Expert converge on the assessment that OMTSF maps to current regulatory requirements (CSDDD, EUDR, LkSG, CBAM, AMLD) but lacks the cryptographic attestation binding needed for regulatory-grade evidence.

**5. The Excel templates are the highest-leverage adoption bridge.** The Procurement Expert, Open Source Strategy Expert, and Enterprise Integration Expert all identify Excel import/export as the feature most likely to drive initial adoption among non-technical users.

**6. Merge safety limits are dangerously underspecified.** The Entity ID Expert and Graph Modeling Expert independently flag that the "unexpectedly large merge group" warning in SPEC-003 Section 4.1 lacks a concrete threshold, creating divergent implementations and latent false-positive merge risk.

---

## Critical Issues

### C1. WASM crate is empty — no browser/JS API surface
**Flagged by:** Systems Engineering Expert
**Detail:** `omtsf-wasm/src/lib.rs` is zero bytes. No `wasm-bindgen` exports exist. The `just wasm-check` target only verifies `omtsf-core` compilation, not consumer-facing WASM APIs. Browser and JS integrators have no surface to call.

### C2. `on_path` bitset assumes dense NodeIndex space — correctness bug
**Flagged by:** Graph Modeling Expert
**Detail:** In `dfs_paths`, the on-path vector is allocated as `vec![false; graph.node_count()]` but indexed by `NodeIndex::index()`. If any node is removed from a `StableDiGraph`, `node_count()` decreases while max live index does not, causing out-of-bounds panics. Fix: use `node_bound()` instead of `node_count()`.

### C3. No normative mechanism for third-party attestation binding
**Flagged by:** Regulatory Compliance Expert, Standards Expert
**Detail:** EUDR Article 4(1) requires due diligence statements submitted to TRACES. OMTSF can represent a DDS as an attestation node, but there is no cryptographic binding to the TRACES reference or issuing authority. The W3C VC reference in SPEC-006 is informative only. Without normative signed attestation support, OMTSF files cannot serve as regulatory evidence.

### C4. Merge-group safety limits lack concrete thresholds
**Flagged by:** Entity Identification Expert, Graph Modeling Expert
**Detail:** SPEC-003 Section 4.1 says implementations "SHOULD emit a warning" for unexpectedly large merge groups but provides no numeric guidance. The Entity ID Expert recommends warning at 4+ nodes (excluding same_as), with 10+ almost certainly indicating data quality problems.

### C5. No explicit relationship to UN/CEFACT Transparency Protocol (UNTP)
**Flagged by:** Standards Expert
**Detail:** UNTP Digital Product Passport and UNECE Recommendation No. 49 target the same domain with overlapping vocabulary. If UNTP becomes the regulatory baseline for EUDR/CSDDD machine-readable reporting, OMTSF risks orphan status without a defined mapping.

### C6. DUNS HQ vs. Branch disambiguation is advisory only
**Flagged by:** Entity Identification Expert
**Detail:** Branch DUNS numbers on organization nodes will cause false-positive merges when two organizations share a physical plant. The SHOULD-level guidance is insufficient; branch DUNS on organization nodes needs at minimum an L2 validation warning.

### C7. PO-derived supply edges without temporal bounding create graph noise
**Flagged by:** Enterprise Integration Expert
**Detail:** SPEC-005 derives `supplies` edges from purchase orders without specifying recency filters. A single PO from 2011 produces a current supply relationship. Without guidance on date filtering, exports will include every vendor a company has ever transacted with.

### C8. Regex fallback chains in newtypes.rs silently degrade validation
**Flagged by:** Systems Engineering Expert
**Detail:** Five-deep nested `unwrap_or_else` chains substitute progressively more general regexes on hypothetical regex engine failure, silently changing validation semantics instead of failing loudly. Should use `regex_lite` or compile-time assertion.

---

## Major Issues

### M1. No supplier-facing onboarding workflow or template
**Flagged by:** Procurement Expert
**Detail:** Excel templates assume buyer-populated data. No template exists for supplier self-completion during onboarding, and no mechanism distinguishes buyer-asserted from supplier-asserted data.

### M2. `tier` property conflicts on merge are unspecified
**Flagged by:** Supply Chain Expert, Graph Modeling Expert
**Detail:** Two files with different `reporting_entity` values will produce conflicting `tier` values on the same `supplies` edge after merge. No reconciliation rules are specified.

### M3. No normative mechanism to signal known-but-undisclosed supply tiers
**Flagged by:** Supply Chain Expert
**Detail:** No way to record "tier-1 has sub-suppliers that are not disclosed in this file," preventing consumers from distinguishing genuinely flat supply chains from incompletely mapped ones.

### M4. `content_hash` in file_integrity has no normative definition of what is hashed
**Flagged by:** Data Format Expert
**Detail:** The schema defines `file_integrity.content_hash` (SHA-256) but no spec section defines the byte sequence being hashed. Without this, two implementations may compute different hashes for the same logical file.

### M5. CBOR non-deterministic encoding conflicts with integrity hashing
**Flagged by:** Data Format Expert
**Detail:** SPEC-007 explicitly states deterministic key ordering is not required. Two logically equivalent CBOR files will produce different byte sequences and therefore different content hashes.

### M6. Merge pipeline clones entire node/edge vectors before processing
**Flagged by:** Systems Engineering Expert
**Detail:** `merge_pipeline/pipeline.rs` calls `.clone()` on every node and edge when building working lists. For merging 10 files of 50K nodes each, this is 500K full struct clones with heap-allocated BTreeMaps before any deduplication.

### M7. EUDR DDS annual submission model lacks aggregate coverage semantics
**Flagged by:** Regulatory Compliance Expert
**Detail:** One DDS may cover all consignments of a commodity placed during a year. No mechanism expresses this aggregate relationship between an attestation and a set of consignments.

### M8. GLEIF Level 2 coverage gap not acknowledged
**Flagged by:** Entity Identification Expert
**Detail:** Level 2 "who owns whom" data only covers entities where both parent and child hold LEIs — a minority. The spec does not warn that absence of `legal_parentage` edges does not mean absence of a corporate parent.

### M9. VAT number normalization is undefined
**Flagged by:** Procurement Expert, Entity Identification Expert
**Detail:** Two exporters may produce `DE123456789` vs `123456789` (with authority `DE`) for the same VAT number. The canonical form is unspecified, breaking merge identity.

### M10. `all_paths` DFS is recursive and stack-unbounded
**Flagged by:** Systems Engineering Expert
**Detail:** With user-controlled depth parameters on untrusted input, stack depth is proportional to path length. An iterative implementation with explicit stack is the safe choice.

### M11. D365 cross-company data isolation not addressed
**Flagged by:** Enterprise Integration Expert
**Detail:** D365 `VendorsV2` is company-scoped by default. Multi-entity tenants produce colliding internal identifiers without per-company authority scoping.

### M12. SAP subcontracting edge derivation is incorrect
**Flagged by:** Enterprise Integration Expert
**Detail:** SPEC-005 maps BSART='UB' to subcontracting, but UB is stock transport. Subcontracting uses EKPO-PSTYP='L'.

### M13. Conformance test suite does not exist as a formal artifact
**Flagged by:** Standards Expert, Open Source Strategy Expert
**Detail:** The fixture files exist but are not framed as a formal conformance test suite with expected outcomes. This is a prerequisite for standards body submission and enterprise certification.

### M14. No community infrastructure beyond the repository
**Flagged by:** Open Source Strategy Expert
**Detail:** No public communication channel, no project website, no issue templates, no "good first issue" labeling. Projects relying solely on GitHub issues for community interaction plateau at small contributor counts.

### M15. Producer can downgrade person node identifier sensitivity with no validation gate
**Flagged by:** Security & Privacy Expert
**Detail:** A producer can set `sensitivity: "public"` on a person node identifier, causing it to survive into partner-scope output. No implemented validation rule flags this override.

### M16. No `composed_of` DAG constraint
**Flagged by:** Graph Modeling Expert
**Detail:** Bills of materials are DAGs by physical necessity, but no validation rule prohibits cycles in the `composed_of` subgraph, which would cause infinite loops in BOM explosion algorithms.

### M17. No disruption/substitution data model
**Flagged by:** Supply Chain Expert
**Detail:** No fields for lead time, alternate suppliers, or supply capacity constraints — the properties that separate compliance mapping from risk modeling.

### M18. UFLPA entity list representation is static and unverified
**Flagged by:** Regulatory Compliance Expert
**Detail:** The `uflpa-entity-list` boolean label has no required `data_quality.last_verified` date. A label from January may be stale by March. No mechanism expresses negative screening ("screened and not found").

### M19. CBAM installation identifier not linked to EU Registry
**Flagged by:** Regulatory Compliance Expert
**Detail:** EORI and CBAM installation ID are not named identifier schemes. Producers must use `internal` or `nat-reg`, breaking cross-organization interoperability for CBAM data.

### M20. Unrecognized identifier schemes default to public with no warning
**Flagged by:** Security & Privacy Expert
**Detail:** Extension schemes carrying sensitive data silently pass into public-scope output unless the producer explicitly sets sensitivity. No L2 validation rule warns on this.

---

## Minor Issues

- `tolls` direction convention is counterintuitive relative to all other supply edges (Graph Modeling)
- `composed_of` permits `consignment → good` edges which is semantically odd (Graph Modeling)
- `merge_scalars` uses JSON equality for numeric comparison — `1.0` vs `1` produces false conflicts (Graph Modeling)
- `newtypes.rs` at 888 lines exceeds the project's own 800-line file limit (Systems Engineering)
- `DynValue::Object` uses `BTreeMap` but comments claim "insertion order preservation" — should be `IndexMap` (Systems Engineering)
- `OmtsGraph` uses `HashMap<String, NodeIndex>` requiring `.to_owned()` at every lookup (Systems Engineering)
- No IANA media type registered for `.omts` files (Data Format)
- No `format: "date"` annotation in JSON Schema for calendar validity checking (Data Format)
- `file_salt` persists unchanged in redacted output, enabling cross-file boundary ref correlation with same-salt files (Security & Privacy)
- `disclosure_scope` is optional — files accidentally missing it receive no sensitivity enforcement (Security & Privacy)
- No CSRD/ESRS Scope 3 emissions aggregation pathway (Regulatory Compliance)
- No LkSG §10 complaint/grievance mechanism representation (Regulatory Compliance)
- ISO 5009 compatibility claim in SPEC-006 needs elaboration (Standards)
- ISO 8601 date profile has no datetime guidance (Standards)
- `operational_control` type `tolling` creates ambiguity with `tolls` edge type (Supply Chain)
- No webhook or change-notification pattern guidance (Procurement)
- `BU_SORT1` suggested for fuzzy deduplication is unreliable in practice (Enterprise Integration)
- `verification_status: "inferred"` is undefined in terms of inference method (Entity ID)
- `org.opencorporates` jurisdiction code case normalization is unspecified (Entity ID)
- No code of conduct document (Open Source Strategy)
- No issue or PR templates (Open Source Strategy)

---

## Consolidated Recommendations

### P0 — Immediate (before format stabilization)

| # | Recommendation | Originating Expert(s) |
|---|---------------|----------------------|
| 1 | **Fix `on_path` allocation bug** — use `node_bound()` not `node_count()` in `dfs_paths` | Graph Modeling |
| 2 | **Implement `omtsf-wasm`** with `wasm-bindgen` exports for parse, validate, merge, query | Systems Engineering |
| 3 | **Fix regex initialization in newtypes.rs** — use `regex_lite` or compile-time assertion | Systems Engineering |
| 4 | **Define canonical serialized form for `content_hash`** — specify exact byte sequence hashed | Data Format |
| 5 | **Add CSPRNG requirement for `file_salt`** to SPEC-007 (currently only in SPEC-001/004) | Data Format |
| 6 | **Specify default merge-group size warning threshold** in SPEC-003 (recommend 4+ nodes) | Entity ID, Graph Modeling |
| 7 | **Require temporal fields on DUNS and GLN** — promote L2-EID-07 to MUST for reassignable schemes | Entity ID |
| 8 | **Add normative signed attestation profile** — W3C VC or equivalent for regulatory evidence | Regulatory Compliance |
| 9 | **Define DDS aggregate coverage semantics** for EUDR annual submission model | Regulatory Compliance |
| 10 | **Engage with UN/CEFACT UNTP** — submit liaison statement, draft SPEC-006 mapping | Standards |
| 11 | **Publish conformance test suite** as formal artifact with expected validation outcomes | Standards, Open Source Strategy |
| 12 | **Add supplier-facing onboarding template** with buyer-vs-supplier data source distinction | Procurement |
| 13 | **Define VAT normalization rules** — canonical form for value field, strip or include country prefix | Procurement, Entity ID |
| 14 | **Add L2 validation rule for `sensitivity: "public"` on person node identifiers** | Security & Privacy |
| 15 | **Fix SAP subcontracting mapping** — EKPO-PSTYP='L', not BSART='UB' | Enterprise Integration |
| 16 | **Specify D365 company-scoped authority format** — `d365-{tenant}-{company}` | Enterprise Integration |
| 17 | **Add `visibility_depth` or `opaque_sub_suppliers` signal** for undisclosed supply tiers | Supply Chain |

### P1 — Before v1.0

| # | Recommendation | Originating Expert(s) |
|---|---------------|----------------------|
| 18 | Specify `tier` reconciliation rules for merged files with different `reporting_entity` | Supply Chain, Graph Modeling |
| 19 | Add `derived_from` edge type for consignment-level provenance chains | Supply Chain |
| 20 | Add supply disruption properties: `lead_time_days`, `sole_source` on `supplies` edges | Supply Chain |
| 21 | Extend `tier` as first-class property to all supply relationship edge types | Procurement |
| 22 | Add spend data representation pattern (`spend_ytd`, `payment_terms_days`) | Procurement |
| 23 | Add GQL graph type definition annex to SPEC-001 | Standards, Graph Modeling |
| 24 | Recategorize DUNS or document its deprecation trajectory | Standards |
| 25 | Operationalize W3C VC linkage in SPEC-004/006 | Standards |
| 26 | Refactor merge pipeline to avoid pre-cloning all nodes | Systems Engineering |
| 27 | Convert `all_paths` DFS to iterative with explicit stack | Systems Engineering |
| 28 | Replace `DynValue::Object` BTreeMap with `IndexMap` for insertion-order preservation | Systems Engineering |
| 29 | Add L2 validation rule for `composed_of` DAG constraint | Graph Modeling |
| 30 | Add `is_ultimate` boolean to `legal_parentage` edges (GLEIF Level 2 alignment) | Entity ID |
| 31 | Document GLEIF Level 2 coverage gap as known limitation | Entity ID |
| 32 | Specify EU VAT value field normalization (strip/include country prefix) | Entity ID |
| 33 | Add extraction performance guidance per ERP system | Enterprise Integration |
| 34 | Add temporal bounding guidance for PO-derived edges | Enterprise Integration |
| 35 | Define ERP-specific change detection strategies for delta format | Enterprise Integration |
| 36 | Add EORI and CBAM installation ID as named identifier schemes | Regulatory Compliance |
| 37 | Mandate `data_quality.last_verified` for regulatory scope labels | Regulatory Compliance |
| 38 | Add negative screening label semantics (`uflpa-screened-clear`) | Regulatory Compliance |
| 39 | Consider opt-in CBOR deterministic encoding profile | Data Format |
| 40 | Document JSON Schema 2020-12 minimum validator requirements | Data Format |
| 41 | Establish TSC with external members and publish v1.0 timeline | Open Source Strategy |
| 42 | Add formal conformance sections to all four normative specs | Open Source Strategy |
| 43 | Launch community infrastructure (GitHub Discussions, project website) | Open Source Strategy |
| 44 | Produce one public reference deployment or case study | Open Source Strategy |
| 45 | Emit L2 warning for unrecognized schemes in public-scope files | Security & Privacy |
| 46 | Strengthen SPEC-004 documentation on salt persistence and cross-file correlation | Security & Privacy |
| 47 | Enforce intake integrity verification before redaction in CLI | Security & Privacy |

### P2 — Future iterations

| # | Recommendation | Originating Expert(s) |
|---|---------------|----------------------|
| 48 | Define VC-to-attestation resolution pattern for EUDR DDS | Supply Chain |
| 49 | Publish EUDR compliance profile document | Supply Chain |
| 50 | Add onboarding workflow status representation | Procurement |
| 51 | Add snapshot change detection guidance for procurement workflows | Procurement |
| 52 | Add ISO 5009 alignment table | Standards |
| 53 | Address datetime precision in versioned errata note | Standards |
| 54 | Split `newtypes.rs` per the 800-line file limit | Systems Engineering |
| 55 | Evaluate CSR graph representation for read-only query workloads | Systems Engineering |
| 56 | Reverse `tolls` direction convention or add prominent explanation | Graph Modeling |
| 57 | Add cycle detection guidance for `ownership` traversal | Entity ID |
| 58 | Define `org.opencorporates` jurisdiction normalization rules | Entity ID |
| 59 | Document SAP Ariba Network ID as extension scheme | Enterprise Integration |
| 60 | Add worked end-to-end extraction example | Enterprise Integration |
| 61 | RMAP smelter conformance attestation pattern | Regulatory Compliance |
| 62 | Add Scope 3 upstream emissions aggregation path | Regulatory Compliance |
| 63 | Register IANA media type (`application/omts`) | Data Format |
| 64 | Investigate streaming parse for multi-gigabyte files | Data Format |
| 65 | Evaluate OASIS or Linux Foundation hosting | Open Source Strategy |
| 66 | Publish Python and TypeScript/npm packages | Open Source Strategy |
| 67 | Create community extension scheme registry | Open Source Strategy |
| 68 | Add `unknown_scheme_sensitivity` file header field option | Security & Privacy |
| 69 | Add CCPA 2026 data broker note for person node files | Security & Privacy |

---

## Cross-Domain Interactions

**Identity ↔ Merge ↔ Graph:** The Entity ID Expert's finding that DUNS branch numbers cause false merges directly impacts the Graph Modeling Expert's merge transitive closure and the Supply Chain Expert's multi-tier visibility. A single erroneous branch DUNS match cascades through the entire connected component.

**Attestation ↔ Regulatory ↔ Security:** The Regulatory Compliance Expert's need for cryptographically bound attestations depends on the Data Format Expert's content hash specification and the Security & Privacy Expert's integrity verification model. These three concerns form a single dependency chain that must be resolved together.

**ERP Integration ↔ Entity ID ↔ Merge:** The Enterprise Integration Expert's finding that SAP's STCD1/STCD2 disambiguation is critical connects to the Entity ID Expert's VAT normalization concern and the Procurement Expert's merge interoperability concern. VAT format inconsistency is the most likely cause of failed merges in European supply chain data.

**Tier Property ↔ Reporting Entity ↔ Merge:** The Supply Chain Expert and Graph Modeling Expert both flag that `tier` is perspective-dependent (anchored to `reporting_entity`). After merging files from two reporting entities, `tier` conflicts are structurally expected. The merge semantics should explicitly acknowledge this as a known conflict class, not a data quality failure.

**WASM ↔ Adoption ↔ Ecosystem:** The Systems Engineering Expert's critical finding that the WASM crate is empty directly blocks the Open Source Strategy Expert's npm publishing recommendation and the Procurement Expert's vision of browser-based tooling for non-technical users.

**Excel Templates ↔ Procurement ↔ Adoption:** The Procurement Expert's recommendation for a supplier-facing onboarding template and the Open Source Strategy Expert's identification of Excel as the highest-leverage adoption bridge converge: a supplier-completed Excel template is the single most impactful artifact for driving initial adoption.

**Regulatory Timeline ↔ Format Stability:** CSDDD large-company obligations begin for financial years starting January 2028. EUDR large-operator DDS submission begins December 2026. CBAM definitive phase started January 2026. The format must stabilize its attestation model and regulatory label vocabulary before these deadlines to capture the compliance tooling market.

---

## Individual Expert Reports

### Supply Chain Expert — Supply Chain Visibility & Risk Analyst

#### Assessment

Having spent 18 years trying to answer "who, at tier N, is actually making that part, and how confident are you in that data?" across automotive, electronics, and FMCG, OMTSF is the first open format that treats the supply network as what it is — a directed graph with uncertain, time-varying, multi-source data — rather than as a flat vendor list with columns. The graph model, the merge algebra, and the data quality metadata together form a foundation more coherent than anything currently standardized by GS1, GLEIF, or the ISO supply chain working groups for structural network representation.

The format is well-timed. CSDDD compliance obligations begin in 2028 for the largest companies. EUDR large-operator obligations took effect December 2025. LkSG has been in force since 2023. Every one of these requires companies to know their supply chain beyond tier 1. OMTSF provides the data exchange layer those programs need.

The primary concern is not the model itself but populating it with trustworthy multi-tier data. The spec is well-constructed for a world where tier-2+ suppliers voluntarily share data; the real world involves coercion, commercial confidentiality, and systematic misreporting. The format needs stronger normative guidance on how to signal incomplete visibility.

#### Strengths
- Tier-relative edge property on `supplies` anchored to `reporting_entity` — the field every due diligence system needs
- `share_of_buyer_demand` for concentration analysis — enables single-source dependency detection
- `boundary_ref` as formalized opacity stub preserves graph connectivity without leaking identity
- Data quality metadata (confidence, source, last_verified) on every element
- Attestation model covers EUDR and CSDDD surface area with risk_severity/risk_likelihood
- Regulatory label vocabulary (Appendix B) is immediately actionable
- Merge algebra enables federated multi-party mapping
- `former_identity` edge traces risk through corporate history
- `facility` geo with GeoJSON polygon support for EUDR plot-level requirements

#### Concerns
- **[Major]** No normative mechanism to signal known-but-undisclosed tiers
- **[Major]** `tier` is producer-supplied, not graph-computed — conflicting values on merge are unspecified
- **[Major]** No disruption/substitution data model (lead time, alternate supplier, capacity)
- **[Minor]** EUDR DDS linkage is underspecified for consignment propagation
- **[Minor]** `consignment` lacks batch provenance chain linkage
- **[Minor]** `operational_control` tolling vs. `tolls` edge ambiguity
- **[Minor]** No vocabulary for supplier corrective action plans (CSDDD Article 10)

#### Recommendations
1. (P0) Add `visibility_depth` or `opaque_sub_suppliers` signal
2. (P0) Add `opaque_sub_suppliers` boolean on supply edges
3. (P1) Specify tier reconciliation rules for merged files
4. (P1) Add `derived_from` edge for consignment provenance
5. (P1) Add disruption properties: `lead_time_days`, `sole_source`
6. (P2) Define VC-to-attestation resolution pattern
7. (P2) Publish EUDR compliance profile document
8. (P2) Consider `risk_profile` extension for country-level risk context

---

### Procurement Expert — Chief Procurement Officer

#### Assessment

Managing 4,000+ direct suppliers across SAP, Coupa, and Jaggaer, the problem OMTSF targets — fragmented supplier data with no neutral exchange format — is one I live daily. The Excel interface is the feature I would lead with in any internal pitch. The supplier list template covers every field category managers actually care about. The deduplication logic solves the multi-purchasing-org problem. The CLI is clean enough for procurement analysts without training.

The ERP integration guide demonstrates genuine SAP implementation experience. However, most organizations will start with internal-only files that cannot cross-file merge, and the enrichment path requires third-party data services that many mid-market suppliers will not have.

#### Strengths
- Excel supplier list template with procurement-native fields
- Multi-BU deduplication via supplier_id
- Internal identifiers are first-class — no onboarding barrier
- SAP mapping depth including BUT000/BUT0ID
- Tiered validation enables incremental adoption
- Non-auto-resolving merge conflicts
- `reporting_entity` and org-scoped label keys
- Diversity classification and regulatory labels

#### Concerns
- **[Critical]** No supplier-facing onboarding workflow or template
- **[Critical]** No contract lifecycle or onboarding status fields
- **[Major]** Merge requires shared external identifiers most exports will lack
- **[Major]** No spend data representation
- **[Major]** `tier` only on `supplies` edges, not all relationship types
- **[Minor]** No change-notification pattern
- **[Minor]** VAT normalization undefined
- **[Minor]** No PO reference portability guidance

#### Recommendations
1. (P0) Add supplier-completed onboarding template
2. (P0) Define VAT normalization rules
3. (P1) Publish recommended label vocabulary appendix
4. (P1) Extend `tier` to all supply relationship edge types
5. (P1) Add spend data representation pattern
6. (P2) Add onboarding workflow status
7. (P2) Add snapshot change detection guidance

---

### Standards Expert — Standards Development & Interoperability Specialist

#### Assessment

OMTSF is one of the more technically coherent pre-standardization supply chain data formats at this maturity stage. The composite identifier model directly addresses the persistent failure of assuming one authoritative identifier exists. The ISO 6523 ICD mapping table is the right cross-walk for PEPPOL/EDI ecosystems. The GQL alignment positions OMTSF for query tooling interoperability.

The principal concern is that the specification has not engaged with UN/CEFACT UNTP, which is producing Digital Product Passport vocabulary with substantial overlap. Without explicit positioning, there is risk of divergence at the moment regulators are choosing reference models.

#### Strengths
- ISO 6523 alignment is correct and complete
- GLEIF integration is authoritative with quarterly snapshot discipline
- GQL conceptual alignment with ISO/IEC 39075:2024
- GLEIF Level 2 extension is semantically grounded
- LEI lifecycle handling is precisely calibrated
- UNTDID 3055 cross-walk noted for EDIFACT bridge
- Three-level validation mirrors ISO standards practice

#### Concerns
- **[Critical]** No explicit relationship to UN/CEFACT UNTP
- **[Major]** DUNS is proprietary with no ISO anchor — being replaced by SAM.gov UEI
- **[Major]** ISO 5009 compatibility claim needs elaboration
- **[Major]** Conformance profiles defined but certification path absent
- **[Minor]** GQL alignment is conceptual, not operational
- **[Minor]** Date profile has no datetime guidance
- **[Minor]** `file_salt` key management ambiguity

#### Recommendations
1. (P0) Engage with UN/CEFACT UNTP working group
2. (P0) Publish conformance test suite
3. (P1) Define GQL graph type definition annex
4. (P1) Recategorize DUNS or add deprecation trajectory
5. (P1) Operationalize W3C VC linkage
6. (P2) Add ISO 5009 alignment table
7. (P2) Address datetime precision

---

### Systems Engineering Expert — Senior Systems Engineer (Rust)

#### Assessment

The implementation is well-architected with disciplined, idiomatic Rust throughout. Workspace-wide lint enforcement is the strictest I have seen in a production Rust library. The `DynValue` custom type for format-neutral serialization is a genuinely good design decision. The split-weight graph design is explicitly cache-aware.

One structural concern stands out: the `omtsf-wasm` crate is effectively empty. Everything else is production-quality, but shipping with a dead WASM entry point means browser/JS consumers have no API surface today.

#### Strengths
- Workspace lint configuration is exemplary
- `DynValue` avoids JSON number collapse in multi-format pipelines
- `NodeWeight`/`EdgeWeight` cache-friendly indirection design
- `UnionFind` with deterministic tie-breaking ensures merge commutativity
- Buffer reuse in BFS hot loops
- Kahn's algorithm for cycle detection avoids recursion depth issues
- cbor4ii selection justified by benchmarks
- Decompression bomb protection explicitly handled
- 1,035+ tests including property-based testing

#### Concerns
- **[Critical]** WASM crate is empty — no browser/JS API
- **[Major]** Regex fallback chains silently degrade validation semantics
- **[Major]** Merge pipeline clones entire node/edge vecs before processing
- **[Major]** `all_paths` DFS is recursive and stack-unbounded
- **[Minor]** `HashMap<String, NodeIndex>` forces `.to_owned()` at every lookup
- **[Minor]** `newtypes.rs` at 888 lines exceeds 800-line limit
- **[Minor]** `DynValue::Object` BTreeMap claims insertion-order preservation

#### Recommendations
1. (P0) Implement `omtsf-wasm` with `wasm-bindgen` exports
2. (P0) Fix regex initialization with `regex_lite`
3. (P1) Refactor merge pipeline to avoid pre-cloning
4. (P1) Convert `all_paths` DFS to iterative
5. (P1) Replace BTreeMap with IndexMap in DynValue
6. (P2) Split newtypes.rs
7. (P2) Evaluate CSR representation for read-only queries

---

### Graph Modeling Expert — Graph Data Modeling & Algorithm Specialist

#### Assessment

OMTSF is built on a formally sound graph-theoretic foundation. The directed labeled property multigraph is the correct primitive. The merge semantics demonstrate genuine theoretical rigor — identity predicates over algebraic identifier tuples, transitive closure via union-find with inverse-Ackermann complexity, deterministic tie-breaking for commutativity.

The implementation makes competent engineering choices. The split-weight design and hand-rolled BFS/DFS with edge-type filtering are sound. However, the `on_path` bitset has a correctness bug with `StableDiGraph` after node removals.

#### Strengths
- Directed labeled property multigraph is theoretically correct and necessary
- Type constraint table (Section 9.5) is genuine schema-level safety
- Forest constraint on `legal_parentage` enforced by Kahn's topological sort
- Union-find with path-halving and union-by-rank achieves optimal complexity
- `same_as` with confidence levels aligns with entity resolution literature
- Two-level identity (graph-local vs external) is the correct architecture
- `boundary_ref` enables privacy-preserving subgraph projection
- Cycle allowance in supply subgraph while prohibiting in parentage is accurate

#### Concerns
- **[Critical]** `on_path` bitset assumes dense NodeIndex space — panic after removals
- **[Major]** No `composed_of` DAG constraint — cycles cause infinite BOM explosion
- **[Major]** `operates`/`produces` edge merge undefined for parallel edges
- **[Minor]** `tolls` direction counterintuitive vs other supply edges
- **[Minor]** `composed_of` permits `consignment → good` which is semantically odd
- **[Minor]** `merge_scalars` JSON equality produces false conflicts on `1.0` vs `1`
- **[Minor]** GQL alignment not explicitly addressed

#### Recommendations
1. (P0) Fix `on_path` allocation — use `node_bound()` not `node_count()`
2. (P0) Add L2 validation rule for `composed_of` DAG constraint
3. (P1) Add normative guidance on parallel edge identifiers
4. (P1) Fix `merge_scalars` numeric comparison
5. (P2) Reverse `tolls` direction or add prominent explanation
6. (P2) Add GQL compatibility section to SPEC-001
7. (P2) Define default merge-group size advisory threshold

---

### Enterprise Integration Expert — Enterprise Systems Architect

#### Assessment

SPEC-005 demonstrates genuine familiarity with ERP data structures. The dual-track SAP mapping (LFA1 + BUT000/BUT0ID) is correct. The tax number disambiguation warning will save implementers real pain. The authority naming convention is exactly right for multi-system deduplication.

The Oracle and D365 mappings read as desk research rather than implementation experience — they omit pagination behavior, N+1 query patterns, and cross-company isolation that are the first problems every integrator hits.

#### Strengths
- BUT000/BUT0ID mapping correctly reflects S/4HANA Business Partner model
- Tax number disambiguation warning for STCD1/STCD2 is necessary
- Authority naming convention solves multi-client deduplication
- Identifier enrichment lifecycle matches real MDM maturity curves
- `internal` exclusion from merge prevents false-positive golden record poisoning
- EDI coexistence framing is correct separation of concerns
- Delta model essential for production use
- Reverse-domain label key convention anticipates namespace collision

#### Concerns
- **[Critical]** No extraction performance guidance at enterprise scale
- **[Critical]** PO-derived edges without temporal bounding create noise
- **[Major]** VAT authority normalization underspecified
- **[Major]** D365 cross-company data isolation not addressed
- **[Major]** Oracle N+1 site extraction is a known performance trap
- **[Major]** No ERP-specific change detection guidance for delta format
- **[Minor]** Subcontracting via BSART='UB' is incorrect (should be EKPO-PSTYP='L')
- **[Minor]** No SAP Ariba Network ID as supplementary identifier
- **[Minor]** BU_SORT1 for fuzzy dedup is unreliable

#### Recommendations
1. (P0) Add extraction volume and performance guidance per ERP
2. (P0) Fix subcontracting edge derivation (PSTYP='L')
3. (P0) Specify D365 company-scoped authority format
4. (P1) Add temporal bounding for PO-derived edges
5. (P1) Define ERP-specific change detection strategies
6. (P1) Add VAT compound-identifier resolution guidance
7. (P2) Document SAP Ariba Network ID as extension scheme
8. (P2) Add worked end-to-end extraction example

---

### Regulatory Compliance Expert — Supply Chain Regulatory Compliance Advisor

#### Assessment

OMTSF is the most structurally coherent open format for regulatory compliance data I have encountered. The `attestation` nodes as first-class graph citizens, the `consignment` emissions fields matching CBAM Annex III, and the AMLD UBO representation are all legally literate. CSDDD's Omnibus delay to 2028 gives OMTSF more runway. CBAM entered its definitive phase January 2026 with the emissions model already aligned.

The primary concern is the evidentiary layer. A format is only as useful as the assurance that data has not been falsified. Without cryptographic attestation binding, an OMTSF file cannot serve as regulatory evidence in a customs investigation.

#### Strengths
- CSDDD value chain mapping is excellent with tier + reporting_entity + corporate hierarchy
- EUDR geolocation is production-ready (points + GeoJSON polygons)
- LkSG tiering maps directly via labels and tier property
- CBAM emissions structure correctly reflects Implementing Regulation hierarchy
- AMLD UBO representation defers threshold to tooling — correct design
- Selective disclosure enables proportionate regulatory sharing
- Temporal validity on all relationships supports point-in-time proof

#### Concerns
- **[Critical]** No normative mechanism for third-party attestation binding
- **[Critical]** EUDR DDS annual submission model lacks aggregate coverage semantics
- **[Major]** UFLPA entity list representation is static and unverified
- **[Major]** CBAM installation identifier not linked to EU Registry (EORI missing)
- **[Major]** Conflict minerals smelter label has no RMAP linkage
- **[Minor]** No CSRD/ESRS Scope 3 alignment
- **[Minor]** No LkSG §10 grievance mechanism representation

#### Recommendations
1. (P0) Normative signed attestation profile (W3C VC or equivalent)
2. (P0) DDS aggregate coverage semantics
3. (P1) Add EORI and CBAM installation ID as named schemes
4. (P1) Mandate `data_quality.last_verified` for regulatory labels
5. (P1) Add negative screening label semantics
6. (P2) RMAP smelter conformance pattern
7. (P2) Scope 3 upstream emissions aggregation path

---

### Data Format Expert — Data Format Architect

#### Assessment

The serialization design is coherent with genuine awareness of multi-encoding tradeoffs. JSON+CBOR is the right pair. The encoding detection hierarchy is textbook-clean. The `DynValue` type avoiding JSON number collapse is architecturally significant. The compression story (zstd only, feature-gated, bomb-protected) is clean.

The critical gap is that `content_hash` has no normative definition of what is hashed, and CBOR non-deterministic encoding conflicts with any byte-level integrity scheme.

#### Strengths
- Two-encoding strategy with clean separation
- Encoding detection is robust
- `DynValue` preserves numeric type fidelity across formats
- Self-describing files (version, tag, magic bytes)
- Decompression bomb protection
- Hash operations are encoding-independent
- Forward compatibility via unknown field preservation
- JSON Schema draft 2020-12 is correct LTS choice
- CBOR adoption trajectory aligns with IETF supply chain integrity work

#### Concerns
- **[Critical]** No CBOR Deterministic Encoding — conflicts with content_hash
- **[Major]** `content_hash` hashing target is undefined
- **[Major]** zstd compression level unspecified — affects byte-level reproducibility
- **[Major]** JSON Schema 2020-12 tooling maturity uneven across languages
- **[Minor]** CBOR tag 55799 stripped on round-trip conversion
- **[Minor]** `file_salt` CSPRNG requirement missing from SPEC-007
- **[Minor]** No IANA media type registered

#### Recommendations
1. (P0) Define canonical serialized form for `content_hash`
2. (P0) Add CSPRNG requirement for `file_salt` in SPEC-007
3. (P1) Offer opt-in CBOR deterministic encoding profile
4. (P1) Document JSON Schema minimum validator requirements
5. (P1) Investigate streaming parse for large files
6. (P2) Register IANA media type
7. (P2) Add `format: "date"` annotation in JSON Schema

---

### Open Source Strategy Expert — Open Source Strategy & Governance Lead

#### Assessment

The transformation from earlier reviews is impressive. Every prior critical finding has been closed: CONTRIBUTING.md with DCO, clean licensing split (CC-BY-4.0 specs, Apache-2.0 code), reference implementation with 1,622 test functions, and conformance fixtures. This is a working open source project with all foundational infrastructure for community formation.

The adoption flywheel has moving parts but has not been set in motion. What remains is the transition from "capable project seeking adopters" to "project with adopters" — a distribution challenge, not a technical one.

#### Strengths
- Licensing is textbook-correct (CC-BY-4.0 specs, Apache-2.0 code)
- CONTRIBUTING.md with DCO is enterprise-legal-compliant
- Conformance fixture suite covers valid and invalid paths
- Reference implementation is production-quality, not prototype
- Excel templates lower adoption barrier to zero for non-technical users
- Use case documentation maps directly to regulatory obligations
- Extension mechanism enables community experimentation
- GLEIF RA snapshot strategy correctly decouples from external dependency

#### Concerns
- **[Major]** No community infrastructure beyond the repository
- **[Major]** TSC exists on paper but has no external members
- **[Major]** No formal conformance sections in normative specs
- **[Major]** No published deployment or case study
- **[Minor]** No code of conduct document
- **[Minor]** No issue or PR templates
- **[Minor]** Python and TypeScript packages not published

#### Recommendations
1. (P0) Establish TSC with external members, publish v1.0 timeline
2. (P0) Add formal conformance sections to normative specs
3. (P0) Launch community infrastructure
4. (P1) Produce one public reference deployment
5. (P1) Evaluate OASIS or Linux Foundation hosting
6. (P1) Add code of conduct and issue templates
7. (P2) Publish Python and TypeScript packages
8. (P2) Create community extension scheme registry

---

### Security & Privacy Expert — Data Security & Privacy Architect

#### Assessment

OMTSF's privacy architecture reflects deliberate, threat-model-aware design. The three-level sensitivity taxonomy applied uniformly across identifiers and edge properties provides a coherent information classification lattice. Person node hard-omit from public files (not boundary-ref'd) demonstrates genuine privacy-by-design thinking. The boundary reference hash construction is sound for its stated purpose.

The primary concern is at the producer boundary: no mechanism prevents a misconfigured producer from generating a public file with incorrectly tagged identifiers. The sensitivity override mechanism allows reclassification without attestation of authorization.

#### Strengths
- Salt design correctly sized (32 bytes = SHA-256 output width)
- Person node hard-omit is the right GDPR Article 5(1)(c) default
- Beneficial ownership inherits person sensitivity automatically
- Post-redaction validation is defense-in-depth
- No unsafe code, no unwrap in production
- Extension fields default to public — predictable and auditable
- Local processing eliminates server-side attack surface
- `_property_sensitivity` stripped from public output

#### Concerns
- **[Major]** Unrecognized schemes default to public with no warning
- **[Major]** Producer can downgrade person identifier sensitivity with no validation gate
- **[Major]** No integrity verification before redaction in CLI
- **[Minor]** Salt reuse not forbidden at API level
- **[Minor]** `file_salt` persists in redacted output enabling cross-file correlation
- **[Minor]** No authenticated encryption at format level
- **[Minor]** `disclosure_scope` is optional

#### Recommendations
1. (P0) Add L2 validation rule for public sensitivity on person identifiers
2. (P0) Enforce intake integrity verification before redaction
3. (P1) Emit L2 warning for unrecognized schemes in public files
4. (P1) Strengthen salt persistence documentation
5. (P1) Add governance note on sensitivity override mechanism
6. (P2) Consider `unknown_scheme_sensitivity` header field
7. (P2) Add CCPA 2026 data broker note

---

### Entity Identification Expert — Entity Identification & Corporate Hierarchy Specialist

#### Assessment

From 17 years of entity resolution at D&B, this is one of the more thoughtful composite identifier models I have reviewed. The authors understand that LEI covers only ~2.86M active entities — a fraction of global commerce. The decision to make `internal` identifiers first-class while excluding them from merge is exactly right.

The edge cases are where identity falls apart. The transitive closure approach is correct but safety limits are underspecified. DUNS reassignment, branch vs. HQ disambiguation, and GLEIF Level 2 coverage gaps are the operational hazards that need normative treatment.

#### Strengths
- Composite model correctly reflects real-world coverage fragmentation
- `internal` exclusion from cross-file merge prevents the most common false-positive class
- GLEIF RA code validation with quarterly snapshots is pragmatic
- LEI status table with differentiated merge behaviors is correct
- `former_identity` edge maps to GLEIF Level 2 and OpenCorporates history
- `governance_structure` acknowledges non-tree corporate structures
- Extension scheme mechanism with reverse-domain notation is well-designed
- `org.sam.uei` for SAM.gov migration is timely
- L3-EID-03 cross-reference validation is professional-grade

#### Concerns
- **[Critical]** DUNS HQ vs. Branch disambiguation is advisory only — causes false merges
- **[Critical]** Merge-group safety limits lack concrete thresholds
- **[Major]** GLEIF Level 2 coverage gap not acknowledged
- **[Major]** No `is_ultimate` on `legal_parentage` (GLEIF Level 2 distinction)
- **[Major]** Circular ownership guidance absent
- **[Major]** VAT value field normalization undefined
- **[Minor]** `org.opencorporates` jurisdiction normalization unspecified
- **[Minor]** `verification_status: "inferred"` has no provenance
- **[Minor]** `branch_duns` flag missing

#### Recommendations
1. (P0) Require temporal fields on DUNS and GLN (promote L2-EID-07 to MUST)
2. (P0) Specify default merge-group size warning threshold
3. (P1) Add `is_ultimate` boolean to `legal_parentage`
4. (P1) Document GLEIF Level 2 coverage gap
5. (P1) Specify EU VAT normalization rule
6. (P1) Elevate branch DUNS guidance to MUST for organization nodes
7. (P2) Add cycle detection guidance for ownership traversal
8. (P2) Define `org.opencorporates` jurisdiction normalization
