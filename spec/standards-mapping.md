# OMTSF Specification: Standards Mapping

**Spec:** OMTSF-SPEC-006
**Status:** Draft
**Date:** 2026-02-18
**Revision:** 1
**License:** This specification is licensed under [CC-BY-4.0](https://creativecommons.org/licenses/by/4.0/). Code artifacts in this repository are licensed under Apache 2.0.
**Addresses:** P0-11

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

---

## 2. Data Models

| OMTSF Concept | Related Standard | Relationship |
|---------------|-----------------|-------------|
| Directed labeled property multigraph | ISO/IEC 39075 (GQL) Property Graph Model | **Aligns with.** OMTSF adopts the same conceptual model: nodes and edges with independent identity, labels (types), and properties. |
| Identifier scheme qualification | ISO 6523 (ICD), UN/CEFACT UNTDID 3055 | **Informed by.** OMTSF's scheme-qualified identifier pattern follows the same principle as ISO 6523 International Code Designator and UNTDID code list 3055. |
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
| EU CBAM | Identify installations and operators for carbon reporting | `facility` nodes (installations) linked to `organization` nodes (operators) via `operates` edges |
| EU AMLD 5/6 | Identify ultimate beneficial owners (natural persons) | `person` nodes linked to `organization` nodes via `beneficial_ownership` edges |
