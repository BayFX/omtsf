# omtsf-rs Implementation Task Plan

**Date:** 2026-02-20
**Status:** Approved

---

## Phase 1: Workspace Setup

### T-001 -- Initialize Cargo workspace and crate scaffolding

- **Spec Reference:** overview.md Section 3
- **Dependencies:** None
- **Complexity:** S
- **Crate:** Both
- **Acceptance Criteria:**
  - Workspace `Cargo.toml` at `omtsf-rs/` with members `crates/omtsf-core` and `crates/omtsf-cli`
  - `omtsf-core` compiles as a library crate (`lib.rs`) with `serde`, `serde_json`, and `petgraph` dependencies declared
  - `omtsf-cli` compiles as a binary crate (`main.rs`) with `clap` dependency declared and a dependency on `omtsf-core`
  - Stub `omtsf-wasm` crate exists with an empty `lib.rs`
  - `cargo build --workspace` succeeds with no errors

### T-002 -- Configure CI and WASM compile-check

- **Spec Reference:** overview.md Sections 4, 11
- **Dependencies:** T-001
- **Complexity:** S
- **Crate:** Both
- **Acceptance Criteria:**
  - GitHub Actions workflow runs `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, and `cargo fmt -- --check` on push
  - CI includes a `cargo check --target wasm32-unknown-unknown -p omtsf-core` step that verifies the core crate compiles to WASM
  - CI matrix covers stable and MSRV Rust toolchains

---

## Phase 2: Data Model

### T-003 -- Define newtype wrappers with validation-on-construct

- **Spec Reference:** data-model.md Section 3
- **Dependencies:** T-001
- **Complexity:** M
- **Crate:** omtsf-core
- **Acceptance Criteria:**
  - `SemVer`, `CalendarDate`, `FileSalt`, `NodeId`, `CountryCode` newtypes are defined with `TryFrom<&str>` constructors
  - Each newtype validates its regex shape on construction and rejects invalid inputs
  - All newtypes implement `Deref<Target = str>`, `Display`, `Serialize`, and `Deserialize` (with validation in the `Deserialize` impl)
  - Unit tests cover valid inputs, boundary cases, and rejected inputs for each newtype

### T-004 -- Define enums for node types, edge types, and property-level enums

- **Spec Reference:** data-model.md Sections 4.1 -- 4.5
- **Dependencies:** T-001
- **Complexity:** M
- **Crate:** omtsf-core
- **Acceptance Criteria:**
  - `NodeType`, `EdgeType`, `DisclosureScope`, `AttestationType`, `Confidence`, `Sensitivity`, `VerificationStatus`, `OrganizationStatus`, and all other property-level enums from data-model.md Section 4.4 are defined
  - All enums use `#[serde(rename_all = "snake_case")]` and round-trip correctly through JSON
  - `NodeTypeTag` and `EdgeTypeTag` two-variant enums (`Known` / `Extension`) are defined with custom `Deserialize` impls that fall back to `Extension` for unrecognized strings
  - Unit tests confirm serialization of each variant and correct fallback for extension strings

### T-005 -- Define Node struct with flat optional fields and custom deserializer

- **Spec Reference:** data-model.md Sections 5.1 -- 5.4
- **Dependencies:** T-003, T-004
- **Complexity:** L
- **Crate:** omtsf-core
- **Acceptance Criteria:**
  - `Node` struct defined with all fields from data-model.md Section 5.1, including `#[serde(flatten)] pub extra`
  - Custom `Deserialize` impl routes the JSON `"status"` field to `status` (OrganizationStatus) or `attestation_status` (AttestationStatus) based on the node's `type` tag
  - `Option<Option<CalendarDate>>` for `valid_to` correctly distinguishes absent, `null`, and present values
  - Round-trip tests: deserialize a Node JSON, re-serialize, and confirm byte-identical output for each node type (organization, facility, good, person, attestation, consignment, boundary_ref)

### T-006 -- Define Edge and EdgeProperties structs

- **Spec Reference:** data-model.md Sections 6.1 -- 6.3
- **Dependencies:** T-003, T-004
- **Complexity:** M
- **Crate:** omtsf-core
- **Acceptance Criteria:**
  - `Edge` struct with `id`, `edge_type`, `source`, `target`, `properties`, `identifiers`, and `extra` fields
  - `EdgeProperties` struct with all fields from data-model.md Section 6.2, including `control_type` as `Option<serde_json::Value>`
  - `EdgeTypeTag` custom deserializer works identically to `NodeTypeTag`
  - Round-trip serde tests for at least five edge types with varying property sets

### T-007 -- Define shared types: Identifier, DataQuality, Label, Geo

- **Spec Reference:** data-model.md Sections 7.1 -- 7.4
- **Dependencies:** T-003, T-004
- **Complexity:** M
- **Crate:** omtsf-core
- **Acceptance Criteria:**
  - `Identifier`, `DataQuality`, `Label` structs defined with `#[serde(flatten)] pub extra` on each
  - `Geo` enum with `Point { lat, lon }` and `GeoJson(Value)` variants, accessible via `Node::geo_parsed()` method
  - `Identifier.scheme` is a plain `String`; scheme-specific validation deferred to the validation engine
  - Serde round-trip tests for each shared type, including unknown-field preservation via the `extra` map

### T-008 -- Define OmtsFile top-level struct and serde strategy

- **Spec Reference:** data-model.md Sections 2, 8.1 -- 8.5
- **Dependencies:** T-005, T-006, T-007
- **Complexity:** M
- **Crate:** omtsf-core
- **Acceptance Criteria:**
  - `OmtsFile` struct with all fields from data-model.md Section 2, field declaration order matches required JSON key order
  - `#[serde(rename_all = "snake_case")]` applied; `type` fields use `#[serde(rename = "type")]`
  - `#[serde(skip_serializing_if = "Option::is_none")]` on all optional fields except `valid_to` (uses custom serializer)
  - End-to-end test: parse a fixture JSON string into `OmtsFile`, re-serialize, and confirm the output is semantically equivalent (key order, null handling, unknown field preservation)

### T-009 -- Create .omts fixture files for testing

- **Spec Reference:** overview.md Section 3 (tests/ directory)
- **Dependencies:** T-008
- **Complexity:** M
- **Crate:** Both (tests/)
- **Acceptance Criteria:**
  - At least 5 fixture files: minimal valid file, realistic supply chain graph, file with extension types, file with boundary_ref nodes, file with deliberate L1 violations
  - All fixtures deserialize cleanly into `OmtsFile` (except the invalid fixture)
  - Fixtures are committed under `omtsf-rs/tests/fixtures/` and are reusable by all subsequent test tasks

---

## Phase 3: Validation Engine

### T-010 -- Define Diagnostic, RuleId, Severity, Location types

- **Spec Reference:** validation.md Sections 2, 2.1, 2.2
- **Dependencies:** T-003
- **Complexity:** S
- **Crate:** omtsf-core
- **Acceptance Criteria:**
  - `Diagnostic`, `Severity`, `Location`, `RuleId` types defined as specified
  - `RuleId` is `#[non_exhaustive]` and includes all L1/L2/L3 variants plus `Internal` and `Extension(String)`
  - `RuleId::code()` returns the correct hyphenated string for every variant
  - `ValidationResult` with `has_errors()`, `errors()`, `warnings()`, `infos()`, `is_conformant()` methods
  - Unit tests for `RuleId::code()` mapping and `ValidationResult` filtering

### T-011 -- Build ValidationRule trait, registry, and ValidationContext

- **Spec Reference:** validation.md Sections 3.1 -- 3.4
- **Dependencies:** T-008, T-010
- **Complexity:** M
- **Crate:** omtsf-core
- **Acceptance Criteria:**
  - `ValidationRule` trait with `id()`, `level()`, `severity()`, and `check()` methods
  - `ValidationConfig` struct with `run_l1`, `run_l2`, `run_l3`, and `external_data` fields
  - `build_registry(config)` returns a `Vec<Box<dyn ValidationRule>>` with rules gated by level
  - `ValidationContext` constructed from `&OmtsFile` with `node_by_id`, `edge_by_id`, `node_ids`, `edge_ids` maps
  - `ValidateOutput` enum with `ParseFailed` and `Validated` variants
  - Test: an empty registry produces zero diagnostics; a single stub rule pushes exactly one diagnostic

### T-012 -- Implement check-digit functions (MOD 97-10, GS1 mod-10)

- **Spec Reference:** validation.md Sections 5.1, 5.2
- **Dependencies:** T-001
- **Complexity:** S
- **Crate:** omtsf-core
- **Acceptance Criteria:**
  - `mod97_10_check(lei: &str) -> bool` passes all test vectors from validation.md Section 5.1
  - `gs1_mod10_check(gln: &str) -> bool` passes all test vectors from validation.md Section 5.2
  - Both functions operate on `&str`, return `bool`, and do not allocate
  - Edge case tests: all-zeros GLN, max-value inputs, wrong-length inputs return false

### T-013 -- Implement L1 validation rules (SPEC-001 GDM rules)

- **Spec Reference:** validation.md Section 4.1 (L1-GDM-01 through L1-GDM-06)
- **Dependencies:** T-011, T-009
- **Complexity:** L
- **Crate:** omtsf-core
- **Acceptance Criteria:**
  - Six rule structs implementing `ValidationRule`, one per L1-GDM rule
  - L1-GDM-01: duplicate node IDs detected and reported; first occurrence wins in context map
  - L1-GDM-03: dangling edge source/target references produce diagnostics with edge ID and field name
  - L1-GDM-06: source/target node type constraints enforced per SPEC-001 Section 9.5 table; extension edge types exempt
  - Test each rule individually against fixture files; confirm correct `RuleId`, `Severity::Error`, and `Location`

### T-014 -- Implement L1 validation rules (SPEC-002 EID rules)

- **Spec Reference:** validation.md Section 4.1 (L1-EID-01 through L1-EID-11)
- **Dependencies:** T-011, T-012, T-009
- **Complexity:** L
- **Crate:** omtsf-core
- **Acceptance Criteria:**
  - Eleven rule structs implementing `ValidationRule`, one per L1-EID rule
  - L1-EID-05: regex + MOD 97-10 check for LEI identifiers
  - L1-EID-07: regex + GS1 mod-10 check for GLN identifiers
  - L1-EID-09: `valid_from <= valid_to` enforcement with correct null handling
  - L1-EID-11: duplicate `{scheme, value, authority}` tuple detection within a single node
  - Tests: valid and invalid identifiers per scheme, temporal range violations, duplicate tuples

### T-015 -- Implement L1 validation rules (SPEC-004 SDI rules)

- **Spec Reference:** validation.md Section 4.1 (L1-SDI-01, L1-SDI-02)
- **Dependencies:** T-011, T-009
- **Complexity:** S
- **Crate:** omtsf-core
- **Acceptance Criteria:**
  - L1-SDI-01: boundary_ref nodes verified to have exactly one identifier with scheme `opaque`
  - L1-SDI-02: disclosure_scope constraints enforced (public scope forbids restricted/confidential identifiers and person nodes; partner scope forbids confidential identifiers)
  - Tests with fixture files covering each constraint violation

### T-016 -- Implement L2 validation rules

- **Spec Reference:** validation.md Section 4.2 (L2-GDM-01 through L2-GDM-04, L2-EID-01 through L2-EID-08)
- **Dependencies:** T-011, T-009
- **Complexity:** L
- **Crate:** omtsf-core
- **Acceptance Criteria:**
  - Twelve rule structs, one per L2 rule
  - All produce `Severity::Warning` diagnostics
  - L2-GDM-01: facility-to-organization connectivity check
  - L2-EID-01: organization with no external identifiers flagged
  - L2-EID-08: verified identifiers without verification_date flagged
  - Tests confirm each rule fires on appropriate fixture input and does NOT fire on conformant input

### T-017 -- Implement L3 validation stubs and ExternalDataSource trait

- **Spec Reference:** validation.md Sections 4.3, 3.2
- **Dependencies:** T-011
- **Complexity:** M
- **Crate:** omtsf-core
- **Acceptance Criteria:**
  - `ExternalDataSource` trait defined with `lei_status()` and `nat_reg_lookup()` methods
  - Seven L3 rule structs registered when `config.run_l3` is true
  - L3-MRG-01 and L3-MRG-02 implemented with real logic (ownership percentage sum check and legal_parentage cycle detection)
  - L3-EID-01 through L3-EID-05 implemented as stubs that delegate to `ExternalDataSource` and produce `Severity::Info` diagnostics
  - Tests with mock `ExternalDataSource` confirm rules fire correctly

---

## Phase 4: Graph Engine

### T-018 -- Implement OmtsGraph construction from OmtsFile (petgraph wrapper)

- **Spec Reference:** graph-engine.md Sections 2.1 -- 2.4
- **Dependencies:** T-008
- **Complexity:** M
- **Crate:** omtsf-core
- **Acceptance Criteria:**
  - `OmtsGraph` struct wrapping `StableDiGraph<NodeWeight, EdgeWeight>` and `HashMap<String, NodeIndex>`
  - `build_graph(&OmtsFile) -> Result<OmtsGraph, GraphBuildError>` constructs graph in O(N+E)
  - `GraphBuildError::DuplicateNodeId` and `GraphBuildError::DanglingEdgeRef` error variants
  - Accessor methods: `node_count()`, `edge_count()`, `node_index()`, `node_weight()`, `edge_weight()`
  - Tests: build from fixture file, verify node/edge counts, verify ID lookup correctness

### T-019 -- Implement BFS reachability query

- **Spec Reference:** graph-engine.md Section 3
- **Dependencies:** T-018
- **Complexity:** M
- **Crate:** omtsf-core
- **Acceptance Criteria:**
  - `reachable_from(graph, start, direction, edge_filter)` returns `HashSet<NodeIndex>` of all reachable nodes
  - Supports `Forward`, `Backward`, and `Both` directions
  - Optional `edge_filter` restricts traversal to specified edge types
  - Start node excluded from result set
  - Tests: reachability on a known graph fixture with forward/backward/both directions; edge-type filtering; disconnected components

### T-020 -- Implement shortest path and all-paths queries

- **Spec Reference:** graph-engine.md Sections 4.1 -- 4.3
- **Dependencies:** T-018
- **Complexity:** M
- **Crate:** omtsf-core
- **Acceptance Criteria:**
  - `shortest_path(graph, from, to, direction, edge_filter)` returns `Option<Vec<NodeIndex>>` via BFS with predecessor map
  - `all_paths(graph, from, to, max_depth, direction, edge_filter)` returns `Vec<Vec<NodeIndex>>` via iterative-deepening DFS
  - `DEFAULT_MAX_DEPTH = 20`; all_paths enforces simple paths (no node revisited within a single path)
  - Tests: shortest path on linear chain, diamond graph, no-path case; all_paths with depth limit; from == to case

### T-021 -- Implement induced subgraph extraction and ego-graph

- **Spec Reference:** graph-engine.md Sections 5.1 -- 5.3
- **Dependencies:** T-018, T-019
- **Complexity:** M
- **Crate:** omtsf-core
- **Acceptance Criteria:**
  - `induced_subgraph(graph, file, node_ids)` returns a valid `OmtsFile` with only specified nodes and edges between them
  - `ego_graph(graph, file, center, radius, direction)` returns subgraph of nodes within `radius` hops
  - `reporting_entity` set to `None` if referenced node absent from subgraph
  - Output round-trips through serde; L1 validation passes on output
  - Tests: subgraph of known fixture, ego_graph with radius 0 and radius 1, reporting_entity preservation/clearing

### T-022 -- Implement cycle detection (Kahn's topological sort)

- **Spec Reference:** graph-engine.md Sections 6.1 -- 6.4
- **Dependencies:** T-018
- **Complexity:** M
- **Crate:** omtsf-core
- **Acceptance Criteria:**
  - `detect_cycles(graph, edge_types)` returns `Vec<Vec<NodeIndex>>` -- empty if acyclic
  - Uses Kahn's algorithm with in-degree map; cyclic nodes identified when queue drains prematurely
  - Individual cycle extraction via DFS on remaining nodes
  - Cycles represented as closed sequences (first == last element)
  - Tests: acyclic graph returns empty vec; single cycle; multiple disjoint cycles; graph mixing acyclic and cyclic subgraphs

---

## Phase 5: Merge Engine

### T-023 -- Implement UnionFind data structure

- **Spec Reference:** merge.md Section 2.1
- **Dependencies:** T-001
- **Complexity:** S
- **Crate:** omtsf-core
- **Acceptance Criteria:**
  - `UnionFind::new(n)`, `find(&mut self, x) -> usize`, `union(&mut self, a, b)`
  - Path halving compression in `find`; union-by-rank with lower-ordinal-wins tie-breaking
  - Deterministic: `find` returns the same representative regardless of union call order
  - Unit tests: basic union/find, transitive closure, deterministic representative selection, idempotent union

### T-024 -- Implement CanonicalId and identifier index construction

- **Spec Reference:** merge.md Sections 2.2, 3.3
- **Dependencies:** T-007, T-023
- **Complexity:** M
- **Crate:** omtsf-core
- **Acceptance Criteria:**
  - `CanonicalId` newtype with percent-encoding of colons, percent signs, newlines, and carriage returns
  - Canonical form: `{scheme}:{value}` for most schemes; `{scheme}:{authority}:{value}` for `nat-reg` and `vat`
  - `build_identifier_index(nodes) -> HashMap<CanonicalId, Vec<usize>>` excludes `internal` scheme and ANNULLED LEIs
  - Tests: canonical form for each scheme type; percent-encoding edge cases; index construction with overlapping identifiers

### T-025 -- Implement node identity predicate and pairwise matching

- **Spec Reference:** merge.md Sections 3.1, 2.2
- **Dependencies:** T-024, T-023
- **Complexity:** M
- **Crate:** omtsf-core
- **Acceptance Criteria:**
  - `identifiers_match(a, b) -> bool` implements the five rules from merge.md Section 3.1
  - `internal` scheme always returns false; authority comparison is case-insensitive; temporal compatibility checked
  - Index-driven candidate detection: for each key with 2+ nodes, evaluate pairwise predicate and union matches
  - Tests: matching by LEI, DUNS, nat-reg with authority; rejection for scheme mismatch, temporal incompatibility, internal scheme

### T-026 -- Implement edge identity predicate and composite key index

- **Spec Reference:** merge.md Sections 3.2, 3.3
- **Dependencies:** T-025
- **Complexity:** M
- **Crate:** omtsf-core
- **Acceptance Criteria:**
  - `EdgeCompositeKey` struct with `source_rep`, `target_rep`, `edge_type`
  - `edge_identity_properties_match(a, b, edge_type)` encodes the SPEC-003 S3.1 table
  - Floating-point properties compared via `to_bits()`; `same_as` edges excluded from matching
  - `build_edge_candidate_index(edges, uf) -> HashMap<EdgeCompositeKey, Vec<usize>>` constructs the index
  - Tests: edge matching per type; floating-point edge cases (NaN, -0.0); same_as exclusion

### T-027 -- Implement property merge, conflict recording, and deterministic output

- **Spec Reference:** merge.md Sections 4.1 -- 4.3, 5
- **Dependencies:** T-024
- **Complexity:** L
- **Crate:** omtsf-core
- **Acceptance Criteria:**
  - `merge_scalars` compares N `(Option<T>, source_file)` pairs and returns `Agreed(value)` or `Conflict(entries)`
  - `merge_identifiers` deduplicates by `CanonicalId`, sorts by canonical string in UTF-8 byte order
  - `merge_labels` deduplicates by `{key, value}` pair, sorts by key then value (None before Some)
  - Conflict entries sorted by `(source_file, json_value_as_string)`
  - `MergeMetadata` struct populated with source_files, reporting_entities, timestamp, counts
  - Tests: scalar agree and conflict cases; identifier deduplication and ordering; label merging; conflict serialization

### T-028 -- Implement same_as edge handling

- **Spec Reference:** merge.md Section 7
- **Dependencies:** T-023, T-025
- **Complexity:** M
- **Crate:** omtsf-core
- **Acceptance Criteria:**
  - `SameAsThreshold` enum with `Definite`, `Probable`, `Possible` variants and `honours(confidence_str)` method
  - `apply_same_as_edges(edges, uf, threshold)` unions source/target for edges that pass the threshold
  - Absent confidence treated as `"possible"`; unrecognized confidence strings treated as `"possible"`
  - Honoured same_as edges collected and returned for provenance reporting
  - Tests: threshold filtering at each level; absent confidence handling; transitive closure via same_as

### T-029 -- Implement eight-step merge pipeline with post-merge validation

- **Spec Reference:** merge.md Sections 8, 9
- **Dependencies:** T-025, T-026, T-027, T-028, T-013
- **Complexity:** XL
- **Crate:** omtsf-core
- **Acceptance Criteria:**
  - `merge_with_config(files, config) -> Result<MergeOutput, MergeError>` orchestrates all eight steps from merge.md Section 8
  - Colliding graph-local IDs across files handled correctly (per-file ID maps)
  - Merged node IDs assigned deterministically (`n-0`, `n-1`, ...)
  - Nodes sorted by lowest canonical identifier; edges sorted by `(source_canonical, target_canonical, type, edge_canonical, representative_ordinal)`
  - Post-merge L1 validation runs; `MergeError::PostMergeValidationFailed` returned on failure
  - Merge-group safety limits: warning emitted for groups exceeding `group_size_limit` (default 50)
  - Tests: disjoint merge, full-overlap merge, partial-overlap merge, conflicting properties, transitive chains

### T-030 -- Implement merge algebraic property tests

- **Spec Reference:** merge.md Section 6
- **Dependencies:** T-029
- **Complexity:** L
- **Crate:** omtsf-core
- **Acceptance Criteria:**
  - `proptest` dependency added; strategy `arb_omts_file()` generates small graphs with DUNS identifiers from a shared pool
  - Commutativity test: `stable_hash(merge(A,B)) == stable_hash(merge(B,A))`
  - Associativity test: `stable_hash(merge(merge(A,B),C)) == stable_hash(merge(A,merge(B,C)))`
  - Idempotency test: `assert_structurally_equal(A, merge(A,A))` comparing node partition and edge connectivity
  - `stable_hash` zeros `file_salt` and `timestamp` before hashing
  - All three properties pass on 256+ generated cases without failure

---

## Phase 6: Redaction Engine

### T-031 -- Implement sensitivity classification and effective_sensitivity

- **Spec Reference:** redaction.md Sections 2.1 -- 2.3
- **Dependencies:** T-007, T-004
- **Complexity:** M
- **Crate:** omtsf-core
- **Acceptance Criteria:**
  - `effective_sensitivity(identifier, node_type) -> Sensitivity` implements the cascade: explicit override > person-node rule > scheme default
  - Scheme-default table implemented: LEI/DUNS/GLN = public, nat-reg/vat/internal = restricted, unrecognized = public
  - Edge property sensitivity defaults table implemented, dispatching `percentage` on edge type
  - `_property_sensitivity` override map consulted first for edge properties
  - Tests: each scheme default; person-node override to confidential; explicit override wins; edge property defaults per edge type

### T-032 -- Implement boundary reference hashing

- **Spec Reference:** redaction.md Sections 4.1 -- 4.4
- **Dependencies:** T-024, T-003
- **Complexity:** M
- **Crate:** omtsf-core
- **Acceptance Criteria:**
  - `boundary_ref_value(public_ids, salt) -> Result<String, BoundaryHashError>` implemented
  - Deterministic path: sort canonical strings, join with newline, concatenate with decoded salt, SHA-256, hex-encode
  - Random path: 32 CSPRNG bytes via `getrandom`, hex-encoded
  - `sha2` and `getrandom` dependencies added (no `ring` or `openssl`)
  - All four test vectors from redaction.md Section 4.3 pass (TV1--TV3 deterministic, TV4 format-only)

### T-033 -- Implement node classification and edge filtering logic

- **Spec Reference:** redaction.md Sections 3, 5, 6
- **Dependencies:** T-031
- **Complexity:** M
- **Crate:** omtsf-core
- **Acceptance Criteria:**
  - `classify_node(node, target_scope) -> NodeAction` returns Retain/Replace/Omit per the classification table
  - Producer retain-set promotion: nodes not in retain set and not boundary_ref promoted from Retain to Replace
  - `classify_edge(edge, source_action, target_action, target_scope) -> EdgeAction` implements priority-ordered rules: beneficial_ownership in public scope omitted; either endpoint omitted -> omit; both endpoints replaced -> omit; otherwise retain
  - Tests for each combination of node type, scope, and edge disposition

### T-034 -- Implement full redaction pipeline with post-redaction validation

- **Spec Reference:** redaction.md Sections 5 -- 7
- **Dependencies:** T-031, T-032, T-033, T-013
- **Complexity:** L
- **Crate:** omtsf-core
- **Acceptance Criteria:**
  - `redact(file, target_scope, retain_ids) -> Result<OmtsFile, RedactError>` orchestrates full redaction
  - Identifier filtering per scope: partner removes confidential; public removes confidential and restricted
  - Edge property stripping per scope's sensitivity threshold
  - Boundary ref nodes created with original node ID, type `boundary_ref`, single `opaque` identifier
  - `boundary_ref_values` HashMap ensures one hash per replaced node
  - Output `disclosure_scope` set to target scope; `file_salt` preserved
  - Post-redaction L1 validation runs; `RedactError::InvalidOutput` on failure
  - Tests: partner scope redaction, public scope redaction, boundary ref determinism, person-node omission in public scope, beneficial_ownership edge omission

---

## Phase 7: Diff Engine

### T-035 -- Implement node and edge matching for diff

- **Spec Reference:** diff.md Sections 2.1 -- 2.2
- **Dependencies:** T-024, T-023
- **Complexity:** M
- **Crate:** omtsf-core
- **Acceptance Criteria:**
  - Node matching reuses the SPEC-003 identity predicate via identifier index and union-find
  - Ambiguous match groups (multiple nodes from same file in one group) produce warnings
  - Edge matching applies endpoint-group + type + identifier/property matching per the SPEC-003 S3.1 table
  - `same_as` edges never matched; excess edges reported as additions or deletions
  - Tests: exact match, partial overlap, disjoint files, ambiguous match groups

### T-036 -- Implement property comparison and DiffResult construction

- **Spec Reference:** diff.md Sections 3, 4.1 -- 4.3
- **Dependencies:** T-035
- **Complexity:** M
- **Crate:** omtsf-core
- **Acceptance Criteria:**
  - `diff(a, b) -> DiffResult` and `diff_filtered(a, b, filter) -> DiffResult` entry points implemented
  - Scalar properties compared by value; dates normalized before comparison; floating-point epsilon `1e-9`
  - Identifier arrays compared as sets by canonical key; per-identifier field diffs reported
  - Labels compared as `{key, value}` set: changed value = removal + addition
  - `DiffSummary` with accurate counts including `nodes_unchanged` and `edges_unchanged`
  - `DiffResult::is_empty()` returns true when files are identical
  - Tests: identical files produce empty diff; added/removed/modified nodes and edges; date normalization; float epsilon

---

## Phase 8: CLI Shell

### T-037 -- Implement clap argument parsing, global flags, and main dispatch

- **Spec Reference:** cli-interface.md Sections 2, 7, 9
- **Dependencies:** T-001
- **Complexity:** M
- **Crate:** omtsf-cli
- **Acceptance Criteria:**
  - `Cli` struct with `#[derive(Parser)]` and all global flags: `--format`, `--quiet`, `--verbose`, `--max-file-size`, `--no-color`, `--help`, `--version`
  - `Command` enum with all 10 subcommands from cli-interface.md Section 7
  - `PathOrStdin` type with `FromStr` impl; multi-stdin validator for merge and diff
  - `main()` calls `reset_sigpipe()`, parses args, dispatches to `run()`, maps exit codes
  - `CliError` enum with `Io`, `FileTooLarge`, `InvalidUtf8`, `Parse`, `MultipleStdin` variants
  - Tests: clap rejects conflicting `--quiet` and `--verbose`; `--version` prints version; parse of each subcommand

### T-038 -- Implement file I/O module (read pipeline, size enforcement, stdin)

- **Spec Reference:** cli-interface.md Sections 4.1 -- 4.6
- **Dependencies:** T-037
- **Complexity:** M
- **Crate:** omtsf-cli
- **Acceptance Criteria:**
  - `read_file(path_or_stdin, max_size) -> Result<String, CliError>` implements the five-step read pipeline
  - Disk files: metadata size check before reading
  - Stdin: `Read::take(max_size + 1)` to bound allocation; excess detected and rejected
  - UTF-8 validation with byte-offset error reporting
  - File-not-found and permission-denied produce descriptive stderr messages and exit code 2
  - Tests: file too large rejection, invalid UTF-8 rejection, stdin read (via piped test)

### T-039 -- Implement output formatting (human and JSON modes)

- **Spec Reference:** cli-interface.md Sections 5.1 -- 5.4
- **Dependencies:** T-037, T-010
- **Complexity:** M
- **Crate:** omtsf-cli
- **Acceptance Criteria:**
  - Human-mode diagnostic formatter: `[E]` / `[W]` / `[I]` prefixes with ANSI color codes (red/yellow/cyan)
  - JSON-mode diagnostic formatter: NDJSON to stderr, one JSON object per finding
  - Color detection logic: disabled when `--no-color`, `NO_COLOR` env, or non-TTY stderr
  - Summary line for validate command: `"N errors, N warnings, N info (checked N nodes, N edges)"`
  - Human-mode diff formatter: `+` / `-` / `~` prefix lines per diff.md Section 5.1
  - Tests: formatter output matches expected strings for sample diagnostics in both modes

### T-040 -- Wire validate, inspect, convert, and init commands

- **Spec Reference:** cli-interface.md Sections 3.1, 3.4, 3.6, 3.10
- **Dependencies:** T-038, T-039, T-011, T-018
- **Complexity:** M
- **Crate:** omtsf-cli
- **Acceptance Criteria:**
  - `validate`: reads file, calls validation engine at requested `--level`, formats diagnostics to stderr, returns correct exit code (0/1/2)
  - `inspect`: reads file, builds graph, prints summary (node/edge counts by type, identifier counts by scheme, header fields) to stdout
  - `convert`: reads file, re-serializes with `--pretty` (default) or `--compact`, writes to stdout
  - `init`: generates minimal valid `.omts` (fresh salt, today's date, empty arrays) or `--example` with sample data
  - Integration tests for each command using fixture files; verify exit codes

### T-041 -- Wire merge and redact commands

- **Spec Reference:** cli-interface.md Sections 3.2, 3.3
- **Dependencies:** T-038, T-039, T-029, T-034
- **Complexity:** M
- **Crate:** omtsf-cli
- **Acceptance Criteria:**
  - `merge`: reads 2+ files, validates each (reject on L1 failure), runs merge engine, writes merged `.omts` to stdout, diagnostics to stderr
  - `redact`: reads file, applies redaction for `--scope`, writes redacted `.omts` to stdout, statistics to stderr
  - Exit codes match cli-interface.md Section 6 table
  - Integration tests: merge two fixture files and validate output; redact to public scope and verify no person nodes

### T-042 -- Wire reach, path, and subgraph commands

- **Spec Reference:** cli-interface.md Sections 3.7, 3.8, 3.9
- **Dependencies:** T-038, T-039, T-019, T-020, T-021
- **Complexity:** M
- **Crate:** omtsf-cli
- **Acceptance Criteria:**
  - `reach`: builds graph, runs BFS reachability with `--direction` and optional `--depth`, prints node IDs to stdout
  - `path`: builds graph, runs shortest_path or all_paths with `--max-paths` and `--max-depth`, prints paths to stdout
  - `subgraph`: builds graph, extracts induced subgraph with optional `--expand`, writes `.omts` to stdout
  - Human and JSON output modes for each command
  - Exit codes: 0 on success, 1 on node-not-found or no-path, 2 on parse failure
  - Integration tests for each command

### T-043 -- Wire diff command

- **Spec Reference:** cli-interface.md Section 3.5
- **Dependencies:** T-038, T-039, T-036
- **Complexity:** M
- **Crate:** omtsf-cli
- **Acceptance Criteria:**
  - `diff`: reads two files, calls `diff()` or `diff_filtered()`, formats output to stdout
  - `--ids-only` flag suppresses property-level detail
  - `--format json` serializes `DiffResult` as a single JSON object
  - Exit code 0 when identical, 1 when differences found, 2 on parse failure
  - Integration tests: identical files -> exit 0; modified fixture -> exit 1 with expected output

---

## Phase 9: Integration & Polish

### T-044 -- End-to-end pipeline tests ✅

- **Spec Reference:** All specs
- **Dependencies:** T-040, T-041, T-042, T-043
- **Complexity:** L
- **Crate:** Both
- **Acceptance Criteria:**
  - Test: `omtsf init --example | omtsf validate -` exits 0
  - Test: `omtsf init --example | omtsf redact --scope public - | omtsf validate -` exits 0
  - Test: merge two fixture files, validate output, inspect output, diff output against expected
  - Test: `omtsf subgraph` output passes validation
  - Test: stdin piping works for all commands that accept `-`
  - All tests pass in CI

### T-045 -- Error message review and UX polish

- **Spec Reference:** cli-interface.md Sections 4.6, 5, 6
- **Dependencies:** T-044
- **Complexity:** M
- **Crate:** omtsf-cli
- **Acceptance Criteria:**
  - Every error path produces a human-readable message identifying the file, field, and issue
  - Broken pipe handling verified: `omtsf inspect file.omts | head -1` exits 0, no error output
  - `--quiet` verified to suppress all stderr except fatal errors
  - `--verbose` verified to produce timing and count information
  - Exit codes verified against the complete table in cli-interface.md Section 6

### T-046 -- Write comprehensive test fixtures for merge edge cases ✅

- **Spec Reference:** merge.md Section 6.3
- **Dependencies:** T-029, T-009
- **Complexity:** M
- **Crate:** Both (tests/)
- **Acceptance Criteria:**
  - Fixture pairs for all regression scenarios listed in merge.md Section 6.3: disjoint graphs, full overlap, partial overlap with conflicts, transitive chains, same_as at each confidence level, temporal incompatibility, ANNULLED LEI exclusion, oversized merge groups, colliding node IDs
  - Each fixture pair has a corresponding expected-output file or assertion
  - All fixture-based merge tests pass

### T-047 -- Verify WASM compatibility of omtsf-core ✅

- **Spec Reference:** overview.md Section 4; data-model.md Section 11
- **Dependencies:** T-029, T-034, T-036, T-022
- **Complexity:** S
- **Crate:** omtsf-core
- **Acceptance Criteria:**
  - `cargo build --target wasm32-unknown-unknown -p omtsf-core` succeeds
  - No `std::fs`, `std::net`, `std::process` imports in `omtsf-core`
  - `sha2` and `getrandom` compile for wasm32 target with appropriate feature flags
  - Grep confirms no OS-level I/O anywhere in `omtsf-core` source

### T-048 -- Documentation: crate-level docs and public API rustdoc

- **Spec Reference:** overview.md
- **Dependencies:** T-044
- **Complexity:** M
- **Crate:** Both
- **Acceptance Criteria:**
  - `omtsf-core` has a crate-level doc comment explaining purpose, WASM constraint, and module organization
  - Every public type, trait, function, and method has a `///` doc comment
  - `cargo doc --workspace --no-deps` produces clean documentation with no warnings
  - `omtsf-cli/README.md` has build instructions and usage examples for all 10 commands

---

## Phase 10: Serialization Bindings (SPEC-007)

### T-049 -- Implement encoding detection (magic byte inspection)

- **Spec Reference:** serialization-bindings.md (SPEC-007) Section 2
- **Dependencies:** T-008
- **Complexity:** M
- **Crate:** omtsf-core
- **Acceptance Criteria:**
  - `detect_encoding(bytes: &[u8]) -> Result<Encoding, EncodingDetectionError>` inspects initial bytes per SPEC-007 Section 2
  - Detection order: zstd (`0x28 0xB5 0x2F 0xFD`), CBOR tag 55799 (`0xD9 0xD9 0xF7`), JSON (first non-whitespace `{`)
  - `Encoding` enum with `Json`, `Cbor`, `Zstd` variants
  - Whitespace skipping for JSON detection (0x09, 0x0A, 0x0D, 0x20)
  - Unrecognized bytes return `EncodingDetectionError`
  - Unit tests for each encoding type, whitespace-prefixed JSON, and rejection of unknown bytes

### T-050 -- Add CBOR serialization and deserialization for OmtsFile

- **Spec Reference:** serialization-bindings.md (SPEC-007) Sections 4.1--4.6
- **Dependencies:** T-008, T-049
- **Complexity:** L
- **Crate:** omtsf-core
- **Acceptance Criteria:**
  - `ciborium` dependency added for CBOR encoding/decoding
  - `encode_cbor(file: &OmtsFile) -> Result<Vec<u8>, CborError>` serializes with self-describing tag 55799 prepended
  - `decode_cbor(bytes: &[u8]) -> Result<OmtsFile, CborError>` accepts files with or without tag 55799
  - All map keys are text strings (major type 3); no integer keys
  - Dates serialized as text strings, NOT CBOR tags 0/1
  - All string data uses CBOR text strings (major type 3), not byte strings (major type 2)
  - `omtsf_version` key present but position not enforced in CBOR
  - Round-trip test: JSON fixture → parse → encode CBOR → decode CBOR → re-encode JSON → compare with original
  - Unknown field preservation verified through CBOR round-trip

### T-051 -- Add zstd compression and decompression layer

- **Spec Reference:** serialization-bindings.md (SPEC-007) Section 6
- **Dependencies:** T-049
- **Complexity:** M
- **Crate:** omtsf-core
- **Acceptance Criteria:**
  - `zstd` crate dependency added
  - `compress_zstd(data: &[u8]) -> Result<Vec<u8>, CompressionError>` compresses with default level
  - `decompress_zstd(data: &[u8], max_size: usize) -> Result<Vec<u8>, CompressionError>` decompresses with size limit to prevent decompression bombs
  - After decompression, encoding re-detection applied per SPEC-007 Section 2
  - Round-trip test: serialize JSON → compress → decompress → detect encoding → parse → compare
  - Round-trip test: serialize CBOR → compress → decompress → detect encoding → parse → compare

### T-052 -- Implement unified parse pipeline (auto-detect and decode)

- **Spec Reference:** serialization-bindings.md (SPEC-007) Sections 2, 3, 4, 6
- **Dependencies:** T-049, T-050, T-051
- **Complexity:** M
- **Crate:** omtsf-core
- **Acceptance Criteria:**
  - `parse_omts(bytes: &[u8], max_decompressed: usize) -> Result<(OmtsFile, Encoding), ParseError>` auto-detects encoding, decompresses if needed, parses JSON or CBOR
  - Returns both the parsed file and the detected encoding (for informational purposes)
  - Existing JSON-only `parse` paths updated to delegate to `parse_omts`
  - All existing tests continue to pass
  - New tests: CBOR input, zstd+JSON input, zstd+CBOR input

### T-053 -- Implement lossless JSON↔CBOR conversion

- **Spec Reference:** serialization-bindings.md (SPEC-007) Section 5
- **Dependencies:** T-050
- **Complexity:** M
- **Crate:** omtsf-core
- **Acceptance Criteria:**
  - `convert(file: &OmtsFile, target: Encoding, compress: bool) -> Result<Vec<u8>, ConvertError>` produces output in the requested encoding
  - Field names preserved (including unknown fields)
  - Null vs. absent distinction preserved
  - Array element order preserved for `nodes`, `edges`, `identifiers`, `labels`
  - Test: JSON → CBOR → JSON round-trip produces logically equivalent output
  - Test: CBOR → JSON → CBOR round-trip produces logically equivalent output
  - Logical equivalence defined as identical abstract model after parsing

### T-054 -- Update CLI file I/O to support multi-encoding input

- **Spec Reference:** cli-interface.md Sections 4.1--4.6; serialization-bindings.md (SPEC-007)
- **Dependencies:** T-038, T-052
- **Complexity:** M
- **Crate:** omtsf-cli
- **Acceptance Criteria:**
  - `read_file` pipeline updated to call `parse_omts` instead of JSON-only parsing
  - All commands that read `.omts` files transparently accept JSON, CBOR, and zstd-compressed inputs
  - `--verbose` mode reports detected encoding to stderr
  - Decompression bomb guard enforced (`4 * max_file_size` for zstd inputs)
  - Integration tests: validate/inspect/diff commands work with CBOR and zstd inputs

### T-055 -- Wire convert command with --to and --compress flags

- **Spec Reference:** cli-interface.md Section 3.6; serialization-bindings.md (SPEC-007) Section 7
- **Dependencies:** T-040, T-053, T-054
- **Complexity:** M
- **Crate:** omtsf-cli
- **Acceptance Criteria:**
  - `convert` command accepts `--to <json|cbor>` flag (default: json)
  - `convert` command accepts `--compress` flag to wrap output in zstd
  - `--pretty` and `--compact` flags apply to JSON output only
  - CBOR output ignores `--pretty`/`--compact`
  - Exit code 0 on success, 2 on parse failure
  - Integration tests: JSON→CBOR, CBOR→JSON, JSON→zstd+JSON, CBOR→zstd+CBOR

### T-056 -- Add CBOR and compression test fixtures

- **Spec Reference:** serialization-bindings.md (SPEC-007)
- **Dependencies:** T-009, T-050, T-051
- **Complexity:** S
- **Crate:** Both (tests/)
- **Acceptance Criteria:**
  - At least 3 CBOR fixture files generated from existing JSON fixtures via the convert pipeline
  - At least 2 zstd-compressed fixtures (one JSON, one CBOR)
  - All CBOR fixtures decode to logically equivalent abstract models as their JSON counterparts
  - Fixtures committed under `omtsf-rs/tests/fixtures/` alongside existing JSON fixtures

### T-057 -- Verify WASM compatibility of CBOR and compression dependencies

- **Spec Reference:** serialization-bindings.md (SPEC-007) Section 7; overview.md Section 4
- **Dependencies:** T-050, T-051, T-047
- **Complexity:** S
- **Crate:** omtsf-core
- **Acceptance Criteria:**
  - `ciborium` compiles for `wasm32-unknown-unknown` target
  - Determine if `zstd` compiles for WASM; if not, gate compression behind a cargo feature flag (`compression`) that is disabled for WASM builds
  - `cargo build --target wasm32-unknown-unknown -p omtsf-core` succeeds (with or without compression feature)
  - Document any WASM limitations in `omtsf-rs/docs/data-model.md` Section 11

---

## Phase 11: Performance Optimization

Findings from a parallel performance review of the codebase by two independent
senior Rust engineers. Tasks are ordered by estimated impact.

### T-059 -- Eliminate exponential cloning in `all_paths` query

- **Spec Reference:** graph-engine.md Section 4
- **Dependencies:** T-020
- **Complexity:** M
- **Crate:** omtsf-core
- **Location:** `graph/queries/mod.rs:326-381`
- **Issue:** Each IDDFS stack frame clones both `path: Vec<NodeIndex>` and `on_path: HashSet<NodeIndex>` for every neighbour explored. The `seen_paths: HashSet<Vec<NodeIndex>>` also hashes full path vectors. At M tier / depth 10, this costs 193 ms — the single slowest benchmark.
- **Acceptance Criteria:**
  - Replace per-frame clone with a single mutable path vector using push/pop backtracking
  - Replace `on_path` HashSet clones with a `Vec<bool>` or bitset indexed by `NodeIndex`, toggled on push/pop
  - `seen_paths` deduplication replaced with rolling hash or sorted insertion check
  - `all_paths` M/depth_10 benchmark improves by at least 10x
  - All existing `all_paths` tests continue to pass with identical results

### T-060 -- Fix O(N*E) node lookup in diff edge matching

- **Spec Reference:** diff.md Section 2.2
- **Dependencies:** T-035
- **Complexity:** S
- **Crate:** omtsf-core
- **Location:** `diff/matching.rs:219-233`
- **Issue:** `node_type_allowed_for_id` closure does a linear search through ALL `nodes_a` and `nodes_b` to find a node by its ID string. Called for every edge endpoint, this is O(N) per call, O(N*E) total.
- **Acceptance Criteria:**
  - Pre-build `HashMap<&str, &Node>` for both `nodes_a` and `nodes_b` before edge matching begins
  - `node_type_allowed_for_id` uses O(1) HashMap lookup instead of linear scan
  - Diff L/XL benchmarks show measurable improvement
  - All existing diff tests continue to pass

### T-061 -- Replace Vec.contains() with HashSet in diff node matching

- **Spec Reference:** diff.md Section 2.1
- **Dependencies:** T-035
- **Complexity:** S
- **Crate:** omtsf-core
- **Location:** `diff/matching.rs:54`
- **Issue:** `active_a` and `active_b` are `Vec<usize>` and containment is checked with `.contains()` which is O(N) linear scan per call, inside a nested loop over all identifier buckets.
- **Acceptance Criteria:**
  - Replace `active_a` and `active_b` with `HashSet<usize>` or `Vec<bool>` for O(1) lookup
  - All existing diff tests continue to pass

### T-062 -- Eliminate double String allocation in newtype deserialization

- **Spec Reference:** data-model.md Section 3
- **Dependencies:** T-003
- **Complexity:** S
- **Crate:** omtsf-core
- **Location:** `newtypes.rs:171-174` (SemVer), `:221-225` (CalendarDate), `:270-274` (FileSalt), `:320-324` (NodeId), `:374-378` (CountryCode)
- **Issue:** Every newtype's `Deserialize` impl does `String::deserialize(d)?` then calls `Self::try_from(s.as_str())`, which internally calls `s.to_owned()` — allocating a second copy. The original String is dropped. At Huge tier, NodeId alone accounts for ~7-10M unnecessary String allocations.
- **Acceptance Criteria:**
  - Add `TryFrom<String>` impl to each newtype that moves the String instead of cloning
  - Update `Deserialize` impls to use `TryFrom<String>` for the owned-String path
  - Huge-tier CBOR decode benchmark shows measurable improvement (~5-8%)
  - All existing tests continue to pass

### T-063 -- Visitor-based deserialization for NodeTypeTag/EdgeTypeTag

- **Spec Reference:** data-model.md Section 4.1
- **Dependencies:** T-004
- **Complexity:** M
- **Crate:** omtsf-core
- **Location:** `enums.rs:63-71` (NodeTypeTag), `:133-142` (EdgeTypeTag)
- **Issue:** Deserializes into a full `String`, then creates a `StrDeserializer` to try `NodeType::deserialize()`. For known types (the vast majority), the String is allocated and immediately dropped. ~2.2M unnecessary String allocations at Huge tier.
- **Acceptance Criteria:**
  - Implement custom `Visitor` with `visit_str` that matches known variants by `&str` without allocation
  - `visit_string` captures the owned String only for `Extension` variants
  - All existing enum serde tests continue to pass
  - Decode benchmarks show ~2-4% improvement at L+ tiers

### T-064 -- Fix O(N*E) edge scan in L3-MRG-01 validation rule

- **Spec Reference:** validation.md Section 4.3
- **Dependencies:** T-017
- **Complexity:** S
- **Crate:** omtsf-core
- **Location:** `validation/rules_l3.rs:105-171`
- **Issue:** For each organization node, the rule scans ALL edges to find ownership edges targeting that node. With N org nodes and E edges, this is O(N*E). Same pattern as the L2 O(E*N) bug that was already fixed.
- **Acceptance Criteria:**
  - Pre-build `HashMap<&str, Vec<&Edge>>` keyed by target node ID, filtered to ownership edges
  - Each org node lookup is O(1) amortized; total becomes O(N+E)
  - Validation benchmarks at L+ tiers show measurable improvement
  - All existing validation tests continue to pass

### T-065 -- Eliminate String allocations in build_graph

- **Spec Reference:** graph-engine.md Section 2
- **Dependencies:** T-018
- **Complexity:** M
- **Crate:** omtsf-core
- **Location:** `graph/mod.rs:230-267`
- **Issue:** Two sources of unnecessary allocation: (1) `edge.source.to_string()` and `edge.target.to_string()` allocate new Strings solely for HashMap lookups (~3M allocations at Huge tier); (2) `node.id.to_string()` creates a String, then `local_id.clone()` creates a second copy for NodeWeight (~1.5M extra allocations).
- **Acceptance Criteria:**
  - Use `&str` borrows from node/edge IDs for HashMap lookups (via `HashMap::get<Q>` where `String: Borrow<str>`)
  - Eliminate or reduce the double String allocation for node IDs in NodeWeight (e.g., store data_index and look up ID via file reference, or use single owned String)
  - Huge-tier graph construction benchmark shows ~5-10% improvement
  - All existing graph tests continue to pass

### T-066 -- Replace serde_json tag_to_string with direct enum-to-str

- **Spec Reference:** diff.md Section 2
- **Dependencies:** T-035
- **Complexity:** S
- **Crate:** omtsf-core
- **Location:** `diff/helpers.rs:8-14`
- **Issue:** Uses `serde_json::to_value()` to convert an enum tag to a string — allocates a full `serde_json::Value` just to extract a `String`. Called very frequently throughout diff and edge matching.
- **Acceptance Criteria:**
  - Implement `AsRef<str>` or a method returning `&str` / `Cow<str>` on `NodeTypeTag` and `EdgeTypeTag`
  - `tag_to_string` replaced with zero-allocation string access for known variants
  - Extension variants return the inner String by reference
  - All existing diff tests continue to pass

### T-067 -- Return iterator from neighbours() instead of Vec allocation

- **Spec Reference:** graph-engine.md Section 3
- **Dependencies:** T-018
- **Complexity:** M
- **Crate:** omtsf-core
- **Location:** `graph/queries/mod.rs:86-125`
- **Issue:** `neighbours()` allocates a new `Vec<NodeIndex>` on every call during BFS traversal (reachability, shortest path, ego graph, etc.).
- **Acceptance Criteria:**
  - `neighbours()` returns an iterator (or accepts a `&mut Vec<NodeIndex>` buffer) instead of allocating
  - All BFS/DFS callers updated to use the iterator or reusable buffer pattern
  - Graph query benchmarks show measurable improvement at L+ tiers
  - All existing graph query tests continue to pass

### T-068 -- Reuse identifier index in merge pipeline

- **Spec Reference:** merge.md Section 8
- **Dependencies:** T-029
- **Complexity:** S
- **Crate:** omtsf-core
- **Location:** `merge_pipeline/pipeline.rs:444-463`
- **Issue:** `node_rep_to_canonical` iterates ALL nodes and recomputes `CanonicalId` for every identifier. The identifier index with canonical IDs was already built earlier in the pipeline (lines 65-83). This is redundant work.
- **Acceptance Criteria:**
  - Reuse the identifier index already computed at the start of the pipeline
  - Eliminate the redundant `CanonicalId` computation pass
  - All existing merge tests continue to pass (including proptest algebraic properties)

### T-069 -- Pre-compute lowercased selector patterns

- **Spec Reference:** graph-engine.md Section 5
- **Dependencies:** T-021
- **Complexity:** S
- **Crate:** omtsf-core
- **Location:** `graph/selectors/mod.rs:228-237`
- **Issue:** `matches_node` calls `name.to_lowercase()` and `pat.to_lowercase()` on every selector evaluation. For `selector_subgraph` iterating all nodes, this allocates new strings per node per pattern.
- **Acceptance Criteria:**
  - Pre-compute lowercased patterns at `SelectorSet` construction time
  - Compute `to_lowercase()` once per node evaluation, not per pattern
  - Selector match benchmarks show measurable improvement
  - All existing selector tests continue to pass

---

## Phase 12: Quality Review Findings

Findings from a parallel quality review by independent senior Rust engineer
and senior QA engineer assessments. Tasks ordered by priority.

### T-070 -- Implement Default for Node struct

- **Source:** Both reviewers (Rust engineer M-1, QA engineer m-1)
- **Dependencies:** T-005
- **Complexity:** M
- **Crate:** omtsf-core
- **Location:** `structures/node.rs:22-173`
- **Issue:** The flat Node struct (32+ fields) causes massive boilerplate in both production code and tests. Test constructors require 30+ lines of `None` fields, duplicated across 10+ test files (~800+ lines). The merge pipeline (`pipeline.rs:347-372`) and redaction (`redaction/mod.rs:597-645`) copy fields individually, fragile to schema evolution.
- **Acceptance Criteria:**
  - `Default` impl for `Node` with a sentinel `id` and `node_type`
  - All test constructors simplified to use struct update syntax (`..Default::default()`)
  - Merge pipeline field-copying simplified where possible
  - All existing tests continue to pass

### T-071 -- Add unit tests for union_find, check_digits, newtypes, canonical, encoding

- **Source:** QA engineer M-1
- **Dependencies:** T-023, T-012, T-003, T-024, T-049
- **Complexity:** L
- **Crate:** omtsf-core
- **Issue:** Several core algorithmic modules have zero direct unit tests (only indirect coverage through higher-level tests). A bug in check-digit arithmetic, union-find, or encoding detection would not necessarily be caught.
- **Acceptance Criteria:**
  - `union_find.rs`: tests for singleton find, two-element union+find, transitive union chain, path compression, rank balancing, idempotent union, large component merge
  - `check_digits.rs`: tests for valid/invalid LEI MOD 97-10, valid/invalid GLN GS1 mod-10, boundary values (all zeros, all nines), non-digit characters, wrong-length inputs
  - `newtypes.rs`: boundary tests for TryFrom impls — empty strings, too-long strings, special characters, exact boundary lengths (FileSalt 64 hex chars), invalid dates (Feb 30, month 13), lowercase country codes
  - `canonical.rs`: unit tests for canonical ID construction per scheme
  - `encoding.rs`: unit tests for encoding detection heuristic (JSON, CBOR, zstd, unknown bytes, whitespace-prefixed JSON)

### T-072 -- Add cmd_query.rs CLI integration tests

- **Source:** QA engineer M-2
- **Dependencies:** T-040
- **Complexity:** M
- **Crate:** omtsf-cli
- **Issue:** The `query` command is the only CLI subcommand without integration tests. All other commands have dedicated `cmd_*.rs` integration test files.
- **Acceptance Criteria:**
  - `--node-type` filtering produces correct output
  - `--label` key and key=value matching
  - `--identifier` scheme and scheme:value matching
  - `--jurisdiction` filtering
  - `--name` substring matching
  - `--count` output format (human and JSON modes)
  - `--format json` output structure
  - Empty result → exit code 1
  - Nonexistent file → exit code 2
  - No selectors → exit code 2
  - Mixed selectors with AND/OR composition

### T-073 -- Add L3 validation rule tests ✅

- **Source:** QA engineer M-3
- **Dependencies:** T-017
- **Complexity:** M
- **Crate:** omtsf-core
- **Issue:** L3 validation rules exist in production code (`rules_l3.rs`) but have zero tests. L1 has ~2000 lines of tests, L2 has ~700 lines, L3 has none.
- **Acceptance Criteria:**
  - Tests following the same pattern as `rules_l1_gdm/tests.rs` and `rules_l2/tests.rs`
  - L3-MRG-01 (ownership percentage sum) tested with valid and invalid inputs
  - L3-MRG-02 (legal_parentage cycle detection) tested with acyclic and cyclic graphs
  - L3-EID rules tested with mock `ExternalDataSource`
  - Tests confirm correct `RuleId`, `Severity::Info`, and `Location`

### T-074 -- Fix hardcoded merge timestamp placeholder

- **Source:** Rust engineer M-5
- **Dependencies:** T-029
- **Complexity:** S
- **Crate:** omtsf-core
- **Location:** `merge_pipeline/pipeline.rs:690`
- **Issue:** Merge metadata always emits `"2026-02-20T00:00:00Z"` instead of actual merge time. This is a placeholder left from development.
- **Acceptance Criteria:**
  - Timestamp reflects actual wall clock time at merge execution
  - Since omtsf-core denies stdout/stderr but system time is fine, either get time in core or pass timestamp from CLI layer
  - Existing merge tests updated to not assert on exact timestamp value
  - proptest `stable_hash` already zeros timestamp, so algebraic tests unaffected

### T-075 -- Extract shared test helpers to reduce duplication

- **Source:** Both reviewers (Rust engineer n-2, QA engineer m-1)
- **Dependencies:** T-070
- **Complexity:** M
- **Crate:** omtsf-core
- **Issue:** Helper functions (`org_node()`, `facility_node()`, `supplies_edge()`, `minimal_file()`, `node_id()`, `edge_id()`) are copy-pasted across 10+ test files (~800+ lines total).
- **Acceptance Criteria:**
  - Shared `#[cfg(test)]` test_helpers module in omtsf-core (or dev-dependency helper crate)
  - All test files import shared constructors instead of defining their own
  - Net reduction of ~500+ lines of duplicated test code
  - All existing tests continue to pass
  - Blocked by T-070 (`Default` for Node makes helpers much simpler)

### T-076 -- Expand shared fixture files and L1 validation test coverage ✅

- **Source:** Test coverage gap analysis
- **Dependencies:** T-009, T-013, T-014, T-015
- **Complexity:** M
- **Crate:** omtsf-core (tests)
- **Issue:** The repo root `tests/fixtures/valid/` had only 2 fixtures (`minimal.omts`, `full-featured.omts`). 8 of 16 edge types had no valid fixture file. 9 of 19 invalid fixtures were undocumented in schema conformance tests. No L1 validation integration test existed against fixture files.
- **Acceptance Criteria:**
  - 6 new valid fixture files: `all-edge-types.omts` (all 16 edge types), `delta.omts` (delta mode), `identifiers-full.omts` (all identifier schemes + verification + temporal fields), `geo-polygon.omts` (GeoJSON Polygon), `labels-and-quality.omts` (labels + all confidence levels), `nullable-dates.omts` (`valid_to: null`)
  - `existing_valid_fixtures_pass_schema` auto-discovers all `valid/*.omts` files
  - `existing_invalid_fixtures_documented` covers all 19 invalid fixtures (7 schema-rejects, 12 schema-accepts)
  - `existing_valid_fixtures_pass_l1_validation` runs L1-only validation on all valid fixtures
  - `existing_invalid_fixtures_trigger_expected_rules` asserts each parseable invalid fixture triggers its expected L1 `RuleId`
  - Parse + round-trip tests in `parse_fixtures.rs` for all 6 new fixtures via `repo_fixtures_dir()` helper
  - `just pre-commit` passes; no file exceeds 800 lines

---

## Phase 13: Excel Import

### T-077 -- Implement `import-excel` command

- **Spec Reference:** Expert panel report (`docs/reviews/excel-import-format-panel-report.md`), SPEC-001 through SPEC-005
- **Dependencies:** T-008, T-011, T-013, T-014, T-015, T-040
- **Complexity:** XL
- **Crate:** omtsf-cli (Excel I/O), omtsf-core (tabular-to-graph conversion logic)
- **Description:** Read a multi-sheet `.xlsx` workbook in the OMTS Excel import format (see `tests/fixtures/excel/`) and convert it to a valid `.omts` file. The template has 12 sheets: README, Metadata, Organizations, Facilities, Goods, Persons, Attestations, Consignments, Supply Relationships, Corporate Structure, Same As, and Identifiers.
- **Acceptance Criteria:**
  - `omtsf import-excel <input.xlsx> [-o output.omts]` reads an Excel file and writes a valid `.omts` file
  - Uses `calamine` crate for `.xlsx` reading; dependency confined to `omtsf-cli` (not in `omtsf-core`)
  - **Metadata sheet:** reads `snapshot_date` (required), `reporting_entity`, `disclosure_scope`, data quality defaults
  - **Node sheets** (Organizations, Facilities, Goods, Persons, Attestations, Consignments): each row becomes a node with the appropriate `type`; common identifier columns on Organizations sheet (lei, duns, nat_reg, vat, internal) converted to identifier records
  - **Edge sheets** (Supply Relationships, Corporate Structure): each row becomes an edge; domain-friendly column names (supplier_id/buyer_id, subsidiary_id/parent_id) mapped to source/target per edge type direction convention
  - **Same As sheet:** each row becomes a `same_as` edge with confidence and basis properties
  - **Identifiers sheet:** rows merged with inline identifier columns from node sheets; deduplicated per L1-EID-11
  - **Attestations sheet:** `attested_entity_id` and `scope` columns generate `attested_by` edges automatically
  - **Auto-generated fields:** `file_salt` via CSPRNG (always fresh), `omtsf_version` set to current, node/edge IDs generated as stable slugs when blank (e.g., `org-bolt-supplies-ltd`)
  - **Two-pass parse:** (1) collect all nodes into ID map, (2) resolve edge source/target references
  - **Validation:** structural errors (missing required columns, unresolved references) fail fast with `{Sheet}!{Column}{Row}` cell references; L1 validation (check digits, referential integrity, type constraints) runs on the constructed graph before writing output; L2 warnings emitted but do not block output
  - **Sensitivity defaults:** applied per SPEC-004 Section 2 (lei/duns/gln=public, nat-reg/vat/internal=restricted, person identifiers=confidential); `disclosure_scope` validated against identifier sensitivity (L1-SDI-02)
  - **Person node + public scope:** import refuses to produce output, emits diagnostic referencing SPEC-004 Section 5
  - **Header row validation:** column headers validated against expected manifest before reading data rows; unrecognized headers produce warnings
  - Output passes `omtsf validate` at L1 with zero errors
  - Integration test: `omtsf import-excel tests/fixtures/excel/omts-import-example.xlsx | omtsf validate -` exits 0
  - Integration test: round-trip — import example Excel, validate output, inspect node/edge counts match expected

---

## Notes: Spec Ambiguities Discovered During Planning

1. **`control_type` disambiguation (data-model.md Section 4.5).** The spec reuses the JSON key `"control_type"` across two edge types with disjoint variant sets (`operational_control` vs `beneficial_ownership`). The data model stores this as `serde_json::Value`. Implementors should verify that the validation engine enforces the correct variant set per edge type, and that the diff engine compares `control_type` values structurally without assuming a single enum.

2. **`status` field routing on Node (data-model.md Section 5.3).** The custom deserializer that routes the JSON `"status"` field to either `OrganizationStatus` or `AttestationStatus` based on the node `type` tag is complex and error-prone. If a file contains an extension node type that also uses `"status"`, the routing logic must fall through gracefully (likely storing the value in `extra`). The spec does not address this case.

3. **Merge `--strategy intersect` (cli-interface.md Section 3.2).** The CLI spec mentions an `intersect` merge strategy but the merge engine spec (merge.md) does not define intersect semantics. This strategy may need to be deferred or specced separately. Task T-029 implements `union` only; `intersect` should be tracked as a follow-up.

4. **`same_as` confidence default (merge.md Section 7.1).** The spec states the default for absent confidence is `"probable"` (SPEC-003 S7.1) but the implementation spec says absent confidence is treated as `"possible"` (weakest). Implementors should confirm which default is authoritative.

5. **Edge property stripping and required properties (redaction.md Section 6.5).** The spec states "If stripping removes a property that would normally be required, the edge remains valid." This implies the validation engine must relax required-property checks on redacted files, or that no edge property is truly required at L1. This relaxation is not explicitly stated in validation.md.

6. **`omtsf diff` header comparison (diff.md Section 7).** The diff engine compares graph elements only, not header fields. The CLI "MAY report header differences as a separate informational section." The implementation should decide whether to implement header diff or defer it. Task T-036 does not include header comparison.

7. **`reporting_entity` on merge output (merge.md Section 4.3).** When source files have conflicting `reporting_entity` values, the merged header omits it and records all values in `merge_metadata`. But SPEC-001 does not define `merge_metadata` as a recognized field -- it will end up in `extra`. This is presumably intentional (future-compatible), but should be documented.

8. **Redact node selection mechanism (cli-interface.md Section 3.3).** The CLI defines `--scope` but does not describe how the user specifies which non-person nodes to retain vs. replace (e.g., `--retain <node-id>...`, a config file, or retaining all nodes by default). This flag set needs further specification before T-034 and T-041 can be fully implemented.

9. **L2 rule L2-EID-03 (validation.md Section 4.2).** The rule references "valid GLEIF RA codes per snapshot" but no list of valid RA codes is provided in the spec or in the implementation documents. The implementor must source this data or stub the rule.
