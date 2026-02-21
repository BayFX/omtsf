# omtsf-core Technical Specification: Selective Disclosure / Redaction Engine

**Status:** Draft
**Date:** 2026-02-21

---

## 1. Purpose

This document specifies the selective disclosure and redaction engine in `omtsf-core`. The engine transforms an `.omts` file from a higher-trust scope (typically `internal`) to a lower-trust scope (`partner` or `public`) by removing sensitive identifiers, stripping confidential edge properties, replacing out-of-scope nodes with boundary references, and omitting person nodes and beneficial ownership edges where required. Invoked via `omtsf redact <file> --scope <partner|public>`, it emits a valid `.omts` on stdout that must pass L1 validation.

The engine lives entirely in `omtsf-core` (no filesystem I/O, no stdout/stderr, WASM-compatible). The CLI layer in `omtsf-cli` handles argument parsing, file reading, and output serialization.

---

## 2. Sensitivity Classification

### 2.1 Identifier Sensitivity Defaults

Every identifier record carries an optional `sensitivity` field (SPEC-002 Section 3). When absent, the engine applies scheme-based defaults:

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

The resolution function `effective_sensitivity` implements this three-level cascade:

```rust
pub fn effective_sensitivity(identifier: &Identifier, node_type: &NodeTypeTag) -> Sensitivity {
    // 1. Explicit override always wins.
    if let Some(explicit) = &identifier.sensitivity {
        return explicit.clone();
    }
    // 2. Person-node rule (Section 2.2).
    if let NodeTypeTag::Known(NodeType::Person) = node_type {
        return Sensitivity::Confidential;
    }
    // 3. Scheme-based default.
    scheme_default(&identifier.scheme)
}

fn scheme_default(scheme: &str) -> Sensitivity {
    match scheme {
        "lei" | "duns" | "gln" => Sensitivity::Public,
        "nat-reg" | "vat" | "internal" => Sensitivity::Restricted,
        _ => Sensitivity::Public,
    }
}
```

### 2.2 Person Node Override

All identifiers on `person` nodes default to `confidential` regardless of scheme defaults. An explicit override to `restricted` is permitted; an override to `public` is technically respected by the engine (the file declares it), but validators should flag this as semantically suspect. The person-node rule fires only when `identifier.sensitivity` is `None`; explicit values always take priority.

### 2.3 Edge Property Sensitivity Defaults

Edge properties carry sensitivity analogous to identifiers. The following defaults apply:

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

Producers may override defaults via a `_property_sensitivity` JSON object inside the edge's `properties.extra` map. The engine reads this object first when determining per-property sensitivity.

Resolution order: (1) consult the `_property_sensitivity` override map on the edge; (2) fall through to the default table, where `percentage` dispatches on edge type.

```rust
pub fn effective_property_sensitivity(edge: &Edge, property_name: &str) -> Sensitivity {
    // Step 1: consult the _property_sensitivity override map.
    if let Some(override_val) = read_property_sensitivity_override(edge, property_name) {
        return override_val;
    }
    // Step 2: apply the default table.
    property_default(&edge.edge_type, property_name)
}

fn property_default(edge_type: &EdgeTypeTag, property_name: &str) -> Sensitivity {
    match property_name {
        "contract_ref" | "annual_value" | "value_currency" | "volume" => Sensitivity::Restricted,
        "volume_unit" => Sensitivity::Public,
        "percentage" => match edge_type {
            EdgeTypeTag::Known(EdgeType::Ownership) => Sensitivity::Public,
            EdgeTypeTag::Known(EdgeType::BeneficialOwnership) => Sensitivity::Confidential,
            _ => Sensitivity::Public,
        },
        _ => Sensitivity::Public,
    }
}
```

The `read_property_sensitivity_override` helper examines `edge.properties.extra["_property_sensitivity"]`, looks up the property name as a key, and parses the string value as a `Sensitivity` variant. Unrecognized strings (e.g. `"ultra-secret"`) silently fall through to the default table rather than producing an error.

---

## 3. Disclosure Scopes and Filtering Rules

The engine accepts a target scope and applies the following constraints:

### 3.1 `partner` Scope

- **Identifiers:** Remove all identifiers with effective sensitivity `confidential`. Retain `public` and `restricted`.
- **Edge properties:** Remove properties with effective sensitivity `confidential`. Retain `restricted` and `public`. The `_property_sensitivity` object is retained (it may be useful to the partner).
- **Person nodes:** Retain, but their identifiers are filtered per the rules above (all default to `confidential`, so most will be stripped).
- **Beneficial ownership edges:** Retain, but property filtering applies (`percentage` defaults to `confidential` on these edges).
- **Nodes with no remaining identifiers:** Retained with `identifiers: None`. Not replaced with a boundary reference.

### 3.2 `public` Scope

- **Identifiers:** Remove all identifiers with effective sensitivity `confidential` OR `restricted`. Retain only `public`.
- **Edge properties:** Remove properties with effective sensitivity `confidential` or `restricted`. The `_property_sensitivity` object itself is omitted entirely.
- **Person nodes:** Omit entirely. Not replaced with boundary references -- they are simply absent from the output graph.
- **Beneficial ownership edges:** Omit entirely (SPEC-004 Section 5).
- **Edges referencing omitted nodes:** Any edge whose `source` or `target` references an omitted `person` node is itself omitted. The engine must not produce dangling edge references.

### 3.3 `internal` Scope

No filtering. The file is returned as-is with `disclosure_scope` set to `internal`. This is a no-op path, useful for validation-only workflows.

### 3.4 Sensitivity Gate Function

The core predicate used throughout identifier and property filtering:

```rust
fn sensitivity_allowed(sensitivity: &Sensitivity, scope: &DisclosureScope) -> bool {
    match scope {
        DisclosureScope::Internal => true,
        DisclosureScope::Partner => match sensitivity {
            Sensitivity::Public | Sensitivity::Restricted => true,
            Sensitivity::Confidential => false,
        },
        DisclosureScope::Public => match sensitivity {
            Sensitivity::Public => true,
            Sensitivity::Restricted | Sensitivity::Confidential => false,
        },
    }
}
```

---

## 4. Boundary Reference Hashing

When the producer designates a node for redaction (the node falls outside the subgraph being exported), the engine replaces it with a `boundary_ref` node. The opaque identifier value is computed deterministically from the node's public identifiers and the file salt.

### 4.1 Algorithm

1. Collect all identifiers on the original node whose effective sensitivity is `public`.
2. Compute the canonical string form of each identifier per SPEC-002 Section 4. The canonical form is `{scheme}:{value}` for schemes without a required authority, or `{scheme}:{authority}:{value}` for schemes requiring authority (`nat-reg`, `vat`). Colons within authority or value fields are percent-encoded as `%3A`; percent signs as `%25`; newlines as `%0A`; carriage returns as `%0D`.
3. Sort the canonical strings lexicographically by UTF-8 byte order. This is a plain byte-wise comparison -- no Unicode collation.
4. Join the sorted strings with a single newline byte (`0x0A`).
5. If the joined string is **non-empty**: concatenate the UTF-8 bytes of the joined string with the raw 32 bytes of `file_salt` (decoded from hex). Compute SHA-256 over this concatenation. The boundary reference value is the lowercase hex encoding of the 32-byte digest.
6. If the joined string is **empty** (the node has zero public identifiers): generate 32 random bytes from a CSPRNG and hex-encode them. The result must be a 64-character lowercase hexadecimal string.

### 4.2 Implementation

Use `sha2` (pure Rust, WASM-compatible) for SHA-256, `getrandom` for CSPRNG (delegates to `crypto.getRandomValues` on wasm32), and a hand-written hex codec to avoid external crate dependencies. Do not use `ring` or `openssl` -- both have C/asm components that break wasm compilation.

```rust
use sha2::{Digest, Sha256};

pub fn boundary_ref_value(
    public_ids: &[CanonicalId],
    salt: &[u8; 32],
) -> Result<String, BoundaryHashError> {
    if public_ids.is_empty() {
        // Random path: 32 CSPRNG bytes, hex-encoded.
        let mut buf = [0u8; 32];
        getrandom::getrandom(&mut buf).map_err(BoundaryHashError::CsprngFailure)?;
        return Ok(hex_encode(&buf));
    }

    // Deterministic path: sort, join, hash with salt.
    let mut canonicals: Vec<&str> = public_ids.iter().map(CanonicalId::as_str).collect();
    canonicals.sort_unstable(); // UTF-8 byte-order sort

    let joined = canonicals.join("\n");

    let mut hasher = Sha256::new();
    hasher.update(joined.as_bytes());
    hasher.update(salt.as_slice());

    Ok(hex_encode(&hasher.finalize()))
}
```

The `hex_encode` function is a hand-rolled constant-time-friendly encoder that produces only lowercase hex:

```rust
fn hex_encode(bytes: &[u8]) -> String {
    const HEX_CHARS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX_CHARS[(b >> 4) as usize] as char);
        out.push(HEX_CHARS[(b & 0x0f) as usize] as char);
    }
    out
}
```

The salt decoder `hex_decode_salt` rejects uppercase hex digits, matching the `FileSalt` newtype invariant (`^[0-9a-f]{64}$`).

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

Hash input: UTF-8 bytes of `duns:081466849\nlei:5493006MHB84DD0ZWV18` || raw 32 salt bytes

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

The `file_salt` is a 64-character lowercase hex string in the JSON header, decoded to a 32-byte array before hashing. The salt must match `^[0-9a-f]{64}$` (SPEC-001 Section 9.4); the decoder rejects uppercase hex digits, matching the `FileSalt` newtype invariant. If decoding fails, the engine returns `BoundaryHashError::InvalidSalt` before redaction begins.

When `omtsf init` generates a new file, it produces a fresh salt from the platform CSPRNG via `generate_file_salt()`. Fresh salt per export prevents cross-file correlation of boundary references.

```rust
pub fn decode_salt(salt: &FileSalt) -> Result<[u8; 32], BoundaryHashError> {
    hex_decode_salt(salt.as_ref())
}

pub fn generate_file_salt() -> Result<FileSalt, BoundaryHashError> {
    let mut buf = [0u8; 32];
    getrandom::getrandom(&mut buf).map_err(BoundaryHashError::CsprngFailure)?;
    let hex = hex_encode(&buf);
    FileSalt::try_from(hex.as_str())
        .map_err(|e| BoundaryHashError::InvalidSalt(e.to_string()))
}
```

---

## 5. Node Classification and Redaction Decisions

The engine classifies each node in the input graph into one of three dispositions:

| Disposition | Meaning |
|-------------|---------|
| **Retain** | Node appears in output, possibly with filtered identifiers and properties. |
| **Replace** | Node is replaced with a `boundary_ref` stub. Original identifiers, name, and all node properties are stripped. |
| **Omit** | Node is removed entirely. All edges referencing it are also removed. |

### 5.1 Classification Algorithm

Classification is a two-step process.

**Step 1: Base classification by node type and target scope.** The function `classify_node` determines whether the node is *eligible* for retention or must be omitted:

```rust
pub fn classify_node(node: &Node, target_scope: &DisclosureScope) -> NodeAction {
    match target_scope {
        DisclosureScope::Internal | DisclosureScope::Partner => NodeAction::Retain,
        DisclosureScope::Public => match &node.node_type {
            NodeTypeTag::Known(NodeType::Person) => NodeAction::Omit,
            _ => NodeAction::Retain,
        },
    }
}
```

| Node Type | `partner` Scope | `public` Scope |
|-----------|----------------|---------------|
| `organization` | Retain or Replace (producer choice) | Retain or Replace |
| `facility` | Retain or Replace | Retain or Replace |
| `good` | Retain or Replace | Retain or Replace |
| `consignment` | Retain or Replace | Retain or Replace |
| `attestation` | Retain or Replace | Retain or Replace |
| `person` | Retain (identifiers filtered) | **Omit** |
| `boundary_ref` | Pass through | Pass through |
| Extension types | Retain or Replace | Retain or Replace |

**Step 2: Producer choice promotion.** The caller supplies a `retain_ids: HashSet<NodeId>` identifying which nodes to keep in the output. The pipeline promotes `Retain` to `Replace` for nodes absent from `retain_ids`, with one exception: existing `boundary_ref` nodes always pass through regardless of `retain_ids` membership:

```rust
let action = match classify_node(node, &scope) {
    NodeAction::Omit => NodeAction::Omit,
    _ => {
        let is_bref = matches!(&node.node_type, NodeTypeTag::Known(NodeType::BoundaryRef));
        if is_bref || retain_ids.contains(&node.id) {
            NodeAction::Retain
        } else {
            NodeAction::Replace
        }
    }
};
```

### 5.2 Boundary Reference Node Structure

A replaced node becomes a minimal `boundary_ref` stub:

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

The `id` is preserved from the original node so existing edge `source`/`target` references remain valid. The node carries exactly one `opaque` identifier (L1-SDI-01). All other node fields (`name`, `jurisdiction`, `labels`, `data_quality`, etc.) are set to `None`.

---

## 6. Edge Handling During Redaction

### 6.1 Boundary-Crossing Edges

An edge connecting a retained node to a replaced (`boundary_ref`) node is preserved -- this is the primary purpose of boundary references. It allows the recipient to see that the retained entity has a relationship to some unknown entity at the boundary.

### 6.2 Both Endpoints Replaced

When both endpoints are replaced with boundary references, the edge is **omitted**. An edge between two opaque stubs leaks relationship existence with no informational value to the recipient.

### 6.3 Edges Referencing Omitted Nodes

When either endpoint references an omitted node (e.g. a `person` node in `public` scope), the edge is omitted. This is critical for maintaining structural validity: the output must not contain dangling edge references.

### 6.4 Edge Type Filtering

In `public` scope, `beneficial_ownership` edges are unconditionally omitted regardless of endpoint disposition (SPEC-004 Section 5). This rule fires before endpoint checks, ensuring these edges are dropped even if both endpoints happen to be retained.

### 6.5 Edge Classification Function

The rules above are applied in priority order:

```rust
pub fn classify_edge(
    edge: &Edge,
    source_action: &NodeAction,
    target_action: &NodeAction,
    target_scope: &DisclosureScope,
) -> EdgeAction {
    // Section 6.4: beneficial_ownership unconditionally omitted in public scope.
    if matches!(target_scope, DisclosureScope::Public) {
        if let EdgeTypeTag::Known(EdgeType::BeneficialOwnership) = &edge.edge_type {
            return EdgeAction::Omit;
        }
    }
    // Section 6.3: either endpoint omitted.
    if matches!(source_action, NodeAction::Omit) || matches!(target_action, NodeAction::Omit) {
        return EdgeAction::Omit;
    }
    // Section 6.2: both endpoints replaced.
    if matches!(source_action, NodeAction::Replace)
        && matches!(target_action, NodeAction::Replace)
    {
        return EdgeAction::Omit;
    }
    // Section 6.1 + both-Retain case.
    EdgeAction::Retain
}
```

### 6.6 Property Stripping on Retained Edges

For edges that survive the classification pass, properties are stripped according to the target scope's sensitivity threshold:

- **`partner` scope:** Remove properties with effective sensitivity `confidential`. Retain `restricted` and `public`. Keep `_property_sensitivity` object.
- **`public` scope:** Remove properties with effective sensitivity `confidential` or `restricted`. Also remove the `_property_sensitivity` object entirely from `extra`.

Each named field on `EdgeProperties` is checked individually via `effective_property_sensitivity`. Extension fields in `extra` are also checked. If stripping removes a property that would normally be required for the edge type, the edge remains valid -- redacted files may have sparser property sets than internal files.

---

## 7. Top-Level Redaction Pipeline

The `redact` function orchestrates the full transformation:

```rust
pub fn redact(
    file: &OmtsFile,
    scope: DisclosureScope,
    retain_ids: &HashSet<NodeId>,
) -> Result<OmtsFile, RedactError>
```

**Steps:**

1. **Internal scope short-circuit.** If `scope` is `Internal`, clone the file, set `disclosure_scope`, return immediately. No filtering.

2. **Decode salt.** Decode `file.file_salt` to a 32-byte array. Fail with `RedactError::BoundaryHash(InvalidSalt)` if malformed.

3. **Classify nodes.** Build a `HashMap<NodeId, NodeAction>` by applying `classify_node` then the `retain_ids` promotion logic for each node.

4. **Compute boundary refs.** For each `Replace` node, collect its public identifiers (those with `effective_sensitivity == Public`), compute `CanonicalId` for each, call `boundary_ref_value`. Store results in a `HashMap<NodeId, String>` -- one hash per node, pre-computed and deduplicated.

5. **Build output nodes.** For each input node, emit according to its action:
   - `Retain`: clone with filtered identifiers. Set `identifiers` to `None` if all are filtered out.
   - `Replace`: emit a `boundary_ref` stub with the pre-computed opaque value.
   - `Omit`: skip.

6. **Build output edges.** For each input edge, look up source/target actions (default `Omit` for unknown IDs), classify the edge, and if retained, apply property stripping.

7. **Assemble output.** Copy the file header, set `disclosure_scope` to the target scope, preserve `file_salt` and all other header fields.

8. **Post-redaction L1 validation.** Run L1 validation on the output. Return `RedactError::InvalidOutput` if any errors are found. A post-redaction validation failure indicates a bug in the engine, not in the input.

---

## 8. Output Validation

### 8.1 Post-Redaction Invariants

The engine runs a validation pass on the output before returning it. The following invariants must hold:

1. **No dangling edges.** Every edge `source` and `target` must reference a node `id` present in the output (L1-GDM-03).
2. **Boundary ref structure.** Every `boundary_ref` node has exactly one identifier with `scheme: "opaque"` (L1-SDI-01).
3. **Scope consistency.** The output declares `disclosure_scope` matching the target scope, and the sensitivity constraints from SPEC-004 Section 3 are satisfied (L1-SDI-02).
4. **No person nodes in public output.** If the target scope is `public`, the output contains zero nodes with `type: "person"`.
5. **No beneficial_ownership edges in public output.** If the target scope is `public`, the output contains zero edges with `type: "beneficial_ownership"`.
6. **Salt preserved.** The output file retains the original `file_salt`. Boundary reference hashes are only meaningful relative to the salt that produced them.

The engine invokes L1 validation (with L2 and L3 disabled) and returns `RedactError::InvalidOutput` on failure. The error message includes all rule IDs and messages joined by semicolons.

### 8.2 Boundary Reference Consistency

If the same original node appears as source or target on multiple edges, the engine must produce exactly one `boundary_ref` node for it, not multiple copies. The hash computation is deterministic (same public identifiers + same salt = same hash), so this is a correctness check on the engine's node deduplication logic.

For the CSPRNG path (no public identifiers), the engine generates the random token once per node and reuses it for all references. The implementation pre-computes all values into a `HashMap<NodeId, String>` before building output nodes:

```rust
let mut boundary_ref_values: HashMap<NodeId, String> = HashMap::new();
for node in &file.nodes {
    if !matches!(node_actions.get(&node.id), Some(NodeAction::Replace)) {
        continue;
    }
    let public_ids: Vec<CanonicalId> = node
        .identifiers
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .filter(|id| {
            matches!(
                effective_sensitivity(id, &node.node_type),
                Sensitivity::Public
            )
        })
        .map(CanonicalId::from_identifier)
        .collect();

    let hash = boundary_ref_value(&public_ids, &salt)?;
    boundary_ref_values.insert(node.id.clone(), hash);
}
```

---

## 9. Error Types

```rust
pub enum BoundaryHashError {
    /// The file salt hex string could not be decoded (not 64 lowercase hex chars).
    InvalidSalt(String),
    /// The platform CSPRNG failed when generating a random token.
    CsprngFailure(getrandom::Error),
}

pub enum RedactError {
    /// Salt decoding or CSPRNG call failed.
    BoundaryHash(BoundaryHashError),
    /// The redacted output failed L1 validation -- a logic error in the engine.
    InvalidOutput(String),
}
```

`BoundaryHashError` implements `Display` and `Error`. `RedactError` implements `From<BoundaryHashError>` for ergonomic `?` propagation.

---

## 10. Security Considerations

### 10.1 Information Leakage Through Graph Structure

Boundary references preserve graph topology. An adversary receiving a redacted file can:

- Count `boundary_ref` nodes and observe their degree.
- Infer structural properties of the hidden graph (e.g. a high in-degree boundary_ref on `supplies` edges reveals a hub supplier).
- Distinguish between nodes that have public identifiers (deterministic hashes, reproducible with the right inputs) and nodes that have only restricted/confidential identifiers (random tokens).

This is an inherent tradeoff: boundary references exist to preserve connectivity for downstream analysis. Producers concerned about topology leakage should omit edges to boundary refs entirely (producing a disconnected subgraph). The rule that edges between two replaced nodes are omitted (Section 6.2) partially mitigates this by hiding internal connectivity of the redacted portion.

### 10.2 Timing Side-Channels

The SHA-256 vs. CSPRNG branch is observable via timing (SHA-256 computation takes measurable CPU time; random byte generation does not) but is not a meaningful threat. The branch condition is already visible in the output: deterministic hashes are reproducible by an adversary who knows the public identifiers and salt; random tokens are not. There is no secret to extract.

The `sha2` pure-Rust implementation avoids secret-dependent branches in the compression function. The hash input (canonical identifier strings and file salt) is not secret -- the salt is in the file header, and the public identifiers are, by definition, public.

### 10.3 Salt Entropy Requirements

The file salt must have at least 256 bits of CSPRNG entropy. A weak or predictable salt enables precomputation attacks: an adversary could hash all ~2.5M LEIs in the GLEIF database against a known salt and match boundary references to entities.

The `getrandom` crate provides the correct entropy source per platform:
- Linux: `getrandom(2)` syscall
- macOS: `getentropy(2)`
- Windows: `BCryptGenRandom`
- WASM: `crypto.getRandomValues`

The salt is visible in the file header -- its purpose is anti-enumeration, not secrecy. It forces O(N) work per file where N is the candidate identifier count, and with 2^256 possible salts, rainbow tables are infeasible. Fresh salt per file prevents cross-file correlation of redacted entities.

### 10.4 Threat Model

**Trusted:** The producer (the entity running `omtsf redact`). The producer has access to the full unredacted graph and decides which nodes to retain, replace, or omit.

**Untrusted:** The recipient of the redacted file. The recipient may attempt to:

- **Reverse boundary reference hashes** to recover entity identities. Mitigated by the salt: the adversary must guess both the entity identifier(s) and verify against the file-specific salt. With a fresh salt per file, precomputed tables are useless.
- **Correlate boundary references across files** from the same producer. Mitigated by fresh salt per file. Two exports of the same graph produce different hashes for the same redacted entity. This is an explicit design choice: privacy over temporal tracking of redacted entities.
- **Infer sensitive properties from graph structure.** Partially mitigated by the edge omission rules (Sections 6.2, 6.3, 6.4). Topology leakage at the boundary remains (Section 10.1).
- **Tamper with the file** to inject false data. Out of scope for the redaction engine; integrity is addressed by digital signatures, which are a separate concern.

**Out of scope:** Compromised producers (the adversary already has the unredacted graph) and access control (the engine transforms files, it does not gate access to them).

### 10.5 CSPRNG Failure Mode

If the platform CSPRNG is unavailable, `getrandom` returns an error propagated as `BoundaryHashError::CsprngFailure`. The engine must **never** fall back to a weaker random source -- a failed redaction is preferable to predictable boundary references. Callers surface this as a hard error to the user.

### 10.6 Boundary Reference Stability

Fresh salt per file means the same entity's boundary reference hash changes with every export. This prevents cross-file correlation but makes it impossible to track a specific redacted entity across successive snapshots. If sensitivity reclassification is necessary, the file should be re-exported with a fresh salt. This is an explicit tradeoff documented in SPEC-004 Section 4.

---

## 11. Module Layout

| File | Contents |
|------|----------|
| `crates/omtsf-core/src/sensitivity.rs` | `effective_sensitivity`, `effective_property_sensitivity`, scheme/property default tables |
| `crates/omtsf-core/src/boundary_hash.rs` | `boundary_ref_value`, `decode_salt`, `generate_file_salt`, hex codec, `BoundaryHashError` |
| `crates/omtsf-core/src/canonical.rs` | `CanonicalId` newtype, percent-encoding, `build_identifier_index` |
| `crates/omtsf-core/src/redaction.rs` | `classify_node`, `classify_edge`, `filter_identifiers`, `filter_edge_properties`, `redact` pipeline, `NodeAction`, `EdgeAction`, `RedactError` |
| `crates/omtsf-core/src/validation/rules_l1_sdi.rs` | L1-SDI-01 and L1-SDI-02 validation rules |

All modules live in `omtsf-core`, enforce `#![deny(unsafe_code)]`, and compile to `wasm32-unknown-unknown` without modification. No `unwrap()`, `expect()`, `panic!()`, or `todo!()` in production code -- all errors propagate via `Result<T, E>`.
