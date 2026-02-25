# Expert Review: OMTS Entity Identification Specification

**Reviewer:** Supply Chain Expert, Supply Chain Visibility & Risk Analyst
**Spec Reviewed:** OMTS-SPEC-001 -- Entity Identification (Draft, 2026-02-17)
**Date:** 2026-02-17

---

## Assessment

This specification represents a substantial and well-reasoned response to what the vision review panel unanimously identified as the single most critical gap in the OMTS architecture. The composite identifier model -- treating LEI, DUNS, GLN, national registry numbers, tax IDs, and internal ERP codes as co-equal peers in a scheme-qualified array -- is the correct design for the realities of global supply chain data. In eighteen years of multi-tier supply chain mapping across automotive, electronics, and FMCG, I have never encountered a supplier base where a single identifier scheme covered even 60% of entities. The decision to make `internal` identifiers first-class (Section 4.1, `internal` scheme) while explicitly excluding them from cross-file merge is precisely the right tradeoff: it preserves ERP export fidelity without polluting the entity resolution logic.

The specification also makes strong architectural choices in areas I flagged as critical during the vision review. The three-type entity taxonomy (`organization`, `facility`, `good`) with geolocation on facilities directly addresses EUDR traceability requirements. The corporate hierarchy edge types (`ownership`, `operational_control`, `legal_parentage`, `former_identity`) with temporal validity cover the structural relationships required by the EU CSDDD and German LkSG. The tiered validation model (Level 1 structural, Level 2 completeness, Level 3 enrichment) resolves the tension between strict validation and real-world data quality that procurement and enterprise panelists rightly raised.

However, from a supply chain visibility and risk analysis perspective, I see meaningful gaps. The specification focuses heavily on entity identification and corporate structure but is comparatively thin on the operational supply relationships that constitute the actual supply chain. There is no taxonomy of supply edge types -- the example uses `supplies`, `operates`, and `produces`, but these are not formally specified. For disruption analysis ("if this node goes offline, what downstream flows are affected?"), for regulatory due diligence (distinguishing a direct supplier from a subcontractor from a tolling partner), and for practical n-tier mapping, the relationship type taxonomy is as important as entity identification. The specification also lacks a data quality or confidence signal on identifier records themselves -- a DUNS number sourced from a supplier self-declaration questionnaire and one verified against the D&B database are not the same thing, and risk analysts need to distinguish them.

## Strengths

- **Composite identifier model with no mandatory global scheme.** This is the only design that works in practice. Mandating LEI would exclude 99% of supply chain entities; mandating DUNS would create a proprietary dependency. The array-of-identifiers approach mirrors how mature supply chain transparency platforms (e.g., Sedex, EcoVadis supplier profiles) actually store entity references.
- **Separation of graph-local IDs from external identifiers.** Prevents the common antipattern of overloading internal keys with cross-system semantics. Clean separation enables ERP export without requiring a translation layer.
- **Sensitivity classification on identifiers.** The `public`/`restricted`/`confidential` model with sensible defaults (VAT = restricted, LEI = public) directly enables selective disclosure for multi-party data sharing, which is table stakes for real-world supply chain transparency programs.
- **Temporal validity on identifiers and edges.** Supply chains are not static. Companies merge, registrations lapse, facilities close. Temporal fields enable "as-of" queries that are essential for regulatory reporting periods.
- **Boundary reference design with salted hashing.** The anti-enumeration property (Section 8.2) is critical. Without the file-specific salt, an adversary could pre-compute hashes of known LEIs to discover whether a target entity appears in a redacted subgraph. This is a real attack vector in competitive supply chain intelligence.
- **GLEIF RA code list for national registry disambiguation.** Using the GLEIF Registration Authority list (700+ entries) as the controlled vocabulary for `nat-reg` authority values is the right choice -- it is the most comprehensive, maintained, and freely available registry-of-registries.
- **ERP integration mappings (Section 11).** Concrete SAP, Oracle, and Dynamics 365 field-level mappings are invaluable for adoption. This is the kind of practical detail that turns a spec from academic to implementable.
- **Facility nodes with GeoJSON geometry support.** Polygon boundaries for production plots are a hard EUDR requirement for commodities like palm oil, soy, cocoa, and timber. Point coordinates alone are insufficient.

## Concerns

- **[Critical] No formal supply relationship edge type taxonomy.** The specification defines corporate hierarchy edge types in detail (Section 6) but does not formally specify the supply relationship edge types (`supplies`, `produces`, `operates` appear only in the example). For supply chain due diligence under CSDDD Article 6 and LkSG Section 5, the distinction between direct supply, subcontracting, tolling, licensed manufacturing, brokerage, and logistics intermediation has concrete legal consequences. Without a formal taxonomy, two parties modeling the same supply relationship may use incompatible edge types, breaking merge semantics and regulatory reporting.

- **[Major] No data quality or confidence signal on identifier records.** The `sensitivity` field tells you *who can see* an identifier, but nothing about *how reliable* it is. A DUNS number from a supplier's self-reported questionnaire, one scraped from a website, and one verified against D&B's API have very different reliability profiles. Risk analysts routinely discount unverified identifiers. The vision review (P0-3) recommended a `confidence` field (`confirmed`/`reported`/`inferred`/`unverified`) and `verification_method` -- neither appears in this spec.

- **[Major] No capacity or volume attributes on supply edges.** Disruption modeling -- "if Facility X goes offline, can the remaining network cover demand?" -- requires quantitative data on supply relationships: contracted volume, capacity, lead time, percentage of spend. The specification models the *topology* of the network but not the *weight* of the edges. While these could be added as edge properties, the absence of any guidance means every implementation will invent its own schema.

- **[Major] Merge semantics for conflicting property values are deferred to tooling.** Section 7.3 states that when merged nodes have differing property values, "the merger MUST record both values with their provenance ... Conflict resolution is a tooling concern." For supply chain risk analysis, this is problematic. If two files disagree on whether a facility is in Xinjiang province (UFLPA relevance) or on the ownership percentage of a subsidiary (CSDDD relevance), the merge output must carry structured conflict metadata, not just "both values." The spec should define the conflict record structure.

- **[Minor] No explicit handling of sub-tier visibility depth.** The flat graph model implicitly supports n-tier visibility (any chain of edges), but there is no metadata for recording the *depth* of visibility or the *completeness* of mapping at each tier. In practice, a buyer has high-confidence data for tier-1, partial data for tier-2, and sparse data for tier-3+. A `visibility_depth` or `mapping_completeness` indicator per subgraph region would help risk analysts calibrate their analysis.

- **[Minor] `good` node type lacks batch/lot-level granularity.** The EUDR requires lot-level traceability -- not just "palm oil" but a specific shipment traceable to specific GPS-located plots. The `good` type as defined is closer to a product master record. Lot-level instances (linking a specific quantity to a specific production event at a specific facility) would require either subtypes or a separate node type.

## Recommendations

1. **(P0) Define a formal supply relationship edge type taxonomy.** At minimum: `supplies` (direct commercial supply), `subcontracts` (delegated production), `tolls` (tolling/processing arrangement), `distributes` (logistics/distribution), `brokers` (intermediary without possession). Each type should have defined properties (contract reference, commodity, volume, currency). This is as important for interoperability as entity identification.

2. **(P1) Add a `confidence` field to identifier records.** Enum values: `verified` (confirmed against authoritative source), `reported` (declared by the entity itself), `inferred` (derived from secondary data), `unverified` (no validation performed). Optionally add `verification_date` and `verification_source`. This directly addresses vision review recommendation P0-3.

3. **(P1) Define a structured conflict record for merge property disagreements.** Rather than deferring conflict resolution entirely to tooling, specify a `conflicts` array on merged nodes/edges that records: the property name, the competing values, the source file or reporting entity for each value, and the assertion date. This makes merge output machine-processable for risk analysis.

4. **(P1) Provide guidance for quantitative supply edge properties.** Define recommended (not required) properties for supply relationship edges: `volume` (with unit), `capacity`, `lead_time_days`, `spend_share_pct`. These enable disruption modeling and are routinely collected in supplier risk management platforms.

5. **(P2) Add lot/batch-level support to the `good` node type.** Either introduce a `lot` subtype or define a pattern for representing specific batches as `good` nodes linked to their product master `good` node via a `instance_of` edge. This is necessary for EUDR lot-level traceability.

6. **(P2) Introduce a `mapping_completeness` metadata field.** Allow file producers to declare the estimated completeness of their supply network mapping at each tier depth (e.g., "tier-1: 95%, tier-2: 40%, tier-3: 10%"). This is a data quality signal that helps downstream consumers calibrate their analysis.

## Cross-Expert Notes

- **For Prof. Graph Modeling Expert (Graph Modeling):** The absence of a supply edge type taxonomy means the formal graph model is only half-specified. Corporate hierarchy edges are well-defined, but the commercial/operational edges that constitute the actual supply network remain informal. The merge semantics for edges (Section 7.2) depend on edge `type` equality, but if types are uncontrolled strings, merge correctness is implementation-dependent.

- **For Enterprise Integration Expert (Enterprise Systems):** The ERP integration mappings in Section 11 are excellent for entity data, but supply relationship edges also need ERP mapping guidance. SAP's purchasing info records (table `EINA`/`EINE`), scheduling agreements, and source lists all carry relationship data that should map to typed supply edges in OMTS. Without this, the ERP export produces nodes without meaningful edges.

- **For Dr. Regulatory Compliance Expert (Regulatory Compliance):** The regulatory alignment table in Section 10.3 correctly identifies the regulations but understates the data granularity required. EUDR demands lot-level linkage to geolocated production plots, not just facility-level coordinates. CSDDD requires risk assessment results and remediation actions, not just network topology. The spec should either define extension points for regulatory-specific data or acknowledge that additional specs are needed.

- **For Dr. Security & Privacy Expert (Security & Privacy):** The boundary reference design in Section 8.2 is well-constructed, but its security depends on the salt being truly random and unique per file generation. If a producer reuses salts, the anti-enumeration property collapses. The spec should add a validation rule requiring salt uniqueness or at minimum a SHOULD-level recommendation for cryptographic random generation.

- **For Entity Identification Expert (Entity Identification):** Open Question #2 (minimum identifier requirement at Level 1 vs. Level 2) should remain at Level 2. In my experience mapping sub-tier suppliers in emerging markets, many entities have no global identifier at all -- only a name, a city, and an internal vendor code. Blocking these files at Level 1 would make the format unusable for precisely the supply chain tiers where visibility is most needed.
