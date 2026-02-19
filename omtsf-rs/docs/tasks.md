# omtsf-rs Implementation Task List

**Date:** 2026-02-19
**Status:** Draft

This document contains the ordered, dependency-aware task breakdown for implementing the `omtsf-rs` workspace (`omtsf-core` library and `omtsf-cli` binary) from the technical specification suite.

---

## Phase 1: Workspace Setup

### T-001 — Initialize Cargo workspace, crate scaffolding, and CI

- **Spec Reference:** overview.md Sections 2, 3
- **Dependencies:** None
- **Complexity:** S
- **Crate:** Both
- **Acceptance Criteria:**
  - Cargo workspace root at `omtsf-rs/Cargo.toml` with members `crates/omtsf-core` and `crates/omtsf-cli`
  - `omtsf-core` compiles as a library crate with `serde`, `serde_json`, and `petgraph` in `[dependencies]`
  - `omtsf-cli` compiles as a binary crate with `clap` and a dependency on `omtsf-core`
  - Placeholder `omtsf-wasm` crate exists with an empty `lib.rs`
  - Workspace lints configured (`warnings` denied, common clippy lints enabled), `rust-toolchain.toml` pins stable edition (2024)
  - `tests/` directory exists at workspace root; `cargo build` and `cargo test` succeed

---

## Phase 2: Data Model

### T-002 — Define newtype wrappers with validation

- **Spec Reference:** data-model.md Section 3 (SemVer, CalendarDate, FileSalt, NodeId, CountryCode)
- **Dependencies:** T-001
- **Complexity:** M
- **Crate:** `omtsf-core`
- **Acceptance Criteria:**
  - Each newtype (`SemVer`, `CalendarDate`, `FileSalt`, `NodeId`, `CountryCode`) implements `TryFrom<&str>` with regex-based shape validation
  - Each newtype implements `Deref<Target = str>`, `Display`, `Serialize`, and `Deserialize` (with validation in the `Deserialize` impl); none implement `DerefMut`
  - Unit tests cover valid inputs, boundary cases, and rejection of malformed inputs for each type

### T-003 — Define enums for node types, edge types, and property-level enums

- **Spec Reference:** data-model.md Sections 4.1 through 4.4
- **Dependencies:** T-001
- **Complexity:** M
- **Crate:** `omtsf-core`
- **Acceptance Criteria:**
  - All enums from the spec defined with `#[serde(rename_all = "snake_case")]`: `NodeType`, `EdgeType`, `DisclosureScope`, `AttestationType`, `Confidence`, `Sensitivity`, `VerificationStatus`, `OrganizationStatus`, `AttestationOutcome`, `AttestationStatus`, `RiskSeverity`, `RiskLikelihood`, `EmissionFactorSource`, `ControlType`, `ConsolidationBasis`, `EventType`, `ServiceType`
  - `NodeTypeTag` and `EdgeTypeTag` wrapper enums with `Known(T)` and `Extension(String)` variants with custom `Deserialize` implementations
  - Round-trip serde tests confirm known variants serialize correctly and extension strings survive deserialization

### T-004 — Define shared types: Identifier, DataQuality, Label, Geo

- **Spec Reference:** data-model.md Sections 7.1 through 7.4
- **Dependencies:** T-002, T-003
- **Complexity:** S
- **Crate:** `omtsf-core`
- **Acceptance Criteria:**
  - `Identifier`, `DataQuality`, `Label` structs with all fields per spec, each carrying `#[serde(flatten)]` extras
  - `Geo` enum with `Point { lat, lon }` and `GeoJson(Value)` variants
  - `Node::geo_parsed()` helper method returns `Option<Result<Geo, GeoParseError>>`
  - Unit tests for each shared type's serialization and deserialization

### T-005 — Define Node, Edge, and EdgeProperties structs

- **Spec Reference:** data-model.md Sections 5.1 through 5.3, 6.1 through 6.3
- **Dependencies:** T-002, T-003, T-004
- **Complexity:** L
- **Crate:** `omtsf-core`
- **Acceptance Criteria:**
  - `Node` struct with all fields from data-model.md Section 5.1 as `Option<T>` (except `id` and `node_type`), with `#[serde(flatten)]` for unknown field preservation
  - `valid_to` uses `Option<Option<CalendarDate>>` with a custom deserializer distinguishing null from absent
  - `Edge` struct with `id`, `edge_type`, `source`, `target`, `identifiers`, `properties`, and `extra` fields
  - `EdgeProperties` struct with all optional property fields from data-model.md Section 6.2 and `#[serde(flatten)]` catch-all
  - Serde round-trip tests for nodes and edges with known types, extension types, and null-vs-absent `valid_to`

### T-006 — Define OmtsFile top-level struct and end-to-end serde round-trip

- **Spec Reference:** data-model.md Sections 2, 8.1 through 8.5, 9, 10
- **Dependencies:** T-005
- **Complexity:** M
- **Crate:** `omtsf-core`
- **Acceptance Criteria:**
  - `OmtsFile` struct with fields in declaration order matching spec (field order is load-bearing for serialization)
  - `#[serde(rename_all = "snake_case")]` applied; `#[serde(skip_serializing_if = "Option::is_none")]` on optional fields
  - `#[serde(flatten)]` catch-all for unknown top-level fields
  - End-to-end test: parse a complete JSON fixture into `OmtsFile`, re-serialize, verify structural equality
  - Round-trip preserves unknown fields added to the fixture

### T-007 — Create initial .omts test fixtures

- **Spec Reference:** data-model.md (all sections), overview.md Section 4.3
- **Dependencies:** T-006
- **Complexity:** S
- **Crate:** Both (fixtures live in workspace `tests/fixtures/`)
- **Acceptance Criteria:**
  - At least three fixture files: minimal valid, realistic example, and one with extension types and unknown fields
  - All fixtures parse successfully with `serde_json::from_str::<OmtsFile>()` in unit tests
  - Fixtures committed under `tests/fixtures/`

---

## Phase 3: Validation Engine

### T-008 — Define Diagnostic, Severity, RuleId, Location, and ValidationResult types

- **Spec Reference:** validation.md Sections 2, 2.1, 2.2
- **Dependencies:** T-002
- **Complexity:** S
- **Crate:** `omtsf-core`
- **Acceptance Criteria:**
  - `Diagnostic` struct with `rule_id`, `severity`, `location`, `message` fields
  - `Severity` enum (`Error`, `Warning`, `Info`); `RuleId` enum with one variant per L1/L2/L3 rule plus `Extension(String)` and `Internal`; `RuleId::code()` returns `&'static str` in hyphenated form (e.g., `"L1-GDM-03"`)
  - `Location` enum with `Header`, `Node`, `Edge`, `Identifier`, `Global` variants
  - `ValidationResult` with `has_errors()`, `is_conformant()`, filtering iterators
  - `ValidateOutput` enum distinguishing `ParseFailed` from `Validated`

### T-009 — Implement ValidationRule trait, registry, and dispatch

- **Spec Reference:** validation.md Sections 3.1, 3.2, 3.3
- **Dependencies:** T-008, T-006
- **Complexity:** M
- **Crate:** `omtsf-core`
- **Acceptance Criteria:**
  - `ValidationRule` trait with `id()`, `level()`, `severity()`, and `check(&self, file: &OmtsFile, diags: &mut Vec<Diagnostic>)` methods
  - `ValidationConfig` struct with `run_l1`, `run_l2`, `run_l3` booleans
  - `build_registry(config: &ValidationConfig) -> Vec<Box<dyn ValidationRule>>` factory function
  - `validate(file: &OmtsFile, config: &ValidationConfig) -> ValidationResult` top-level dispatch
  - Test that an empty registry produces zero diagnostics

### T-010 — Implement L1-GDM rules (graph data model structural checks)

- **Spec Reference:** validation.md Section 4.1 (L1-GDM-01 through L1-GDM-06)
- **Dependencies:** T-009
- **Complexity:** L
- **Crate:** `omtsf-core`
- **Acceptance Criteria:**
  - L1-GDM-01: duplicate node ID detection; L1-GDM-02: duplicate edge ID detection
  - L1-GDM-03: dangling edge source/target references
  - L1-GDM-04: edge type validation (core type, `same_as`, or reverse-domain extension)
  - L1-GDM-05: `reporting_entity` references a valid organization node
  - L1-GDM-06: edge source/target node type compatibility per SPEC-001 Section 9.5
  - Each rule has dedicated unit tests with fixtures triggering exactly that rule; all rules collect all violations (no early exit)

### T-011 — Implement check digit functions and L1-EID rules

- **Spec Reference:** validation.md Sections 4.1 (L1-EID-01 through L1-EID-11), 5.1, 5.2
- **Dependencies:** T-009
- **Complexity:** L
- **Crate:** `omtsf-core`
- **Acceptance Criteria:**
  - `mod97_10(lei: &str) -> bool` and `gs1_mod10(gln: &str) -> bool` pure functions, zero allocations, with known-valid test vectors and corrupted variants
  - L1-EID-01 through L1-EID-11 rules implemented: non-empty scheme/value, authority when required, scheme validation, LEI/DUNS/GLN format+checksum, date validity and ordering, sensitivity enum, duplicate identifier tuple detection
  - Each rule has at least two tests (one passing, one failing)

### T-012 — Implement L1-SDI rules (selective disclosure structural checks)

- **Spec Reference:** validation.md Section 4.1 (L1-SDI-01, L1-SDI-02)
- **Dependencies:** T-009
- **Complexity:** S
- **Crate:** `omtsf-core`
- **Acceptance Criteria:**
  - L1-SDI-01: `boundary_ref` nodes have exactly one identifier with scheme `opaque`
  - L1-SDI-02: if `disclosure_scope` is declared, sensitivity constraints are satisfied
  - Unit tests with boundary_ref nodes that violate and satisfy each constraint

### T-013 — Implement L2 rules (semantic warnings)

- **Spec Reference:** validation.md Section 4.2
- **Dependencies:** T-009
- **Complexity:** M
- **Crate:** `omtsf-core`
- **Acceptance Criteria:**
  - At least six L2 rules implemented: facility with no organization edge (L2-GDM-01), ownership edge missing `valid_from` (L2-GDM-02), organization with no external identifiers (L2-EID-01), invalid ISO 3166-1 alpha-2 country code (L2-EID-04), and two additional L2 rules derived from SPEC-001/SPEC-002 SHOULD constraints
  - Each rule has passing/failing test cases; all produce `Severity::Warning`

### T-014 — Define L3 ExternalDataSource trait and stub rules

- **Spec Reference:** validation.md Sections 4.3, 3.3
- **Dependencies:** T-009
- **Complexity:** S
- **Crate:** `omtsf-core`
- **Acceptance Criteria:**
  - `ExternalDataSource` trait with `lei_status()` and `nat_reg_lookup()` methods
  - At least two L3 rule structs (L3-EID-01, L3-MRG-01) registered when `run_l3` is true
  - L3 rules accept `Option<&dyn ExternalDataSource>` and skip gracefully when `None`
  - Test with a mock `ExternalDataSource` producing expected diagnostics

---

## Phase 4: Graph Engine

### T-015 — Implement graph construction from OmtsFile (petgraph integration)

- **Spec Reference:** graph-engine.md Sections 2.1 through 2.4
- **Dependencies:** T-006
- **Complexity:** M
- **Crate:** `omtsf-core`
- **Acceptance Criteria:**
  - `OmtsGraph` struct wrapping `StableDiGraph<NodeWeight, EdgeWeight>` and `HashMap<String, NodeIndex>`
  - `build_graph(file: &OmtsFile) -> Result<OmtsGraph, GraphBuildError>` two-pass construction with pre-allocated capacity
  - `GraphBuildError::DuplicateNodeId` and `GraphBuildError::DanglingEdgeRef` error variants
  - Test: construct graph from a fixture, verify node count, edge count, and ID lookups

### T-016 — Implement BFS reachability and path-finding queries

- **Spec Reference:** graph-engine.md Sections 3, 4.1, 4.2, 4.3
- **Dependencies:** T-015
- **Complexity:** L
- **Crate:** `omtsf-core`
- **Acceptance Criteria:**
  - `reachable_from(graph, start, direction, edge_filter)` supporting `Forward`, `Backward`, `Both` directions with optional edge-type filter; start node excluded from result
  - `shortest_path(graph, from, to, direction)` returning `Option<Vec<NodeIndex>>`
  - `all_paths(graph, from, to, max_depth, direction)` with iterative-deepening DFS and default max_depth of 20
  - All three accept optional `edge_filter`
  - Tests: linear chain, branching tree, cycle handling, edge-type filtering, no-path case, depth limit enforcement, node-not-found error

### T-017 — Implement subgraph extraction and ego-graph

- **Spec Reference:** graph-engine.md Sections 5.1, 5.2, 5.3
- **Dependencies:** T-015, T-016
- **Complexity:** M
- **Crate:** `omtsf-core`
- **Acceptance Criteria:**
  - `induced_subgraph(graph, node_ids) -> Result<OmtsFile, QueryError>` function
  - `ego_graph(graph, center, radius, direction) -> Result<OmtsFile, QueryError>` function
  - Output is a valid `OmtsFile` with preserved header fields (except `reporting_entity` omitted if not in subgraph)
  - Tests: extract known subset, verify edges only between included nodes, ego-graph radius limiting

### T-018 — Implement cycle detection (Kahn's algorithm)

- **Spec Reference:** graph-engine.md Sections 6.1, 6.2, 6.3
- **Dependencies:** T-015
- **Complexity:** M
- **Crate:** `omtsf-core`
- **Acceptance Criteria:**
  - `detect_cycles(graph, edge_types) -> Vec<Vec<NodeIndex>>` using Kahn's algorithm (BFS-based topological sort)
  - Returns empty vec for acyclic subgraphs, cycle node sequences for cyclic ones
  - Edge-type filtering isolates `legal_parentage` subgraph for L3-MRG-02
  - Tests: DAG (no cycles), simple cycle, multiple disjoint cycles, mixed acyclic/cyclic graph

---

## Phase 5: Merge Engine

### T-019 — Implement CanonicalId type and identifier indexing

- **Spec Reference:** merge.md Sections 2.2, 3.3; redaction.md Section 4.1
- **Dependencies:** T-004
- **Complexity:** M
- **Crate:** `omtsf-core`
- **Acceptance Criteria:**
  - `CanonicalId` newtype with percent-encoding of `:`, `%`, `\n`, `\r`
  - Canonical form `{scheme}:{value}` or `{scheme}:{authority}:{value}` for authority-required schemes
  - `build_identifier_index(nodes: &[Node]) -> HashMap<CanonicalId, Vec<usize>>` excluding `internal` scheme
  - Tests: encoding edge cases (colons in values, percent signs), index construction from overlapping identifiers

### T-020 — Implement Union-Find and node identity predicates

- **Spec Reference:** merge.md Sections 2.1, 3.1
- **Dependencies:** T-019
- **Complexity:** M
- **Crate:** `omtsf-core`
- **Acceptance Criteria:**
  - `UnionFind` struct with `find` (path-halving) and `union` (union-by-rank, lower ordinal wins on tie)
  - `identifiers_match(a, b) -> bool` pure function: excludes `internal`, case-insensitive authority, whitespace-trimmed values
  - `temporal_compatible()` helper: interval overlap check, missing fields treated as open-ended
  - ANNULLED LEI exclusion from index construction
  - Tests: basic union/find, transitive closure, deterministic representative, scheme matching, authority mismatch, temporal incompatibility

### T-021 — Implement edge identity predicates

- **Spec Reference:** merge.md Section 3.2; diff.md Section 2.2
- **Dependencies:** T-020
- **Complexity:** M
- **Crate:** `omtsf-core`
- **Acceptance Criteria:**
  - Edge candidate detection using composite key `(find(source), find(target), type)`
  - Per-edge-type identity property table implemented (ownership: percentage+direct; supplies: commodity+contract_ref; etc.)
  - `same_as` edges excluded from matching
  - Tests: edges matched by resolved endpoints and shared identifier, matched by type-specific properties, non-matching edges

### T-022 — Implement property merge, conflict recording, and same_as handling

- **Spec Reference:** merge.md Sections 4.1 through 4.3, 7.1 through 7.3
- **Dependencies:** T-019, T-020
- **Complexity:** L
- **Crate:** `omtsf-core`
- **Acceptance Criteria:**
  - Scalar property merge: equal values retained, differing values produce `_conflicts` array
  - Identifier set-union deduplicated by canonical string, sorted; label set-union sorted by `(key, value)`
  - `Conflict`, `ConflictEntry`, `MergeMetadata` structs with deterministic ordering
  - `same_as` edges processed after identifier-based pass with configurable confidence threshold (`definite`/`probable`/`possible`)
  - `same_as` edges retained in output with rewritten source/target
  - Tests: identical properties, conflicting properties, identifier dedup, label merge, same_as at each threshold

### T-023 — Implement full merge pipeline with deterministic output

- **Spec Reference:** merge.md Sections 5, 8
- **Dependencies:** T-021, T-022
- **Complexity:** L
- **Crate:** `omtsf-core`
- **Acceptance Criteria:**
  - `merge(files: &[OmtsFile]) -> Result<OmtsFile, MergeError>` top-level function
  - Deterministic ordering: nodes by lowest canonical identifier, edges by `(source canonical, target canonical, type, edge canonical)`
  - Merge-group safety limit (default 50) emits warnings for oversized groups
  - Post-merge L1 validation runs; output always passes L1
  - Tests: disjoint merge, full overlap, partial overlap with conflicts, three-file merge

### T-024 — Implement merge algebraic property tests

- **Spec Reference:** merge.md Section 6
- **Dependencies:** T-023
- **Complexity:** M
- **Crate:** `omtsf-core`
- **Acceptance Criteria:**
  - `proptest` added to `[dev-dependencies]`
  - `arb_omts_file()` strategy generating small graphs (1-30 nodes, 0-50 edges) with controlled identifier overlap
  - Commutativity: `sha256(merge(A,B)) == sha256(merge(B,A))`
  - Associativity: `sha256(merge(merge(A,B),C)) == sha256(merge(A,merge(B,C)))`
  - Idempotency: `assert_structurally_equal(A, merge(A,A))`

---

## Phase 6: Redaction Engine

### T-025 — Implement sensitivity classification with scheme and property defaults

- **Spec Reference:** redaction.md Sections 2.1, 2.2, 2.3
- **Dependencies:** T-004, T-003
- **Complexity:** S
- **Crate:** `omtsf-core`
- **Acceptance Criteria:**
  - `effective_sensitivity(identifier, node_type) -> Sensitivity` applying scheme defaults and person-node overrides
  - `effective_property_sensitivity(edge, property_name) -> Sensitivity` applying property defaults and `_property_sensitivity` overrides
  - Tests: default for each scheme, explicit override, person-node confidential default, edge property defaults by type

### T-026 — Implement boundary reference hashing

- **Spec Reference:** redaction.md Sections 4.1 through 4.4
- **Dependencies:** T-019, T-002
- **Complexity:** M
- **Crate:** `omtsf-core`
- **Acceptance Criteria:**
  - `boundary_ref_value(public_ids, salt) -> String` using `sha2` for SHA-256 and `getrandom` for CSPRNG
  - Deterministic path: sort canonical strings, join with newline, concatenate with decoded salt, SHA-256, hex-encode
  - Random path: 32 CSPRNG bytes hex-encoded for nodes with zero public identifiers
  - All four test vectors from redaction.md Section 4.3 pass (TV1-TV3 exact match; TV4 format-only)

### T-027 — Implement node classification, scope filtering, and edge handling

- **Spec Reference:** redaction.md Sections 3.1, 3.2, 3.3, 5, 6
- **Dependencies:** T-025, T-026
- **Complexity:** L
- **Crate:** `omtsf-core`
- **Acceptance Criteria:**
  - Node classification into Retain/Replace/Omit based on node type and target scope
  - `partner` scope: remove confidential identifiers, retain person nodes (filtered), retain beneficial_ownership (filtered)
  - `public` scope: remove confidential+restricted identifiers, omit person nodes, omit beneficial_ownership
  - Edge handling: boundary-crossing preserved, both-endpoints-replaced omitted, edges to omitted nodes omitted
  - Tests: classification per node type in both scopes, identifier filtering, person node omission

### T-028 — Implement full redaction pipeline with output validation

- **Spec Reference:** redaction.md Sections 7, 8
- **Dependencies:** T-027, T-010
- **Complexity:** M
- **Crate:** `omtsf-core`
- **Acceptance Criteria:**
  - `redact(file: &OmtsFile, scope: DisclosureScope, retain_ids: &HashSet<NodeId>) -> Result<OmtsFile, RedactError>`
  - Edge property stripping per scope threshold; `internal` scope is no-op
  - Post-redaction validation: no dangling edges, boundary_ref structure valid (L1-SDI-01), output sets `disclosure_scope`, salt preserved
  - One boundary_ref per replaced node (deduplication)
  - Tests: full redaction to partner, full redaction to public, verify L1 validity of output

---

## Phase 7: Diff Engine

### T-029 — Implement node and edge matching for diff

- **Spec Reference:** diff.md Sections 2.1, 2.2
- **Dependencies:** T-019, T-020, T-021
- **Complexity:** M
- **Crate:** `omtsf-core`
- **Acceptance Criteria:**
  - Node matching reuses merge identity predicates (canonical identifier index, transitive closure)
  - Ambiguity detection: warning when a match group contains multiple nodes from the same file
  - Edge matching using resolved endpoints + type + per-type identity properties table
  - Unmatched nodes classified as additions (in B) or deletions (in A); same for edges
  - Tests: exact match, no match (addition/deletion), ambiguous match warning

### T-030 — Implement property comparison and DiffResult assembly

- **Spec Reference:** diff.md Sections 3, 4, 5
- **Dependencies:** T-029
- **Complexity:** M
- **Crate:** `omtsf-core`
- **Acceptance Criteria:**
  - `diff(a: &OmtsFile, b: &OmtsFile) -> DiffResult` and `diff_filtered(a, b, filter) -> DiffResult`
  - Scalar comparison with date normalization and numeric epsilon (1e-9)
  - Identifier set diff (added/removed/modified by canonical key); label set diff (added/removed by `{key, value}`)
  - `DiffSummary` statistics; `DiffFilter` restricts by node type, edge type, and ignored fields
  - Tests: identical files (empty diff), property changes, identifier/label add/remove, filtering

---

## Phase 8: CLI Shell

### T-031 — Implement clap root struct, global flags, and PathOrStdin type

- **Spec Reference:** cli-interface.md Sections 2, 7, 8
- **Dependencies:** T-001
- **Complexity:** M
- **Crate:** `omtsf-cli`
- **Acceptance Criteria:**
  - Root `Cli` struct with `--format`, `--quiet`, `--verbose`, `--max-file-size`, `--no-color`, `--help`, `--version`
  - All subcommands defined in `Command` enum with correct argument types per cli-interface.md Section 7
  - `PathOrStdin` type parsing `"-"` as stdin variant; `--quiet`/`--verbose` conflicting; `--max-file-size` reads env var fallback
  - Test: help output structure for root and each subcommand

### T-032 — Implement file I/O and output formatting modules

- **Spec Reference:** cli-interface.md Sections 4, 5
- **Dependencies:** T-031, T-008
- **Complexity:** L
- **Crate:** `omtsf-cli`
- **Acceptance Criteria:**
  - File reading with size check via `std::fs::metadata`; stdin reading capped with `Read::take`
  - UTF-8 validation with byte offset on failure; broken pipe handling (SIGPIPE)
  - Error messages for file not found, permission denied, size exceeded, invalid UTF-8; all produce exit code 2
  - Human-mode diagnostic formatter: `[E]`/`[W]`/`[I]` color-coded, respects `--no-color`/`NO_COLOR`/TTY detection
  - JSON-mode diagnostic formatter: NDJSON to stderr
  - Quiet mode suppresses non-error stderr; verbose mode adds timing and metadata

### T-033 — Wire validate command

- **Spec Reference:** cli-interface.md Section 3.1; validation.md Section 7
- **Dependencies:** T-032, T-009, T-010, T-011, T-012, T-013
- **Complexity:** S
- **Crate:** `omtsf-cli`
- **Acceptance Criteria:**
  - `omtsf validate <file>` reads file, parses, runs validation at `--level` (default 2), emits diagnostics to stderr
  - Exit code 0 for valid, 1 for L1 errors, 2 for parse failure; `--level` flag controls depth
  - Stdin support via `-`; summary line in human mode
  - Integration test: validate known-good fixture (exit 0), validate known-bad fixture (exit 1)

### T-034 — Wire merge and redact commands

- **Spec Reference:** cli-interface.md Sections 3.2, 3.3
- **Dependencies:** T-032, T-023, T-028
- **Complexity:** M
- **Crate:** `omtsf-cli`
- **Acceptance Criteria:**
  - `omtsf merge <file>...` reads 2+ files, validates, merges, writes to stdout; diagnostics to stderr
  - Exit code 0 success, 1 merge conflict, 2 parse/validation failure
  - `omtsf redact <file> --scope <scope>` reads file, redacts, writes to stdout; statistics to stderr
  - Exit code 0 success, 1 scope error, 2 parse failure
  - Integration tests: merge two fixtures and validate output; redact to public and validate output

### T-035 — Wire inspect, convert, and init commands

- **Spec Reference:** cli-interface.md Sections 3.4, 3.6, 3.10
- **Dependencies:** T-032, T-006
- **Complexity:** M
- **Crate:** `omtsf-cli`
- **Acceptance Criteria:**
  - `omtsf inspect <file>` prints node/edge/identifier counts, version, date, scope; human and JSON modes
  - `omtsf convert <file>` round-trips through data model; `--pretty` (default) and `--compact` flags; unknown fields preserved
  - `omtsf init` generates minimal valid file with CSPRNG salt and today's date; `--example` adds realistic sample content
  - `omtsf init | omtsf validate -` exits 0
  - Integration tests for each command

### T-036 — Wire diff command with output formatters

- **Spec Reference:** cli-interface.md Section 3.5; diff.md Sections 5, 6
- **Dependencies:** T-032, T-030
- **Complexity:** M
- **Crate:** `omtsf-cli`
- **Acceptance Criteria:**
  - `omtsf diff <a> <b>` computes diff, writes to stdout
  - Human mode: unified-diff-style `+`/`-`/`~` output with summary; JSON mode: structured diff object
  - `--ids-only` and `--summary-only` flags; `--node-type`, `--edge-type`, `--ignore-field` filter flags
  - Exit code 0 identical, 1 differences, 2 parse failure
  - Integration test: diff identical files (exit 0), diff modified files (exit 1)

### T-037 — Wire graph query commands (reach, path, subgraph)

- **Spec Reference:** cli-interface.md Sections 3.7, 3.8, 3.9
- **Dependencies:** T-032, T-016, T-017
- **Complexity:** M
- **Crate:** `omtsf-cli`
- **Acceptance Criteria:**
  - `omtsf reach <file> <node-id>` outputs reachable nodes with `--depth` and `--direction` flags
  - `omtsf path <file> <from> <to>` outputs paths with `--max-paths` and `--max-depth` flags
  - `omtsf subgraph <file> <node-id>...` outputs valid `.omts` file with `--expand` flag
  - Human and JSON output modes for each; exit codes per spec
  - Integration test for each command with a fixture

---

## Phase 9: Integration & Polish

### T-038 — Build end-to-end integration test suite

- **Spec Reference:** merge.md Section 6.3; all spec documents
- **Dependencies:** T-033, T-034, T-035, T-036, T-037
- **Complexity:** L
- **Crate:** Both (workspace-level `tests/`)
- **Acceptance Criteria:**
  - At least 15 integration tests covering cross-command workflows: `init | validate`, `merge A B | validate`, `redact --scope public | validate`, `merge | redact | diff` pipeline, `subgraph | inspect`
  - Merge regression fixtures (disjoint, full overlap, partial overlap, transitive chain, temporal incompatibility, ANNULLED LEI)
  - Redaction fixtures with person nodes, boundary refs, and scope transitions
  - All tests run under `cargo test` from workspace root

### T-039 — Review error messages and eliminate unwrap/expect in user paths

- **Spec Reference:** cli-interface.md Section 4.5; validation.md Section 6
- **Dependencies:** T-038
- **Complexity:** M
- **Crate:** Both
- **Acceptance Criteria:**
  - Every user-facing error message includes: what went wrong, which file/node/edge is affected, and guidance
  - No raw `unwrap()` or `expect()` in CLI code paths handling user input
  - Parse errors include line/column from `serde_json::Error`
  - Internal errors (bugs) produce a message requesting the user file a report per validation.md Section 6.3

### T-040 — Verify WASM compatibility of omtsf-core

- **Spec Reference:** overview.md Section 2; data-model.md Sections 11.1 through 11.3
- **Dependencies:** T-023, T-028, T-030, T-018
- **Complexity:** S
- **Crate:** `omtsf-core`
- **Acceptance Criteria:**
  - `cargo build --target wasm32-unknown-unknown -p omtsf-core` succeeds
  - No `std::fs`, `std::net`, `std::process` imports in `omtsf-core`
  - `sha2` and `getrandom` compile for wasm32 target
  - CI check added for WASM build (build only, not test)

---

## Spec Ambiguities and Notes

The following ambiguities or underspecified areas were identified during task planning. They may require clarification before or during implementation.

1. **L2 rule enumeration.** validation.md Section 4.2 provides examples of L2 rules (L2-GDM-01 through L2-GDM-04, L2-EID-01 through L2-EID-08) but does not exhaustively list them with check descriptions as it does for L1. The implementor will need to derive the full L2 rule set from the SHOULD constraints in SPEC-001 and SPEC-002.

2. **Edge source/target type compatibility table.** L1-GDM-06 references a "permitted types table (Section 9.5)" in SPEC-001, but the full table is not reproduced in the implementation spec. The implementor must consult SPEC-001 Section 9.5 directly.

3. **`merge --strategy intersect`.** cli-interface.md Section 3.2 mentions an `intersect` strategy, but merge.md does not describe its semantics. The implementor must decide: does intersect retain only nodes present in all input files, or only nodes with identity matches? This needs spec clarification.

4. **Redact node selection mechanism.** redaction.md Section 5 states the producer chooses which nodes to retain vs. replace, and cli-interface.md Section 3.3 mentions `--scope` but does not describe how the user specifies which nodes to retain (e.g., `--retain <node-id>...`, a separate config file, or retaining all nodes by default). The CLI flag set for `redact` needs further specification.

5. **`convert --pretty` vs `--compact` mutual exclusivity.** cli-interface.md Section 3.6 models `--pretty` as default-true with `--compact` conflicting. Cleaner UX would be just `--compact` as opt-in with pretty as the unmarked default. Minor, but worth aligning before implementation.

6. **Additional enums not fully listed.** data-model.md Section 6.2 references `ConsolidationBasis`, `EventType`, and `ServiceType` enums for edge properties, but their variants are not enumerated in the implementation spec. These must be derived from SPEC-001.

7. **`name` field scope on Node.** data-model.md Section 5.1 lists `name: Option<String>` under "organization" fields, but SPEC-001 may allow `name` on other node types. The flat struct handles this naturally, but validation rules checking required fields per type need to consult the normative spec.

8. **Diff header comparison.** diff.md Section 7 notes header field comparison is "outside the scope of the structural diff" but the CLI "MAY report header differences as a separate informational section." Whether to implement this is left to implementor judgment.
