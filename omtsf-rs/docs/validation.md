# omtsf-cli Technical Specification: Validation Engine

**Status:** Draft
**Date:** 2026-02-19

---

## 1. Purpose

This document specifies the architecture of the three-level validation engine in `omtsf-core`. The engine implements the conformant validator requirements from SPEC-001 Section 11.3: all L1 rules produce errors, L2 rules produce warnings, L3 rules produce informational findings, and unknown fields or extension types never cause rejection.

The engine lives entirely in `omtsf-core`. It operates on parsed data model types (`&Graph`), not raw JSON. Parse errors are a separate concern handled at the deserialization boundary.

---

## 2. Diagnostic Types

Every validation finding is a `Diagnostic`:

```rust
struct Diagnostic {
    rule_id: RuleId,         // e.g., RuleId::L1_GDM_03
    severity: Severity,      // Error, Warning, Info
    location: Location,      // where in the graph
    message: String,         // human-readable explanation
}

enum Severity {
    Error,    // L1 — structural violation, file is non-conformant
    Warning,  // L2 — semantic concern, file is conformant but suspect
    Info,     // L3 — enrichment observation from external data
}
```

`RuleId` is an enum with one variant per rule. This ensures every diagnostic carries a machine-readable identifier that maps directly to the spec (e.g., `L1-GDM-03`, `L2-EID-05`). The enum has a `code(&self) -> &'static str` method that returns the hyphenated string form for serialized output.

### 2.1 Location Tracking

```rust
enum Location {
    Header { field: &'static str },
    Node { node_id: String, field: Option<String> },
    Edge { edge_id: String, field: Option<String> },
    Identifier { node_id: String, index: usize, field: Option<String> },
    Global,
}
```

Every diagnostic pinpoints where in the graph the problem was found. `node_id` and `edge_id` are the graph-local `id` values from the file, not internal indices. The optional `field` narrows to a specific property when relevant (e.g., `"source"` on an edge for a dangling reference). `Identifier` locations include the index within the node's identifiers array so the user can locate the exact entry.

### 2.2 Collected Results

```rust
struct ValidationResult {
    diagnostics: Vec<Diagnostic>,
}

impl ValidationResult {
    fn has_errors(&self) -> bool;
    fn errors(&self) -> impl Iterator<Item = &Diagnostic>;
    fn warnings(&self) -> impl Iterator<Item = &Diagnostic>;
    fn infos(&self) -> impl Iterator<Item = &Diagnostic>;
    fn is_conformant(&self) -> bool; // true iff zero errors
}
```

The engine always collects all diagnostics. It never fails fast on the first error. A file with 50 L1 violations returns all 50, because a user fixing problems one-at-a-time against a reference validator is a miserable experience.

---

## 3. Rule Registry Architecture

### 3.1 Rule Trait

Each validation rule implements a common trait:

```rust
trait ValidationRule {
    fn id(&self) -> RuleId;
    fn level(&self) -> Level;          // L1, L2, L3
    fn severity(&self) -> Severity;    // derived from level, but explicit
    fn check(&self, graph: &Graph, diags: &mut Vec<Diagnostic>);
}
```

Rules push zero or more diagnostics into the `diags` vector. A rule that finds nothing wrong pushes nothing. The `graph: &Graph` parameter is the fully parsed, typed data model — rules never touch raw JSON.

### 3.2 Registry and Dispatch

The registry is a `Vec<Box<dyn ValidationRule>>` built at initialization. It is not a plugin system; rules are compiled into `omtsf-core`. The registry is constructed by a factory function:

```rust
fn build_registry(config: &ValidationConfig) -> Vec<Box<dyn ValidationRule>>;
```

`ValidationConfig` controls which levels are active:

```rust
struct ValidationConfig {
    run_l1: bool,  // always true in a conformant validator
    run_l2: bool,  // default true
    run_l3: bool,  // default false (requires external data)
}
```

Dispatch is a linear walk over the registry. Each rule's `check` method is called once with the full graph. Rules are stateless; they receive the graph by shared reference and emit diagnostics. Ordering within a level does not matter because rules are independent.

This is deliberately simple. There is no dependency graph between rules, no priority system, no early-exit. The graph is small enough (even at millions of nodes) that a full pass per rule is acceptable. If profiling later shows hot spots, individual rules can be optimized internally without changing the dispatch model.

### 3.3 Extensibility

Extension rules (e.g., organization-specific checks) can be added by implementing `ValidationRule` and appending to the registry. The trait is public. However, the built-in `RuleId` enum is non-exhaustive, and extension rules use `RuleId::Extension(String)` to carry their own identifiers. Extension rules MUST NOT use the `L1-*`, `L2-*`, or `L3-*` prefixes — those are reserved for spec-defined rules.

---

## 4. Validation Levels

### 4.1 L1 — Structural (Errors)

L1 rules enforce the MUST constraints from SPEC-001 and SPEC-002. A file that violates any L1 rule is non-conformant. The complete L1 rule set:

**Graph Data Model (SPEC-001):**

| Rule | Check |
|------|-------|
| L1-GDM-01 | Every node has a non-empty `id`, unique within the file |
| L1-GDM-02 | Every edge has a non-empty `id`, unique within the file |
| L1-GDM-03 | Every edge `source` and `target` references an existing node `id` |
| L1-GDM-04 | Edge `type` is a recognized core type, `same_as`, or reverse-domain extension |
| L1-GDM-05 | `reporting_entity` if present references an existing organization node `id` |
| L1-GDM-06 | Edge source/target node types match the permitted types table (Section 9.5). Extension edges are exempt. |

**Entity Identification (SPEC-002):**

| Rule | Check |
|------|-------|
| L1-EID-01 | Every identifier has a non-empty `scheme` |
| L1-EID-02 | Every identifier has a non-empty `value` |
| L1-EID-03 | `authority` is present when scheme is `nat-reg`, `vat`, or `internal` |
| L1-EID-04 | `scheme` is a core scheme or reverse-domain extension |
| L1-EID-05 | LEI matches `^[A-Z0-9]{18}[0-9]{2}$` and passes MOD 97-10 |
| L1-EID-06 | DUNS matches `^[0-9]{9}$` |
| L1-EID-07 | GLN matches `^[0-9]{13}$` and passes GS1 mod-10 |
| L1-EID-08 | `valid_from` / `valid_to` if present are valid ISO 8601 dates |
| L1-EID-09 | `valid_from` <= `valid_to` when both present |
| L1-EID-10 | `sensitivity` if present is `public`, `restricted`, or `confidential` |
| L1-EID-11 | No duplicate `{scheme, value, authority}` tuple on the same node |

**Selective Disclosure (SPEC-004):**

| Rule | Check |
|------|-------|
| L1-SDI-01 | `boundary_ref` nodes have exactly one identifier with scheme `opaque` |
| L1-SDI-02 | If `disclosure_scope` is declared, sensitivity constraints are satisfied |

Implementation note: L1 rules build temporary lookup structures (e.g., a `HashMap<&str, &Node>` for id-to-node mapping) at the start of a validation pass. Multiple rules that need the same index share it via a pre-computed `ValidationContext` passed alongside the graph. This avoids redundant hash map construction across rules.

### 4.2 L2 — Semantic (Warnings)

L2 rules enforce SHOULD constraints. They flag likely modeling errors without rejecting the file. Rules L2-GDM-01 through L2-GDM-04 and L2-EID-01 through L2-EID-08 are included in the registry when `run_l2` is true.

Examples: a facility with no edge connecting it to an organization (L2-GDM-01), an ownership edge missing `valid_from` (L2-GDM-02), an organization node with no external identifiers (L2-EID-01), a country code that is not a valid ISO 3166-1 alpha-2 (L2-EID-04).

### 4.3 L3 — Enrichment (Info)

L3 rules cross-reference external data sources. They are off by default and require network access or a local cache of external data. The engine itself does not perform HTTP calls — L3 rules receive external data through an injected trait:

```rust
trait ExternalDataSource {
    fn lei_status(&self, lei: &str) -> Option<LeiRecord>;
    fn nat_reg_lookup(&self, authority: &str, value: &str) -> Option<NatRegRecord>;
}
```

This keeps `omtsf-core` free of network dependencies. The CLI wires in a concrete implementation that calls GLEIF or other registries. WASM consumers can provide their own adapter.

L3 rules include L3-EID-01 through L3-EID-05 (registry verification), L3-MRG-01 (ownership percentage sum), and L3-MRG-02 (legal parentage cycle detection). L3-MRG-02 uses a topological sort on the subgraph of `legal_parentage` edges; a cycle produces an Info diagnostic with the cycle's node ids.

---

## 5. Check Digit Implementations

These are pure functions in a `check_digits` module. They operate on string slices and return `bool`. They do not allocate.

### 5.1 MOD 97-10 (ISO 7064) for LEI

**Input:** `&str` of length 20, already confirmed to match `^[A-Z0-9]{18}[0-9]{2}$` by the regex check.

**Algorithm:**
1. Convert each character to its numeric value: digits 0-9 stay as-is, letters A=10, B=11, ..., Z=35.
2. Concatenate all numeric values into a single large integer representation. Because the result can exceed 128 bits, compute the modulus incrementally: maintain a running remainder, and for each character append its one-or-two digit numeric value by multiplying the accumulator by 10 or 100 and adding, then taking mod 97.
3. The final remainder must equal 1.

**Output:** `bool`. True if the check digit is valid.

**Error cases:** The function assumes the regex pre-check has passed. If called on a string that does not match the pattern, behavior is unspecified (not unsafe, but the boolean result is meaningless). The L1-EID-05 rule implementation calls the regex check first and only proceeds to MOD 97-10 on match.

### 5.2 GS1 Mod-10 for GLN

**Input:** `&str` of length 13, already confirmed to match `^[0-9]{13}$`.

**Algorithm:**
1. Number positions 1-13 from left to right.
2. Apply alternating weights starting from the rightmost digit (position 13): positions are weighted 1, 3, 1, 3, ... counting from the right. Equivalently, position 13 has weight 1, position 12 has weight 3, position 11 has weight 1, and so on.
3. Sum the weighted products of positions 1 through 12 (all except the check digit at position 13).
4. Check digit = (10 - (sum mod 10)) mod 10.
5. Compare computed check digit against the actual digit at position 13.

**Output:** `bool`. True if the check digit matches.

**Error cases:** Same as MOD 97-10 — regex pre-check is assumed. The function operates on ASCII bytes directly (`b'0'..=b'9'`), no parsing into integers needed beyond byte subtraction.

---

## 6. Error Handling Strategy

Three categories of errors exist. They are distinct types and must not be conflated.

### 6.1 Parse Errors

Produced by `serde_json` deserialization. These mean the input is not a valid `.omts` file at all — malformed JSON, missing required fields, wrong types. Parse errors prevent validation from running. They are reported as a single `ParseError` (wrapping `serde_json::Error`) with the byte offset or line/column from serde's error.

Parse errors are not `Diagnostic` values. They are a separate variant in the top-level result type:

```rust
enum ValidateOutput {
    ParseFailed(ParseError),
    Validated(ValidationResult),
}
```

### 6.2 Validation Errors

These are `Diagnostic` values with `Severity::Error`. They mean the file parsed successfully but violates one or more L1 rules. The file is non-conformant. Validation warnings and info findings are also `Diagnostic` values at lower severities.

### 6.3 Internal Errors

Bugs in the validator itself — index out of bounds, unexpected `None`, logic errors. These must never be swallowed. In debug builds they panic. In release builds they produce a diagnostic with a special `RuleId::Internal` variant and `Severity::Error`, with a message asking the user to report the bug. The validator continues if possible.

### 6.4 Unknown Fields and Extensions

Per SPEC-001 Section 11.3, the validator MUST NOT reject files based on unknown fields, extension edge types, or unrecognized `data_quality` values. Serde deserialization uses `#[serde(deny_unknown_fields)]` only on the top-level structure; node and edge property bags use `#[serde(flatten)]` with a `HashMap<String, Value>` to capture extensions. Extension edge types (matching reverse-domain format) bypass L1-GDM-04 and L1-GDM-06.

---

## 7. CLI Integration

The CLI's `validate` command calls into `omtsf-core` and maps the result:

| Outcome | stderr | stdout | Exit code |
|---------|--------|--------|-----------|
| Parse failure | Parse error message | nothing | 2 |
| L1 errors present | All diagnostics | nothing | 1 |
| Only L2/L3 findings | All diagnostics | nothing | 0 |
| Clean | "Valid." | nothing | 0 |

Diagnostics are formatted one-per-line to stderr. The default format is human-readable: `[E] L1-GDM-03 edge "edge-042": target "node-999" does not reference an existing node`. A `--format json` flag emits each diagnostic as a JSON object (one per line, NDJSON) for machine consumption.
