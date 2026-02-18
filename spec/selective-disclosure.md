# OMTSF Specification: Selective Disclosure

**Spec:** OMTSF-SPEC-004
**Status:** Draft
**Date:** 2026-02-18
**Revision:** 1
**License:** [CC-BY-4.0](LICENSE)
---

## Related Specifications

| Spec | Relationship |
|------|-------------|
| OMTSF-SPEC-001 (Graph Data Model) | **Prerequisite.** Defines the node types (including `boundary_ref` and `person`) and file header fields (`file_salt`, `disclosure_scope`) governed by this spec. |
| OMTSF-SPEC-002 (Entity Identification) | **Prerequisite.** Defines the `sensitivity` field on identifier records and the canonical string format used for boundary reference hashing. |

---

## 1. Overview

Supply chain graphs contain competitively sensitive information. This specification defines the privacy and selective disclosure model for OMTSF files: how individual identifiers are classified by sensitivity, how files declare their intended audience, how nodes are redacted via boundary references, and the privacy constraints on `person` nodes.

---

## 2. Identifier Sensitivity Levels

Each identifier record (defined in OMTSF-SPEC-002, Section 3) carries an optional `sensitivity` field.

| Level | Meaning | Behavior in Subgraph Projection |
|-------|---------|-------------------------------|
| `public` | No restrictions on sharing | Always included |
| `restricted` | Share only with direct trading partners | MAY be omitted in files shared beyond direct partners |
| `confidential` | Do not share outside the originating organization | MUST be omitted in any file shared externally |

Default sensitivity by scheme:
- `lei`: `public`
- `duns`: `public`
- `gln`: `public`
- `nat-reg`: `restricted`
- `vat`: `restricted`
- `internal`: `restricted`

Producers MAY override defaults by setting `sensitivity` explicitly on any identifier record.

### 2.1 Edge Property Sensitivity

Edge properties (defined in OMTSF-SPEC-001, Sections 5--7) carry sensitivity classifications analogous to identifier sensitivity. The following default sensitivity levels apply to edge properties that contain competitively sensitive information:

| Edge Property | Default Sensitivity | Rationale |
|--------------|-------------------|-----------|
| `contract_ref` | `restricted` | Contract references reveal commercial relationships |
| `annual_value` | `restricted` | Monetary values are competitively sensitive |
| `value_currency` | `restricted` | Currency combined with value enables competitive intelligence |
| `volume` | `restricted` | Supply volumes reveal demand and capacity |
| `volume_unit` | `public` | Unit of measure alone is not sensitive |
| `percentage` (ownership) | `public` | Ownership percentages are typically public record |
| `percentage` (beneficial_ownership) | `confidential` | UBO percentages are protected under AMLD |
| All other edge properties | `public` | Properties like `valid_from`, `commodity`, `scope` are not sensitive by default |

Producers MAY override default sensitivity on any edge property by including a `_property_sensitivity` object inside `properties`:

```json
{
  "properties": {
    "valid_from": "2023-01-15",
    "annual_value": 2500000,
    "_property_sensitivity": {
      "annual_value": "confidential"
    }
  }
}
```

When generating files with `disclosure_scope: "public"`, edges MUST omit properties with sensitivity `restricted` or `confidential`. When generating files with `disclosure_scope: "partner"`, edges MUST omit properties with sensitivity `confidential`. The `_property_sensitivity` object itself MUST be omitted from files with `disclosure_scope: "public"`.

---

## 3. Disclosure Scope

Files MAY declare a `disclosure_scope` in the file header (defined in OMTSF-SPEC-001, Section 2) to indicate the intended audience:

| Scope | Meaning |
|-------|---------|
| `internal` | For use within the originating organization only |
| `partner` | Shared with direct trading partners |
| `public` | Shared without restriction |

When `disclosure_scope` is declared:
- If `disclosure_scope` is `public`: the file MUST NOT contain identifiers with `sensitivity: "confidential"` or `sensitivity: "restricted"`. `person` nodes MUST NOT be present (see Section 5).
- If `disclosure_scope` is `partner`: the file MUST NOT contain identifiers with `sensitivity: "confidential"`.

Validators MUST enforce these constraints at Level 1 when `disclosure_scope` is present.

---

## 4. Boundary References (Redacted Nodes)

When a node is redacted in a subgraph projection (the file represents only a portion of the full graph), the redacted node is replaced with a **boundary reference**: a minimal node stub that preserves graph connectivity without revealing the entity's identity.

A boundary reference node:
- Has `type` set to `boundary_ref`
- Has a single identifier with `scheme` set to `opaque`
- The `value` of the opaque identifier is computed as follows:

**Hash computation:**

1. Collect all `public` identifiers on the original node.
2. Compute the canonical string form of each identifier (OMTSF-SPEC-002, Section 4).
3. Sort the canonical strings lexicographically by UTF-8 byte order.
4. Join the sorted strings with a newline delimiter (`0x0A`).
5. If the resulting string is **non-empty**: `value` = hex-encoded `SHA-256(joined_string_bytes || file_salt_bytes)`
6. If the resulting string is **empty** (the node has no `public` identifiers): `value` = hex-encoded random 32-byte token generated by a CSPRNG. This ensures that each restricted-only entity produces a unique boundary reference, preventing the collision where all such entities would otherwise hash to the same value.

**`file_salt`** is a 32-byte value generated by a cryptographically secure pseudorandom number generator (CSPRNG, e.g., `/dev/urandom`, `getrandom(2)`, `crypto.getRandomValues()`). It is included in the file header as a 64-character lowercase hexadecimal string.

**Test vectors:**

Given identifiers:
- `lei:5493006MHB84DD0ZWV18` (public)
- `duns:081466849` (public)
- `vat:DE:DE123456789` (restricted, excluded from hash)

And `file_salt` = `0x00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff`:

1. Public canonical strings: `duns:081466849`, `lei:5493006MHB84DD0ZWV18`
2. Sorted: `duns:081466849`, `lei:5493006MHB84DD0ZWV18`
3. Joined: `duns:081466849\nlei:5493006MHB84DD0ZWV18`
4. Hash input: UTF-8 bytes of joined string || raw salt bytes
5. `value` = `SHA-256(hash_input)` = `e8798687b081da98b7cd1c4e5e2423bd3214fbab0f1f476a2dcdbf67c2e21141`

**Test vector 2: Single identifier**

Given identifiers:
- `lei:5493006MHB84DD0ZWV18` (public)

And `file_salt` = `0x00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff`:

1. Public canonical strings: `lei:5493006MHB84DD0ZWV18`
2. (Only one — no sorting needed)
3. Joined: `lei:5493006MHB84DD0ZWV18`
4. Hash input: UTF-8 bytes of joined string || raw salt bytes
5. `value` = `SHA-256(hash_input)` = `7849e55c4381ba852a2ada50f15e58d871de085893b7be8826f75560854c78c8`

**Test vector 3: Identifier requiring percent-encoding**

Given identifiers:
- `nat-reg:RA000548:HRB%3A86891` (public — a registry number containing a literal colon, percent-encoded per OMTSF-SPEC-002, Section 4)

And `file_salt` = `0x00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff`:

1. Public canonical strings: `nat-reg:RA000548:HRB%3A86891`
2. (Only one — no sorting needed)
3. Joined: `nat-reg:RA000548:HRB%3A86891`
4. Hash input: UTF-8 bytes of joined string || raw salt bytes
5. `value` = `SHA-256(hash_input)` = `7b33571d3bba150f4dfd9609c38b4f9acc9a3a8dbfa3121418a35264562ca5d9`

**Test vector 4: No public identifiers (random token path)**

Given identifiers:
- `internal:sap-prod:V-100234` (restricted — excluded from hash)
- `vat:DE:DE123456789` (restricted — excluded from hash)

No public identifiers exist. The boundary reference `value` MUST be a hex-encoded random 32-byte token generated by a CSPRNG. No deterministic test vector is possible for this path; implementations MUST verify that the output is a 64-character lowercase hexadecimal string.

This design prevents enumeration attacks: an adversary cannot hash known LEIs to discover whether a specific entity appears in the redacted graph, because the salt is file-specific.

**Boundary reference stability.** Fresh salt per file means the same entity's boundary reference hash changes with every export, preventing cross-file correlation of redacted entities. If sensitivity reclassification is necessary, the file SHOULD be re-exported with a fresh salt. This is an explicit tradeoff: privacy over temporal tracking of redacted entities.

---

## 5. Person Node Privacy Rules

`person` nodes (defined in OMTSF-SPEC-001, Section 4.4) are subject to additional privacy constraints reflecting GDPR data minimization requirements:

- All identifiers on `person` nodes default to `sensitivity: "confidential"` regardless of scheme-level defaults. Producers MAY override to `restricted` where legally permitted.
- `person` nodes MUST be omitted entirely (not replaced with boundary references) when generating files with `disclosure_scope: "public"`. This reflects GDPR data minimization requirements.
- Producers MUST assess whether including `person` nodes complies with applicable data protection law (GDPR, CCPA, etc.) before generating files.
- `beneficial_ownership` edges (OMTSF-SPEC-001, Section 5.5) inherit the sensitivity constraints of `person` nodes. They default to `sensitivity: "confidential"` and MUST be omitted from files with `disclosure_scope: "public"`.

---

## 6. Validation Rules

### 6.1 Level 1 -- Structural Integrity

These rules MUST pass for a file to be considered structurally valid.

| Rule | Description |
|------|-------------|
| L1-SDI-01 | `boundary_ref` nodes MUST have exactly one identifier with `scheme: "opaque"` |
| L1-SDI-02 | If `disclosure_scope` is declared, sensitivity constraints (Section 3) MUST be satisfied |
