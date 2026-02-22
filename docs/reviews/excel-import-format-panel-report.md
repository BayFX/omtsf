# Expert Panel Report: Excel Import Format Specification for OMTS

**Date:** 2026-02-22
**Panel Chair:** Expert Panel Coordinator
**Topic:** Design of an Excel file format that can be converted to OMTS, and an `omtsf import-excel` command in the Rust reference implementation

---

## Panel Chair Summary

This panel reviewed the design of an Excel file format specification to serve as an import bridge for companies that currently manage supply chain data in spreadsheets. All seven commissioned experts completed their reviews: Supply Chain Expert, Procurement Expert, Graph Modeling Expert, Data Serialization Expert, Rust Engineer, Entity Identification Expert, and ERP Integration Expert.

The panel reached strong consensus on the core architecture: **a multi-sheet Excel workbook** with separate sheets for metadata, node types (organizations, facilities, goods, attestations, consignments), relationships (edges), and identifiers. All seven panelists independently converged on this structure, with the Graph Theorist providing the formal justification (multi-sheet mirrors adjacency-list decomposition), the Procurement Expert grounding it in operational reality, and the Rust Engineer defining the crate boundaries (Excel I/O in `omtsf-cli`, graph construction in `omtsf-core`).

The most significant area of consensus was the **separate Identifiers sheet**. Six of seven experts independently recommended this as the correct approach for handling the variable-length identifier array (SPEC-002 Section 3). The Entity Identification Expert provided the definitive argument: fixed identifier columns on node sheets cannot accommodate multiple identifiers of the same scheme (e.g., two `internal` IDs from different ERP systems), and a separate sheet with `node_id` foreign key is the only layout that maps correctly to the SPEC-002 identifier array. The Data Serialization Expert and Procurement Expert additionally recommended embedding common schemes (LEI, DUNS, VAT) as direct columns on node sheets for the 80% simple case, with the Identifiers sheet serving as the general-purpose overflow.

Areas of productive disagreement included edge property representation. The Graph Theorist proposed a single "Edges" sheet with sparse columns; the Procurement Expert preferred separate sheets per edge category; the ERP Integration Expert recommended a four-sheet minimal structure. The panel chair recommends the hybrid: separate sheets per edge *category* (Supply Relationships, Corporate Structure, Attestation Links), which reduces from 16 sheets to 3 while keeping columns manageable.

The Entity Identification Expert and Rust Engineer raised concerns not covered by the initial four reviewers: the need for a **same_as / entity deduplication sheet** (critical for merge viability), **HQ vs. branch DUNS disambiguation**, **GLEIF RA code lookup at import time**, and **Excel formula cell / cached value behavior**. The ERP Integration Expert contributed the important insight that a `SourceSystem` column mapping to SPEC-005's authority naming convention is essential for traceability.

## Panel Composition

| Panelist | Role | Key Focus Area |
|----------|------|----------------|
| Supply Chain Expert | Supply Chain Visibility & Risk Analyst | Multi-tier representation, regulatory alignment, practical adoption |
| Procurement Expert | Chief Procurement Officer | Operational usability, template design, SME burden |
| Graph Modeling Expert | Graph Data Modeling & Algorithm Specialist | Graph-to-table projection, referential integrity, lossless conversion |
| Data Format Expert | Data Format Architect | Schema design, validation strategy, format versioning |
| Rust Engineer | Senior Systems Engineer | Crate architecture, parsing safety, WASM compat, error handling |
| Entity Identification Expert | Corporate Hierarchy Specialist | Identifier validation, check digits, authority codes, M&A history |
| ERP Integration Expert | Enterprise Systems Architect | ERP field mapping, authority naming, multi-system landscapes |

---

## Consensus Findings

The following findings were independently raised by 3 or more panelists:

1. **Multi-sheet workbook is the correct architecture.** All seven experts converged on separate sheets for nodes, edges, and identifiers. The Graph Theorist formalized this as the natural tabular projection of an adjacency-list graph representation. (All 7 panelists)

2. **A separate Identifiers sheet is essential.** Six experts independently recommended a dedicated `Identifiers` sheet with `node_id`, `scheme`, `value`, `authority`, `sensitivity` columns. The Entity Identification Expert and Rust Engineer both identified the structural mismatch between Excel's flat row model and SPEC-002's variable-length identifier array as a critical design constraint. (6 panelists: all except Supply Chain)

3. **A simplified single-sheet "Supplier List" mode is essential for adoption.** The Procurement Expert and Supply Chain Expert emphasized that procurement teams will not fill 5+ sheet workbooks for simple tier-1 lists. All panelists agreed this should be the default entry point. (All 7 panelists)

4. **Edge direction conventions are counterintuitive and must be abstracted.** The OMTS convention (source = supplier, target = buyer for `supplies` edges) conflicts with how procurement teams think. All panelists recommended using domain-friendly column names. The Entity Identification Expert specifically recommended plain-English headers like "This entity IS OWNED BY this parent." (All 7 panelists)

5. **The import command must auto-generate technical fields.** `file_salt` (CSPRNG), graph-local IDs, `boundary_ref` nodes, and edge IDs should never appear in user-authored spreadsheets. The Rust Engineer emphasized that the CSPRNG salt generation must be explicit in the spec and never left to user input. (All 7 panelists)

6. **Cell-level error reporting with Excel coordinates is essential.** Validation errors must reference sheet names, row numbers, and column headers. The Rust Engineer specified the error strategy: parse errors fail fast, L1 failures collect all diagnostics and produce no output, L2 warnings emit but still produce output. (5 panelists: Supply Chain, Procurement, Data Serialization, Rust Engineer, ERP Integration)

7. **Sensitivity defaults should be applied automatically per SPEC-004 Section 2.** Per-cell sensitivity markings are impractical. The import command should apply scheme-level defaults. The ERP Integration Expert specifically flagged that `disclosure_scope` must be validated against identifier sensitivity levels at import time (L1-SDI-02). (5 panelists: Supply Chain, Procurement, Data Serialization, Rust Engineer, ERP Integration)

8. **A downloadable `.xlsx` template with data validation dropdowns is a must.** Enum fields should use Excel Data Validation to prevent invalid values at entry time. (All 7 panelists)

9. **Check-digit validation must happen at import time.** LEI MOD 97-10 (L1-EID-05), GLN mod-10 (L1-EID-07), and DUNS format validation must be enforced before writing any `.omts` output. The import command must never produce an invalid file. (5 panelists: Data Serialization, Graph Theorist, Entity ID, Rust Engineer, ERP Integration)

10. **Default all imported identifiers to `verification_status: "reported"`.** Data entered in Excel is almost always manually reported, not verified. The Entity Identification Expert recommended defaulting this and setting `data_quality.source` to `"excel-import"` for provenance. (3 panelists: Entity ID, ERP Integration, Supply Chain)

---

## Critical Issues

### 1. Multi-tier relationship representation is underspecified

**Raised by:** Supply Chain Expert, Procurement Expert, Graph Theorist, Data Serialization Expert, ERP Integration Expert

A flat "Relationships" sheet with source/target columns works for star-topology graphs but fails for true multi-tier mapping. Users must understand that a tier-2 relationship requires two rows (T2 -> T1 -> Buyer), but they will naturally enter one row with `tier: 2` pointing directly to the buyer. Without a "Chain View" entry mode or clear guidance, imported graphs will be topologically incorrect.

**Resolution:** Provide both a chain-view entry mode (columns: `Reporting Entity | Tier 1 | Tier 2 | Tier 3 | Commodity`) and a direct edge-entry mode. The import command should auto-generate intermediary nodes and edge chains from the chain view.

### 2. Structural mismatch: flat rows vs. multi-identifier array

**Raised by:** Entity Identification Expert, Rust Engineer, Graph Theorist, Data Serialization Expert, ERP Integration Expert

A single organization node in OMTS can carry 5-10 identifier records across different schemes. Excel has no native array-of-objects column type. Fixed columns per scheme cannot accommodate multiple identifiers of the same scheme (e.g., two `internal` IDs from different ERP systems). The Rust Engineer noted this is the single highest-impact structural decision, determining whether the parser uses `Vec<IdentifierRecord>` or a fixed-size array.

**Resolution:** Use a separate Identifiers sheet with `node_id`, `scheme`, `value`, `authority`, `sensitivity`, `valid_from`, `valid_to` columns. Optionally embed common schemes (LEI, DUNS, VAT) as direct columns on node sheets for the simple case. The import command must merge both sources and deduplicate per L1-EID-11.

### 3. Identifier validation must happen at import time, not post-import

**Raised by:** Data Serialization Expert, Graph Theorist, Entity Identification Expert, Rust Engineer, ERP Integration Expert

LEI check digits (MOD 97-10), GLN mod-10, DUNS format, and authority-requirement rules (L1-EID-03 through L1-EID-07) cannot be enforced in Excel. The import command must never produce an invalid `.omts` file. The Rust Engineer specified the two-phase validation: structural (pre-conversion) and OMTS-spec (post-conversion).

**Resolution:** Run full SPEC-002 L1 validation on parsed identifier data. Report failures with cell references before generating the output file. L1 failures block output; L2 warnings are emitted but output is still produced.

### 4. No mechanism for same_as edges in the Excel format

**Raised by:** Entity Identification Expert, Supply Chain Expert

The `same_as` edge (SPEC-003 Section 7) is the explicit deduplication mechanism for when entity resolution is uncertain. Without a same_as sheet, the import command will produce disconnected nodes that a merge engine will never unify. Same_as is not optional for realistic supply chain data.

**Resolution:** Add a "Same As / Entity Deduplication" sheet with columns: `entity_key_a`, `entity_key_b`, `confidence` (definite/probable/possible), `basis`. The import command should also scan for potential duplicates by identifier match.

### 5. CSPRNG salt generation is unspecified

**Raised by:** Rust Engineer, Procurement Expert, Data Serialization Expert

SPEC-001 Section 2 requires a 64-char lowercase hex CSPRNG salt. An Excel cell cannot generate one. The spec must be explicit that the Metadata sheet salt cell is left blank and filled by the importer, or pre-filled with a value the importer validates against `^[0-9a-f]{64}$`.

**Resolution:** The import command MUST generate a fresh CSPRNG salt if the cell is blank. Document this behavior explicitly. Never allow users to type a weak manually-generated string.

---

## Major Issues

### 1. Edge property heterogeneity across edge types

**Raised by:** Procurement Expert, Graph Theorist, Entity Identification Expert, ERP Integration Expert

A single Edges sheet covering 16 edge types with different required/optional properties would have 25+ mostly-sparse columns. The Rust Engineer specified that without a typed column manifest per edge type, the parser cannot distinguish "absent property" from "user forgot to fill it in."

**Resolution:** Use edge-category sheets: `Supply Relationships` (supplies, subcontracts, tolls, distributes, brokers, sells_to), `Corporate Structure` (ownership, legal_parentage, operational_control, beneficial_ownership), `Attestation Links` (attested_by edges with attestation node data). The Rust Engineer recommends an exhaustive `match edge_type { ... }` dispatch in the parser.

### 2. Authority fields require GLEIF RA codes users won't know

**Raised by:** Entity Identification Expert, ERP Integration Expert

The `authority` field on nat-reg identifiers MUST contain a valid GLEIF RA code (L1-EID-03). Procurement officers don't know that UK Companies House is "RA000585." The ERP Integration Expert flagged the same issue for VAT authorities and SAP STCD1/STCD2 country-dependent disambiguation.

**Resolution:** Provide a bundled RA code lookup in the import command. Accept both RA codes and plain-English names ("Companies House" -> "RA000585"). Reject nat-reg entries where authority cannot be resolved. Add a `TaxIDCountry` column adjacent to tax ID columns.

### 3. HQ vs. branch DUNS disambiguation

**Raised by:** Entity Identification Expert

D&B assigns separate DUNS numbers for HQ and branch locations. Branch DUNS should go on facility nodes, not organization nodes. If users paste a branch DUNS into an Organizations sheet, it violates L3-EID-05 and causes incorrect merge behavior.

**Resolution:** Separate columns for "HQ DUNS" and "Site DUNS" with tooltip guidance. The import command should route site DUNS to facility nodes. Emit an advisory when a DUNS on an organization node cannot be confirmed as HQ-level.

### 4. Facility-to-organization linkage is error-prone

**Raised by:** Supply Chain Expert

Users often list facilities as sub-rows of organizations or use organization names instead of IDs. The import command must handle referential integrity gracefully.

**Resolution:** Support both `operator` as a column on the Facilities sheet (referencing an org ID) and name-based fuzzy matching with confirmation warnings.

### 5. Person node privacy handling needs explicit behavior

**Raised by:** Rust Engineer, ERP Integration Expert

SPEC-004 Section 5 requires that `person` nodes be omitted from `public`-scoped files. If the Metadata sheet declares `disclosure_scope: "public"` and the Nodes sheet contains `person`-type rows, the import pipeline must define explicit behavior.

**Resolution:** The import command MUST refuse to produce the output file (not silently strip rows). Emit a diagnostic referencing SPEC-004 Section 5. The ERP Integration Expert recommends a named warning code (`OMTS-IMPORT-W001`).

### 6. Data quality metadata should default to file-level

**Raised by:** Supply Chain Expert, Entity Identification Expert

Per-row `data_quality` fields are unlikely to be filled in. The Entity Identification Expert recommended defaulting `data_quality.source` to `"excel-import"` and `verification_status` to `"reported"` on all imported records.

**Resolution:** Support file-level defaults on the Metadata sheet, applied to all nodes and edges unless overridden per-row.

### 7. Excel formula cells and cached values

**Raised by:** Rust Engineer

Excel cells may contain formulas whose cached values may be stale if the file was saved without recalculating. `calamine` reads cached values from `calcChain` XML.

**Resolution:** Document that the importer reads cached values only and users MUST save with full recalculation enabled. Validate header rows before reading data.

---

## Minor Issues

- **GeoJSON polygon geometries** for EUDR facility nodes cannot be represented in Excel cells. Support `Latitude` / `Longitude` columns for points; accept WKT strings or external file references for polygons. (Supply Chain Expert, Graph Theorist)
- **Labels** (SPEC-001, Section 8.4) with arbitrary keys need a dedicated `Labels` sheet. Embed recommended label keys (Appendix B) as optional columns on node sheets. (Graph Theorist, Data Serialization Expert, Rust Engineer)
- **Excel row limit** (1,048,576 rows per sheet) is below the advisory OMTS edge limit (5M). Document this limitation. (Procurement Expert)
- **BOM structures** (`composed_of` edges) are difficult to represent intuitively in flat tables. (Graph Theorist)
- **CBAM emissions columns** add significant width for a niche use case. Consider a separate CBAM sheet. (Supply Chain Expert)
- **No provision for OpenCorporates or Refinitiv PermID** in template columns. Include optional columns for extension schemes. (Entity ID Expert)
- **`_property_sensitivity` overrides** have no obvious Excel representation. Either ignore for Excel import (use schema defaults) or define a column convention (e.g., `annual_value_sensitivity`). (Rust Engineer, Data Serialization Expert)
- **Ownership percentage format ambiguity**: users will enter "51" or "51%". The import command must strip percent signs and validate range 0-100. (Entity ID Expert)
- **No temporal identity columns**: SPEC-002 supports `valid_from`/`valid_to` on identifiers. The import command should populate from `snapshot_date` when absent. (Entity ID Expert)
- **Round-trip precision loss**: Excel's 15-significant-digit float limit and date serial representation can corrupt `ownership.percentage` and dates. Document constraints. (Rust Engineer)

---

## Consolidated Recommendations

### P0 — Immediate (before implementation begins)

| # | Recommendation | Source Expert(s) |
|---|---------------|-----------------|
| 1 | **Define multi-sheet workbook structure:** Metadata, Organizations, Facilities, Goods, Attestations, Consignments, Supply Relationships, Corporate Structure, Identifiers, Same As | All 7 |
| 2 | **Commit to a separate Identifiers sheet** with `node_id`, `scheme`, `value`, `authority`, `sensitivity`, `valid_from`, `valid_to` columns as the primary identifier vehicle | Entity ID, Rust Engineer, Graph Theorist, Data Serialization, ERP Integration |
| 3 | **Use domain-friendly column names** instead of Source/Target: Supplier/Buyer for supplies, Subsidiary/Parent for legal_parentage, Operator/Facility for operates | All 7 |
| 4 | **Auto-generate technical fields:** file_salt (CSPRNG, always generate fresh), graph-local IDs (deterministic slugs or UUIDs), edge IDs, boundary_ref nodes | All 7 |
| 5 | **Implement cell-reference error reporting:** Validation errors must map to `{Sheet}!{Column}{Row}` coordinates with OMTS rule IDs | Supply Chain, Procurement, Data Serialization, Rust Engineer, ERP Integration |
| 6 | **Enforce referential integrity at import time:** Every edge source/target must resolve to a node; every identifier row must reference a valid node | Graph Theorist, Rust Engineer, ERP Integration |
| 7 | **Run full L1 identifier validation at import time:** LEI MOD 97-10, GLN mod-10, DUNS format, authority requirements. Never produce an invalid .omts file | Entity ID, Rust Engineer, Data Serialization, Graph Theorist, ERP Integration |
| 8 | **Publish a per-edge-type column manifest** documenting required and optional columns per edge type. Implement as exhaustive match dispatch in the parser | Rust Engineer |
| 9 | **Add a Same As sheet** for entity deduplication with confidence and basis columns | Entity ID, Supply Chain |
| 10 | **Add a Corporate Hierarchy sheet** with subsidiary/parent/relationship_type/percentage/dates columns | Entity ID |

### P1 — Before v1 release

| # | Recommendation | Source Expert(s) |
|---|---------------|-----------------|
| 11 | **Design a "Supplier List" simplified single-sheet mode** for flat tier-1 lists with auto-generated organization nodes and supplies edges | Procurement, Supply Chain, Graph Theorist |
| 12 | **Provide a downloadable `.xlsx` template** with Data Validation dropdowns for all enum fields, conditional formatting, and example rows | All 7 |
| 13 | **Apply sensitivity defaults automatically** per SPEC-004 Section 2; validate disclosure_scope against identifier sensitivity (L1-SDI-02) | Supply Chain, Procurement, Data Serialization, ERP Integration |
| 14 | **Bundle a GLEIF RA code lookup** in the import command; accept both RA codes and plain-English authority names | Entity ID, ERP Integration |
| 15 | **Support file-level data_quality defaults** on the Metadata sheet; default `verification_status` to `"reported"` and `data_quality.source` to `"excel-import"` | Supply Chain, Entity ID, ERP Integration |
| 16 | **Support auto-ID generation with stable slugs** when the `id` column is left blank (e.g., `org-bolt-supplies-ltd`) | Data Serialization |
| 17 | **Add `TaxIDCountry` column** adjacent to tax ID columns; validate ISO 3166-1 alpha-2; apply SPEC-005 Section 2.5 country-specific logic | ERP Integration, Entity ID |
| 18 | **Fuzz the Excel reader from day one** with `cargo-fuzz` targeting arbitrary bytes as fake XLSX. Enforce advisory size limits before allocation | Rust Engineer |
| 19 | **Define `SourceSystem` column** on Identifiers sheet mapping to SPEC-005 authority naming convention; default to `excel-manual` | ERP Integration |

### P2 — Future enhancements

| # | Recommendation | Source Expert(s) |
|---|---------------|-----------------|
| 20 | **Support a "Chain View" sheet** for multi-tier entry (columns: Reporting Entity, Tier 1, Tier 2, Tier 3, Commodity) with auto-generated edge chains | Procurement, Supply Chain |
| 21 | **Add a Former Identity / M&A Events sheet** with predecessor/successor/event_type/effective_date columns | Entity ID |
| 22 | **Support CSV directory fallback** for users without Excel | Data Serialization |
| 23 | **Publish a canonical column-name-to-OMTS-field mapping table** versioned alongside the schema | Data Serialization, ERP Integration |
| 24 | **Support EUDR geolocation** with lat/lon columns and external GeoJSON file references | Supply Chain |
| 25 | **Include a "README" sheet** within the workbook with field definitions and instructions | Procurement, Entity ID |
| 26 | **Include optional columns for extension identifier schemes** (org.opencorporates, org.refinitiv.permid) | Entity ID |

---

## Cross-Domain Interactions

1. **Graph structure <-> Procurement usability:** The Graph Theorist's concern about edge property heterogeneity directly conflicts with the Procurement Expert's demand for simplicity. The hybrid solution (edge-category sheets) was endorsed by the Supply Chain Expert and ERP Integration Expert as the pragmatic middle ground.

2. **Identifier model <-> Template design:** The Entity Identification Expert's recommendation for a separate Identifiers sheet was independently endorsed by the Rust Engineer (for parser type safety), the Graph Theorist (for correct graph modeling), and the ERP Integration Expert (for multi-system identifier handling). The consensus is: embed common schemes as direct columns on node sheets for the 80% case, with the Identifiers sheet for the general case. The import command must merge both sources.

3. **Rust implementation <-> Validation strategy:** The Rust Engineer defined clear crate boundaries: Excel I/O in `omtsf-cli` (or a dedicated `omtsf-excel` crate), graph construction in `omtsf-core` as pure functions. The `calamine` crate handles reading; `rust_xlsxwriter` handles template generation. The ERP Integration Expert endorsed this and added that the two-phase validation (structural pre-conversion + OMTS-spec post-conversion) should reuse existing `omtsf-core` validators.

4. **Regulatory compliance <-> Template defaults:** The Supply Chain Expert emphasized that LkSG and CSDDD compliance workflows start with Excel supplier lists. The simplified single-sheet mode directly serves this use case. The Entity Identification Expert added that `former_identity` edges are essential for compliance continuity across M&A events.

5. **Sensitivity model <-> Import defaults:** All panelists agreed that SPEC-004's per-identifier sensitivity is too granular for Excel entry. The ERP Integration Expert flagged the specific compliance risk: failing to set `disclosure_scope` correctly before sharing a file containing VAT or nat-reg identifiers violates L1-SDI-02. The import command should validate this constraint and refuse to produce output when the combination is invalid.

6. **Entity identification <-> Merge viability:** The Entity Identification Expert's most impactful insight: without a same_as sheet and check-digit validation at import time, Excel-imported files will be "disconnected islands" that never merge correctly with anyone else's files. The Rust Engineer endorsed this, noting that the same_as mechanism is the escape valve when identifiers disagree.

7. **ERP authority naming <-> Identifier deduplication:** The ERP Integration Expert and Entity Identification Expert both flagged that inconsistent authority strings (`sap-prod` vs `SAP-PROD` vs `sap_prod`) prevent cross-file deduplication. The template must constrain authority values via dropdowns using the SPEC-005 naming convention.

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

---

### Rust Engineer — Senior Systems Engineer

#### Assessment

From a crate architecture standpoint, the `import-excel` pipeline introduces a new parsing surface that must slot cleanly into the existing workspace without contaminating `omtsf-core`. The correct decomposition is: all Excel I/O belongs in `omtsf-cli` (or a dedicated `omtsf-excel` crate), while the resulting in-memory graph is constructed using the same public types already defined in `omtsf-core`. The parsing logic should produce a typed `ImportResult<OmtsGraph, Vec<ImportDiagnostic>>` -- never a panic, never a `String`-typed error. The Excel dependency (`calamine` for reading) must be confined to the CLI crate and MUST NOT appear in `omtsf-core`'s dependency tree. The `wasm-check` step in `just ci` will catch any accidental bleed immediately.

The hardest parsing safety problem is that Excel files are user-authored, untrusted binary blobs. XLSX is a ZIP archive containing XML. Malformed or adversarially crafted files can trigger zip-bomb payloads, unbounded string allocations on merged cells, and formula injection in string fields. The import pipeline must enforce the advisory size limits from SPEC-001 Section 9.4 at read time -- 1M nodes, 5M edges, 10,000 UTF-8 bytes per string field -- before constructing any in-memory structure.

The two-sheet structure maps cleanly onto the OMTS graph model and gives a deterministic two-pass parse: (1) collect all nodes into a `HashMap<LocalId, NodeRecord>`, then (2) resolve each edge's `source`/`target` columns against that map, returning typed `EdgeRefError`s for dangling references. This mirrors the in-memory validation pipeline already present in `omtsf-core`.

#### Strengths

- The two-sheet (Nodes + Edges) tabular layout naturally separates structural concerns.
- Graph-local IDs as explicit columns allow human-readable slugs that survive row reordering.
- Scheme-qualified identifier sub-columns avoid fragile substring splitting.
- Sensitivity as a dropdown-constrained column enables row-read-time enum validation.
- A dedicated Metadata sheet is the correct analogue to the JSON top-level object.

#### Concerns

- **[Critical]** CSPRNG salt generation: Excel cells cannot produce a cryptographically adequate `file_salt`. The import command must always generate fresh if blank.
- **[Critical]** Multi-identifier encoding: the parser's type model (`Vec<IdentifierRecord>` vs. fixed-size array) depends on the structural choice (separate sheet vs. fixed columns).
- **[Critical]** Edge property encoding is ambiguous without a typed column manifest per edge type.
- **[Major]** `_property_sensitivity` overrides have no obvious Excel representation.
- **[Major]** `person` node handling when `disclosure_scope: "public"` is declared.
- **[Major]** Excel formula cells: `calamine` reads cached values that may be stale.
- **[Minor]** Header row validation is unspecified; misspelled headers cause silent wrong-column mapping.
- **[Minor]** Round-trip precision loss: Excel's 15-digit float limit and date serial numbers.

#### Recommendations

1. (P0) Define Metadata sheet with required columns; import command generates CSPRNG salt if blank.
2. (P0) Commit to separate Identifiers sheet with `node_id`, `scheme`, `value`, `authority`, `sensitivity`, `verification_status`, `valid_from`, `valid_to`.
3. (P0) Publish per-edge-type column manifest; implement as exhaustive match dispatch with `wildcard_enum_match_arm` lint.
4. (P1) Define error handling strategy: parse errors fail fast, L1 failures collect all diagnostics (no output), L2 warnings produce output.
5. (P1) Fuzz the Excel reader from day one with `cargo-fuzz`. Enforce size limits before allocation.
6. (P2) Define explicit `person` node import behavior: refuse output when combined with `disclosure_scope: "public"`.
7. (P2) Document Excel float/date precision constraints; cap ownership percentages at 6 decimal places.

---

### Entity Identification Expert — Corporate Hierarchy Specialist

#### Assessment

I come at this from the hardest end of the problem -- getting two supply chain graphs from two different parties to merge correctly when neither party used the same identifier for the same legal entity. That is the core challenge for any Excel import path, and the identifier strategy for the Excel template will determine whether the resulting .omts files are mergeable in practice or just disconnected islands.

The OMTS specification foundation (SPEC-002) is genuinely well-designed for this problem. The composite identifier model -- treating LEI, DUNS, nat-reg, VAT, GLN, and internal codes as peers in an array -- is exactly right. No single global identifier has sufficient coverage for a realistic supply chain. The check-digit validation rules (MOD 97-10 for LEI, GS1 mod-10 for GLN) and the L1 validation rules (L1-EID-05 through L1-EID-07) are thorough and implementable.

My concern is that the Excel format is the first user-facing surface where most practitioners will enter identifier data, and it has to do two things simultaneously: be simple enough for a procurement officer to fill out without reference documentation, and be expressive enough to capture the multi-identifier, multi-scheme reality that SPEC-002 demands. If this is not resolved thoughtfully, the import command will silently produce files with only internal identifiers, which fail L2-EID-01 and -- more critically -- never merge correctly with anyone else's files.

The corporate hierarchy and M&A dimensions add another layer of complexity. SPEC-001 Sections 5.1 through 5.4 define ownership, legal_parentage, operational_control, and former_identity edges. These are sophisticated relationship types genuinely needed for regulatory compliance. But Excel users default to "flat list of suppliers" thinking, not graph thinking. The template design must guide users toward capturing hierarchy data without requiring them to understand directed labeled property multigraphs.

#### Strengths

- SPEC-002's composite identifier model is the correct foundation.
- The GLEIF RA code requirement for nat-reg authority prevents free-text chaos.
- Check-digit validation rules are specified precisely at L1.
- The `former_identity` edge type with M&A event enums is the right mechanism for identity history.
- The `governance_structure` field captures non-standard corporate forms.
- The `status` field (active, dissolved, merged, suspended) enables data quality warnings during import.
- The sensitivity classification system with per-identifier defaults is well-designed.

#### Concerns

- **[Critical]** Structural mismatch: flat rows vs. multi-identifier array. Fixed columns cannot handle multiple identifiers of the same scheme.
- **[Critical]** No guidance on HQ vs. branch DUNS disambiguation. Branch DUNS on organization nodes violates L3-EID-05.
- **[Critical]** No mechanism for same_as edges in the Excel format. Without it, files are merge-incompatible islands.
- **[Major]** Authority field for nat-reg requires GLEIF RA codes that users will not know.
- **[Major]** No temporal identity columns (valid_from/valid_to on identifiers).
- **[Major]** Ownership percentage format ambiguity (integer vs. float vs. percent-sign).
- **[Major]** VAT authority disambiguation is country-specific and error-prone.
- **[Minor]** No provision for OpenCorporates or Refinitiv PermID extension schemes.
- **[Minor]** `verification_status` has no Excel analog; default to "reported".
- **[Minor]** Confidentiality column scope is ambiguous (node-level vs. identifier-level).

#### Recommendations

1. (P0) Design a separate Identifiers sheet as the primary vehicle for external IDs.
2. (P0) Enforce blocking warning when an org node has zero non-internal identifiers (L2-EID-01).
3. (P0) Implement LEI MOD 97-10 and GLN mod-10 check digit validation at import time.
4. (P0) Add a Corporate Hierarchy sheet for ownership/legal_parentage/operational_control edges.
5. (P0) Add a Former Identity / M&A Events sheet for former_identity edges.
6. (P1) Bundle a GLEIF RA code lookup; accept both RA codes and plain-English names.
7. (P1) Add a Same As / Entity Deduplication sheet.
8. (P1) Default all imported identifiers to `verification_status: "reported"` and `data_quality.source: "excel-import"`.
9. (P1) Add a Tax ID Type dropdown; apply country-specific logic from SPEC-005 Section 2.5.
10. (P1) Add HQ/Branch indicator on DUNS identifier rows; route branch DUNS to facility nodes.
11. (P2) Include optional columns for OpenCorporates and Refinitiv PermID extension schemes.
12. (P2) Generate `previous_snapshot_ref` when importing into an existing .omts file.

---

### ERP Integration Expert — Enterprise Systems Architect

#### Assessment

From my position having spent two decades in enterprise systems integration -- deploying SAP, Oracle, and Dynamics instances for mid-market and global manufacturers -- the Excel bridge use case is entirely credible and will be the primary onboarding path for the first three to five years of OMTS adoption. Most companies do not have a clean ERP-to-anything export pipeline. They have Excel trackers maintained by a procurement analyst, a Coupa-generated supplier list, and SharePoint folders of supplier self-disclosure forms. An `import-excel` command that meets these people where they are is pragmatically correct.

The critical design challenge is that Excel forces a tabular, denormalized representation onto a graph that is inherently relational and multi-valued. The OMTS composite identifier scheme (SPEC-002 Section 3) -- where a single organization node can carry LEI, DUNS, VAT, nat-reg, and multiple internal identifiers simultaneously -- does not decompose naturally into one row per entity. Similarly, a multigraph where multiple `supplies` edges can exist between the same pair of nodes requires careful thought about how rows map to edges.

The selective disclosure dimension is particularly tricky for Excel. Unlike JSON, Excel has no field-level sensitivity annotation mechanism. The import command must take ownership of that translation, and the column layout must provide enough metadata for it to do so.

#### Strengths

- SPEC-005 ERP field mappings provide a concrete prior art base for Excel column layout.
- The `internal` scheme is first-class by design, lowering the barrier to entry.
- The authority naming convention (`{system_type}-{instance_id}`) makes source system traceability explicit.
- The enrichment lifecycle model (SPEC-005 Section 6) positions Level 1 Excel import as the starting point of an additive workflow.

#### Concerns

- **[Critical]** Multi-valued identifier columns will be mishandled without an explicit design decision on separate sheet vs. fixed columns.
- **[Critical]** Edge property representation for `supplies` and related types is structurally ambiguous without a dedicated Edges sheet that preserves the multigraph property.
- **[Major]** Disclosure scope and sensitivity handling require explicit UX conventions; failing to set `disclosure_scope` correctly violates L1-SDI-02.
- **[Major]** SAP STCD1/STCD2 country-dependent tax ID disambiguation has a direct Excel analog. Users will enter tax IDs without country context.
- **[Major]** Multi-tier relationship depth requires explicit tier annotation on edges.
- **[Minor]** LEI check digit validation will catch transcription errors; error messages must reference specific row/column.
- **[Minor]** Person node GDPR constraints with `disclosure_scope: "public"` need explicit warning behavior.

#### Recommendations

1. (P0) Define a four-sheet template: Config, Nodes, Identifiers, Edges. Config holds file-level fields; Identifiers has one row per identifier record with foreign key to node ID.
2. (P0) Implement two-phase validation: structural (pre-conversion) and OMTS-spec (post-conversion). Report errors with sheet/row/column.
3. (P1) Require `TaxIDCountry` column adjacent to tax ID columns; validate ISO 3166-1 alpha-2.
4. (P1) Define `SourceSystem` column mapping to SPEC-005 authority naming; default to `excel-manual`.
5. (P1) Require `Tier` column on Edges for supplies/subcontracts/tolls/distributes; optionally offer `--infer-tiers` flag.
6. (P2) Provide .xlsx template with data validation dropdowns for node type, edge type, scheme, sensitivity.
7. (P2) Document person node + public disclosure_scope as an explicit warning condition, not silent row-drop.
