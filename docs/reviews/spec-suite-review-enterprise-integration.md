# Enterprise Integration Expert Review: OMTS Spec Suite (Post-Panel Revision)

**Reviewer:** Enterprise Integration Expert, Enterprise Systems Architect
**Date:** 2026-02-18
**Specs Reviewed:** OMTS-SPEC-001 through OMTS-SPEC-006 (all six specifications, post-panel revision)
**Review Focus:** ERP export/import feasibility, master data alignment, EDI coexistence, data quality realities, batch vs. incremental update support
**Prior Review:** Pre-panel review (same date); this review assesses changes made in response to the panel report.

---

## Assessment

The spec suite has undergone a significant maturation pass since the panel report. My two Critical findings -- the missing SAP Business Partner model mapping (C19) and the missing delta/patch mechanism (C18) -- received different treatment. The SAP Business Partner mapping (BUT000/BUT0ID) has been added to SPEC-005 Section 2.4 with correct IDTYPE-to-OMTS-scheme mappings (DUNS to `duns`, LEI to `lei`, HRNR to `nat-reg`, UST1/UID to `vat`), and the `INSTITUTE` field is correctly mapped to the `authority` field for jurisdictional schemes. This was my P0-2 recommendation and it is resolved: SPEC-005 now covers both legacy LFA1 vendor master and greenfield S/4HANA Business Partner implementations. The delta/patch mechanism (my P0-1) has been deferred to P2-27 in the panel report -- a scope decision I disagree with but understand given the panel's prioritization of model correctness over operational concerns. I will re-state why this remains a deployment blocker in the Concerns section.

The panel's other resolutions directly benefit enterprise integration. The `composed_of` edge type (SPEC-001 Section 6.8) now exists with correct direction convention (source = parent good, target = component) and explicit mapping to SAP STPO/STKO and Oracle BOM_STRUCTURES_B -- this was my P1-4 and cross-domain interaction #2 with Regulatory Compliance and Supply Chain. The EDI coexistence statement (SPEC-005 Section 6) correctly positions OMTS as complementary to EDI rather than competing, and even includes a PEPPOL Participant Identifier example as an extension scheme. The STCD1/STCD2 disambiguation table (SPEC-005 Section 2.5) addresses my Minor concern about country-specific tax number mapping with the correct guidance that scheme assignment depends on `LAND1`, not field position. The edge property serialization ambiguity (C2) is resolved: the `"properties"` wrapper is now normative per Section 2.1 of SPEC-001, with all edge property tables documented as logical properties nested inside `properties`. This was the correct choice for ERP integration because it cleanly separates structural fields (`id`, `type`, `source`, `target`) from domain data, matching how ERP extractors naturally construct edge objects.

The addition of `consignment` nodes (SPEC-001 Section 4.6), `sells_to` edges (Section 6.9), `data_quality` metadata (Section 8.3), `verification_status` and `verification_date` on identifier records (SPEC-002 Section 3), and the file integrity mechanism (SPEC-004 Section 6) collectively address concerns from multiple panelists that also affected enterprise integration. The `data_quality.confidence` enum (`verified`, `reported`, `inferred`, `estimated`) maps directly to how ERP data quality is actually categorized in MDM programs. The `sells_to` edge means SPEC-005 will eventually need SAP SD and Oracle Order Management mappings alongside the current procurement-side mappings -- a concern I flagged in cross-domain interaction #10 with Regulatory Compliance.

---

## Strengths

- **SAP Business Partner model now fully mapped.** SPEC-005 Section 2.4 covers BUT000 entity header, BUT0ID identification numbers with explicit IDTYPE-to-scheme mapping, and BP Category (TYPE=1 for organization, TYPE=2 for person). The `-bp` authority suffix convention (`sap-prod-bp`) correctly distinguishes BP namespace from legacy LIFNR namespace in migrated systems.
- **`composed_of` edge enables BOM representation.** The direction convention (source = parent, target = component) and optional `quantity`/`unit` properties map cleanly to SAP BOM item table (STPO) quantity fields and Oracle BOM_STRUCTURES_B component quantities. This unblocks material traceability for EUDR and embedded emissions for CBAM.
- **EDI coexistence statement is accurate and pragmatic.** SPEC-005 Section 6 correctly states that OMTS captures structural topology while EDI handles transactional document flow. The PEPPOL Participant Identifier example (`org.peppol.participant`) demonstrates how EDI-world identifiers integrate without collision.
- **STCD1/STCD2 disambiguation table addresses country-specific mapping.** Section 2.5 covers the four most common country scenarios (DE, US, BR, GB) with correct field-to-scheme assignments. The "when in doubt, map to `internal`" guidance is operationally sound -- it prevents incorrect scheme assignment at the cost of reduced merge capability, which is the right tradeoff for messy ERP data.
- **Edge property wrapper resolution is correct.** The `"properties"` nesting convention in SPEC-001 Section 2.1 matches how ERP extractors naturally construct JSON: structural fields at the top level, domain properties in a sub-object. This is implementable in ABAP, PL/SQL, and X++ without awkward flattening.
- **`data_quality` metadata on nodes and edges.** The `confidence` enum and `source` provenance string (SPEC-001 Section 8.3) directly address the MDM maturity signal: a GLEIF-verified LEI is `verified` with `source: "gleif-api"`, while a supplier questionnaire DUNS is `reported` with `source: "supplier-questionnaire"`.
- **`verification_status` on identifier records.** SPEC-002 Section 3 now carries `verified`/`reported`/`inferred`/`unverified` on individual identifiers, resolving the asymmetry with `same_as` confidence that three panelists flagged.
- **Consignment node type enables lot-level traceability.** SPEC-001 Section 4.6 with `lot_id`, `quantity`, `unit`, and `production_date` maps to SAP batch management (MCHA/MCH1) and Oracle lot structures.

---

## Concerns

- **[Critical] Delta/patch mechanism remains absent.** The panel deferred this to P2-27. I re-state: full-file re-export is infeasible at enterprise scale. A manufacturer with 40,000 vendors, 200,000 purchasing info records, and 50,000 BOM items cannot regenerate a complete `.omts` file on every vendor master change. SAP change document tables (CDHDR/CDPOS), Oracle audit columns (`LAST_UPDATE_DATE`), and D365 change tracking all produce incremental deltas natively. Without a delta envelope, every OMTS integration will require a custom reconciliation layer on top, negating the standardization benefit. The longer this is deferred, the more proprietary delta formats will emerge in the ecosystem. This is my single remaining P0 recommendation.
- **[Major] Oracle SCM Cloud and D365 mappings remain at field-name level.** SPEC-005 Sections 3 and 4 still list field names without REST API endpoints, OData entity names, or pagination patterns. An Oracle implementer needs to know they are hitting `/fscmRestApi/resources/11.13.18.05/suppliers` (version-qualified) and that the Suppliers REST API has a 500-record per-call limit requiring offset-based pagination. For D365, they need the `VendorV2` OData entity path and the DMF (Data Management Framework) pattern for bulk extraction. The SAP mapping quality is now production-grade; Oracle and D365 are still at whiteboard level. Oracle specifically recommends BICC (Business Intelligence Cloud Connector) over REST API for bulk data extraction -- this operational guidance is absent.
- **[Major] `authority` naming convention for `internal` identifiers remains informal.** SPEC-005 Section 1.1 recommends `{system_type}-{instance_id}[-{client}]` but this is a recommendation in an informative document, not an L2 validation rule. In practice, if one extractor uses `sap-prod` and another uses `sap-s4h-prd-100` for the same system, provenance tracking and intra-file deduplication are undermined. I recommended L2 validation enforcement; the panel recommended P2-25 formalization. At minimum, the recommended vocabulary (`sap`, `oracle-scm`, `d365`, `ariba`) should be stated as SHOULD-level guidance in SPEC-002 rather than buried in an informative guide.
- **[Major] No mapping from SAP purchasing organization (EKORG) or company code (BUKRS) to graph structure.** In multi-org SAP deployments, a single vendor (`LIFNR`) may have different purchasing data (info records, blocked status, payment terms) across purchasing organizations. SPEC-005 mentions `EKORG` only as "context for `internal` authority scoping" without specifying whether purchasing-org-specific data maps to separate edges, edge properties, or is discarded. For a German automotive OEM with 15+ purchasing organizations, this ambiguity matters.
- **[Major] STCD1/STCD2 disambiguation table is incomplete.** Section 2.5 covers DE, US, BR, and GB but omits high-volume SAP deployment countries: India (GSTIN in STCD3/STCD4), China (Unified Social Credit Code), Mexico (RFC), and Italy (Codice Fiscale in STCD1 vs. Partita IVA in STCEG). SAP supports STCD3 and STCD4 fields that are not mentioned at all in SPEC-005. The table also does not reference SAP's T005-TAXBS configuration or KBA 2865204, which is the authoritative source for country-specific tax field mapping.
- **[Minor] No guidance on ERP extraction scheduling or triggering.** SPEC-005 describes the enrichment lifecycle but not when or how extraction occurs. SAP offers change pointers (BDCP/BDCPS for ALE), change documents (CDHDR/CDPOS), and Business Workflow events. Oracle provides Business Events and Scheduled Processes. D365 has Data Management Framework (DMF) recurring jobs and Business Events. This operational guidance belongs in the informative SPEC-005.
- **[Minor] `sells_to` edge has no ERP mapping in SPEC-005.** The edge type was added per Regulatory Compliance recommendation for CSDDD downstream due diligence, but SPEC-005 contains no guidance on how to derive `sells_to` edges from SAP SD (VBAK/VBAP sales orders, KNA1 customer master), Oracle Order Management, or D365 sales data entities. This is expected for an initial release but should be flagged for SPEC-005 revision.

---

## Recommendations

1. **(P0) Define a delta/patch envelope specification.** This remains the single highest-priority gap for enterprise adoption. Define a `"update_type": "delta"` file variant with an operations array supporting `add`, `modify`, and `remove` operations on nodes and edges. Reference entities by external identifiers for cross-file operations. Require delta files to inherit `disclosure_scope` from the base snapshot. Per cross-domain interaction #9 with Security & Privacy, delta files should default to `restricted` sensitivity.

2. **(P1) Expand Oracle SCM Cloud mapping to REST API level.** Reference the Suppliers resource path (`/fscmRestApi/resources/{version}/suppliers`), child resources for sites and contacts, pagination guidance (500-record limit, offset parameter), and the BICC alternative for bulk extraction. Similarly for D365: reference the `VendorV2` OData entity, `DirPartyTable` joins, and DMF batch extraction pattern.

3. **(P1) Expand STCD1/STCD2 disambiguation to cover STCD3/STCD4 and additional countries.** Add India (GSTIN in STCD3, PAN in STCD1), China (USCC in STCD1), Mexico (RFC in STCD1), and Italy (Codice Fiscale in STCD1, Partita IVA in STCEG). Reference SAP table T005-TAXBS and KBA 2865204. Rename the section to "Tax Number Field Disambiguation" to reflect that it covers all four tax number fields plus STCEG.

4. **(P1) Clarify purchasing organization and company code mapping.** Add guidance to SPEC-005 Section 2.2 on how multi-purchasing-org data maps to the graph. Recommended approach: one `supplies` edge per vendor-purchasing-org-material combination, with `contract_ref` or a custom property carrying the purchasing org context. Alternative: aggregate to vendor-company-code level for organizations that prefer simpler graphs.

5. **(P1) Promote `authority` naming convention to SPEC-002 as SHOULD-level guidance.** Move the `{system_type}-{instance_id}[-{client}]` pattern and recommended vocabulary from SPEC-005 to SPEC-002 Section 5.1 (`internal` scheme definition) as a SHOULD-level recommendation. This ensures that producers encounter it in the normative spec, not only in the informative guide.

6. **(P2) Add `sells_to` edge ERP mapping to SPEC-005.** Map to SAP SD tables (VBAK header, VBAP items, KNA1 customer master) and Oracle Order Management REST APIs. This will be needed when organizations begin modeling downstream supply chains for CSDDD Article 8(2) compliance.

7. **(P2) Add extraction scheduling guidance to SPEC-005.** Document event-driven extraction (SAP CDHDR/CDPOS, Oracle Business Events, D365 Business Events), scheduled batch extraction (SAP ABAP report / CDS view extraction, Oracle BICC/BI Publisher, D365 DMF recurring jobs), and on-demand extraction patterns.

---

## Cross-Expert Notes

- **To Security & Privacy Expert:** The delta/patch question remains unresolved, and I want to reinforce cross-domain interaction #9: delta files are categorically more intelligence-dense than snapshots. When the delta specification is eventually defined, it must inherit `disclosure_scope`, and I recommend `restricted` as the floor for any delta file containing node additions or removals. Edge-only deltas (updating properties on existing relationships) may be less sensitive.

- **To Regulatory Compliance Expert:** The `sells_to` edge is now in SPEC-001 but has no ERP extraction path in SPEC-005. When you draft CSDDD downstream due diligence guidance, please coordinate on which SAP SD, Oracle OM, and D365 sales entities should be mapped. The sales-side data model is structurally different from procurement (customer master vs. vendor master, sales orders vs. purchase orders) and will need its own field-level mapping table.

- **To Standards Expert:** The STCD1/STCD2 disambiguation I am requesting for additional countries (India GSTIN, China USCC, Mexico RFC) intersects with your ISO 6523 ICD mapping work. India's GSTIN has no standard ICD assignment; China's USCC has one (`0200`). If these identifiers will be represented as `nat-reg` scheme identifiers, we need corresponding GLEIF RA codes or a documented fallback.

- **To Procurement Expert:** The `authority` naming convention still needs formalization. If your organization runs SAP for direct materials and Oracle for indirect, deduplication depends on parseable authority strings. The current informative guidance in SPEC-005 is insufficient; it needs to live in SPEC-002 where every extractor implementer will encounter it. I recommend we jointly propose the vocabulary list for TSC review.

- **To Graph Modeling Expert:** The `composed_of` edge now exists with the direction convention we discussed (source = parent, target = component). One implementation note: SAP BOM items (STPO) carry phantom assembly flags and alternative items. Phantom assemblies should be flattened during extraction (their components become direct children of the parent). Alternative items should be represented as parallel `composed_of` edges to the same parent. I recommend we document these extraction patterns in SPEC-005 alongside the BOM mapping.

- **To Supply Chain Expert:** Your request for volume/value on supply edges is now addressed (`volume`, `volume_unit`, `annual_value`, `value_currency` on `supplies` edges per SPEC-001 Section 6.1). These map directly to SAP purchasing info record fields (EINE-APLFZ for planned delivery time can inform volume estimates, EINE-NETPR for net price). I recommend the extraction guidance in SPEC-005 map specific SAP/Oracle/D365 fields to these new edge properties.

Sources:
- [SAP Business Partner FAQ: Tax](https://community.sap.com/t5/enterprise-resource-planning-blog-posts-by-sap/faq-business-partner-tax/ba-p/13604725)
- [SAP Tax Number Category Mapping (KBA 3096751)](https://userapps.support.sap.com/sap/support/knowledge/en/3096751)
- [Oracle SCM REST API Documentation](https://docs.oracle.com/en/cloud/saas/supply-chain-and-manufacturing/25d/faips/scm-rest-services.html)
- [Oracle Data Extraction Guidelines](https://www.ateam-oracle.com/data-extraction-options-and-guidelines-for-oracle-fusion-applications-suite)
- [D365 OData Integration](https://learn.microsoft.com/en-us/dynamics365/fin-ops-core/dev-itpro/data-entities/odata)
- [D365 Data Integration APIs](https://learn.microsoft.com/en-us/dynamics365/supply-chain/procurement/contract-lifecycle-management/developer/clm-data-integration-apis)
- [SAP S/4HANA Business Partner CVI](https://community.sap.com/t5/enterprise-resource-planning-blog-posts-by-sap/s-4hana-business-partner-customer-vendor-integration/bc-p/13346111)
- [SAP Italy STCD1/STCD2 Usage (KBA 2562174)](https://userapps.support.sap.com/sap/support/knowledge/en/2562174)
