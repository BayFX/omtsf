# OMTS Specification: ERP Integration Guide

**Spec:** OMTS-SPEC-005
**Status:** Draft
**Date:** 2026-02-18
**Revision:** 1
**License:** [CC-BY-4.0](LICENSE)

**This document is informative, not normative.** It provides reference mappings for ERP systems but does not define required behavior.

---

## Related Specifications

| Spec | Relationship |
|------|-------------|
| OMTS-SPEC-001 (Graph Data Model) | Defines the node types and edge types that ERP data maps to. |
| OMTS-SPEC-002 (Entity Identification) | Defines the identifier schemes (`internal`, `vat`, `duns`, etc.) used in ERP mappings. |
| OMTS-SPEC-003 (Merge Semantics) | Defines intra-file deduplication guidance relevant to ERP duplicate vendor records (OMTS-SPEC-003, Section 8). |
| OMTS-SPEC-001, Section 8.4 (Labels) | Defines the `labels` array that ERP classification fields map to. See Section 5 of this document. |
| OMTS-SPEC-001, Appendix B (Recommended Label Keys) | Defines recommended label keys for common classifications. ERP-specific keys use reverse-domain notation. |

---

## 1. Overview

This guide provides reference mappings for how entity identifiers and relationships in common ERP systems correspond to OMTS node types, identifier records, and edge types. These mappings are informative and intended to assist producers building OMTS export tooling. Field names and API endpoints are approximate and should be verified against actual system documentation for the version in use.

### 1.1 Authority Naming Convention

For `internal` scheme identifiers, the `authority` field identifies the source system. Producers should follow this convention:

```
{system_type}-{instance_id}[-{client}]
```

Examples:
- `sap-prod-100` — SAP production system, client 100
- `sap-prod-bp` — SAP Business Partner namespace
- `oracle-scm-us` — Oracle SCM Cloud US instance
- `d365-fin-eu` — Dynamics 365 Finance, EU tenant
- `ariba-network` — SAP Ariba Network

This convention enables downstream tooling to group and deduplicate identifiers by source system. It is a recommendation, not a normative requirement.

---

## 2. SAP S/4HANA

### 2.1 Node Derivation (Vendor Master)

| SAP Field | Table/Structure | OMTS Mapping |
|-----------|----------------|---------------|
| `LIFNR` (Vendor Number) | `LFA1` | `scheme: "internal"`, `authority: "{sap_system_id}"` |
| `STCD1` (Tax Number 1) | `LFA1` | `scheme: "vat"`, `authority` from `LAND1` (country key) |
| `STCD2` (Tax Number 2) | `LFA1` | `scheme: "vat"`, `authority` from `LAND1` |
| Custom DUNS field | `LFA1` (via append structure) | `scheme: "duns"` |
| `NAME1`--`NAME4` | `LFA1` | Node `name` property |
| `LAND1` (Country Key) | `LFA1` | Node `jurisdiction` property |
| `EKORG` (Purchasing Org) | `LFM1` | Context for `internal` authority scoping |

### 2.2 Edge Derivation (Supply Relationships)

| SAP Table | Structure | OMTS Mapping |
|-----------|-----------|---------------|
| `EINA` / `EINE` (Purchasing Info Record) | Vendor-material relationship | `supplies` edge from vendor `organization` to buyer `organization`, with `commodity` from material group |
| `EKKO` (PO Header) + `EKPO` (PO Item) | Purchase order | `supplies` edge (if no info record exists). Derive from `EKKO-LIFNR` (vendor) and `EKKO-BUKRS` (company code). |
| `EKPO-PSTYP` (Item Category) | Item category `L` = subcontracting | `subcontracts` edge (when item category indicates subcontracting). Note: `BSART='UB'` indicates stock transport orders, not subcontracting. |
| `MARA` / `MARC` (Material Master) | Material → `good` node | `good` node with `scheme: "internal"`, `authority: "{sap_system_id}"`, `value` from `MATNR` |
| `RSEG` (Invoice Document) | Invoice line to vendor | Confirms `supplies` edge; provides volume/quantity data for edge properties |

**Temporal bounding for PO-derived edges.** When deriving `supplies` edges from purchase orders (`EKKO`/`EKPO`), producers SHOULD apply a recency filter to avoid generating current supply relationships from stale data. A single purchase order from years ago does not indicate a current supply relationship. Recommended approach: only generate `supplies` edges from POs where `EKKO-BEDAT` (PO date) falls within a configurable lookback window (e.g., 24 months from `snapshot_date`). For relationships derived from older POs, producers SHOULD set `valid_to` to the last PO or invoice date rather than leaving it open-ended. This guidance applies equally to Oracle and D365 PO-derived edges (Sections 3.2 and 4.2).

### 2.3 Deduplication Note

In multi-client SAP landscapes, the same legal entity may appear as different `LIFNR` values across clients. The `authority` field on `internal` identifiers should include the client number (e.g., `sap-prod-100`, `sap-prod-200`) to distinguish these. See OMTS-SPEC-003, Section 8 for intra-file deduplication guidance.

### 2.4 SAP Business Partner Model (S/4HANA)

SAP S/4HANA's Business Partner model (`BUT000`/`BUT0ID`) replaces the legacy vendor master (`LFA1`) as the primary entity data store. New S/4HANA implementations use the Business Partner model exclusively; legacy migrations retain parallel data in both structures.

| SAP Field | Table | OMTS Mapping |
|-----------|-------|---------------|
| `PARTNER` (BP Number) | `BUT000` | `scheme: "internal"`, `authority: "{sap_system_id}-bp"` |
| `BU_SORT1` (Search Term 1) | `BUT000` | May assist fuzzy deduplication |
| `TYPE` (BP Category) | `BUT000` | `1` = Organization → `organization` node; `2` = Person → `person` node |
| `IDNUMBER` (ID Number) | `BUT0ID` | `scheme` depends on `IDTYPE`: see mapping below |
| `IDTYPE` (ID Type) | `BUT0ID` | Maps to OMTS scheme: `DUNS` → `duns`, `LEI` → `lei`, `HRNR` → `nat-reg`, `UST1` → `vat` |
| `INSTITUTE` (Issuing Institute) | `BUT0ID` | Maps to `authority` field for `nat-reg` and `vat` schemes |

**`BUT0ID` to OMTS scheme mapping:**

| SAP `IDTYPE` | OMTS Scheme | Notes |
|-------------|-------------|-------|
| `DUNS` | `duns` | Direct mapping |
| `LEI` | `lei` | Direct mapping |
| `HRNR` | `nat-reg` | `authority` from `INSTITUTE` or derived from country |
| `UST1` | `vat` | EU VAT number; `authority` from country key |
| `UID` | `vat` | Non-EU tax ID; `authority` from country key |
| Other | `internal` | Custom ID types → `authority: "{sap_system_id}-bp"` |

**Note:** The `BUT0ID` table provides cleaner identifier type disambiguation than the legacy `STCD1`/`STCD2` fields, which store different identifier types depending on country configuration.

### 2.5 Tax Number Field Disambiguation

SAP's `STCD1` and `STCD2` fields in `LFA1` store different types of tax identifiers depending on the vendor's country. Do not blindly map `STCD1`/`STCD2` to `vat`. Inspect the `LAND1` (country key) field and apply country-specific logic. When in doubt, map to `internal` with a descriptive authority (e.g., `sap-stcd1-{country}`).

---

## 3. Oracle SCM Cloud

### 3.1 Supplier Data

Oracle SCM Cloud exposes supplier data via the Fusion REST API. The base URL pattern is `https://{host}/fscmRestApi/resources/11.13.18.05/`.

| Oracle REST Endpoint | OData Entity | Field | OMTS Mapping |
|---------------------|-------------|-------|---------------|
| `GET /suppliers` | `PrcPozSuppliersVO` | `SupplierId` | `scheme: "internal"`, `authority: "{oracle_instance}"` |
| `GET /suppliers` | `PrcPozSuppliersVO` | `SupplierNumber` | Alternative `internal` identifier (user-visible number) |
| `GET /suppliers` | `PrcPozSuppliersVO` | `Supplier` (name) | Node `name` property |
| `GET /suppliers` | `PrcPozSuppliersVO` | `TaxRegistrationNumber` | `scheme: "vat"`, `authority` from `TaxRegistrationCountry` |
| `GET /suppliers` | `PrcPozSuppliersVO` | `DUNSNumber` | `scheme: "duns"` |
| `GET /suppliers/{id}/child/sites` | `PrcPozSupplierSitesVO` | `SupplierSiteId` | Separate `facility` node with `internal` identifier |
| `GET /suppliers/{id}/child/sites` | `PrcPozSupplierSitesVO` | `AddressLine1`--`AddressLine4`, `City`, `State`, `PostalCode` | `facility` node `address` property |
| `GET /suppliers/{id}/child/sites` | `PrcPozSupplierSitesVO` | `Country` | `facility` node `jurisdiction` property |

### 3.2 Procurement Data

| Oracle REST Endpoint | OData Entity | Field | OMTS Mapping |
|---------------------|-------------|-------|---------------|
| `GET /purchaseOrders` | `PurchaseOrdersAllVO` | `POHeaderId`, `OrderNumber` | Derive `supplies` edge from vendor → buying org |
| `GET /purchaseOrders/{id}/child/lines` | `PurchaseOrderLineVO` | `ItemDescription`, `CategoryName` | `supplies` edge `commodity` property |
| `GET /purchaseOrders/{id}/child/lines` | `PurchaseOrderLineVO` | `Quantity`, `UOMCode` | `supplies` edge `volume` and `volume_unit` properties |
| `GET /purchaseOrders` | `PurchaseOrdersAllVO` | `ProcurementBUId` | Identifies the buying organization for the `supplies` edge target |
| `GET /receipts` | `ReceiptHeadersVO` | Receipt lines | Confirms `supplies` edge; provides actual receipt volume data |

---

## 4. Microsoft Dynamics 365

Dynamics 365 Finance and Supply Chain Management expose data via OData v4 endpoints at `https://{environment}.operations.dynamics.com/data/`.

### 4.1 Vendor Data

| D365 OData Entity | OData Path | Field | OMTS Mapping |
|-------------------|-----------|-------|---------------|
| `VendorsV2` | `GET /data/VendorsV2` | `VendorAccountNumber` | `scheme: "internal"`, `authority: "d365-{tenant}-{company}"` (see note below) |
| `VendorsV2` | `GET /data/VendorsV2` | `VendorOrganizationName` | Node `name` property |
| `VendorsV2` | `GET /data/VendorsV2` | `VendorGroupId` | Useful for segmenting supplier types during export |
| `DirPartyTable` (via `VendorsV2` navigation) | `$expand=DirPartyTable` | `Name` | Alternative name source (legal name from global address book) |
| `DirPartyTable` | `GET /data/DirParties` | `DunsNumber` | `scheme: "duns"` |
| `TaxRegistrationId` | `GET /data/TaxRegistrationIds` | `RegistrationNumber` | `scheme: "vat"`, `authority` from `CountryRegionId` |
| `LogisticsPostalAddress` | `GET /data/LogisticsPostalAddresses` | `Street`, `City`, `State`, `ZipCode`, `CountryRegionId` | `facility` node `address` and `jurisdiction` properties |

**Cross-company data isolation.** D365 `VendorsV2` is company-scoped by default. In multi-entity tenants, the same vendor may have different `VendorAccountNumber` values in different legal entities, or different vendors may share the same number across companies. The `authority` field MUST include both the tenant identifier and the company code (e.g., `d365-contoso-usop`, `d365-contoso-eumf`) to prevent colliding internal identifiers. The recommended format is `d365-{tenant}-{company}` where `{tenant}` is a short tenant identifier and `{company}` is the D365 legal entity (company) code.

### 4.2 Procurement Data

| D365 OData Entity | OData Path | Field | OMTS Mapping |
|-------------------|-----------|-------|---------------|
| `PurchaseOrderHeadersV2` | `GET /data/PurchaseOrderHeadersV2` | `OrderVendorAccountNumber` | Derive `supplies` edge (vendor → buying legal entity) |
| `PurchaseOrderHeadersV2` | `GET /data/PurchaseOrderHeadersV2` | `InvoiceVendorAccountNumber` | Identifies invoice party (may differ from order vendor) |
| `PurchaseOrderLinesV2` | `GET /data/PurchaseOrderLinesV2` | `ItemNumber`, `ProcurementCategoryName` | `supplies` edge `commodity` property |
| `PurchaseOrderLinesV2` | `GET /data/PurchaseOrderLinesV2` | `OrderedPurchaseQuantity`, `PurchaseUnitSymbol` | `supplies` edge `volume` and `volume_unit` properties |
| `VendInvoiceJour` | `GET /data/VendorInvoiceJournalLines` | Invoice journal lines | Confirms supply relationship; provides value data for `annual_value` |

---

## 5. Label Mapping

This section provides reference mappings for how ERP classification fields map to the OMTS `labels` array (OMTS-SPEC-001, Section 8.4). The recommended label keys defined in OMTS-SPEC-001, Appendix B should be used where the semantics match. ERP-specific classifications that do not map to a recommended key should use reverse-domain notation.

### 5.1 SAP S/4HANA

| SAP Field | Table/Structure | OMTS Label Mapping |
|-----------|----------------|---------------------|
| `EKGRP` (Vendor Group) | `LFM1` | `{ "key": "com.sap.vendor-group", "value": "{EKGRP}" }` |
| Purchasing block indicator | `LFM1` (`SPERM`) | `{ "key": "com.sap.purchasing-blocked" }` (boolean flag) |
| Vendor classification (scope item 19E) | `BUT000` classification tab | Use recommended keys where applicable (e.g., `kraljic-quadrant`, `risk-tier`). Map other classifications to `com.sap.{classification-type}`. |
| `EKORG` (Purchasing Organization) | `LFM1` | Embed in label key when classifications vary by purchasing org (see organizational scope guidance in OMTS-SPEC-001, Section 8.4). |

**Example: SAP vendor with group and org-scoped classification:**

```json
"labels": [
  { "key": "com.sap.vendor-group", "value": "ZSTR" },
  { "key": "kraljic-quadrant", "value": "strategic" },
  { "key": "com.acme.ekorg-1000.vendor-group", "value": "strategic" },
  { "key": "com.acme.ekorg-2000.vendor-group", "value": "standard" }
]
```

### 5.2 Oracle SCM Cloud

| Oracle Field | Source | OMTS Label Mapping |
|-------------|--------|---------------------|
| `ClassificationCode` | `POZ_SUPPLIER_BUSINESS_CLASSIFICATIONS` | `{ "key": "com.oracle.business-classification", "value": "{ClassificationCode}" }` |
| `SupplierType` | `PrcPozSuppliersVO` | `{ "key": "com.oracle.supplier-type", "value": "{SupplierType}" }` |
| Supplier diversity certification | `POZ_SUPPLIER_BUSINESS_CLASSIFICATIONS` | Use `diversity-classification` recommended key. |
| `ProcurementBUId` | `PurchaseOrdersAllVO` | Embed in label key for BU-scoped classifications. |

**Example: Oracle supplier with business classification and diversity:**

```json
"labels": [
  { "key": "com.oracle.supplier-type", "value": "preferred" },
  { "key": "diversity-classification", "value": "minority-owned" },
  { "key": "com.oracle.bu-us-proc.supplier-tier", "value": "strategic" }
]
```

### 5.3 Microsoft Dynamics 365

| D365 Field | Source | OMTS Label Mapping |
|-----------|--------|---------------------|
| `VendorGroupId` | `VendorsV2` | `{ "key": "com.microsoft.d365.vendor-group", "value": "{VendorGroupId}" }` |
| `VendorOnHoldStatus` | `VendorsV2` | `{ "key": "com.microsoft.d365.on-hold" }` (boolean flag when on hold) |
| Procurement category | `ProcurementCategoryName` | Use `commodity-group` recommended key when semantics match. |

**Example: D365 vendor with group and hold status:**

```json
"labels": [
  { "key": "com.microsoft.d365.vendor-group", "value": "30" },
  { "key": "com.microsoft.d365.on-hold" },
  { "key": "approval-status", "value": "blocked" }
]
```

### 5.4 Cross-ERP Mapping Summary

| Classification Concept | SAP | Oracle | D365 | Recommended OMTS Key |
|----------------------|-----|--------|------|-----------------------|
| Vendor grouping | `EKGRP` | `SupplierType` | `VendorGroupId` | ERP-specific (`com.sap.*`, `com.oracle.*`, `com.microsoft.d365.*`) |
| Purchasing block | `SPERM` | `SupplierEnabledFlag` | `VendorOnHoldStatus` | `approval-status` with value `blocked` |
| Supplier diversity | Custom classification | `POZ_SUPPLIER_BUSINESS_CLASSIFICATIONS` | Custom | `diversity-classification` |
| Risk classification | Custom classification | Custom | Custom | `risk-tier` or `kraljic-quadrant` |

---

## 6. Identifier Enrichment Lifecycle

Files typically begin with minimal identifiers (internal ERP codes only) and are enriched over time as external identifiers are obtained.

### 6.1 Enrichment Levels

| Level | Description | Typical Identifiers | Merge Capability |
|-------|-------------|--------------------|--------------------|
| **Internal-only** | Raw ERP export | `internal` only | No cross-file merge possible |
| **Partially enriched** | Some external IDs obtained | `internal` + one of (`duns`, `nat-reg`, `vat`) | Cross-file merge possible where identifiers overlap |
| **Fully enriched** | Multiple external IDs verified | `internal` + `lei` + `nat-reg` + `vat` (+ `duns` where available) | High-confidence cross-file merge |

### 6.2 Enrichment Workflow

1. **Export:** Producer generates an `.omts` file from ERP data. Nodes carry `internal` identifiers and whatever external identifiers the ERP already holds (typically `vat` and sometimes `duns`).
2. **Match:** An enrichment tool takes the internal-only nodes and attempts to resolve them to external identifiers using available data sources (GLEIF, OpenCorporates, D&B, national registries).
3. **Augment:** The enrichment tool adds external identifiers to the nodes, preserving the original `internal` identifiers.
4. **Re-export:** The enriched file is written. It now passes Level 2 completeness checks (OMTS-SPEC-002, L2-EID-01).

**Merge interaction:** See OMTS-SPEC-003, Section 9 for guidance on how enrichment affects merge groups.

**Important:** Enrichment should not remove or modify existing identifiers. It is an additive process. The original `internal` identifiers are preserved for reconciliation with the source system.

### 6.3 Validation Level Alignment

- A file with only `internal` identifiers is valid at Level 1 (structural integrity).
- A file where most `organization` nodes have at least one external identifier satisfies Level 2 (completeness).
- A file where identifiers have been verified against authoritative sources satisfies Level 3 (enrichment).

---

## 7. EDI Coexistence

OMTS is not a replacement for EDI (EDIFACT, ANSI X12) or B2B messaging standards (PEPPOL BIS, cXML). EDI handles transactional document exchange (purchase orders, invoices, advance ship notices); OMTS handles supply chain graph representation (who supplies whom, ownership, attestation).

In a typical deployment:
- EDI continues to handle day-to-day procurement transactions.
- OMTS captures the structural supply chain graph derived from aggregated EDI transaction data, ERP master data, and external enrichment.
- An OMTS export tool reads ERP master data (informed by EDI-updated fields like vendor status, last PO date) and produces `.omts` files.

OMTS files may reference EDI identifiers. For example, a PEPPOL Participant Identifier can be stored as an extension scheme identifier: `scheme: "org.peppol.participant"`, `value: "0088:5790000436057"`, where `0088` is the ISO 6523 ICD for EAN.UCC (GS1).
