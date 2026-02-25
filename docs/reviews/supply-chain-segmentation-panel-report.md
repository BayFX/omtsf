# Expert Panel Report: Supply Chain Segmentation Support in OMTS

**Date:** 2026-02-18
**Panel Chair:** Expert Panel Review Process
**Topic:** Whether the OMTS graph data model supports typical supply chain segmentation patterns (tier, geography, risk level, commodity type, business unit, compliance status) or whether an additional concept like tags, labels, or classification properties is required.

---

## Panel Chair Summary

All five experts independently reached the same core conclusion: **the OMTS graph data model lacks a general-purpose classification mechanism on nodes and edges, and one is required before v1.** The model embeds several segmentation dimensions directly into its type system -- `tier` on `supplies` edges, `jurisdiction` on organizations, `geo` on facilities, `commodity` on supply edges, and risk severity/likelihood on attestation nodes -- but these fixed properties cover only a subset of how customers actually segment their supply chains. The dimensions that are missing -- Kraljic quadrant, business unit assignment, regulatory scope, compliance status, supplier diversity, approval status -- are the ones that drive day-to-day procurement operations and regulatory reporting workflows.

The panel converged strongly on the need for a lightweight, namespaced annotation mechanism (variously called "tags", "labels", "classifications", or "annotations" by different panelists). All five experts explicitly recommended this as a P0 or P1 addition. The Graph Modeling Expert made the most architecturally precise proposal: distinguish between **labels** (set-membership flags for fast graph traversal filtering, aligned with the GQL/ISO 39075 property graph model) and **annotations** (key-value pairs for structured classifications like Kraljic scores or risk tiers). The Supply Chain Expert and Procurement Expert converged on a `{key, value}` pair model with namespacing. The Enterprise Integration Expert grounded the need in specific ERP fields (`VendorGroupId`, `EKORG`, `ProcurementBUId`) that SPEC-005 acknowledges but cannot map to any OMTS property.

A strong secondary consensus emerged around the **reporting entity gap**: the `tier` property on `supplies` edges is defined as "relative to the reporting entity" but no file header field identifies which entity the tiers are relative to. Three experts (Supply Chain, Graph Modeling, Regulatory Compliance) independently flagged this as a P0 issue that makes `tier` values semantically incomplete after merge. A third consensus finding concerns the **`tier` property scope**: it exists only on `supplies` edges, but LkSG and CSDDD due diligence obligations apply equally to subcontracting, tolling, and brokering relationships. Two experts (Procurement, Regulatory Compliance) recommended extending `tier` to all supply relationship edge types.

There was no disagreement between panelists on the diagnosis. The variation was in the proposed solution design -- whether to use a single `tags` array, separate `labels` and `annotations`, or a `classifications` array modeled after the `identifiers` pattern. These are design tradeoffs that should be resolved during implementation, not fundamental disagreements.

---

## Panel Composition

| Panelist | Role | Key Focus Area |
|----------|------|---------------|
| Supply Chain Expert | Supply Chain Visibility & Risk Analyst | Real-world segmentation workflows, regulatory risk analysis, Kraljic matrix |
| Procurement Expert | Chief Procurement Officer | ERP segmentation fields, UNSPSC taxonomy, supplier lifecycle status |
| Graph Modeling Expert | Graph Data Modeling & Algorithm Specialist | GQL label model alignment, traversal efficiency, merge semantics |
| Enterprise Integration Expert | Enterprise Systems Architect | ERP export mapping, VendorGroupId/EKORG/ProcurementBUId gaps |
| Regulatory Compliance Expert | Supply Chain Regulatory Compliance Advisor | LkSG/CSDDD/EUDR/UFLPA classification requirements |

---

## Consensus Findings

### 1. A general-purpose classification mechanism is required (5/5 experts)

Every panelist independently concluded that the current model cannot carry the multi-dimensional classifications that customers use to segment their supply chains. The extension mechanism (custom node/edge types via reverse-domain notation) is too heavyweight for simple label/tag use cases. Without a standard mechanism, every adopter will invent incompatible custom properties, fracturing interoperability on the most operationally important data dimensions.

### 2. The reporting entity is not identified in the file header (3/5 experts)

The Supply Chain Expert, Graph Modeling Expert, and Regulatory Compliance Expert all flagged that the `tier` property is semantically incomplete without a `reporting_entity` or `producer` field in the file header. After merge, `tier` values from different perspectives become uninterpretable.

### 3. Business unit / organizational scope is absent (3/5 experts)

The Supply Chain Expert, Enterprise Integration Expert, and Regulatory Compliance Expert identified that multi-division enterprises classify the same supplier differently by business unit (SAP purchasing org, Oracle procurement BU, D365 legal entity). The model has no way to associate a supply relationship or classification with a particular organizational context.

### 4. The `tier` property should be on all supply relationship edge types (2/5 experts)

The Procurement Expert and Regulatory Compliance Expert noted that `tier` exists only on `supplies` edges, but regulatory frameworks (LkSG, CSDDD) apply due diligence obligations to all forms of supply chain relationships including subcontracting, tolling, and brokering.

---

## Critical Issues

No expert rated any concern as **[Critical]** (blocking viability). The model's existing properties and extension mechanism provide enough coverage to avoid complete blockers, but the [Major] issues below significantly impact adoption quality.

---

## Major Issues

### M1. No general-purpose classification/tagging mechanism
**Flagged by:** All 5 experts
**Severity:** Major (unanimous)

The model has no standard property for attaching classification labels (Kraljic quadrant, approval status, diversity classification, regulatory scope, business unit, risk tier) to nodes or edges. The extension mechanism covers custom types, not custom properties on core types. The `data_quality` metadata object is the closest analog but serves a different purpose. Every adopter will independently invent incompatible property conventions.

### M2. No reporting entity in the file header
**Flagged by:** Supply Chain Expert, Graph Modeling Expert, Regulatory Compliance Expert
**Severity:** Major

The `tier` property on `supplies` edges is defined as "relative to the reporting entity" but no field identifies which entity the tiers are relative to. After merge (SPEC-003), tier values from different perspectives become meaningless.

### M3. Business unit / buying entity segmentation absent
**Flagged by:** Supply Chain Expert, Enterprise Integration Expert, Regulatory Compliance Expert
**Severity:** Major

Multi-division enterprises classify the same supplier differently by division. SAP purchasing organizations (`EKORG`), Oracle procurement BUs (`ProcurementBUId`), and D365 legal entities all represent the buying side's organizational segmentation. The `supplies` edge connects two organization nodes but does not indicate which part of the buyer's organization established the relationship.

### M4. SPEC-005 acknowledges ERP segmentation fields but does not map them
**Flagged by:** Enterprise Integration Expert, Procurement Expert
**Severity:** Major

SPEC-005 Section 4.1 notes D365 `VendorGroupId` as "useful for segmenting supplier types during export" and mentions SAP `EKORG` and Oracle `ProcurementBUId`, but provides no OMTS property to carry them.

### M5. Commodity classification limited to HS codes
**Flagged by:** Procurement Expert
**Severity:** Major

HS codes cover physical goods but not services, which represent 30-50% of typical procurement spend. UNSPSC (used in PEPPOL and most procurement platforms) covers both goods and services. The `commodity` field is underspecified for a large portion of procurement activity.

### M6. `tier` property missing from non-`supplies` edge types
**Flagged by:** Procurement Expert, Regulatory Compliance Expert
**Severity:** Major (Procurement: Minor, Regulatory: Major)

LkSG Section 2(7) explicitly includes subcontracting as a supply chain relationship subject to due diligence. CSDDD applies to the full "chain of activities." The `tier` property exists only on `supplies` edges, not on `subcontracts`, `tolls`, `distributes`, or `brokers`.

### M7. Unknown property preservation without namespacing creates collision risk
**Flagged by:** Graph Modeling Expert
**Severity:** Major

The forward-compatibility rule (SPEC-001, Section 2.2) preserves unknown fields during round-trip, but two independent producers who both add a custom `risk_category` property will create collisions during merge. The extension mechanism's reverse-domain notation addresses type names but not property names.

### M8. Attestation indirection impedes segmentation queries
**Flagged by:** Graph Modeling Expert, Supply Chain Expert
**Severity:** Major (Graph) / Minor (Supply Chain)

Filtering by compliance status or risk level requires traversing `attested_by` edges to `attestation` nodes, adding O(E) overhead to what should be O(1) label-scan operations. This conflates risk assessments with simple classification labels.

---

## Minor Issues

| # | Issue | Flagged By |
|---|-------|-----------|
| m1 | No controlled vocabulary declarations for free-text fields (`scope`, `commodity`) | Supply Chain Expert, Graph Modeling Expert, Regulatory Compliance Expert |
| m2 | No aggregate risk profile on organization/facility nodes (risk lives only on individual attestation nodes) | Procurement Expert |
| m3 | No supplier relationship status concept (approved/blocked/phase-out) distinct from legal entity status | Procurement Expert, Enterprise Integration Expert |
| m4 | No multi-valued or hierarchical commodity classification support | Enterprise Integration Expert |
| m5 | EUDR commodity-level classification requires consumer-side HS-to-EUDR mapping | Regulatory Compliance Expert |
| m6 | No `facility_type` enum for smelter/refinery/mine distinction (Conflict Minerals Regulation) | Regulatory Compliance Expert |

---

## Consolidated Recommendations

### P0 -- Immediate

| # | Recommendation | Originating Expert(s) |
|---|---------------|----------------------|
| R1 | **Add a general-purpose classification mechanism to all nodes and edges.** Design options: (a) a `tags` array of `{key, value}` pairs with namespaced keys (Supply Chain Expert), (b) separate `labels` (set-membership flags) and `annotations` (key-value pairs) arrays aligned with GQL (Graph Modeling Expert), or (c) a `classifications` array modeled after `identifiers` with `{taxonomy, code, label}` entries (Procurement Expert). All proposals use reverse-domain notation for namespacing. Must NOT participate in merge identity predicates (SPEC-003). Merge behavior: set union with provenance. | All 5 experts |
| R2 | **Add a `reporting_entity` or `producer` field to the file header** identifying the organization whose perspective the file represents. At minimum: `{entity_id, name}` where `entity_id` references a graph-local node ID. Resolves `tier` ambiguity and anchors perspective-dependent classifications. | Supply Chain Expert, Graph Modeling Expert, Regulatory Compliance Expert |

### P1 -- Before v1

| # | Recommendation | Originating Expert(s) |
|---|---------------|----------------------|
| R3 | **Define recommended classification keys/taxonomies** in an informative appendix or SPEC-006. Cover: Kraljic quadrant, regulatory scope (LkSG, CSDDD, EUDR, UFLPA, CBAM), compliance status, UNSPSC commodity codes, supplier diversity. Keep recommended, not required. | Supply Chain Expert, Procurement Expert, Graph Modeling Expert |
| R4 | **Map ERP segmentation fields to the classification mechanism in SPEC-005.** Provide concrete JSON examples for SAP vendor groups, Oracle DFFs, D365 `VendorGroupId`, SAP `EKORG`, Oracle `ProcurementBUId`. | Enterprise Integration Expert, Procurement Expert |
| R5 | **Extend the `tier` property to `subcontracts`, `tolls`, `distributes`, and `brokers` edge types.** LkSG and CSDDD do not distinguish between supply relationship types when counting tiers. | Procurement Expert, Regulatory Compliance Expert |
| R6 | **Expand commodity classification to support UNSPSC** via a `commodity_scheme` property on supply edges or a prefix convention (e.g., `unspsc:72101500`). Services procurement is too large to leave unaddressable. | Procurement Expert |
| R7 | **Define merge semantics for classifications in SPEC-003.** Labels merge by set union. Key-value classifications merge by key with conflict recording following the existing `_conflicts` pattern. Namespace collisions prevented by reverse-domain convention. | Graph Modeling Expert, Supply Chain Expert |
| R8 | **Add a `buying_org` property to `supplies` and `subcontracts` edges** for multi-org purchasing context. Maps directly to SAP `EKORG`, Oracle `ProcurementBUId`, D365 legal entity. | Enterprise Integration Expert |
| R9 | **Add a `regulatory_context` array to the file header** declaring which regulatory programs the file supports (e.g., `eu-csddd`, `de-lksg`, `eu-eudr`). Enables context-aware validation. | Supply Chain Expert |

### P2 -- Future

| # | Recommendation | Originating Expert(s) |
|---|---------------|----------------------|
| R10 | **Add a `supplier_status` property on supply relationship edges** with values like `active`, `blocked`, `conditional`, `pending_approval`, `phase_out`. Distinct from organization legal status. | Enterprise Integration Expert, Procurement Expert |
| R11 | **Add a `vocabulary` declaration to the file header** allowing producers to declare that specific fields use a controlled vocabulary. Supports automated validation and graph database index optimization. | Graph Modeling Expert, Supply Chain Expert |
| R12 | **Add a `facility_type` enum to facility nodes** (e.g., `factory`, `warehouse`, `farm`, `mine`, `smelter`, `refinery`, `port`, `office`). Supports Conflict Minerals Regulation smelter/refiner identification and CBAM installation classification. | Regulatory Compliance Expert |
| R13 | **Support multi-valued commodity classification** either by changing `commodity` to an array or documenting that parallel edges should be used. | Enterprise Integration Expert |

---

## Cross-Domain Interactions

### Classification mechanism design affects graph traversal performance (Graph <-> All)
The Graph Modeling Expert's distinction between labels (O(1) set-membership checks) and annotations (O(log n) key-value lookups) has direct implications for how procurement teams filter suppliers and how compliance teams run regulatory screening queries. The classification mechanism design should be evaluated against Neo4j/GQL label indexing patterns to ensure it supports efficient graph database loading.

### ERP segmentation fields need a classification target (ERP Integration <-> Procurement)
The Enterprise Integration Expert and Procurement Expert converged on the same gap from different angles: ERP systems universally provide supplier classification fields (SAP vendor groups, Oracle DFFs, D365 VendorGroupId), and SPEC-005 acknowledges their existence but has nowhere to map them. Solving R1 (classification mechanism) directly unblocks R4 (ERP mapping).

### Regulatory classifications are not attestations (Regulatory <-> Graph)
The Regulatory Compliance Expert and Graph Modeling Expert both noted that conflating simple regulatory status labels (e.g., "LkSG priority supplier") with attestation nodes (which require `attestation_type`, `valid_from`, etc.) is both semantically incorrect and computationally expensive. A lightweight label mechanism would properly separate classification metadata from audit/certification records.

### Reporting entity anchors perspective-dependent data (Supply Chain <-> Graph <-> Regulatory)
Three experts independently identified that `tier` values, business unit classifications, and regulatory scope assignments are all perspective-dependent -- they are meaningful only relative to a specific reporting entity. The `reporting_entity` field (R2) is the common prerequisite for all perspective-dependent segmentation features.

### Commodity taxonomy gap spans procurement and regulatory domains (Procurement <-> Regulatory)
The Procurement Expert's UNSPSC recommendation and the Regulatory Compliance Expert's EUDR commodity classification concern both point to the same underlying gap: the `commodity` field is a single string with no declared taxonomy. Solving R6 (commodity scheme support) addresses both the services procurement gap and the regulatory commodity filtering need.

---

## Individual Expert Reports

### Supply Chain Expert (Supply Chain Visibility & Risk Analyst)

**Assessment:** The model covers some segmentation dimensions well (tier, geography, commodity, risk via attestation) but lacks a general-purpose classification mechanism. Fixed spec-defined properties cover "textbook" segmentation axes but miss organization-specific, evolving classifications -- Kraljic quadrants, business unit assignments, compliance statuses, regulatory prioritization categories.

**Key Concerns:**
- [Major] No general-purpose tags/labels mechanism -- organizations will independently invent incompatible property conventions
- [Major] No business unit or organizational scope association
- [Major] Reporting entity not identified in file header, making `tier` values ambiguous after merge

**Top Recommendations:**
- (P0) Add optional `tags` array of namespaced `{key, value}` pairs to all nodes and edges
- (P0) Add `producer` object to file header identifying reporting entity
- (P1) Define recommended tag keys for common segmentation dimensions
- (P1) Add `regulatory_context` array to file header

**Sources cited:** CIPS Kraljic Matrix, LkSG Risk Analysis Guidance (Envoria), German Supply Chain Act FAQ, CSDDD Update (Fieldfisher), Gartner Supply Chain Segmentation

---

### Procurement Expert (Chief Procurement Officer)

**Assessment:** The model works well for the structural graph (who supplies whom, who owns whom) but does not support the operational classification layer that procurement teams need. SAP S/4HANA, Coupa, and Jaggaer all provide multi-dimensional supplier classification as core functionality. Without a standard classification mechanism, every company's OMTS export will be incompatible with another's procurement analytics.

**Key Concerns:**
- [Major] No standard classification/tagging mechanism for Kraljic, approval status, diversity, business unit
- [Major] Commodity classification limited to HS codes -- services (30-50% of spend) unaddressable
- [Minor] `tier` missing from `subcontracts`, `tolls`, `brokers` edges
- [Minor] No aggregate risk profile at supplier/facility level
- [Minor] No supplier relationship status concept (approved/blocked/phase-out)

**Top Recommendations:**
- (P0) Add `classifications` array to all node types (modeled after `identifiers` with `taxonomy`, `code`, `label`)
- (P1) Expand commodity support to include UNSPSC
- (P1) Add `tier` to all supply relationship edge types
- (P2) Define recommended vocabulary for common classification taxonomies

**Sources cited:** SAP Supplier Classification (scope item 19E), Coupa AI-Driven Commodity Classification, SAP Ariba Category Management, UNSPSC, NIST Supply Chain Mapping

---

### Graph Modeling Expert (Graph Data Modeling & Algorithm Specialist)

**Assessment:** The model departs from the GQL property graph model it claims alignment with by providing only a single `type` per node, with no secondary label mechanism. In GQL/Cypher, nodes carry multiple labels serving as set-membership assertions for pattern matching (`MATCH (n:Organization:HighRisk:EUDR_InScope)`). This limits multi-dimensional segmentation that is essential for supply chain graph analysis.

**Key Concerns:**
- [Major] No multi-label mechanism aligned with GQL (ISO/IEC 39075)
- [Major] Unknown property preservation without namespacing creates collision risk during merge
- [Major] Attestation indirection adds O(E) overhead to what should be O(1) label-scan segmentation queries
- [Minor] No mechanism to declare controlled vocabularies
- [Minor] `tier` lacks defined anchor node, ambiguous in merged graphs

**Top Recommendations:**
- (P0) Add `labels` array (set-membership flags, namespaced, for fast graph traversal)
- (P0) Add `annotations` object (key-value pairs, namespaced, for structured classifications)
- (P0) Add `reporting_entity` field to file header
- (P1) Define merge behavior for labels (set union) and annotations (key-union with conflicts)
- (P2) Add `vocabulary` declaration to file header

**Sources cited:** ISO/IEC 39075 (GQL), Neo4j Graph Database Concepts, JSON-Graph Specification, Knowledge Graph research on supply chain resilience (Belhadi et al. 2023)

---

### Enterprise Integration Expert (Enterprise Systems Architect)

**Assessment:** Every SAP implementation configures vendor groups, every Oracle deployment uses supplier classification DFFs, every D365 rollout assigns VendorGroupId. These are day-one configuration decisions. The model lacks a standard property to land these classification fields, and SPEC-005 acknowledges their existence without providing a mapping target.

**Key Concerns:**
- [Major] No general-purpose classification/label mechanism
- [Major] SPEC-005 notes ERP segmentation fields but does not map them
- [Major] Business unit / buying entity segmentation is absent
- [Minor] Compliance status (approved/blocked) not modeled as edge property
- [Minor] No multi-valued commodity classification support

**Top Recommendations:**
- (P1) Add `labels` array (key-value pairs with authority attribution) to all nodes and edges
- (P1) Map ERP segmentation fields to labels in SPEC-005 with concrete JSON examples
- (P1) Add `buying_org` property to `supplies` and `subcontracts` edges
- (P2) Add `status` property to supply relationship edges
- (P2) Support multi-valued commodity classification

**Sources cited:** SAP Ariba Supplier Risk Segmentation, Oracle Fusion SCM DFFs, Dynamics 365 Vendor Groups, Supplier Segmentation Matrix (SCMDojo)

---

### Regulatory Compliance Expert (Supply Chain Regulatory Compliance Advisor)

**Assessment:** Supply chain segmentation is the operational core of regulatory compliance. Every regulation (CSDDD, LkSG, EUDR, UFLPA, CBAM, Conflict Minerals) requires classification and filtering by regulation-specific criteria. The model's scattered segmentation properties cover a subset of needs, but the absence of a lightweight classification mechanism forces misuse of attestation nodes or heavy reliance on extensions for routine regulatory categorization.

**Key Concerns:**
- [Major] No lightweight classification mechanism for regulatory labels (UFLPA entity status, LkSG risk priority, EUDR commodity scope)
- [Major] `tier` only on `supplies` edges but LkSG/CSDDD apply to all relationship types
- [Major] No reporting entity / scope owner concept for multi-entity corporate group compliance
- [Minor] No controlled vocabulary for `attested_by` scope values
- [Minor] EUDR commodity classification requires consumer-side mapping
- [Minor] No `facility_type` enum for Conflict Minerals smelter/refiner identification

**Top Recommendations:**
- (P0) Add `labels` array with structured entries (`key`, `value`, temporal validity) to all nodes and edges
- (P1) Extend `tier` to all supply relationship edge types
- (P1) Add `reporting_entity` or `scope_owner` concept to file header
- (P2) Define controlled vocabulary for `attested_by` scope values
- (P2) Add `facility_type` enum to facility nodes

**Sources cited:** EU CSDDD (European Commission), German LkSG FAQ (CSR Germany), EUDR Geolocation (Green Forum), UFLPA Entity List (DHS), EU CBAM (Taxation and Customs), EU Conflict Minerals Regulation, BAFA LkSG Guidance
