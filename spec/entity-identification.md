# OMTSF Specification: Entity Identification

**Spec:** OMTSF-SPEC-002
**Status:** Draft
**Date:** 2026-02-18
**Revision:** 3 (decomposed from monolithic spec)
**License:** This specification is licensed under [CC-BY-4.0](https://creativecommons.org/licenses/by/4.0/). Code artifacts in this repository are licensed under Apache 2.0.
**Addresses:** C1, C8, M3, M5, P0-1, P1-20

---

## Related Specifications

| Spec | Relationship |
|------|-------------|
| OMTSF-SPEC-001 (Graph Data Model) | **Prerequisite.** Defines the node types, edge types, and file structure that carry the identifiers defined here. |
| OMTSF-SPEC-003 (Merge Semantics) | Uses external identifiers defined here as the basis for cross-file merge identity predicates. |
| OMTSF-SPEC-004 (Selective Disclosure) | Defines sensitivity levels and redaction behavior for the `sensitivity` field on identifier records. |

---

## 1. Problem Statement

Entity identification is the load-bearing foundation of the OMTSF architecture. Without a defined identifier strategy, the merge-by-concatenation model described in the vision is theoretical: if two parties export files using different identifiers for the same legal entity, merge produces duplicates instead of a unified graph.

No single global business identifier exists:

- **LEI** (Legal Entity Identifier) covers ~2.7 million entities, skewed toward financial institutions. Open and free to query, but costs $50--200/year per entity to obtain. Does not cover facilities or unregistered entities.
- **DUNS** (Dun & Bradstreet) covers ~500 million entities -- the broadest coverage -- but is proprietary. Hierarchy data is a premium product. Redistribution is restricted by license.
- **GLN** (GS1 Global Location Number) covers locations and parties within the GS1 membership base (~2 million companies). Requires GS1 membership. No comprehensive public registry.
- **National company registry numbers** are authoritative within their jurisdiction but use incompatible formats across ~200 countries. The US has no federal registry; Germany fragments by court. A number is only meaningful paired with its jurisdiction.
- **Tax IDs** (VAT, EIN, TIN) have high coverage but are legally confidential in most jurisdictions. Using them as primary keys in exchanged files raises GDPR and privacy concerns.

The consequence: any specification that mandates a single identifier scheme excludes the majority of supply chain participants. The solution is a composite identifier model that treats all schemes as peers.

---

## 2. Design Principles

**No single mandatory scheme.** The format MUST NOT require any single proprietary or paid identifier system. An entity with only an internal ERP vendor number is as representable as one with an LEI.

**Composite identity.** Every entity node carries an array of zero or more external identifiers from multiple schemes. The more identifiers an entity carries, the higher the probability of successful cross-file merge.

**Graph-local vs. external identity.** File-local IDs (used for edge source/target references within a single file, defined in OMTSF-SPEC-001, Section 3) are structurally distinct from external identifiers (used for cross-file merge). They serve different purposes and MUST NOT be conflated.

**Scheme-qualified identifiers.** Every identifier declares its scheme. A bare number is meaningless; `duns:081466849` is unambiguous.

**Internal identifiers are first-class.** ERP vendor numbers, buyer-assigned supplier codes, and other system-local IDs are the most common identifiers in practice. They MUST be representable without requiring translation to a global scheme.

**Sensitivity-aware.** Some identifiers (tax IDs, internal codes) carry privacy or confidentiality constraints. The identifier model supports sensitivity classification to enable selective redaction (see OMTSF-SPEC-004).

**Temporally valid.** Identifiers change over time. Companies re-register, merge, acquire new LEIs, or lose DUNS numbers. The model supports temporal validity on every identifier.

---

## 3. External Identifier Structure

Each node (defined in OMTSF-SPEC-001) carries an optional `identifiers` array. Each entry is an **identifier record** with the following fields:

| Field | Required | Type | Description |
|-------|----------|------|-------------|
| `scheme` | Yes | string | Identifier scheme code from the controlled vocabulary (Section 4) |
| `value` | Yes | string | The identifier value within that scheme |
| `authority` | Conditional | string | Issuing authority or jurisdiction qualifier. Required for `nat-reg`, `vat`, and `internal` schemes. |
| `valid_from` | No | string (ISO 8601 date) | Date this identifier became effective for this entity |
| `valid_to` | No | string (ISO 8601 date) | Date this identifier ceased to be valid for this entity. `null` means currently valid. |
| `sensitivity` | No | enum | One of `public`, `restricted`, `confidential`. Default: `public`. See OMTSF-SPEC-004. |

**Rationale for `authority` as conditional:** Some schemes are globally unambiguous (LEI is always issued by a GLEIF-accredited LOU; DUNS is always issued by D&B). Others require disambiguation: a national registry number is meaningless without its jurisdiction, a VAT number needs its country, and an internal ID needs its issuing system.

**Unknown fields:** Conformant parsers MUST preserve unknown fields in identifier records during round-trip serialization. Unknown fields MUST NOT cause validation failure at any level. This ensures forward compatibility when future spec versions add fields (e.g., `confidence`, `verification`).

---

## 4. Canonical Identifier String Format

Each identifier record has a **canonical string form** used for sorting, hashing, and deterministic comparison:

- For schemes requiring `authority`: `{scheme}:{authority}:{value}`
- For schemes without `authority`: `{scheme}:{value}`

Examples:
- `lei:5493006MHB84DD0ZWV18`
- `nat-reg:RA000548:HRB86891`
- `vat:DE:DE123456789`
- `internal:sap-mm-prod:V-100234`
- `duns:081466849`

**Encoding rules:**
- All components are UTF-8 encoded
- The colon (`:`, U+003A) is the delimiter
- If an `authority` or `value` contains a literal colon, it MUST be percent-encoded as `%3A`
- If an `authority` or `value` contains a literal percent sign, it MUST be percent-encoded as `%25`

This canonical form is used in boundary reference hashing (OMTSF-SPEC-004, Section 4) and merge identity comparison (OMTSF-SPEC-003, Section 2).

---

## 5. Identifier Scheme Vocabulary

### 5.1 Core Schemes

Conformant OMTSF validators MUST recognize the following schemes and enforce their format validation rules.

#### `lei` -- Legal Entity Identifier

- **Standard:** ISO 17442
- **Authority:** GLEIF (Global Legal Entity Identifier Foundation)
- **Format:** 20-character alphanumeric string. Characters 1--18 are the entity-specific part (alphanumeric). Characters 19--20 are check digits (numeric).
- **Validation:** MUST match `^[A-Z0-9]{18}[0-9]{2}$`. MUST pass MOD 97-10 check digit verification (ISO 7064).
- **`authority` field:** Not required. The issuing LOU can be derived from the LEI itself via the GLEIF API.
- **Coverage:** ~2.7 million entities worldwide. Strong in financial services, growing in supply chain due to regulatory mandates (EU CSDDD, MiFID II).
- **Data availability:** 100% open. Full database downloadable from GLEIF at no cost. Includes Level 1 (entity data) and Level 2 (corporate hierarchy via accounting consolidation relationships).

**LEI Registration Status and Lifecycle:**

LEIs have a registration status maintained by GLEIF. The following statuses affect OMTSF processing:

| LEI Status | Meaning | OMTSF Merge Behavior | Validation |
|------------|---------|---------------------|------------|
| `ISSUED` | Active, annually renewed | Normal merge candidate | -- |
| `LAPSED` | Failed to renew; entity still exists | Still valid for merge. The entity is unchanged; only the registration fee is unpaid. | L2 warning |
| `RETIRED` | Voluntarily retired by the entity | Still valid for merge for historical data. Producers SHOULD set `valid_to` on the identifier. | L2 warning |
| `MERGED` | Entity merged into another; successor LEI exists | Still valid for merge. Producers SHOULD create a `former_identity` edge (OMTSF-SPEC-001, Section 5.4) linking the retired-LEI node to the successor-LEI node, with `event_type: "merger"`. | L2 warning |
| `ANNULLED` | Issued in error or fraudulently | MUST NOT be used for merge. Treat as invalid. | L2 error |

The GLEIF database provides explicit successor relationships for MERGED LEIs via the `SuccessorEntity` field. Tooling that performs Level 3 enrichment SHOULD retrieve successor LEI data and generate `former_identity` edges automatically.

#### `duns` -- DUNS Number

- **Authority:** Dun & Bradstreet
- **Format:** 9-digit numeric string.
- **Validation:** MUST match `^[0-9]{9}$`.
- **`authority` field:** Not required.
- **Coverage:** ~500 million entities worldwide. Broadest single-system coverage. Includes branches, divisions, and sole proprietorships.
- **Data availability:** Proprietary. Free to obtain a number; expensive to query data or hierarchy. OMTSF files MAY contain DUNS numbers (they are just strings), but enrichment/validation requires D&B data access.
- **Note:** D&B's corporate hierarchy (Family Tree) is a premium product. OMTSF represents hierarchy via edge types (OMTSF-SPEC-001, Section 5), not via the identifier scheme.

**DUNS Branch/HQ Disambiguation:**

D&B assigns separate DUNS numbers to different structural levels of the same legal entity. The D&B Family Tree model defines:

| D&B Level | Description | OMTSF Mapping |
|-----------|-------------|---------------|
| **Global Ultimate** | Topmost entity in the corporate family | `organization` node. Link to subsidiaries via `legal_parentage` or `ownership` edges. |
| **Domestic Ultimate** | Topmost entity within a single country | `organization` node. Link to Global Ultimate via `legal_parentage` edge. |
| **Parent** | Direct legal parent of a subsidiary | `organization` node. Link via `legal_parentage` edge. |
| **Headquarters** | Main office of a company with branches | `organization` node. The HQ DUNS is the primary identifier for the legal entity. |
| **Branch** | A physical location or division of an entity | `facility` node. The branch DUNS identifies the location, not a separate legal entity. |

**Key guidance for producers:**

- A single legal entity may hold multiple DUNS numbers (HQ + branches). The HQ DUNS identifies the entity; branch DUNS numbers identify its locations.
- When a DUNS number identifies a branch, it SHOULD be assigned to a `facility` node, not an `organization` node.
- Merge engines SHOULD be aware that two nodes with different DUNS numbers may represent the same legal entity (one HQ, one branch). Level 3 validation MAY flag this by querying D&B's Family Tree linkage.
- When an ERP system stores only a single DUNS number and it is unclear whether it is an HQ or branch DUNS, producers SHOULD assign it to an `organization` node and note the ambiguity.

#### `gln` -- Global Location Number

- **Standard:** GS1 General Specifications
- **Authority:** GS1 (federated via ~115 national Member Organizations)
- **Format:** 13-digit numeric string.
- **Validation:** MUST match `^[0-9]{13}$`. MUST pass GS1 mod-10 check digit (last digit).
- **`authority` field:** Not required. The GS1 Company Prefix embedded in the GLN identifies the issuing MO.
- **Coverage:** Used by 2+ million GS1 member companies. Strong in retail, FMCG, healthcare. Weaker in mining, heavy industry.
- **Note:** GLN can identify legal entities, functional entities, or physical locations. OMTSF disambiguates via node type (`organization` vs. `facility`), not via the identifier scheme.

#### `nat-reg` -- National Company Registry

- **Authority:** Government company registries (e.g., UK Companies House, German Handelsregister, French RCS)
- **Format:** Varies by jurisdiction.
- **Validation:** `authority` field is REQUIRED and MUST contain a valid GLEIF Registration Authority (RA) code from the OMTSF-maintained RA list snapshot (see Section 5.4). `value` format validation is authority-specific and MAY be deferred to Level 2 validation.
- **`authority` field:** Required. Contains the GLEIF RA code (e.g., `RA000585` for UK Companies House, `RA000548` for German Handelsregister).
- **Coverage:** Collectively comprehensive for all formally registered entities within their jurisdictions.

**Common authority codes:**

| RA Code | Registry | Jurisdiction |
|---------|----------|-------------|
| `RA000585` | Companies House | United Kingdom |
| `RA000548` | Handelsregister | Germany |
| `RA000525` | Registre du Commerce (SIREN) | France |
| `RA000665` | Kamer van Koophandel | Netherlands |
| `RA000476` | National Tax Board (houjin bangou) | Japan |
| `RA000553` | Ministry of Corporate Affairs (CIN) | India |
| `RA000602` | Division of Corporations | Delaware, US |
| `RA000631` | Secretary of State | California, US |

The full GLEIF RA list contains 700+ registration authorities and is available at `https://www.gleif.org/en/about-lei/code-lists/gleif-registration-authorities-list`.

#### `vat` -- VAT / Tax Identification Number

- **Authority:** National tax authorities
- **Format:** Varies by jurisdiction. EU VAT numbers are prefixed by a 2-letter country code.
- **Validation:** `authority` field is REQUIRED and MUST contain a valid ISO 3166-1 alpha-2 country code. Format validation is country-specific and MAY be deferred to Level 2 validation.
- **`authority` field:** Required. ISO 3166-1 alpha-2 country code (e.g., `DE`, `GB`, `US`).
- **Sensitivity:** Default sensitivity for `vat` identifiers is `restricted`. Producers SHOULD explicitly set sensitivity. Validators MUST NOT reject a file for omitting `vat` identifiers.

**Privacy note:** Tax IDs are legally protected data in most jurisdictions. OMTSF files containing `vat` identifiers with `sensitivity: "confidential"` are subject to the selective disclosure rules in OMTSF-SPEC-004.

#### `internal` -- System-Local Identifier

- **Authority:** The issuing system (ERP, procurement platform, internal database)
- **Format:** Opaque string. No format constraints beyond non-empty.
- **Validation:** `authority` field is REQUIRED and MUST be a non-empty string identifying the issuing system.
- **`authority` field:** Required. Free-form string identifying the source system. Recommended convention: `{system-type}-{instance-id}` (e.g., `sap-mm-prod`, `oracle-scm-us`, `ariba-network`).
- **Merge behavior:** `internal` identifiers NEVER trigger cross-file merge. They are scoped to their issuing system and are meaningful only within that context. See OMTSF-SPEC-003, Section 2 for merge identity rules.

### 5.2 Extension Schemes

Conformant validators MAY recognize additional schemes. Extension scheme codes MUST use one of the following patterns to avoid collision with future core schemes:

- **Reverse-domain notation:** `com.example.supplier-id`, `org.gs1.sgln`
- **Known extension codes:**

| Scheme Code | Name | Notes |
|-------------|------|-------|
| `org.opencorporates` | OpenCorporates | Value is `{jurisdiction}/{number}` (e.g., `gb/07228507`) |
| `org.refinitiv.permid` | Refinitiv PermID | Numeric identifier |
| `org.iso.isin` | ISIN | 12-character alphanumeric, ISO 6166 |
| `org.gs1.gtin` | Global Trade Item Number | 8, 12, 13, or 14 digits |

Validators encountering an unrecognized scheme code MUST NOT reject the file. Unknown schemes are passed through without format validation.

### 5.3 Scheme Governance Process

The identifier scheme vocabulary is a controlled registry that requires governance to evolve without fragmenting the ecosystem.

**Adding a new core scheme** requires:
1. A written proposal submitted as a pull request to the OMTSF repository, including: scheme code, issuing authority, format specification, validation rules, coverage estimate, data availability assessment, and at least one production deployment demonstrating use.
2. A 30-day public review period.
3. Approval by the OMTSF Technical Steering Committee (TSC) via lazy consensus (no objection within the review period) or explicit majority vote if objections are raised.

**Criteria for core scheme inclusion:**
- The scheme MUST have a publicly available specification.
- The identifier values MUST NOT be encumbered by intellectual property restrictions that prevent their inclusion in OMTSF files.
- The scheme MUST have demonstrated coverage of a meaningful population of supply chain entities (suggested threshold: 100,000+ entities or regulatory mandate).
- The issuing authority MUST be identifiable and operational.

**Promoting an extension scheme to core** follows the same process as adding a new scheme. Regulatory mandate (e.g., a regulation effectively requiring a particular identifier) is a sufficient basis for promotion.

**Deprecating a core scheme** requires:
1. A written rationale documenting why the scheme should be deprecated (e.g., issuing authority dissolved, scheme superseded).
2. A 90-day notice period.
3. Deprecated schemes remain recognized by validators for at least 2 major spec versions after deprecation.

### 5.4 GLEIF RA List Versioning

The `nat-reg` scheme depends on the GLEIF Registration Authority code list, which is maintained by GLEIF and updated periodically. To decouple OMTSF validation from GLEIF's publication timing:

1. The OMTSF project MUST maintain a versioned snapshot of the GLEIF RA list in the repository (e.g., `data/gleif-ra-list-2026Q1.csv`).
2. Each spec revision MUST reference a specific snapshot version (e.g., "based on GLEIF RA list retrieved 2026-01-15").
3. Snapshots SHOULD be updated quarterly, aligned with GLEIF's publication cadence.
4. **Validator behavior for unknown RA codes:** Validators encountering an `authority` value not present in the referenced snapshot SHOULD emit a warning but MUST NOT reject the file. This ensures that newly added RA codes do not break validation between snapshot updates.
5. The snapshot update process follows the standard pull request workflow and does not require TSC approval.

**Current reference:** GLEIF RA list retrieved 2026-01-15 (700+ registration authorities).

---

## 6. Validation Rules

### 6.1 Level 1 -- Structural Integrity

These rules MUST pass for a file to be considered structurally valid. See also OMTSF-SPEC-001, Section 9 for graph-structural rules.

| Rule | Description |
|------|-------------|
| L1-EID-01 | Every identifier record MUST have a non-empty `scheme` field |
| L1-EID-02 | Every identifier record MUST have a non-empty `value` field |
| L1-EID-03 | For schemes requiring `authority` (`nat-reg`, `vat`, `internal`), the `authority` field MUST be present and non-empty |
| L1-EID-04 | `scheme` MUST be either a core scheme code or a valid extension scheme code (reverse-domain notation) |
| L1-EID-05 | For `lei` scheme: `value` MUST match `^[A-Z0-9]{18}[0-9]{2}$` and MUST pass MOD 97-10 check digit verification |
| L1-EID-06 | For `duns` scheme: `value` MUST match `^[0-9]{9}$` |
| L1-EID-07 | For `gln` scheme: `value` MUST match `^[0-9]{13}$` and MUST pass GS1 mod-10 check digit verification |
| L1-EID-08 | `valid_from` and `valid_to`, if present, MUST be valid ISO 8601 date strings |
| L1-EID-09 | If both `valid_from` and `valid_to` are present, `valid_from` MUST be less than or equal to `valid_to` |
| L1-EID-10 | `sensitivity`, if present, MUST be one of `public`, `restricted`, `confidential` |
| L1-EID-11 | No two identifier records on the same node may have identical `scheme`, `value`, and `authority` |

### 6.2 Level 2 -- Completeness

These rules SHOULD be satisfied. Violations produce warnings, not errors.

| Rule | Description |
|------|-------------|
| L2-EID-01 | Every `organization` node SHOULD have at least one external identifier (scheme other than `internal`) |
| L2-EID-02 | Temporal fields (`valid_from`, `valid_to`) SHOULD be present on all identifier records |
| L2-EID-03 | `nat-reg` authority values SHOULD be valid GLEIF RA codes per the current snapshot (Section 5.4) |
| L2-EID-04 | `vat` authority values SHOULD be valid ISO 3166-1 alpha-2 country codes |
| L2-EID-05 | `lei` values with LAPSED, RETIRED, or MERGED status (when detectable) SHOULD produce a warning |
| L2-EID-06 | `lei` values with ANNULLED status SHOULD produce an error |

### 6.3 Level 3 -- Enrichment

These rules require external data sources and are intended for enrichment tooling, not mandatory validation.

| Rule | Description |
|------|-------------|
| L3-EID-01 | `lei` values SHOULD be verifiable against the GLEIF public database (entity exists and status is not ANNULLED) |
| L3-EID-02 | `nat-reg` values SHOULD be cross-referenceable with the authority's registry |
| L3-EID-03 | If a node has both `lei` and `nat-reg` identifiers, they SHOULD be consistent with GLEIF Level 1 cross-reference data |
| L3-EID-04 | For MERGED LEIs, a `former_identity` edge to the successor entity SHOULD be present |
| L3-EID-05 | DUNS numbers on `organization` nodes SHOULD be HQ-level DUNS, not branch DUNS |

---

## Appendix A: Check Digit Algorithms

### A.1 LEI Check Digit (MOD 97-10, ISO 7064)

1. Replace each letter with its numeric equivalent: A=10, B=11, ..., Z=35.
2. Move the first 4 characters (2-letter prefix + 2-digit check digits) to the end.
3. Compute the integer value modulo 97.
4. Result MUST equal 1.

### A.2 GS1 Check Digit (Mod-10)

For a 13-digit GLN `d1 d2 d3 ... d13`:
1. Multiply odd-positioned digits (d1, d3, d5, ..., d11) by 1 and even-positioned digits (d2, d4, d6, ..., d12) by 3.
2. Sum all products.
3. `d13` = (10 - (sum mod 10)) mod 10.
