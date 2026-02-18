# OMTSF Specification: ERP Integration Guide

**Spec:** OMTSF-SPEC-005
**Status:** Draft
**Date:** 2026-02-18
**Revision:** 1
**License:** [MIT](../LICENSE)

**Addresses:** R1-C9, R1-C15, R1-P0-8, R1-P0-9

**This document is informative, not normative.** It provides reference mappings for ERP systems but does not define required behavior.

---

## Related Specifications

| Spec | Relationship |
|------|-------------|
| OMTSF-SPEC-001 (Graph Data Model) | Defines the node types and edge types that ERP data maps to. |
| OMTSF-SPEC-002 (Entity Identification) | Defines the identifier schemes (`internal`, `vat`, `duns`, etc.) used in ERP mappings. |
| OMTSF-SPEC-003 (Merge Semantics) | Defines intra-file deduplication guidance relevant to ERP duplicate vendor records (OMTSF-SPEC-003, Section 8). |

---

## 1. Overview

This guide provides reference mappings for how entity identifiers and relationships in common ERP systems correspond to OMTSF node types, identifier records, and edge types. These mappings are informative and intended to assist producers building OMTSF export tooling.

---

## 2. SAP S/4HANA

### 2.1 Node Derivation (Vendor Master)

| SAP Field | Table/Structure | OMTSF Mapping |
|-----------|----------------|---------------|
| `LIFNR` (Vendor Number) | `LFA1` | `scheme: "internal"`, `authority: "{sap_system_id}"` |
| `STCD1` (Tax Number 1) | `LFA1` | `scheme: "vat"`, `authority` from `LAND1` (country key) |
| `STCD2` (Tax Number 2) | `LFA1` | `scheme: "vat"`, `authority` from `LAND1` |
| Custom DUNS field | `LFA1` (via append structure) | `scheme: "duns"` |
| `NAME1`--`NAME4` | `LFA1` | Node `name` property |
| `LAND1` (Country Key) | `LFA1` | Node `jurisdiction` property |
| `EKORG` (Purchasing Org) | `LFM1` | Context for `internal` authority scoping |

### 2.2 Edge Derivation (Supply Relationships)

| SAP Table | Structure | OMTSF Mapping |
|-----------|-----------|---------------|
| `EINA` / `EINE` (Purchasing Info Record) | Vendor-material relationship | `supplies` edge from vendor `organization` to buyer `organization`, with `commodity` from material group |
| `EKKO` (PO Header) + `EKPO` (PO Item) | Purchase order | `supplies` edge (if no info record exists). Derive from `EKKO-LIFNR` (vendor) and `EKKO-BUKRS` (company code). |
| `EKKO-BSART` (PO Type) | Document type `UB` = subcontracting | `subcontracts` edge (when PO type indicates subcontracting) |
| `MARA` / `MARC` (Material Master) | Material → `good` node | `good` node with `scheme: "internal"`, `authority: "{sap_system_id}"`, `value` from `MATNR` |
| `RSEG` (Invoice Document) | Invoice line to vendor | Confirms `supplies` edge; provides volume/quantity data for edge properties |

### 2.3 Deduplication Note

In multi-client SAP landscapes, the same legal entity may appear as different `LIFNR` values across clients. The `authority` field on `internal` identifiers SHOULD include the client number (e.g., `sap-prod-100`, `sap-prod-200`) to distinguish these. See OMTSF-SPEC-003, Section 8 for intra-file deduplication guidance.

---

## 3. Oracle SCM Cloud

| Oracle Field | Object | OMTSF Mapping |
|-------------|--------|---------------|
| `VENDOR_ID` | Supplier | `scheme: "internal"`, `authority: "{oracle_instance}"` |
| `VENDOR_SITE_ID` | Supplier Site | Separate `facility` node with `internal` identifier |
| `TAX_REGISTRATION_NUMBER` | Supplier | `scheme: "vat"`, `authority` from country |
| `DUNS_NUMBER` | Supplier | `scheme: "duns"` |
| `VENDOR_NAME` | Supplier | Node `name` property |
| `PO_HEADERS_ALL` + `PO_LINES_ALL` | Purchase orders | `supplies` edge derivation (vendor → buying org) |

---

## 4. Microsoft Dynamics 365

| D365 Field | Entity | OMTSF Mapping |
|-----------|--------|---------------|
| `VendAccount` | VendTable | `scheme: "internal"`, `authority: "{d365_instance}"` |
| `TaxRegistrationId` | VendTable | `scheme: "vat"`, `authority` from country |
| `DunsNumber` | DirPartyTable | `scheme: "duns"` |
| `Name` | DirPartyTable | Node `name` property |

---

## 5. Identifier Enrichment Lifecycle

Files typically begin with minimal identifiers (internal ERP codes only) and are enriched over time as external identifiers are obtained.

### 5.1 Enrichment Levels

| Level | Description | Typical Identifiers | Merge Capability |
|-------|-------------|--------------------|--------------------|
| **Internal-only** | Raw ERP export | `internal` only | No cross-file merge possible |
| **Partially enriched** | Some external IDs obtained | `internal` + one of (`duns`, `nat-reg`, `vat`) | Cross-file merge possible where identifiers overlap |
| **Fully enriched** | Multiple external IDs verified | `internal` + `lei` + `nat-reg` + `vat` (+ `duns` where available) | High-confidence cross-file merge |

### 5.2 Enrichment Workflow

1. **Export:** Producer generates an `.omts` file from ERP data. Nodes carry `internal` identifiers and whatever external identifiers the ERP already holds (typically `vat` and sometimes `duns`).
2. **Match:** An enrichment tool takes the internal-only nodes and attempts to resolve them to external identifiers using available data sources (GLEIF, OpenCorporates, D&B, national registries).
3. **Augment:** The enrichment tool adds external identifiers to the nodes, preserving the original `internal` identifiers.
4. **Re-export:** The enriched file is written. It now passes Level 2 completeness checks (OMTSF-SPEC-002, L2-EID-01).

**Important:** Enrichment MUST NOT remove or modify existing identifiers. It is an additive process. The original `internal` identifiers are preserved for reconciliation with the source system.

### 5.3 Validation Level Alignment

- A file with only `internal` identifiers is valid at Level 1 (structural integrity).
- A file where most `organization` nodes have at least one external identifier satisfies Level 2 (completeness).
- A file where identifiers have been verified against authoritative sources satisfies Level 3 (enrichment).
