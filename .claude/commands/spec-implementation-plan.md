# Spec Implementation Plan

You are the **Implementation Architect** orchestrating a 3-phase pipeline that translates OMTSF specification requirements into implementation specs and ordered tasks for the Rust reference implementation. Your job is to dispatch extraction agents, feed their output to spec-writing agents, and produce a final task plan.

## Scope

$ARGUMENTS

## Instructions

### Pre-Read

Before dispatching any agents, read ALL of the following files and keep their contents available for inclusion in agent prompts:

**Spec files:**
- `spec/graph-data-model.md` (SPEC-001)
- `spec/entity-identification.md` (SPEC-002)
- `spec/merge-semantics.md` (SPEC-003)
- `spec/selective-disclosure.md` (SPEC-004)
- `spec/erp-integration.md` (SPEC-005, informative)
- `spec/standards-mapping.md` (SPEC-006, informative)

**Implementation context:**
- `omtsf-rs/docs/overview.md`

**Persona files (for Phase 2):**
- `.claude/commands/personas/rust-engineer.md`
- `.claude/commands/personas/graph-theorist.md`
- `.claude/commands/personas/security-privacy-expert.md`

### Scoped Execution

If `$ARGUMENTS` specifies a focused scope, run only the relevant subset of agents:

| Scope keyword | Phase 1 agents | Phase 2 agents | Phase 3 |
|---------------|---------------|---------------|---------|
| `model` | 1A | 2A | skip |
| `validation` | 1A | 2B | skip |
| `merge` | 1A, 1B | 2C | skip |
| `redaction` | 1A, 1C | 2D | skip |
| `graph` | 1A, 1B | 2E | skip |
| `diff` | 1A, 1B | 2F | skip |
| `cli` | 1A | 2G | skip |
| `tasks` | skip | skip | Phase 3 only (reads existing spec files from disk) |
| `all` or empty | all | all | yes |

When a scope keyword is provided, include only the relevant spec files in agent prompts rather than all of them.

---

## Phase 1: Spec Requirements Extraction

Launch the following agents in parallel using the **Task** tool with `subagent_type: "general-purpose"`. Each agent is a neutral extractor — no persona, just structured requirement extraction from the specification documents.

### Agent 1A — Data Model & Identifiers

```
You are a specification analyst. Your task is to extract all implementable requirements from the OMTSF data model and entity identification specifications.

Read the following specs carefully:

**SPEC-001 (Graph Data Model):**
{contents of spec/graph-data-model.md}

**SPEC-002 (Entity Identification):**
{contents of spec/entity-identification.md}

## What You Must Produce

A structured requirements extraction document with these sections:

### Type Definitions
Every node type, edge type, header field, identifier type, and label type defined in the specs. For each, list:
- Field name, type, required/optional status
- Validation constraints (format, length, allowed values, regex patterns)
- Default values if any

### Identifier Rules
- All identifier formats (syntax, character sets, length constraints)
- Check digit algorithms with step-by-step procedures and worked examples from the spec
- Identifier resolution and cross-referencing rules
- Identifier equivalence rules (when two identifiers refer to the same entity)

### Structural Constraints
- Graph-level invariants (e.g., edge endpoints must reference existing nodes)
- Header requirements (required fields, format constraints)
- Ordering constraints (if any)
- Uniqueness constraints (node IDs, edge IDs)

### Serialization Conventions
- JSON field naming conventions
- Null vs. absent field semantics
- Array ordering semantics (significant or insignificant)
- String encoding requirements (UTF-8, normalization)

### Conformance Requirements
- All MUST/SHOULD/MAY requirements, enumerated with section references
- Validation levels (L1/L2/L3) and which rules belong to each level

## Guidelines
- Be exhaustive. Every requirement that would affect an implementation must be captured.
- Use exact section references (e.g., "SPEC-001 §4.2") for traceability.
- Preserve test vectors, examples, and worked calculations exactly as given in the spec.
- Do not interpret or editorialize. Extract what the spec says, not what you think it should say.
- Output should be 1500-3000 words depending on spec density.
```

### Agent 1B — Merge & Graph Algorithms

```
You are a specification analyst. Your task is to extract all implementable requirements related to merge semantics and graph algorithms from the OMTSF specifications.

Read the following specs carefully:

**SPEC-003 (Merge Semantics):**
{contents of spec/merge-semantics.md}

**SPEC-001 (Graph Data Model) — relevant sections on graph structure:**
{contents of spec/graph-data-model.md}

## What You Must Produce

A structured requirements extraction document with these sections:

### Identity Predicates
- How two nodes are determined to represent the same entity
- All matching criteria (identifier match, attribute match, composite match)
- Priority or ordering of match criteria
- False positive / false negative handling guidance

### Merge Procedure
- Step-by-step merge algorithm as defined in the spec
- Input preconditions and output postconditions
- Property conflict resolution rules (which value wins, how ties break)
- Edge merging rules (what happens to edges when nodes merge)

### same_as Handling
- How `same_as` relationships are created, consumed, and propagated during merge
- Transitivity rules
- Interaction with identity predicates

### Provenance
- How merge provenance is recorded
- Source tracking through merges
- Auditability requirements

### Algebraic Properties
- Commutativity: merge(A, B) = merge(B, A)?
- Associativity: merge(merge(A, B), C) = merge(A, merge(B, C))?
- Idempotency: merge(A, A) = A?
- Any stated or implied properties with their conditions and exceptions

### Graph Invariants
- Properties that must hold before and after merge (e.g., no orphan edges, ID uniqueness)
- Cycle handling during merge
- Connected component behavior

## Guidelines
- Be exhaustive. Every requirement that would affect an implementation must be captured.
- Use exact section references for traceability.
- Preserve any pseudocode, algorithms, or worked examples exactly as given.
- Do not interpret or editorialize.
- Output should be 1500-3000 words depending on spec density.
```

### Agent 1C — Selective Disclosure & Privacy

```
You are a specification analyst. Your task is to extract all implementable requirements related to selective disclosure and privacy from the OMTSF specifications.

Read the following specs carefully:

**SPEC-004 (Selective Disclosure):**
{contents of spec/selective-disclosure.md}

**SPEC-002 (Entity Identification) — relevant sections on identifier sensitivity:**
{contents of spec/entity-identification.md}

## What You Must Produce

A structured requirements extraction document with these sections:

### Sensitivity Model
- Data sensitivity classification scheme
- Which fields/nodes/edges are sensitive and why
- Sensitivity levels and their definitions
- Default sensitivity assignments

### Disclosure Scopes
- All defined disclosure scopes (e.g., full, redacted, summary)
- What is included/excluded at each scope level
- Scope composition rules (if any)
- Scope validation requirements

### Boundary Reference Hash Algorithm
- The exact hash algorithm used for boundary references
- Input preparation (what data is hashed, in what order, with what encoding)
- Hash function specification (algorithm, output format, truncation)
- All test vectors provided in the spec — preserve these exactly
- Salt handling (file_salt usage, uniqueness requirements)

### Person Node Privacy Rules
- Special handling for person-type nodes
- Redaction requirements for personal data
- Identifier anonymization rules
- What constitutes personal data in the OMTSF context

### Edge Handling During Redaction
- What happens to edges when a node is redacted
- Boundary edge creation rules
- Edge attribute redaction rules
- Graph connectivity preservation requirements

### Conformance Requirements
- All MUST/SHOULD/MAY requirements for selective disclosure
- Validation rules specific to redacted files

## Guidelines
- Be exhaustive. Every requirement that would affect an implementation must be captured.
- Use exact section references for traceability.
- Preserve all test vectors, hash examples, and worked calculations exactly.
- Do not interpret or editorialize.
- Output should be 1500-3000 words depending on spec density.
```

### Agent 1D — Standards & Integration

```
You are a specification analyst. Your task is to extract integration-relevant patterns and reference data from the OMTSF informative specifications. This extraction informs test fixtures and graph queries but does not directly produce code requirements.

Read the following specs carefully:

**SPEC-005 (ERP Integration):**
{contents of spec/erp-integration.md}

**SPEC-006 (Standards Mapping):**
{contents of spec/standards-mapping.md}

## What You Must Produce

A structured extraction document with these sections:

### ERP Integration Patterns
- Import/export workflows described in the spec
- Field mapping tables (OMTSF fields ↔ ERP fields)
- Data transformation rules for common ERP systems
- Batch processing patterns

### Label Vocabulary
- All label types defined or referenced
- Label namespacing conventions
- Standard label sets from referenced standards (GS1, ISO, etc.)
- Label validation rules (if any)

### Regulatory Query Patterns
- Graph queries that support regulatory compliance (CSDDD, EUDR, UFLPA, etc.)
- Traceability query patterns (origin-to-destination paths)
- Due diligence query patterns
- Audit trail queries

### Test Fixture Implications
- Realistic graph structures implied by the integration patterns
- Edge cases highlighted in the specs (missing data, partial graphs, cross-system inconsistencies)
- Example scenarios that should become test fixtures

## Guidelines
- This is informative extraction. Flag what is normative vs. informative.
- Focus on what an implementor needs to build realistic test data and example queries.
- Use exact section references for traceability.
- Output should be 800-1500 words.
```

---

## Phase 2: Implementation Spec Writing

After ALL Phase 1 agents complete, collect their outputs. Then launch the following agents in parallel using the **Task** tool with `subagent_type: "general-purpose"`. Each agent receives:

1. The relevant Phase 1 extraction output(s)
2. The contents of `omtsf-rs/docs/overview.md`
3. A persona definition (primarily Rust Engineer, with Graph Theorist or Security & Privacy Expert where noted)
4. Instructions to write a spec file to a specific path

Each agent **writes its output file directly** to `omtsf-rs/docs/` using the Write tool. Do not rewrite agent output. Agents must write ONLY their assigned markdown spec file — no Rust source code, no Cargo files, no files outside `omtsf-rs/docs/`.

### Agent 2A → `omtsf-rs/docs/data-model.md`

Receives: Agent 1A output, overview.md, Rust Engineer persona.

```
{Rust Engineer persona definition}

## Your Assignment

Using the requirements extraction below and the implementation overview, write the technical specification for the OMTSF Rust data model.

**Requirements Extraction (from spec analysis):**
{Agent 1A output}

**Implementation Overview:**
{contents of omtsf-rs/docs/overview.md}

## What You Must Produce

Write a technical specification document and save it to `omtsf-rs/docs/data-model.md`. The document must cover:

### Rust Type Definitions
- Complete type hierarchy: `OmtsFile`, `Header`, `Node`, `Edge`, `Identifier`, `Label`, etc.
- Enum variants for node types, edge types, identifier types
- Use of `Option<T>` for optional fields vs. required fields
- Newtype wrappers for validated strings (identifiers, dates, etc.)

### Serde Strategy
- `#[serde(rename_all)]` conventions
- Custom deserializers where JSON representation differs from Rust types
- `#[serde(deny_unknown_fields)]` policy
- Handling of null vs. absent fields
- Round-trip fidelity guarantees

### Node and Edge Modeling Decisions
- How nodes and edges reference each other (by ID string, by index, by handle)
- Owned vs. borrowed data in the type model
- Whether the flat JSON structure maps 1:1 to Rust types or uses an intermediate representation

### WASM Compatibility Notes
- Types that must be `Send + Sync` (or not, for WASM)
- Avoiding `std::fs` dependencies
- `wasm-bindgen`-friendly type surface

## Guidelines
- Write as a senior Rust engineer making design decisions.
- Justify choices with trade-off analysis.
- Include Rust code blocks for key type definitions.
- Reference specific spec requirements by section number.
- Target 1500-2500 words.
```

### Agent 2B → `omtsf-rs/docs/validation.md`

Receives: Agent 1A output, overview.md, Rust Engineer persona.

```
{Rust Engineer persona definition}

## Your Assignment

Using the requirements extraction below and the implementation overview, write the technical specification for the OMTSF validation engine.

**Requirements Extraction (from spec analysis):**
{Agent 1A output}

**Implementation Overview:**
{contents of omtsf-rs/docs/overview.md}

## What You Must Produce

Write a technical specification document and save it to `omtsf-rs/docs/validation.md`. The document must cover:

### Rule Registry Architecture
- How validation rules are registered, organized, and dispatched
- Rule metadata: ID, severity, level (L1/L2/L3), human-readable message
- Extensibility: how new rules are added

### Validation Levels
- **L1 (Structural):** JSON schema-level checks — required fields, types, valid enum values
- **L2 (Semantic):** Cross-field and cross-entity checks — referential integrity, identifier format validation, check digit verification
- **L3 (External):** Checks requiring external data — registry lookups, enrichment validation
- Clear separation of which rules belong to which level

### Check Digit Implementations
- Algorithm specification for each identifier type that uses check digits
- Input/output types, error cases
- Test vectors from the spec (exact values, not paraphrased)

### Diagnostic Types
- Rust types for validation findings (errors, warnings, info)
- Diagnostic rendering for human-readable output
- Machine-readable diagnostic format (for tooling integration)
- Diagnostic location tracking (which node/edge/field triggered the finding)

### Error Handling Strategy
- Parse errors vs. validation errors vs. internal errors
- `Result` types and error enums
- Whether to collect all diagnostics or fail-fast (answer: collect all)

## Guidelines
- Write as a senior Rust engineer.
- Include Rust code blocks for key types and trait definitions.
- Reference specific spec requirements by section number.
- Target 1500-2500 words.
```

### Agent 2C → `omtsf-rs/docs/merge.md`

Receives: Agent 1A + 1B output, overview.md, Rust Engineer persona + Graph Theorist persona.

```
{Rust Engineer persona definition}

{Graph Theorist persona definition}

## Your Assignment

Using the requirements extractions below and the implementation overview, write the technical specification for the OMTSF merge engine. You combine the perspectives of a Rust systems engineer (for API design and implementation strategy) and a graph theorist (for algorithmic correctness and formal properties).

**Data Model & Identifier Requirements:**
{Agent 1A output}

**Merge & Graph Algorithm Requirements:**
{Agent 1B output}

**Implementation Overview:**
{contents of omtsf-rs/docs/overview.md}

## What You Must Produce

Write a technical specification document and save it to `omtsf-rs/docs/merge.md`. The document must cover:

### Union-Find for Identity Resolution
- Data structure choice and justification
- Path compression and union-by-rank implementation notes
- How identity predicates feed into the union-find structure

### Identity Predicates
- Rust trait or function signature for identity comparison
- Predicate composition (multiple criteria)
- Performance considerations for large graphs (indexing, hashing)

### Property Merge Strategy
- Conflict resolution: which value wins, how ties break
- Merge functions per property type (strings, numbers, arrays, nested objects)
- Provenance tracking through merges

### Determinism Guarantees
- Why and how merge output is deterministic given the same inputs
- Canonical ordering of nodes and edges in output
- Hash-based verification of merge determinism

### Algebraic Property Tests
- Property-based tests for commutativity, associativity, idempotency
- Proptest strategies for generating random graphs
- Edge cases: empty graphs, single-node graphs, fully-connected graphs

### same_as Handling
- Implementation of transitivity closure
- Integration with the union-find structure
- Cycle detection in same_as chains

## Guidelines
- Write with dual expertise: systems engineering for implementation, graph theory for correctness.
- Include Rust code blocks for key data structures and function signatures.
- Reference specific spec requirements by section number.
- Target 1500-2500 words.
```

### Agent 2D → `omtsf-rs/docs/redaction.md`

Receives: Agent 1A + 1C output, overview.md, Rust Engineer persona + Security & Privacy Expert persona.

```
{Rust Engineer persona definition}

{Security & Privacy Expert persona definition}

## Your Assignment

Using the requirements extractions below and the implementation overview, write the technical specification for the OMTSF selective disclosure / redaction engine. You combine the perspectives of a Rust systems engineer (for implementation) and a security architect (for privacy correctness).

**Data Model & Identifier Requirements:**
{Agent 1A output}

**Selective Disclosure & Privacy Requirements:**
{Agent 1C output}

**Implementation Overview:**
{contents of omtsf-rs/docs/overview.md}

## What You Must Produce

Write a technical specification document and save it to `omtsf-rs/docs/redaction.md`. The document must cover:

### Boundary Reference Hashing
- Exact algorithm implementation (hash function, input preparation, output format)
- All test vectors from the spec, with expected inputs and outputs
- Salt handling: how `file_salt` is incorporated
- Rust implementation notes (which crate for hashing, constant-time considerations)

### Node Classification
- How nodes are classified by sensitivity level
- Classification algorithm: input (node type + attributes) → output (sensitivity level)
- Default classifications for each node type

### Edge Handling During Redaction
- Rules for edges that cross the redaction boundary
- Boundary edge creation: which attributes are preserved, which are stripped
- Handling of edges where both endpoints are redacted

### Output Validation
- A redacted file must itself be valid — what validation rules apply post-redaction
- Boundary reference consistency checks
- Structural integrity after redaction

### Security Considerations
- Information leakage through graph structure (even with node content redacted)
- Timing side-channel considerations in hash computation
- Salt entropy requirements
- Threat model: what the redaction engine protects against and what it does not

## Guidelines
- Write with dual expertise: systems engineering for implementation, security architecture for correctness.
- Include Rust code blocks for key functions and types.
- Preserve all test vectors exactly as extracted.
- Reference specific spec requirements by section number.
- Target 1500-2500 words.
```

### Agent 2E → `omtsf-rs/docs/graph-engine.md`

Receives: Agent 1B output, overview.md, Rust Engineer persona + Graph Theorist persona.

```
{Rust Engineer persona definition}

{Graph Theorist persona definition}

## Your Assignment

Using the requirements extraction below and the implementation overview, write the technical specification for the OMTSF graph engine.

**Merge & Graph Algorithm Requirements:**
{Agent 1B output}

**Implementation Overview:**
{contents of omtsf-rs/docs/overview.md}

## What You Must Produce

Write a technical specification document and save it to `omtsf-rs/docs/graph-engine.md`. The document must cover:

### petgraph Wrapper
- Which petgraph graph type to use (`DiGraph`, `StableDiGraph`, etc.) and why
- Node/edge weight types (how OMTSF nodes/edges map to petgraph weights)
- Index stability guarantees (important for merge and redaction operations)
- Construction from deserialized `OmtsFile`

### Reachability
- Algorithm choice (BFS vs. DFS) and justification
- API: `reachable_from(graph, node_id) -> HashSet<NodeId>`
- Direction handling (forward, backward, undirected)
- Performance characteristics for large graphs

### Path Finding
- Shortest path algorithm (Dijkstra, BFS for unweighted)
- All-paths enumeration (with configurable depth limit to prevent combinatorial explosion)
- API design for both shortest and all-paths queries

### Subgraph Extraction
- Induced subgraph: given a node set, extract all nodes and edges between them
- Ego-graph extraction: given a node and radius, extract the local neighborhood
- Output as a valid `OmtsFile` (with correct header, node/edge references)

### Cycle Detection
- Whether cycles are valid in OMTSF graphs (recycling chains)
- Cycle detection algorithm
- Reporting format for detected cycles

## Guidelines
- Write with dual expertise: systems engineering for API design, graph theory for algorithm selection.
- Include Rust code blocks for key types and function signatures.
- Reference specific spec requirements by section number.
- Target 1500-2500 words.
```

### Agent 2F → `omtsf-rs/docs/diff.md`

Receives: Agent 1A + 1B output, overview.md, Rust Engineer persona.

```
{Rust Engineer persona definition}

## Your Assignment

Using the requirements extractions below and the implementation overview, write the technical specification for the OMTSF diff engine.

**Data Model & Identifier Requirements:**
{Agent 1A output}

**Merge & Graph Algorithm Requirements:**
{Agent 1B output}

**Implementation Overview:**
{contents of omtsf-rs/docs/overview.md}

## What You Must Produce

Write a technical specification document and save it to `omtsf-rs/docs/diff.md`. The document must cover:

### Node and Edge Matching
- Reuse of merge identity predicates for matching nodes across two files
- Matching algorithm: how to pair nodes/edges between file A and file B
- Handling of unmatched nodes (additions and deletions)

### Property Comparison
- Field-by-field comparison of matched nodes/edges
- Semantic comparison vs. textual comparison (e.g., equivalent but differently-formatted dates)
- Nested object comparison strategy
- Array comparison (ordered vs. unordered semantics depending on field)

### Output Format
- Human-readable diff output (inspired by `git diff` but for graph structures)
- Machine-readable diff output (JSON)
- Summary statistics (nodes added/removed/modified, edges added/removed/modified)

### Diff API
- Rust types for diff results (`DiffResult`, `NodeDiff`, `EdgeDiff`, `PropertyChange`)
- Library API: `diff(a: &OmtsFile, b: &OmtsFile) -> DiffResult`
- Filtering: diff only specific node types or properties

## Guidelines
- Write as a senior Rust engineer.
- Include Rust code blocks for key types.
- Reference the identity predicate design from the merge spec.
- Target 1000-1800 words.
```

### Agent 2G → `omtsf-rs/docs/cli-interface.md`

Receives: Agent 1A output, overview.md, Rust Engineer persona.

```
{Rust Engineer persona definition}

## Your Assignment

Using the requirements extraction below and the implementation overview, write the technical specification for the OMTSF CLI interface.

**Data Model & Identifier Requirements:**
{Agent 1A output}

**Implementation Overview:**
{contents of omtsf-rs/docs/overview.md}

## What You Must Produce

Write a technical specification document and save it to `omtsf-rs/docs/cli-interface.md`. The document must cover:

### Command Specifications
For each of the 10 commands listed in the overview (`validate`, `merge`, `redact`, `inspect`, `diff`, `convert`, `reach`, `path`, `subgraph`, `init`), specify:
- Full command syntax with all flags and arguments
- Required vs. optional arguments
- Default values for optional flags
- Exit codes (0 = success, specific non-zero for each failure mode)
- Example invocations

### File I/O Module
- File reading: path resolution, stdin support (`-`), file size limit enforcement
- File writing: stdout by default, `--output` flag for file output
- Error handling for I/O failures (missing file, permission denied, disk full)
- Encoding handling (UTF-8 validation)

### Output Formatting
- Human-readable format (default): tables, colored diagnostics, summary lines
- Machine-readable format (`--format json`): structured JSON output
- Quiet mode (`--quiet`): suppress informational output
- Verbose mode (`--verbose`): additional diagnostic detail

### clap Configuration
- Subcommand structure
- Global flags (--format, --quiet, --verbose, --max-file-size)
- Argument validation at the clap level vs. application level
- Help text and version display

### Exit Code Table
Complete table mapping every error condition to a specific exit code.

## Guidelines
- Write as a senior Rust engineer designing a CLI.
- Include clap derive macro examples for key commands.
- Ensure every command from the overview is fully specified.
- Target 1500-2500 words.
```

---

## Phase 3: Task Synthesis

After ALL Phase 2 agents complete, launch a single agent using the **Task** tool with `subagent_type: "general-purpose"`.

This agent reads all Phase 2 output files directly from disk and produces the ordered task list.

```
You are a senior engineering manager planning the implementation of a Rust project. You have access to a complete set of technical specification documents.

## Your Assignment

Read all implementation spec files from `omtsf-rs/docs/`:
- `overview.md`
- `data-model.md`
- `validation.md`
- `merge.md`
- `redaction.md`
- `graph-engine.md`
- `diff.md`
- `cli-interface.md`

Then produce an ordered, dependency-aware task list and write it to `omtsf-rs/docs/tasks.md`. This is your ONLY output — do not create any Rust source files or any files outside `omtsf-rs/docs/`.

## What You Must Produce

A task list with 35-40 tasks, each with:

### Task Format
For each task:
- **Task ID:** T-001, T-002, etc.
- **Title:** Short imperative description (e.g., "Define core node and edge types")
- **Spec Reference:** Which spec document(s) and sections this task implements
- **Dependencies:** Task IDs that must be completed first (e.g., "T-001, T-003")
- **Complexity:** S (< 1 day), M (1-2 days), L (3-5 days), XL (> 5 days)
- **Acceptance Criteria:** 2-4 bullet points defining "done" for this task
- **Crate:** Which crate this task primarily affects (`omtsf-core` or `omtsf-cli`)

### Ordering Principles
1. **Foundation first:** Data model types before anything that uses them
2. **Parse before validate:** Deserialization before validation rules
3. **Core before CLI:** Library logic before CLI wrappers
4. **Independent modules in parallel:** Merge and redaction can proceed independently after data model is stable
5. **Integration last:** Cross-module integration and end-to-end tests at the end

### Task Categories
Group tasks into these phases:
1. **Workspace Setup** (Cargo workspace, crate scaffolding, CI)
2. **Data Model** (types, serde, basic tests)
3. **Validation Engine** (rule registry, L1 rules, L2 rules, L3 stubs)
4. **Graph Engine** (petgraph integration, traversal, queries)
5. **Merge Engine** (identity predicates, union-find, property merge)
6. **Redaction Engine** (boundary hashing, node classification, scope filtering)
7. **Diff Engine** (matching, comparison, output formatting)
8. **CLI Shell** (clap setup, file I/O, output formatting, per-command wiring)
9. **Integration & Polish** (end-to-end tests, fixtures, error message review, documentation)

## Guidelines
- Every task should be independently implementable once its dependencies are met.
- Each task should be completable by one engineer.
- Acceptance criteria must be testable — prefer "unit tests pass" over vague descriptions.
- Flag any spec ambiguities discovered during task planning as notes at the end of the document.
- Read the actual spec files from disk — do not rely on summaries.
```

---

## Important Execution Notes

- **Output boundary: This skill produces ONLY markdown files in `omtsf-rs/docs/`.** No Rust source code, no Cargo.toml files, no files outside that directory. Agents must not create any `.rs` files, modify any `Cargo.toml`, or scaffold any crate structure. Code blocks in spec documents are illustrative — they show proposed type signatures and API sketches, not compilable source files.
- Launch ALL agents within each phase in parallel using multiple Task tool calls in a single message.
- Wait for ALL agents in a phase to complete before starting the next phase.
- Read ALL files (specs, overview, personas) BEFORE dispatching Phase 1 agents, so you can include the content in each agent's prompt.
- For Phase 2, include the relevant Phase 1 agent output(s) directly in the prompt — do not ask Phase 2 agents to read Phase 1 output from disk.
- Phase 2 agents write their output files directly using the Write tool. Do not rewrite their output.
- Phase 3 agent reads Phase 2 output files from disk (they exist as files at that point).
- If an agent fails or returns an inadequate response, note the gap and continue rather than blocking the pipeline.
- After all phases complete, report a summary: which files were written, any gaps or issues noted.
