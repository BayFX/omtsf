# OMTSF Specification: Standards Mapping

**Spec:** OMTSF-SPEC-006
**Status:** Draft
**Date:** 2026-02-18
**Revision:** 1
**License:** [CC-BY-4.0](LICENSE)
**This document is informative, not normative.** It documents how OMTSF relates to existing standards and regulations but does not define required behavior.

---

## Related Specifications

| Spec | Relationship |
|------|-------------|
| OMTSF-SPEC-001 (Graph Data Model) | Defines the graph model, node types, and edge types mapped to standards here. |
| OMTSF-SPEC-002 (Entity Identification) | Defines the identifier schemes mapped to identifier standards here. |

---

## 1. Identifier Systems

| OMTSF Scheme | Standard | Relationship |
|-------------|----------|-------------|
| `lei` | ISO 17442 | **Reuses.** OMTSF adopts LEI as-is. Format validation follows ISO 17442 check digit rules. |
| `duns` | D&B proprietary | **References.** OMTSF references DUNS as an identifier scheme. No dependency on D&B data products. |
| `gln` | GS1 General Specifications | **Reuses.** OMTSF adopts GLN format and check digit rules from GS1. |
| `nat-reg` | ISO 17442-2 (GLEIF RA list) | **Reuses.** OMTSF uses GLEIF's Registration Authority code list for jurisdiction qualification. |
| `vat` | ISO 3166-1 (country codes) | **Reuses** ISO 3166-1 alpha-2 for jurisdiction qualification. |
| `lei` | ISO 5009 (Official Organizational Roles) | **Forward-compatible.** ISO 5009 defines a framework for official organizational roles and their relationships to identifiers including LEI. OMTSF's `lei` scheme is structurally compatible with the identifier framework referenced by ISO 5009. |

---

## 2. Data Models

| OMTSF Concept | Related Standard | Relationship |
|---------------|-----------------|-------------|
| Directed labeled property multigraph | ISO/IEC 39075 (GQL) Property Graph Model | **Aligns with.** OMTSF adopts the same conceptual model: nodes and edges with independent identity, labels (types), and properties. |
| Identifier scheme qualification | ISO 6523 (ICD), UN/CEFACT UNTDID 3055 | **Aligns with.** OMTSF's scheme-qualified identifier model is structurally compatible with ISO 6523 International Code Designator. The `scheme` field in OMTSF serves the same function as the ICD in ISO 6523: it qualifies an identifier value with its issuing scheme. See Section 4 for the ICD mapping table. |
| Corporate hierarchy | GLEIF Level 2 relationship data | **Extends.** OMTSF includes GLEIF Level 2's accounting consolidation concept (`legal_parentage`) and extends it with `ownership` (including minority stakes), `operational_control`, `beneficial_ownership`, and `former_identity`. |
| Identifier URI format | GS1 EPC URI, GS1 Digital Link | **Compatible with.** OMTSF's `scheme:value` format can be mechanically converted to/from GS1 EPC URIs (e.g., `gln:0614141000036` <-> `urn:epc:id:sgln:0614141.00001.0`). |
| Composite identifier model | PEPPOL Participant Identifiers | **Informed by.** PEPPOL's `{scheme}:{identifier}` pattern (with ISO 6523 ICD scheme codes) directly influenced OMTSF's design. |

---

## 3. Regulatory Alignment

| Regulation | Entity Identification Requirement | OMTSF Coverage |
|-----------|----------------------------------|---------------|
| EU CSDDD | Identify business partners, value chain entities, and beneficial owners | `organization` nodes with external identifiers; `ownership`, `legal_parentage`, and `beneficial_ownership` edges; `person` nodes for UBOs |
| EUDR | Identify operators, traders, and geolocated production plots; due diligence statements | `organization` nodes (operators/traders) + `facility` nodes with `geo` coordinates; `attestation` nodes for DDS |
| German LkSG | Identify direct and indirect suppliers; documented risk analysis | Full graph with `supplies` and `subcontracts` edge types; `attestation` nodes for risk analysis documentation |
| US UFLPA | Map supply chains to identify entities in Xinjiang region | `organization` and `facility` nodes with `jurisdiction` and `geo` properties |

### 3.1 EUDR Geolocation Precision Requirements

For OMTSF files supporting EUDR compliance, `facility` node `geo` coordinates SHOULD use at least **6 decimal digits** of precision, and production plots exceeding **4 hectares** SHOULD use GeoJSON polygon geometry rather than point coordinates.

| EU CBAM | Identify installations and operators for carbon reporting | `facility` nodes (installations) linked to `organization` nodes (operators) via `operates` edges |
| EU AMLD 5/6 | Identify ultimate beneficial owners (natural persons) | `person` nodes linked to `organization` nodes via `beneficial_ownership` edges |

---

## 4. ISO 6523 ICD Mapping

ISO 6523 defines International Code Designators (ICDs) that identify organizations by their registration scheme. OMTSF identifier schemes map to ISO 6523 ICDs as follows:

| OMTSF Scheme | ISO 6523 ICD | ICD Name | Notes |
|-------------|-------------|----------|-------|
| `lei` | `0199` | Legal Entity Identifier (LEI) | Direct mapping. OMTSF `lei:{value}` = ISO 6523 `0199:{value}` |
| `duns` | `0060` | Dun & Bradstreet D-U-N-S Number | Direct mapping. OMTSF `duns:{value}` = ISO 6523 `0060:{value}` |
| `gln` | `0088` | GS1 Global Location Number | Direct mapping. OMTSF `gln:{value}` = ISO 6523 `0088:{value}` |
| `nat-reg` | varies | National registration schemes | ICD depends on the specific registry. E.g., UK Companies House = `0195`, France SIREN = `0002` |
| `vat` | `9906`--`9958` | EU VAT schemes | PEPPOL assigns ICDs per EU country (e.g., `9930` = IT Codice Fiscale) |
| `internal` | N/A | Not applicable | Internal identifiers have no ISO 6523 equivalent |

**Conversion formula:** An OMTSF identifier can be converted to ISO 6523 format: `{ICD}:{value}`. For example, `lei:5493006MHB84DD0ZWV18` becomes `0199:5493006MHB84DD0ZWV18` in ISO 6523 notation.

**Note:** The full ISO 6523 ICD list is maintained by the ISO 6523 Maintenance Agency. The mapping above covers the most common schemes; producers encountering schemes not listed here SHOULD consult the current ICD list.

UNTDID code list 3055 ("Code list responsible agency code") provides a parallel scheme identification mechanism used in UN/EDIFACT messages. OMTSF does not directly use UNTDID 3055 codes but the `scheme` + `authority` pattern serves an equivalent purpose. Organizations bridging OMTSF with EDIFACT can map between OMTSF schemes and UNTDID 3055 agency codes via the ISO 6523 ICD table above.

---

## 5. GS1 EPCIS 2.0

GS1 EPCIS 2.0 (Electronic Product Code Information Services) captures event-level supply chain visibility data: what happened, when, where, and why. OMTSF and EPCIS are complementary:

| Aspect | EPCIS 2.0 | OMTSF |
|--------|-----------|-------|
| **Focus** | Event-level (individual transactions, movements, transformations) | Graph-level (structural relationships, ownership, attestation) |
| **Granularity** | Individual item/lot events | Entity and relationship level |
| **Identifiers** | GS1 keys (GTIN, GLN, SSCC, GRAI) | Multi-scheme composite (LEI, DUNS, GLN, nat-reg, etc.) |
| **Primary use** | Track-and-trace, serialization, provenance | Supply chain due diligence, risk analysis, regulatory reporting |

**Interoperability path:** EPCIS `bizLocation` (GLN) maps to OMTSF `facility` nodes. EPCIS `TransformationEvent` captures input/output relationships that correspond to OMTSF `composed_of` edges at an aggregated level. Organizations using both EPCIS and OMTSF can link event-level data to the structural graph via shared GLN identifiers on `facility` nodes.

---

## 6. W3C Verifiable Credentials

The OMTSF `attestation` node type (OMTSF-SPEC-001, Section 4.5) captures certification and audit data in a format designed for graph integration. The W3C Verifiable Credentials (VC) Data Model provides a complementary approach focused on cryptographic verifiability.

| OMTSF Attestation Property | W3C VC Equivalent | Notes |
|---------------------------|-------------------|-------|
| `issuer` | `vc.issuer` | Entity that issued the credential |
| `valid_from` | `vc.validFrom` | Issuance / effective date |
| `valid_to` | `vc.validUntil` | Expiration date |
| `outcome` | Credential subject claims | VC encodes outcomes as claims within the credential subject |
| `reference` | `vc.id` | Unique identifier for the credential |
| `status` | `vc.credentialStatus` | VC supports status lists for revocation checking |

OMTSF attestation nodes MAY carry a `reference` value that is a Verifiable Credential URI. Tooling that supports VCs can retrieve the full credential for cryptographic verification.
