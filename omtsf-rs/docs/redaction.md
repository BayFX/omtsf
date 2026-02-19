# omtsf-cli Technical Specification: Selective Disclosure / Redaction Engine

**Status:** Draft
**Date:** 2026-02-19

---

## 1. Purpose

This document specifies the selective disclosure and redaction engine implemented in `omtsf-core`. The engine transforms an `.omts` file from a higher-trust scope (typically `internal`) to a lower-trust scope (`partner` or `public`) by removing sensitive identifiers, stripping confidential edge properties, replacing out-of-scope nodes with boundary references, and omitting person nodes and beneficial ownership edges where required.

The redaction engine is invoked by `omtsf redact <file> --scope <partner|public>`. It reads a complete graph, applies scope-dependent filtering, and emits a valid `.omts` file on stdout. The output must itself pass L1 validation -- redaction must never produce a structurally invalid file.

---

## 2. Sensitivity Classification

### 2.1 Identifier Sensitivity Defaults

Every identifier record carries a `sensitivity` field (SPEC-002 Section 3). When the field is absent, the engine applies scheme-based defaults:

| Scheme | Default Sensitivity |
|--------|-------------------|
| `lei` | `public` |
| `duns` | `public` |
| `gln` | `public` |
| `nat-reg` | `restricted` |
| `vat` | `restricted` |
| `internal` | `restricted` |
| Any unrecognized scheme | `public` |

An explicit `sensitivity` value on the identifier record always overrides the scheme default.

### 2.2 Person Node Override

All identifiers on `person` nodes default to `confidential` regardless of scheme defaults. An explicit override to `restricted` is permitted; an override to `public` is not (validators should flag this as semantically suspect, but the engine respects what the file declares).

### 2.3 Edge Property Sensitivity Defaults

Edge properties carry sensitivity analogous to identifiers. Defaults:

| Property | Default |
|----------|---------|
| `contract_ref` | `restricted` |
| `annual_value` | `restricted` |
| `value_currency` | `restricted` |
| `volume` | `restricted` |
| `volume_unit` | `public` |
| `percentage` (on `ownership`) | `public` |
| `percentage` (on `beneficial_ownership`) | `confidential` |
| All other properties | `public` |

Producers may override defaults via a `_property_sensitivity` object inside `properties`. The engine reads this object first when determining per-property sensitivity.

---

## 3. Disclosure Scopes and Filtering Rules

The engine accepts a target scope and applies the following constraints:

### 3.1 `partner` Scope

- **Identifiers:** Remove all identifiers with effective sensitivity `confidential`. Retain `public` and `restricted`.
- **Edge properties:** Remove properties with effective sensitivity `confidential`. Retain `restricted` and `public`. The `_property_sensitivity` object is retained (it may be useful to the partner).
- **Person nodes:** Retain, but their identifiers are filtered per the rules above (all default to `confidential`, so most will be stripped).
- **Beneficial ownership edges:** Retain, but property filtering applies (`percentage` defaults to `confidential` on these edges).
- **Nodes with no remaining identifiers after filtering:** If a node loses all identifiers but is still within scope (not a `person` node in `public` scope), it is retained with an empty `identifiers` array. It is NOT replaced with a boundary reference -- boundary references are for nodes the producer explicitly chooses to redact from the subgraph, not for nodes that merely lost some identifiers.

### 3.2 `public` Scope

- **Identifiers:** Remove all identifiers with effective sensitivity `confidential` OR `restricted`. Retain only `public`.
- **Edge properties:** Remove properties with effective sensitivity `confidential` or `restricted`. The `_property_sensitivity` object itself is omitted entirely.
- **Person nodes:** Omit entirely. Not replaced with boundary references -- they are simply absent from the output graph.
- **Beneficial ownership edges:** Omit entirely.
- **Edges referencing omitted nodes:** Any edge whose `source` or `target` references an omitted `person` node is itself omitted. The engine must not produce dangling edge references.

### 3.3 `internal` Scope

No filtering. The file is emitted as-is. This is a no-op path, useful for validation-only workflows.

---

## 4. Boundary Reference Hashing

When the producer designates a node for redaction (the node falls outside the subgraph being exported), the engine replaces it with a `boundary_ref` node. The opaque identifier value is computed deterministically from the node's public identifiers and the file salt.

### 4.1 Algorithm

1. Collect all identifiers on the original node whose effective sensitivity is `public`.
2. Compute the canonical string form of each identifier per SPEC-002 Section 4. The canonical form is `{scheme}:{value}` for schemes without a required authority, or `{scheme}:{authority}:{value}` for schemes requiring authority (`nat-reg`, `vat`, `internal`). Colons within authority or value fields are percent-encoded as `%3A`; percent signs as `%25`; newlines as `%0A`; carriage returns as `%0D`.
3. Sort the canonical strings lexicographically by UTF-8 byte order. This is a plain byte-wise comparison -- no Unicode collation.
4. Join the sorted strings with a single newline byte (`0x0A`).
5. If the joined string is **non-empty**: concatenate the UTF-8 bytes of the joined string with the raw 32 bytes of `file_salt` (decoded from hex). Compute SHA-256 over this concatenation. The boundary reference value is the lowercase hex encoding of the 32-byte digest.
6. If the joined string is **empty** (the node has zero public identifiers): generate 32 random bytes from a CSPRNG and hex-encode them. The result must be a 64-character lowercase hexadecimal string.

### 4.2 Rust Implementation Notes

Use the `sha2` crate (pure Rust, no C dependencies, compiles to wasm32-unknown-unknown) for SHA-256. Salt decoding from hex uses `hex` crate or a manual two-character-per-byte parser in `omtsf-core`. CSPRNG uses `getrandom` (which delegates to the platform's secure random source, including `crypto.getRandomValues` on wasm32). Do not pull in `ring` or `openssl` -- both have C/asm components that complicate wasm compilation.

```rust
// Illustrative, not compilable
fn boundary_ref_value(public_ids: &[CanonicalId], salt: &[u8; 32]) -> String {
    if public_ids.is_empty() {
        let mut buf = [0u8; 32];
        getrandom::fill(&mut buf).expect("CSPRNG failure");
        return hex::encode(buf);
    }
    let mut canonicals: Vec<String> = public_ids.iter().map(|id| id.canonical_form()).collect();
    canonicals.sort_unstable();  // byte-order sort on UTF-8
    let joined = canonicals.join("\n");
    let mut hasher = Sha256::new();
    hasher.update(joined.as_bytes());
    hasher.update(salt);
    hex::encode(hasher.finalize())
}
```

### 4.3 Test Vectors

All vectors use `file_salt` = `0x00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff`.

**TV1: Multiple public identifiers, one restricted (excluded)**

Input identifiers:
- `lei:5493006MHB84DD0ZWV18` (public)
- `duns:081466849` (public)
- `vat:DE:DE123456789` (restricted, excluded from hash)

Canonical strings (public only): `duns:081466849`, `lei:5493006MHB84DD0ZWV18`

Sorted: `duns:081466849`, `lei:5493006MHB84DD0ZWV18`

Joined: `duns:081466849\nlei:5493006MHB84DD0ZWV18`

SHA-256 = `e8798687b081da98b7cd1c4e5e2423bd3214fbab0f1f476a2dcdbf67c2e21141`

**TV2: Single identifier**

Input identifiers:
- `lei:5493006MHB84DD0ZWV18` (public)

Canonical string: `lei:5493006MHB84DD0ZWV18`

Joined: `lei:5493006MHB84DD0ZWV18`

SHA-256 = `7849e55c4381ba852a2ada50f15e58d871de085893b7be8826f75560854c78c8`

**TV3: Identifier requiring percent-encoding**

Input identifiers:
- `nat-reg:RA000548:HRB%3A86891` (public -- registry number contains a literal colon, percent-encoded per SPEC-002 Section 4)

Canonical string: `nat-reg:RA000548:HRB%3A86891`

Joined: `nat-reg:RA000548:HRB%3A86891`

SHA-256 = `7b33571d3bba150f4dfd9609c38b4f9acc9a3a8dbfa3121418a35264562ca5d9`

**TV4: No public identifiers (random token)**

Input identifiers:
- `internal:sap-prod:V-100234` (restricted, excluded)
- `vat:DE:DE123456789` (restricted, excluded)

No public identifiers exist. Value = hex-encoded 32-byte CSPRNG token. Output must be a 64-character lowercase hexadecimal string. This value is non-deterministic; tests verify format only.

### 4.4 Salt Handling

The `file_salt` field is a 64-character lowercase hex string in the JSON header. The engine decodes it to a 32-byte array before hashing. Validation rules (SPEC-001 Section 9.4) require the salt to match `^[0-9a-f]{64}$`. If the salt fails this check, the engine must reject the file before redaction begins.

When the CLI generates a new file via `omtsf init`, it produces a fresh salt from the platform CSPRNG. The engine never reuses a salt across files -- fresh salt per export is the privacy-critical invariant that prevents cross-file correlation of boundary references.

---

## 5. Node Classification and Redaction Decisions

The engine classifies each node in the input graph into one of three dispositions:

| Disposition | Meaning |
|-------------|---------|
| **Retain** | Node appears in output, possibly with filtered identifiers and properties. |
| **Replace** | Node is replaced with a `boundary_ref` stub. Original identifiers, name, and properties are stripped. |
| **Omit** | Node is removed entirely. All edges referencing it are also removed. |

Classification rules by node type and target scope:

| Node Type | `partner` Scope | `public` Scope |
|-----------|----------------|---------------|
| `organization` | Retain or Replace (producer choice) | Retain or Replace |
| `facility` | Retain or Replace | Retain or Replace |
| `good` | Retain or Replace | Retain or Replace |
| `consignment` | Retain or Replace | Retain or Replace |
| `attestation` | Retain or Replace | Retain or Replace |
| `person` | Retain (identifiers filtered) | **Omit** |
| `boundary_ref` | Pass through | Pass through |

The Retain-vs-Replace decision for non-person nodes is a producer choice -- the `omtsf redact` command accepts a set of node IDs to retain (the subgraph), and everything else outside that set is replaced with boundary references.

### 5.1 Boundary Reference Node Structure

A replaced node becomes:

```json
{
  "id": "<original-node-id>",
  "type": "boundary_ref",
  "identifiers": [
    {
      "scheme": "opaque",
      "value": "<computed-hash-or-random-token>"
    }
  ]
}
```

The `id` field retains the original graph-local ID so that existing edges remain valid without rewriting source/target references. The node carries exactly one identifier with `scheme: "opaque"` (L1-SDI-01).

---

## 6. Edge Handling During Redaction

### 6.1 Boundary-Crossing Edges

When an edge connects a retained node to a replaced (boundary_ref) node, the edge is preserved. This is the primary purpose of boundary references: maintaining graph connectivity at the subgraph boundary.

### 6.2 Both Endpoints Replaced

When both the source and target of an edge are replaced with boundary references, the edge is **omitted**. An edge between two opaque stubs carries no useful information and leaks the existence of a relationship between two redacted entities.

### 6.3 Edges Referencing Omitted Nodes

When either endpoint of an edge references an omitted node (a `person` node in `public` scope), the edge is omitted. This applies transitively: if a `beneficial_ownership` edge references a `person` node, both the person and the edge disappear.

### 6.4 Edge Type Filtering

In `public` scope, `beneficial_ownership` edges are unconditionally omitted regardless of endpoint disposition. This is a blanket rule from SPEC-004 Section 5.

### 6.5 Property Stripping on Retained Edges

For edges that survive the filtering pass, properties are stripped according to the target scope's sensitivity threshold:

- **`partner` scope:** Remove properties with effective sensitivity `confidential`.
- **`public` scope:** Remove properties with effective sensitivity `confidential` or `restricted`. Also remove the `_property_sensitivity` object.

If stripping removes a required property for the edge type (e.g., `percentage` on `ownership`), the edge remains valid -- the property becomes absent, which is acceptable since the output file represents a filtered view. Validators should understand that redacted files may have sparser property sets than internal files.

---

## 7. Output Validation

### 7.1 Post-Redaction Invariants

The engine runs a validation pass on the output before emitting it. The following invariants must hold:

1. **No dangling edges.** Every edge `source` and `target` must reference a node `id` present in the output.
2. **Boundary ref structure.** Every `boundary_ref` node has exactly one identifier with `scheme: "opaque"` (L1-SDI-01).
3. **Scope consistency.** If the output declares `disclosure_scope`, the sensitivity constraints from SPEC-004 Section 3 must be satisfied (L1-SDI-02). The engine sets `disclosure_scope` on the output header to match the target scope.
4. **No person nodes in public output.** If the target scope is `public`, the output must contain zero nodes with `type: "person"`.
5. **No beneficial_ownership edges in public output.** If the target scope is `public`, the output must contain zero edges with `type: "beneficial_ownership"`.
6. **Salt preserved.** The output file retains the original `file_salt`. Boundary reference hashes are only meaningful relative to the salt that produced them.

### 7.2 Boundary Reference Consistency

If the same original node is referenced by multiple edges, the engine must produce exactly one boundary_ref node for it, not multiple copies. The hash computation is deterministic (same public identifiers + same salt = same hash), so this is a correctness check on the engine's node deduplication logic.

For the CSPRNG path (no public identifiers), the engine must generate the random token once per node and reuse it for all references. Generating a fresh token per edge reference would create multiple boundary_ref nodes for the same entity.

---

## 8. Security Considerations

### 8.1 Graph Structure Leakage

Boundary references preserve graph topology. An adversary receiving a redacted file can count the number of boundary_ref nodes, observe their degree (how many edges connect to them), and infer structural properties of the hidden portion of the graph. For example, a boundary_ref with high in-degree on `supplies` edges reveals a hub supplier even if the entity's identity is hidden.

This is an inherent tradeoff in the OMTSF design: boundary references exist to preserve connectivity for downstream graph analysis. Producers concerned about topology leakage should consider omitting edges to boundary refs entirely (producing a disconnected subgraph) rather than using boundary references. The engine does not make this decision automatically -- it is a producer policy choice.

### 8.2 Timing Side-Channels

The boundary reference hash computation branches on whether public identifiers exist (SHA-256 path vs. CSPRNG path). This branch is observable via timing. In practice, this is not a meaningful threat: the attacker would need to observe the redaction process itself, and the branch condition (presence of public identifiers) is already visible in the output (deterministic hash vs. random token). Nonetheless, implementations should avoid data-dependent timing in the SHA-256 computation itself. The `sha2` crate's pure-Rust implementation does not use secret-dependent branches.

### 8.3 Salt Entropy

The file salt must be generated by a CSPRNG with at least 256 bits of entropy. A weak or predictable salt enables precomputation attacks: an adversary who knows the salt can hash all LEIs in the GLEIF database and match boundary references to entities. The `getrandom` crate provides the correct entropy source on all supported platforms (Linux: `getrandom(2)`, macOS: `getentropy(2)`, Windows: `BCryptGenRandom`, WASM: `crypto.getRandomValues`).

The salt is visible in the file header. Its purpose is not secrecy -- it is anti-enumeration. The salt forces the adversary to perform a fresh brute-force pass for every file, rather than building a single rainbow table. With 2^256 possible salts, precomputation is infeasible.

### 8.4 Threat Model

The redaction engine operates under the following threat model:

**Trusted:** The producer (the entity running `omtsf redact`). The producer has access to the full unredacted graph and decides which nodes to retain, replace, or omit.

**Untrusted:** The recipient of the redacted file. The recipient may attempt to:
- Reverse boundary reference hashes to recover entity identities (mitigated by salt).
- Correlate boundary references across files from the same producer (mitigated by fresh salt per file).
- Infer sensitive properties from graph structure (partially mitigated by edge omission rules; topology leakage remains, see Section 8.1).
- Tamper with the file to inject false data (out of scope for the redaction engine; integrity is addressed by digital signatures, which are a separate concern).

**Out of scope:** The engine does not protect against a compromised producer. If the producer's system is compromised, the adversary has access to the unredacted graph and the redaction engine provides no additional protection. The engine also does not enforce access control -- it transforms files, it does not gate access to them.
