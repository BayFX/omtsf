# omtsf-core Technical Specification: Validation Engine

**Status:** Draft
**Date:** 2026-02-21

---

## 1. Purpose

This document specifies the architecture of the three-level validation engine in `omtsf-core`. The engine implements the conformant validator requirements from SPEC-001 Section 11.3: all L1 rules produce errors, L2 rules produce warnings, L3 rules produce informational findings, and unknown fields or extension types never cause rejection.

The engine lives entirely in `omtsf-core`. It operates on parsed data model types (`&OmtsFile`), not raw JSON. Parse errors are a separate concern handled at the deserialization boundary.

---

## 2. Diagnostic Types

Every validation finding is a `Diagnostic`:

```rust
pub struct Diagnostic {
    pub rule_id: RuleId,
    pub severity: Severity,
    pub location: Location,
    pub message: String,
}

pub enum Severity {
    Error,    // L1 -- structural violation, file is non-conformant
    Warning,  // L2 -- semantic concern, file is conformant but incomplete
    Info,     // L3 -- enrichment observation from external data
}
```

`Diagnostic` implements `Display` in the format `[E] L1-GDM-03 edge "edge-042": target "node-999" does not reference an existing node`, where the prefix letter corresponds to the severity (`E`/`W`/`I`).

`RuleId` is an enum with one variant per spec-defined rule. Every diagnostic carries a machine-readable identifier that maps directly to the spec (e.g., `L1-GDM-03`, `L2-EID-05`). The enum has a `code(&self) -> &str` method returning the hyphenated string form for serialized output.

```rust
#[non_exhaustive]
pub enum RuleId {
    // SPEC-001 L1
    L1Gdm01, L1Gdm02, L1Gdm03, L1Gdm04, L1Gdm05, L1Gdm06,
    // SPEC-002 L1
    L1Eid01, L1Eid02, L1Eid03, L1Eid04, L1Eid05,
    L1Eid06, L1Eid07, L1Eid08, L1Eid09, L1Eid10, L1Eid11,
    // SPEC-004 L1
    L1Sdi01, L1Sdi02,
    // SPEC-001 L2
    L2Gdm01, L2Gdm02, L2Gdm03, L2Gdm04,
    // SPEC-002 L2
    L2Eid01, L2Eid02, L2Eid03, L2Eid04,
    L2Eid05, L2Eid06, L2Eid07, L2Eid08,
    // L3 (SPEC-002, SPEC-003)
    L3Eid01, L3Eid02, L3Eid03, L3Eid04, L3Eid05,
    L3Mrg01, L3Mrg02,
    // Special variants
    Extension(String),
    Internal,
}

impl RuleId {
    pub fn code(&self) -> &str {
        match self {
            Self::L1Gdm01 => "L1-GDM-01",
            Self::L1Gdm02 => "L1-GDM-02",
            // ... exhaustive match for all variants
            Self::Extension(s) => s.as_str(),
            Self::Internal => "internal",
        }
    }
}
```

The enum is `#[non_exhaustive]` so that adding new spec-defined rules in future versions does not break downstream callers who match on it. The `RuleId` also implements `Display`, delegating to `code()`.

### 2.1 Location Tracking

```rust
pub enum Location {
    Header { field: &'static str },
    Node { node_id: String, field: Option<String> },
    Edge { edge_id: String, field: Option<String> },
    Identifier { node_id: String, index: usize, field: Option<String> },
    Global,
}
```

`node_id` and `edge_id` are the graph-local `id` values from the file, not internal indices. The optional `field` narrows to a specific property (e.g., `"source"` on an edge for a dangling reference). `Identifier` locations include the zero-based array index so the user can locate the exact entry.

`Location` implements `Display` with the following formats:
- `Header` -> `header.reporting_entity`
- `Node` without field -> `node "org-acme"`
- `Node` with field -> `node "org-acme" field "type"`
- `Edge` without field -> `edge "e-42"`
- `Edge` with field -> `edge "e-42" field "source"`
- `Identifier` without field -> `node "org-acme" identifiers[2]`
- `Identifier` with field -> `node "org-acme" identifiers[0].scheme`
- `Global` -> `(global)`

### 2.2 Collected Results

```rust
pub struct ValidationResult {
    pub diagnostics: Vec<Diagnostic>,
}

impl ValidationResult {
    pub fn has_errors(&self) -> bool;
    pub fn is_conformant(&self) -> bool;  // !has_errors()
    pub fn errors(&self) -> impl Iterator<Item = &Diagnostic>;
    pub fn warnings(&self) -> impl Iterator<Item = &Diagnostic>;
    pub fn infos(&self) -> impl Iterator<Item = &Diagnostic>;
    pub fn by_rule(&self, rule: &RuleId) -> impl Iterator<Item = &Diagnostic>;
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
}
```

The engine always collects all diagnostics -- it never fails fast. A file with 50 L1 violations returns all 50. A file is conformant even if it carries warnings or info findings; only `Severity::Error` makes it non-conformant.

---

## 3. Rule Registry Architecture

### 3.1 Rule Trait

Each validation rule implements a common trait:

```rust
pub trait ValidationRule {
    fn id(&self) -> RuleId;
    fn level(&self) -> Level;
    fn severity(&self) -> Severity { self.level().severity() }
    fn check(
        &self,
        file: &OmtsFile,
        diags: &mut Vec<Diagnostic>,
        external_data: Option<&dyn ExternalDataSource>,
    );
}

pub enum Level { L1, L2, L3 }

impl Level {
    pub fn severity(self) -> Severity {
        match self {
            Self::L1 => Severity::Error,
            Self::L2 => Severity::Warning,
            Self::L3 => Severity::Info,
        }
    }
}
```

Rules push zero or more diagnostics into the `diags` vector. A rule that finds nothing wrong pushes nothing. The `file: &OmtsFile` parameter provides the parsed graph directly. The `external_data` parameter carries an optional reference to an `ExternalDataSource` implementation -- L1 and L2 rules ignore this entirely, L3 rules query it when `Some` and skip silently when `None`.

The trait is object-safe; the registry stores rules as `Vec<Box<dyn ValidationRule>>`.

### 3.2 Registry and Dispatch

The registry is a `Vec<Box<dyn ValidationRule>>` built at initialization. It is not a plugin system; rules are compiled into `omtsf-core`. The registry is constructed by a factory function:

```rust
pub fn build_registry(config: &ValidationConfig) -> Vec<Box<dyn ValidationRule>>;
```

`ValidationConfig` controls which levels are active:

```rust
pub struct ValidationConfig {
    pub run_l1: bool,  // always true in a conformant validator
    pub run_l2: bool,  // default: true
    pub run_l3: bool,  // default: false (requires external data)
}
```

The default configuration runs L1 and L2. L3 is off by default because it requires an external data source. The registry sizes by level: 19 L1 rules (6 GDM + 11 EID + 2 SDI), 6 L2 rules (4 GDM + 2 EID), 2 L3 rules (1 EID + 1 MRG) currently registered.

The top-level dispatch function:

```rust
pub fn validate(
    file: &OmtsFile,
    config: &ValidationConfig,
    external_data: Option<&dyn ExternalDataSource>,
) -> ValidationResult;
```

Dispatch is a linear walk over the registry. Each rule's `check` method is called once with the full `OmtsFile` reference. Rules are stateless and independent -- ordering within a level does not matter. There is no dependency graph between rules, no priority system, no early-exit. If profiling later shows hot spots, individual rules can be optimized internally without changing the dispatch model.

### 3.3 Extensibility

Extension rules can be added by implementing `ValidationRule` and appending to the registry. The trait is public. Extension rules use `RuleId::Extension(String)` to carry their own identifiers. Extension rules MUST NOT use the `L1-*`, `L2-*`, or `L3-*` prefixes -- those are reserved for spec-defined rules.

### 3.4 Per-Rule Lookup Structures

Rules that need index structures (e.g., a node-id-to-node map for reference resolution, a set of node ids for uniqueness checks) build them internally. For example, L1-GDM-01 builds a `HashSet<&str>` over node ids; L1-GDM-03 builds a `HashSet<&str>` of node ids for source/target reference validation; L1-GDM-05 and L1-GDM-06 build a `HashMap<&str, &Node>` for type lookups. Each rule constructs only the structures it needs, keeping the dispatch model simple and avoiding up-front computation of structures that some rule subsets never use.

---

## 4. Validation Levels

### 4.1 L1 -- Structural Integrity (Errors)

L1 rules enforce the MUST constraints from SPEC-001, SPEC-002, and SPEC-004. A file that violates any L1 rule is non-conformant. The complete L1 rule set:

**Graph Data Model (SPEC-001 Section 9.1, 9.5):**

| Rule | Check | Source Module |
|------|-------|---------------|
| L1-GDM-01 | Every node has a non-empty `id`, unique within the file | `rules_l1_gdm::GdmRule01` |
| L1-GDM-02 | Every edge has a non-empty `id`, unique within the file | `rules_l1_gdm::GdmRule02` |
| L1-GDM-03 | Every edge `source` and `target` references an existing node `id` | `rules_l1_gdm::GdmRule03` |
| L1-GDM-04 | Edge `type` is a recognized core type, `same_as`, or reverse-domain extension | `rules_l1_gdm::GdmRule04` |
| L1-GDM-05 | `reporting_entity` if present references an existing organization node `id` | `rules_l1_gdm::GdmRule05` |
| L1-GDM-06 | Edge source/target node types match the permitted types table (SPEC-001 Section 9.5). Extension edge types and `boundary_ref` nodes at endpoints are exempt. | `rules_l1_gdm::GdmRule06` |

**Entity Identification (SPEC-002 Section 6.1):**

| Rule | Check | Source Module |
|------|-------|---------------|
| L1-EID-01 | Every identifier has a non-empty `scheme` | `rules_l1_eid::L1Eid01` |
| L1-EID-02 | Every identifier has a non-empty `value` | `rules_l1_eid::L1Eid02` |
| L1-EID-03 | `authority` is present and non-empty when scheme is `nat-reg`, `vat`, or `internal` | `rules_l1_eid::L1Eid03` |
| L1-EID-04 | `scheme` is a core scheme or reverse-domain extension | `rules_l1_eid::L1Eid04` |
| L1-EID-05 | LEI matches `^[A-Z0-9]{18}[0-9]{2}$` and passes MOD 97-10 check digit | `rules_l1_eid::L1Eid05` |
| L1-EID-06 | DUNS matches `^[0-9]{9}$` | `rules_l1_eid::L1Eid06` |
| L1-EID-07 | GLN matches `^[0-9]{13}$` and passes GS1 mod-10 check digit | `rules_l1_eid::L1Eid07` |
| L1-EID-08 | `valid_from` and `valid_to` if present are valid ISO 8601 dates (`YYYY-MM-DD`) | `rules_l1_eid::L1Eid08` |
| L1-EID-09 | `valid_from` <= `valid_to` when both present | `rules_l1_eid::L1Eid09` |
| L1-EID-10 | `sensitivity` if present is `public`, `restricted`, or `confidential` | `rules_l1_eid::L1Eid10` |
| L1-EID-11 | No duplicate `{scheme, value, authority}` tuple on the same node | `rules_l1_eid::L1Eid11` |

**Selective Disclosure (SPEC-004 Section 6.1):**

| Rule | Check | Source Module |
|------|-------|---------------|
| L1-SDI-01 | `boundary_ref` nodes have exactly one identifier with scheme `opaque` | `rules_l1_sdi::L1Sdi01` |
| L1-SDI-02 | If `disclosure_scope` is declared, sensitivity constraints are satisfied: `public` scope forbids `restricted` and `confidential` identifiers and `person` nodes; `partner` scope forbids `confidential` identifiers | `rules_l1_sdi::L1Sdi02` |

**Implementation notes:**

- L1-GDM-01 and L1-GDM-02 iterate their respective arrays once, tracking seen ids in a `HashSet`. Duplicate ids emit a diagnostic per occurrence beyond the first. The non-empty constraint on ids is enforced by the `NodeId`/`EdgeId` newtypes at deserialization time.
- L1-GDM-03 builds a `HashSet<&str>` of node ids, then checks each edge's `source` and `target` independently. Each dangling reference produces a separate diagnostic with `field` set to `"source"` or `"target"`.
- L1-GDM-04 dispatches on `EdgeTypeTag::Known` vs `EdgeTypeTag::Extension`. Extension strings containing a dot are accepted per SPEC-001 Section 8.2. Unrecognized strings without a dot are rejected.
- L1-GDM-05 resolves `reporting_entity` against a `HashMap<&str, &Node>`. A missing node and a node of wrong type each produce distinct messages.
- L1-GDM-06 encodes the permitted source/target types table from SPEC-001 Section 9.5 as a function returning `Option<(&[NodeType], &[NodeType])>`. Extension edge types skip the check entirely. Dangling references are not double-reported (L1-GDM-03 handles those). Extension node types at endpoints are not constrained. `boundary_ref` nodes are exempt from type-compatibility checks because they may appear at any edge endpoint after redaction (SPEC-004).

### 4.2 L2 -- Semantic Completeness (Warnings)

L2 rules enforce SHOULD constraints. They flag likely modeling errors or missing data without rejecting the file.

**Graph Data Model (SPEC-001 Section 9.2):**

| Rule | Check |
|------|-------|
| L2-GDM-01 | Every `facility` node connects to an `organization` via an edge or `operator` property |
| L2-GDM-02 | `ownership` edges have `valid_from` set |
| L2-GDM-03 | `organization`/`facility` nodes and `supplies`/`subcontracts`/`tolls` edges carry `data_quality` |
| L2-GDM-04 | If any `supplies` edge carries `tier`, the file declares `reporting_entity` |

**Entity Identification (SPEC-002 Section 6.2):**

| Rule | Check |
|------|-------|
| L2-EID-01 | Every `organization` node has at least one non-`internal` identifier |
| L2-EID-02 | Temporal fields (`valid_from`, `valid_to`) present on all identifier records |
| L2-EID-03 | `nat-reg` authority values are valid GLEIF RA codes per snapshot |
| L2-EID-04 | `vat` authority values are valid ISO 3166-1 alpha-2 country codes |
| L2-EID-05 | LEI values with LAPSED/RETIRED/MERGED status produce a warning |
| L2-EID-06 | LEI values with ANNULLED status produce an error-severity warning |
| L2-EID-07 | Identifiers on reassignable schemes (`duns`, `gln`) carry temporal fields |
| L2-EID-08 | Identifiers with `verification_status: "verified"` also carry `verification_date` |

L2 rules are included in the registry when `config.run_l2` is true (the default).

**Implementation notes:**

- L2-GDM-01 uses a helper that collects all facility node IDs connected to an organization via `operates`, `operational_control`, or `tolls` edges, plus the `operator` property. It iterates edges once, then checks all facility nodes against the connected set. The edge match is exhaustive over all `EdgeType` variants.
- L2-GDM-04 short-circuits immediately when `reporting_entity` is present.
- L2-EID-04 embeds a static sorted array of all 249 ISO 3166-1 alpha-2 country codes and uses binary search for O(log n) lookup. This avoids an external dependency while staying WASM-compatible.

### 4.3 L3 -- Enrichment (Info)

L3 rules cross-reference external data sources and are off by default. The engine does not perform HTTP calls -- L3 rules receive external data through an injected trait:

```rust
pub trait ExternalDataSource {
    fn lei_status(&self, lei: &str) -> Option<LeiRecord>;
    fn nat_reg_lookup(&self, authority: &str, value: &str) -> Option<NatRegRecord>;
}

pub struct LeiRecord {
    pub lei: String,
    pub registration_status: String,
    pub is_active: bool,
}

pub struct NatRegRecord {
    pub authority: String,
    pub value: String,
    pub is_active: bool,
}
```

The CLI wires in a concrete implementation; WASM consumers provide their own adapter. L3 rules receive `Option<&dyn ExternalDataSource>`. When the option is `None`, each rule skips its checks entirely without emitting any diagnostics. When the data source returns `None` for a specific lookup, that individual check is skipped silently.

L3 rules:

| Rule | Check |
|------|-------|
| L3-EID-01 | LEI values are verifiable against the GLEIF database |
| L3-EID-02 | `nat-reg` values are cross-referenceable with the authority's registry |
| L3-EID-03 | When a node has both `lei` and `nat-reg`, the GLEIF Level 1 record matches |
| L3-EID-04 | For MERGED LEIs, a `former_identity` edge to the successor entity is present |
| L3-EID-05 | DUNS numbers on `organization` nodes are HQ-level DUNS, not branch |
| L3-MRG-01 | Sum of `ownership.percentage` edges into any org node does not exceed 100 |
| L3-MRG-02 | `legal_parentage` edges form a forest (no cycles) -- detected via topological sort |

**Implementation notes:**

- L3-EID-01 iterates all nodes with `lei` scheme identifiers and queries `lei_status()` for each. Inactive LEIs (where `is_active` is false) produce an Info diagnostic including the registration status string from the data source.
- L3-MRG-01 collects all organization node IDs into a `HashSet`, then for each organization node sums the `percentage` values from all inbound ownership edges whose source is also an organization. Sums exceeding 100.0 produce one Info diagnostic per target node.
- L3-MRG-02 extracts the subgraph of `legal_parentage` edges and runs a topological sort. A cycle produces an Info diagnostic listing the node ids in the cycle.

---

## 5. Check Digit Implementations

These are pure functions in the `check_digits` module within `omtsf-core`. They operate on `&str` and return `bool`. They are zero-allocation: they work directly on the byte slice of the input without any heap allocation.

### 5.1 MOD 97-10 (ISO 7064) for LEI

**Input:** `&str` of length 20, already confirmed to match `^[A-Z0-9]{18}[0-9]{2}$` by the regex check in L1-EID-05.

**Algorithm:**

1. Convert each character to its numeric value: digits `0`-`9` stay as-is, letters `A`=10, `B`=11, ..., `Z`=35.
2. Concatenate all numeric values into a single large integer representation. Because the result can exceed 128 bits, compute the modulus incrementally: maintain a running `u64` remainder. For each character, multiply the accumulator by the appropriate base (10 for digits 0-9, which produce one-digit expansions; 100 for letters A-Z, which produce two-digit expansions 10-35), add the numeric value, then take mod 97.
3. The final remainder must equal 1.

```rust
pub fn mod97_10(lei: &str) -> bool {
    let mut remainder: u64 = 0;
    for byte in lei.as_bytes() {
        match byte {
            b'0'..=b'9' => {
                let digit = u64::from(byte - b'0');
                remainder = (remainder * 10 + digit) % 97;
            }
            b'A'..=b'Z' => {
                let value = u64::from(byte - b'A') + 10;
                remainder = (remainder * 100 + value) % 97;
            }
            _ => {}
        }
    }
    remainder == 1
}
```

Characters outside `[A-Z0-9]` are silently ignored; the regex pre-check in L1-EID-05 ensures the LEI contains only valid characters before this function is called.

**Test vectors:**

| LEI | Expected | Notes |
|-----|----------|-------|
| `5493006MHB84DD0ZWV18` | valid | Known-valid LEI (BIS) |
| `5493006MHB84DD0ZWV19` | invalid | Wrong check digit (changed 8 to 9) |
| `549300TRUWO2CD2G5692` | valid | GLEIF's own LEI |
| `7LTWFZYICNSX8D621K86` | valid | Deutsche Bank AG |
| `HWUPKR0MPOU8FGXBT394` | valid | Apple Inc. |
| `00000000000000000000` | invalid | All zeros (remainder 0, not 1) |

### 5.2 GS1 Mod-10 for GLN

**Input:** `&str` of length 13, already confirmed to match `^[0-9]{13}$`.

**Algorithm:**

1. Number positions 1 through 13 from left to right. The check digit is at position 13.
2. Apply alternating weights starting from position 1. Position `i` (1-indexed) has weight 1 if `i` is odd, weight 3 if `i` is even. Equivalently: index `i` (0-indexed) into positions 1-12 has weight 1 if even-indexed, 3 if odd-indexed.
3. Sum the weighted products of positions 1 through 12 (all except the check digit at position 13).
4. Check digit = `(10 - (sum mod 10)) mod 10`.
5. Compare computed check digit against the actual digit at position 13.

```rust
pub fn gs1_mod10(gln: &str) -> bool {
    let bytes = gln.as_bytes();
    if bytes.len() != 13 {
        return false;
    }
    let mut sum: u32 = 0;
    for (i, byte) in bytes[..12].iter().enumerate() {
        let digit = u32::from(byte - b'0');
        let weight: u32 = if i % 2 == 1 { 3 } else { 1 };
        sum += digit * weight;
    }
    let expected_check = (10 - (sum % 10)) % 10;
    let actual_check = u32::from(bytes[12] - b'0');
    expected_check == actual_check
}
```

The function operates on ASCII bytes directly, converting to digit values via byte subtraction (`byte - b'0'`). No integer parsing is needed.

**Test vectors:**

| GLN | Expected | Notes |
|-----|----------|-------|
| `0614141000418` | valid | Standard GS1 example |
| `5901234123457` | valid | Commonly cited GS1 test vector |
| `4000000000006` | valid | Minimal non-zero prefix |
| `0614141000419` | invalid | Wrong check digit |
| `061414100041` | invalid | Too short (12 digits) |
| `06141410004180` | invalid | Too long (14 digits) |

---

## 6. Error Handling Strategy

Three categories of errors exist. They are distinct types and must not be conflated.

### 6.1 Parse Errors

Produced by `serde_json` deserialization: malformed JSON, missing required fields, wrong types. Parse errors prevent validation from running. They are reported as a `ParseError` with a human-readable message string including location information (byte offset or line/column) where available.

Parse errors are not `Diagnostic` values. They are a separate variant in the top-level result type:

```rust
pub enum ValidateOutput {
    ParseFailed(ParseError),
    Validated(ValidationResult),
}

pub struct ParseError {
    pub message: String,
}
```

`ParseError` implements `Display` (prefixed with `"parse error: "`) and `std::error::Error`.

### 6.2 Validation Errors

These are `Diagnostic` values with `Severity::Error`. They mean the file parsed successfully but violates one or more L1 rules. The file is non-conformant. Validation warnings (`Severity::Warning`) and info findings (`Severity::Info`) are also `Diagnostic` values at lower severities.

A parse failure returns `ValidateOutput::ParseFailed`; a successful parse returns `ValidateOutput::Validated` containing zero or more diagnostics across all three severity levels.

### 6.3 Internal Errors

Bugs in the validator itself -- index out of bounds, unexpected `None`, logic errors. These must never be swallowed. In debug builds they panic. In release builds they produce a diagnostic with `RuleId::Internal` and `Severity::Error`, with a message asking the user to report the bug. The validator continues if possible.

### 6.4 Unknown Fields and Extensions

Per SPEC-001 Section 11.3, the validator MUST NOT reject files based on unknown fields, extension edge types, or unrecognized `data_quality` values. Serde deserialization uses `#[serde(flatten)]` with `serde_json::Map<String, Value>` catch-all fields on every struct to capture extensions. `#[serde(deny_unknown_fields)]` is never used anywhere in the type hierarchy.

Extension edge types (matching the reverse-domain pattern, i.e., containing at least one dot) bypass L1-GDM-04 and L1-GDM-06. Extension identifier schemes bypass format validation in L1-EID-04 through L1-EID-07. Unknown `data_quality.confidence` values are silently preserved.

---

## 7. CLI Integration

The CLI's `validate` command calls into `omtsf-core` and maps the result:

| Outcome | stderr | stdout | Exit code |
|---------|--------|--------|-----------|
| Parse failure | Parse error message | nothing | 2 |
| L1 errors present | All diagnostics | nothing | 1 |
| Only L2/L3 findings | All diagnostics | nothing | 0 |
| Clean | "Valid." | nothing | 0 |

Diagnostics are formatted one-per-line to stderr. The default format is human-readable:

```
[E] L1-GDM-03 edge "edge-042": target "node-999" does not reference an existing node
[W] L2-EID-01 node "org-acme": organization has no external identifiers
[I] L3-MRG-01 node "org-bolt": inbound ownership percentages sum to 112%
```

A `--format json` flag emits each diagnostic as a JSON object (one per line, NDJSON) for machine consumption:

```json
{"rule":"L1-GDM-03","severity":"error","location":{"type":"edge","edge_id":"edge-042","field":"target"},"message":"target \"node-999\" does not reference an existing node"}
```

The `--level` flag controls which levels to run: `--level l1` runs only L1, `--level l1,l2` runs L1 and L2 (the default), `--level l1,l2,l3` runs all three. L1 is always included; specifying `--level l2` implies L1.

---

## 8. Source Layout

| File | Contents |
|------|----------|
| `validation/mod.rs` | `Diagnostic`, `Severity`, `RuleId`, `Location`, `ValidationResult`, `ParseError`, `ValidateOutput`, `Level`, `ValidationRule` trait, `ValidationConfig`, `build_registry`, `validate` |
| `validation/rules_l1_gdm.rs` | `GdmRule01` through `GdmRule06`, permitted-types table, helper functions |
| `validation/rules_l1_sdi.rs` | `L1Sdi01`, `L1Sdi02` |
| `validation/rules_l2.rs` | `L2Gdm01` through `L2Gdm04`, `L2Eid01`, `L2Eid04`, ISO 3166-1 alpha-2 table |
| `validation/rules_l3.rs` | `L3Eid01`, `L3Mrg01` |
| `validation/external.rs` | `ExternalDataSource` trait, `LeiRecord`, `NatRegRecord` |
| `rules_l1_eid.rs` | `L1Eid01` through `L1Eid11` (at crate root, not inside `validation/`) |
| `check_digits.rs` | `mod97_10`, `gs1_mod10` |
