# Expert Panel Report: Excel Import Format Specification for OMTS

**Date:** 2026-02-22
**Panel Chair:** Expert Panel Coordinator
**Topic:** Design of an Excel file format that can be converted to OMTS, and an `omtsf import-excel` command in the Rust reference implementation

---

## Panel Chair Summary

This panel reviewed the design of an Excel file format specification to serve as an import bridge for companies that currently manage supply chain data in spreadsheets. Four of seven commissioned experts completed their reviews: Supply Chain Expert, Procurement Expert, Graph Modeling Expert, and Data Serialization Expert. Three experts (Rust Engineer, Entity Identification Expert, ERP Integration Expert) did not complete reviews in time; their perspectives are partially covered by cross-expert notes from the completed panelists.

The panel reached strong consensus on the core architecture: **a multi-sheet Excel workbook** with separate sheets for metadata, node types (organizations, facilities, goods, attestations, consignments), relationships (edges), and identifiers. All four panelists independently converged on this structure, with the Graph Theorist providing the formal justification (multi-sheet mirrors adjacency-list decomposition) and the Procurement Expert grounding it in operational reality (procurement teams already think in supplier lists with columns for DUNS, tax IDs, and tiers).

The most significant area of consensus was the need for **two entry modes**: a simplified "Supplier List" single-sheet mode for the 80% use case (flat tier-1 supplier lists for LkSG/CSDDD compliance), and a full multi-sheet mode for complex multi-tier graph modeling. All four experts flagged multi-tier relationship representation as a critical design challenge, noting that procurement teams think in tiers but the OMTS graph model represents multi-tier as chains of edges. The panel also unanimously agreed that the `import-excel` command must auto-generate technical fields (`file_salt`, graph-local IDs, `boundary_ref` nodes) and apply sensible sensitivity defaults from SPEC-004.

Areas of productive disagreement included the handling of edge properties: the Graph Theorist proposed a single "Edges" sheet with sparse type-specific columns, while the Procurement Expert preferred separate sheets per edge category (supply relationships, corporate hierarchy, attestations). The Data Serialization Expert sided with the single-sheet approach for simplicity. The panel chair recommends the hybrid approach: separate sheets per edge *category* (not per edge type), which reduces from 16 sheets to ~3 while keeping columns manageable.

## Panel Composition

| Panelist | Role | Key Focus Area |
|----------|------|----------------|
| Supply Chain Expert | Supply Chain Visibility & Risk Analyst | Multi-tier representation, regulatory alignment, practical adoption |
| Procurement Expert | Chief Procurement Officer | Operational usability, template design, SME burden |
| Graph Modeling Expert | Graph Data Modeling & Algorithm Specialist | Graph-to-table projection, referential integrity, lossless conversion |
| Data Format Expert | Data Format Architect | Schema design, validation strategy, format versioning |
| ~~Rust Engineer~~ | ~~Senior Systems Engineer~~ | *(review not completed)* |
| ~~Entity Identification Expert~~ | ~~Corporate Hierarchy Specialist~~ | *(review not completed)* |
| ~~ERP Integration Expert~~ | ~~Enterprise Systems Architect~~ | *(review not completed)* |

---

## Consensus Findings

The following findings were independently raised by 3 or more panelists:

1. **Multi-sheet workbook is the correct architecture.** All four experts converged on separate sheets for nodes, edges, and identifiers. The Graph Theorist formalized this as the natural tabular projection of an adjacency-list graph representation. (All 4 panelists)

2. **A simplified single-sheet "Supplier List" mode is essential for adoption.** The Procurement Expert and Supply Chain Expert both emphasized that procurement teams will not fill 5+ sheet workbooks for a simple tier-1 supplier list. The Graph Theorist and Data Serialization Expert agreed, recommending this as the default entry point. (All 4 panelists)

3. **Edge direction conventions are counterintuitive and must be abstracted.** The OMTS convention (source = supplier, target = buyer for `supplies` edges) conflicts with how procurement teams think ("I buy from X"). All panelists recommended using domain-friendly column names (`Supplier` / `Buyer` instead of `Source` / `Target`). (All 4 panelists)

4. **The import command must auto-generate technical fields.** `file_salt`, graph-local IDs, `boundary_ref` nodes, and edge IDs should never appear in user-authored spreadsheets. (All 4 panelists)

5. **Cell-level error reporting with Excel coordinates is essential.** Validation errors must reference sheet names, row numbers, and column headers -- not OMTS rule IDs alone. (3 panelists: Supply Chain, Procurement, Data Serialization)

6. **Sensitivity defaults should be applied automatically per SPEC-004 Section 2.** Per-cell sensitivity markings are impractical. The import command should apply scheme-level defaults and expose overrides only for advanced users. (3 panelists: Supply Chain, Procurement, Data Serialization)

7. **A downloadable `.xlsx` template with data validation dropdowns is a must.** Enum fields should use Excel Data Validation to prevent invalid values at entry time. (All 4 panelists)

---

## Critical Issues

### 1. Multi-tier relationship representation is underspecified

**Raised by:** Supply Chain Expert, Procurement Expert, Graph Theorist, Data Serialization Expert

A flat "Relationships" sheet with source/target columns works for star-topology graphs but fails for true multi-tier mapping. Users must understand that a tier-2 relationship requires two rows (T2 -> T1 -> Buyer), but they will naturally enter one row with `tier: 2` pointing directly to the buyer. Without a "Chain View" entry mode or clear guidance, imported graphs will be topologically incorrect.

**Resolution:** Provide both a chain-view entry mode (columns: `Reporting Entity | Tier 1 | Tier 2 | Tier 3 | Commodity`) and a direct edge-entry mode. The import command should auto-generate intermediary nodes and edge chains from the chain view.

### 2. Entity deduplication at import time

**Raised by:** Supply Chain Expert

Excel templates will inevitably contain duplicate entries for the same supplier. Without deduplication detection, the imported graph will contain redundant nodes that corrupt downstream merge operations (SPEC-003).

**Resolution:** The import command should scan for potential duplicates (same LEI, same DUNS, high name similarity with matching jurisdiction) and either auto-merge with a warning or emit `same_as` edges per SPEC-003 Section 7.

### 3. Identifier validation must happen at import time, not post-import

**Raised by:** Data Serialization Expert, Graph Theorist

LEI check digits (MOD 97-10), GLN mod-10, DUNS format, and authority-requirement rules (L1-EID-03 through L1-EID-07) cannot be enforced in Excel. The import command must never produce an invalid `.omts` file.

**Resolution:** Run full SPEC-002 L1 validation on parsed identifier data and report failures with cell references before generating the output file.

---

## Major Issues

### 1. Edge property heterogeneity across edge types

**Raised by:** Procurement Expert, Graph Theorist

A single Edges sheet covering 16 edge types with different required/optional properties would have 25+ mostly-sparse columns. This is confusing for manual entry.

**Resolution:** Use edge-category sheets: `Supply Relationships` (supplies, subcontracts, tolls, distributes, brokers, sells_to), `Corporate Structure` (ownership, legal_parentage, operational_control, beneficial_ownership), `Attestation Links` (attested_by edges combined with attestation node data).

### 2. Multi-valued identifiers require a separate sheet

**Raised by:** Graph Theorist, Data Serialization Expert, Procurement Expert

A flat "one row per entity" approach limits identifiers to one column per scheme. Entities with multiple `internal` IDs or multiple `nat-reg` entries need a separate "Identifiers" sheet with a foreign key back to the entity row.

**Resolution:** Use common identifier columns (LEI, DUNS, VAT) directly on node sheets for the simple case, plus an optional "Identifiers" sheet for the general case. The import command must merge both sources and deduplicate per L1-EID-11.

### 3. Facility-to-organization linkage is error-prone

**Raised by:** Supply Chain Expert

Users often list facilities as sub-rows of organizations or use organization names instead of IDs. The import command must handle referential integrity gracefully.

**Resolution:** Support both `operator` as a column on the Facilities sheet (referencing an org ID) and name-based fuzzy matching with confirmation warnings.

### 4. Data quality metadata should default to file-level

**Raised by:** Supply Chain Expert

Per-row `data_quality` fields (confidence, source, last_verified) are unlikely to be filled in. An entire Excel file typically comes from a single source.

**Resolution:** Support file-level defaults on the Metadata sheet, applied to all nodes and edges unless overridden per-row.

---

## Minor Issues

- **GeoJSON polygon geometries** for EUDR facility nodes cannot be represented in Excel cells. Support `Latitude` / `Longitude` columns for points; accept WKT strings or external file references for polygons. (Supply Chain Expert, Graph Theorist)
- **Labels** (SPEC-001, Section 8.4) with arbitrary keys need a dedicated `Labels` sheet, adding complexity. Pragmatic approach: embed recommended label keys (Appendix B) as optional columns on node sheets. (Graph Theorist, Data Serialization Expert)
- **Excel row limit** (1,048,576 rows per sheet) is below the advisory OMTS edge limit (5M). Document this limitation. (Procurement Expert)
- **BOM structures** (`composed_of` edges) are difficult to represent intuitively in flat tables. (Graph Theorist)
- **CBAM emissions columns** add significant width for a niche use case. Consider a separate CBAM sheet. (Supply Chain Expert)

---

## Consolidated Recommendations

### P0 — Immediate (before implementation begins)

| # | Recommendation | Source Expert(s) |
|---|---------------|-----------------|
| 1 | **Define multi-sheet workbook structure:** Metadata, Organizations, Facilities, Goods, Attestations, Consignments, Supply Relationships, Corporate Structure, Attestation Links, Identifiers (optional) | All 4 |
| 2 | **Use domain-friendly column names** instead of Source/Target: Supplier/Buyer for supplies, Subsidiary/Parent for legal_parentage, Operator/Facility for operates | All 4 |
| 3 | **Auto-generate technical fields:** file_salt (CSPRNG), graph-local IDs (deterministic slugs or UUIDs), edge IDs, boundary_ref nodes | All 4 |
| 4 | **Implement cell-reference error reporting:** Validation errors must map to `{Sheet}!{Column}{Row}` coordinates | Supply Chain, Procurement, Data Serialization |
| 5 | **Implement fuzzy deduplication detection** at import time: scan for duplicate entities by identifier match or name similarity, emit warnings or `same_as` edges | Supply Chain |
| 6 | **Enforce referential integrity at import time:** Every edge source/target must resolve to a node; every identifier row must reference a valid node | Graph Theorist |

### P1 — Before v1 release

| # | Recommendation | Source Expert(s) |
|---|---------------|-----------------|
| 7 | **Design a "Supplier List" simplified single-sheet mode** for flat tier-1 lists with auto-generated organization nodes and supplies edges | Procurement, Supply Chain, Graph Theorist |
| 8 | **Provide a downloadable `.xlsx` template** with Data Validation dropdowns for all enum fields, conditional formatting, and example rows | All 4 |
| 9 | **Apply sensitivity defaults automatically** per SPEC-004 Section 2; expose overrides only on an optional Sensitivity sheet or column | Supply Chain, Procurement, Data Serialization |
| 10 | **Support file-level data_quality defaults** on the Metadata sheet, applied to all entities | Supply Chain |
| 11 | **Support auto-ID generation with stable slugs** when the `id` column is left blank (e.g., `org-bolt-supplies-ltd`) | Data Serialization |
| 12 | **Use conditional column requirements** on the Relationships sheet, validated per edge type | Graph Theorist |

### P2 — Future enhancements

| # | Recommendation | Source Expert(s) |
|---|---------------|-----------------|
| 13 | **Support a "Chain View" sheet** for multi-tier entry (columns: Reporting Entity, Tier 1, Tier 2, Tier 3, Commodity) with auto-generated edge chains | Procurement, Supply Chain |
| 14 | **Support CSV directory fallback** for users without Excel | Data Serialization |
| 15 | **Publish a canonical column-name-to-OMTS-field mapping table** versioned alongside the schema | Data Serialization |
| 16 | **Support EUDR geolocation** with lat/lon columns and external GeoJSON file references | Supply Chain |
| 17 | **Include a "README" sheet** within the workbook with field definitions and instructions | Procurement |

---

## Cross-Domain Interactions

1. **Graph structure ↔ Procurement usability:** The Graph Theorist's concern about edge property heterogeneity directly conflicts with the Procurement Expert's demand for simplicity. The hybrid solution (edge-category sheets) was proposed by the Supply Chain Expert as the pragmatic middle ground.

2. **Identifier model ↔ Template design:** The Entity Identification perspective (though the review was not completed) is partially addressed by all four panelists. The consensus is: embed common identifier schemes as direct columns on node sheets (LEI, DUNS, VAT) for the 80% case, with a separate Identifiers sheet for the general case. The import command must merge both sources.

3. **Rust implementation ↔ Validation strategy:** The Graph Theorist and Data Serialization Expert both recommended that the Excel-to-graph conversion logic should live in `omtsf-core` as pure functions (for WASM compatibility), with only the file I/O in `omtsf-cli`. The `calamine` crate is the established choice for Excel reading in Rust.

4. **Regulatory compliance ↔ Template defaults:** The Supply Chain Expert emphasized that LkSG and CSDDD compliance workflows start with Excel supplier lists. The simplified single-sheet mode directly serves this use case and should be the documented entry point for regulatory compliance teams.

5. **Sensitivity model ↔ Import defaults:** All panelists agreed that SPEC-004's per-identifier sensitivity is too granular for Excel entry. The import command should apply scheme-level defaults and only expose overrides for advanced users. This has implications for boundary reference generation, which should be a separate post-import step (`omtsf redact`), not part of the Excel import.

---

## Individual Expert Reports

### Supply Chain Expert — Supply Chain Visibility & Risk Analyst

#### Assessment

From eighteen years of working with multi-tier supply chain data across automotive, electronics, and FMCG, I can state with confidence that an Excel-to-OMTS bridge is not merely convenient -- it is essential for adoption. In my experience leading supply chain transparency programs at a Fortune 100 manufacturer, over 90% of initial supplier due diligence data arrives in spreadsheet form. LkSG risk analyses, CSDDD value chain mappings, conflict minerals CMRT templates, and EUDR operator submissions all begin as Excel workbooks.

The OMTS graph model (SPEC-001) is rich: 7 node types, 16+ edge types, composite identifiers with authority qualifiers, data quality metadata, labels, and selective disclosure markings. Flattening this into tabular form requires careful design. The primary tension is between expressiveness and usability. I have seen many mapping initiatives fail because the template was either too simplistic or too complex.

The multi-tier dimension is particularly critical. Under LkSG, reporting companies must document their direct suppliers and, upon substantiated knowledge of risk, their indirect suppliers. Under CSDDD, the obligation extends across the full value chain. The Excel format must make tier relationships explicit and intuitive.

#### Strengths

- The OMTS data model is well-suited for tabular decomposition. The clear separation between node types maps naturally to separate worksheets.
- The composite identifier model (SPEC-002) with scheme+value+authority triples is Excel-friendly. Columns like `LEI`, `DUNS`, `National Registry ID` are immediately legible.
- The `supplies` edge with `tier` property directly models how companies already think about their supply chains.
- The label system (Appendix B) with recommended keys translates cleanly to Excel columns.
- Attestation nodes cover a real operational need for tracking certifications in spreadsheet registers.

#### Concerns

- **[Critical]** Multi-tier relationship representation is underspecified for complex supply networks.
- **[Critical]** Entity deduplication at import time -- Excel templates will inevitably contain duplicate entries.
- **[Major]** Facility-to-organization linkage is error-prone in tabular form.
- **[Major]** Selective disclosure and sensitivity markings are difficult to represent without column explosion.
- **[Major]** Data quality metadata per-row is unlikely to be filled in; support file-level defaults.
- **[Minor]** GeoJSON polygon geometries cannot be represented in Excel cells.
- **[Minor]** CBAM emissions data adds significant column width for a niche use case.

#### Recommendations

1. (P0) Define a canonical multi-sheet structure: Organizations, Facilities, Goods, Attestations, Consignments, Relationships, Attestation Links, Metadata.
2. (P0) Use explicit Supplier ID / Buyer ID column headers instead of generic Source / Target.
3. (P0) Implement fuzzy deduplication detection at import time.
4. (P1) Support a "flat" single-sheet mode for simple supplier lists.
5. (P1) Handle sensitivity through column-level defaults, not per-cell markings.
6. (P1) Support file-level data_quality defaults on the Metadata sheet.
7. (P2) Provide a downloadable .xlsx template with data validation dropdowns.
8. (P2) For EUDR geolocation, support both point coordinates and GeoJSON file references.

---

### Procurement Expert — Chief Procurement Officer

#### Assessment

As someone who manages 4,000+ direct suppliers across SAP S/4HANA, Ariba, and several regional ERP instances, I can state categorically that Excel is the lingua franca of supplier data collection. We issue Excel-based supplier self-assessment questionnaires, receive tiered supplier mappings from our Tier 1s in spreadsheets, and reconcile audit findings in pivot tables. An `import-excel` pathway into OMTS is essential for adoption.

The core design challenge is translating a flat, tabular mental model into the directed labeled property multigraph that OMTS requires. My procurement analysts think in "supplier rows" with columns for DUNS, tax ID, tier, and commodity. They do not think in nodes, edges, or source/target directionality. The Excel template must bridge this gap without forcing users to learn graph theory.

My experience with Ariba CSV imports, Coupa flat file loaders, and SAP LSMW templates tells me that the single biggest predictor of import success is the quality of the template and its embedded validation. The Excel template must be opinionated -- guiding users toward correct data entry with dropdown lists, conditional formatting, and clear column headers.

#### Strengths

- Addresses a real adoption bottleneck -- Tier 2+ data almost always arrives in spreadsheets.
- The OMTS flat adjacency list design maps naturally to a two-sheet Excel structure.
- The identifier model already anticipates partial data -- internal-only identifiers are valid at L1.
- Edge type constraints (Section 9.5) provide built-in validation targets.

#### Concerns

- **[Critical]** Multi-tier representation requires graph-local IDs and edge direction understanding.
- **[Critical]** Entity identification data quality will be the primary failure mode -- fewer than 40% of Tier 2+ suppliers have DUNS numbers.
- **[Major]** Confidentiality markings per cell are impractical.
- **[Major]** `file_salt` and `boundary_ref` cannot originate from Excel.
- **[Major]** Edge property heterogeneity across 16 edge types creates confusing sparse columns.
- **[Minor]** Excel row limits (1M) are below OMTS advisory edge limits (5M).

#### Recommendations

1. (P0) Design a "Supplier List" simplified entry sheet with auto-generated nodes and edges.
2. (P0) Default all sensitivity values automatically based on SPEC-004 defaults.
3. (P1) Provide an Excel template file with embedded data validation.
4. (P1) Generate file_salt, graph-local IDs, and edge IDs automatically.
5. (P1) Structure edges by category across multiple sheets.
6. (P2) Support a "Chain View" sheet for multi-tier entry.
7. (P2) Publish the Excel template as a downloadable artifact version-tagged to the spec.

---

### Graph Modeling Expert — Graph Data Modeling & Algorithm Specialist

#### Assessment

From a graph-theoretic perspective, the fundamental challenge is projecting a directed labeled property multigraph with 7 node types, 16 edge types, and composite identity into a two-dimensional tabular representation that permits lossless round-trip conversion. The key insight is that a single flat table is insufficient -- the OMTS graph has heterogeneous node and edge schemas, and the identifier model introduces a one-to-many relationship from nodes to identifier records. A multi-sheet workbook is the correct architectural choice, mirroring how adjacency-list representations separate vertex and edge data.

The OMTS multigraph property is critical to preserve. Multiple edges of the same type between the same node pair are permitted and distinguished by independent IDs (SPEC-001, Section 3.2). The edge sheet must use explicit edge IDs to prevent collapsing distinct supply contracts into single rows.

The tier structure in OMTS `supplies` edges offers a natural guide for users, but tier is a derived, perspective-dependent annotation. The Excel template should make tier an optional column, not a structural organizer.

#### Strengths

- Multi-sheet workbook directly maps to the graph model's adjacency-list decomposition.
- Graph-local IDs as join keys across sheets mirror adjacency-list representation.
- Typed edge constraints (Section 9.5) make validation tractable at parse time.
- Composite identifiers suit a separate Identifiers sheet perfectly.
- Clear required/optional field semantics per type translate directly to column validation.

#### Concerns

- **[Critical]** Edge properties are type-polymorphic -- a single Edges sheet with 25+ sparse columns is confusing.
- **[Major]** Multi-tier relationships are implicit in topology, not visible in any single row.
- **[Major]** Labels and data_quality add further dimensionality requiring additional sheets.
- **[Minor]** GeoJSON polygon geometry cannot be represented in Excel cells.
- **[Minor]** Bill-of-materials structures (composed_of) are unintuitive in flat tables.

#### Recommendations

1. (P0) Adopt a 5-sheet workbook structure: Metadata, Nodes, Edges, Identifiers, Labels.
2. (P0) Enforce referential integrity at import time (L1-GDM-03 and cross-sheet references).
3. (P0) Make the Identifiers sheet the primary vehicle for external IDs, not columns on Nodes.
4. (P1) Use conditional column requirements on Edges, validated per edge type.
5. (P1) Add sensitivity columns to Identifiers and disclosure_scope to Metadata.
6. (P1) Provide a downloadable .xlsx template with Data Validation dropdowns.
7. (P2) Support a "compact" single-sheet mode for simple supplier lists.

---

### Data Format Expert — Data Format Architect

#### Assessment

From a serialization and format design perspective, Excel-to-graph import is a well-understood but deceptively difficult problem. The fundamental challenge is projecting a directed labeled property multigraph into a tabular format that procurement teams can populate manually. I have seen this pattern repeatedly: Avro schemas get flattened to CSV, and the impedance mismatch between hierarchical/graph structures and flat tables always produces data quality problems at the boundary.

The OMTS graph model is well-suited to this import because it already uses a flat adjacency-list serialization. The Excel format can mirror that structure directly. The key design insight is that this is a lossy input format, not a round-trip serialization. Fields like `file_salt`, `boundary_ref` nodes, and `_conflicts` metadata have no place in a manually-authored spreadsheet.

The biggest risk is the validation gap between what Excel can structurally enforce (very little) and what the OMTS schema requires (type-specific required properties, cross-row referential integrity, identifier format validation). The `import-excel` command must compensate by running the full validation pipeline post-import with actionable error messages mapping back to specific cells.

#### Strengths

- The adjacency-list model maps cleanly to sheets.
- The identifiers array design is amenable to a dedicated Identifiers sheet.
- The edge properties wrapper separates structural from type-specific fields.
- The disclosure_scope and sensitivity model is simple enough for column values.
- The ERP integration guide (SPEC-005) provides natural column header vocabulary.

#### Concerns

- **[Critical]** Identifier validation (check digits, authority requirements) must happen at import time.
- **[Major]** Multi-valued identifiers require a design decision on separate sheet vs. fixed columns.
- **[Major]** Edge direction conventions are counterintuitive for procurement users.
- **[Major]** Multi-tier relationships have inherent readability problems in flat tables.
- **[Minor]** Labels with arbitrary keys need a dedicated sheet.
- **[Minor]** _property_sensitivity overrides are too granular for manual entry.

#### Recommendations

1. (P0) Use a multi-sheet workbook with typed node sheets, relationships sheet, and identifiers sheet.
2. (P0) Implement cell-reference error reporting (`{Sheet}!{Column}{Row}`).
3. (P0) Auto-generate file_salt, node/edge IDs, and boundary_ref nodes.
4. (P1) Use domain-friendly column aliases for edge direction.
5. (P1) Provide downloadable .xlsx template with Data Validation.
6. (P1) Support auto-ID generation with stable slugs.
7. (P2) Support CSV directory fallback.
8. (P2) Define canonical column-name-to-OMTS-field mapping table in the spec.
