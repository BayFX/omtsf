# Expert Panel Report: Binary Format Support for .omts Files

**Date:** 2026-02-21
**Panel Chair:** Expert Panel Review Process
**Status:** Pre-implementation review — recommendations for user confirmation before spec changes

---

## Panel Chair Summary

Seven domain experts reviewed the proposal to add a compact binary serialization format alongside JSON for `.omts` files. The panel reached **strong consensus** on the fundamental question: OMTSF should support a binary encoding, and **CBOR (RFC 8949)** is the right choice. Every expert who expressed a preference independently converged on CBOR over MessagePack, citing its IETF standardization, self-describing tag (55799 / `0xd9d9f7`), deterministic encoding profiles (CDE/dCBOR), and mature Rust ecosystem via `ciborium`.

The panel identified **one critical architectural blocker** that every technical expert independently flagged: the Rust reference implementation's deep coupling to `serde_json::Value` (243 occurrences across 22 source files in `omtsf-core`). This must be resolved before binary format work can proceed. The panel also reached strong consensus that the specification itself must be refactored to separate the abstract data model from JSON-specific serialization conventions — the current wording ("An .omts file is a JSON document") creates a normative contradiction if a second encoding is added.

The most significant area of **disagreement** was timing. The Data Format Expert, Rust Engineer, and Graph Modeling Expert favored implementing binary support now (pre-1.0) to avoid JSON-specific technical debt. The Open Source Strategist and ERP Integration Expert strongly argued for deferring mandatory implementation until post-1.0 to avoid fragmenting a nascent ecosystem. The Standards Expert offered a middle path: define the binary binding now in an informative annex, promote to normative after JSON stabilizes. The Security Expert prioritized deterministic encoding guarantees regardless of timing. The synthesis recommendation is to **design and specify now, implement in the reference CLI now, but make binary conformance optional for third-party producers until 1.0**.

---

## Panel Composition

| Expert | Role | Key Focus Area |
|--------|------|---------------|
| Data Format Expert | Data Format Architect | Binary format selection, compression, headers, schema evolution |
| Systems Engineering Expert | Senior Rust Engineer | Implementation architecture, serde, WASM, crate design |
| Graph Modeling Expert | Graph Data Modeling Specialist | Serialization round-trip fidelity, graph algorithm implications |
| Standards Expert | Standards & Interoperability Specialist | Spec rigor, media types, conformance clauses, IETF alignment |
| Security & Privacy Expert | Data Security & Privacy Architect | Deterministic encoding, integrity, parser security, auditability |
| Enterprise Integration Expert | Enterprise Systems Architect | ERP export/import, middleware compatibility, adoption burden |
| Open Source Strategy Expert | Open Source Governance Lead | Ecosystem timing, conformance levels, adoption strategy |

---

## Consensus Findings

These findings were independently raised by 3+ experts:

1. **CBOR (RFC 8949) is the right binary encoding.** (All 7 experts) CBOR's IETF standardization, JSON-superset data model, self-describing tag 55799, deterministic encoding profiles, and Rust ecosystem support (`ciborium`) make it the clear choice over MessagePack, Protobuf, or FlatBuffers. The unknown-field preservation requirement eliminates schema-required formats.

2. **`serde_json::Value` coupling is the primary technical blocker.** (Data Format, Rust Engineer, Graph Modeling) The 243 occurrences of `serde_json::Value`/`Map` in `omtsf-core` make direct CBOR serialization impossible. A format-neutral intermediate value type is required.

3. **The spec must separate abstract data model from serialization binding.** (Standards, Data Format, Security) SPEC-001 Section 2 stating "An .omts file is a JSON document" and Section 2.1's JSON-specific first-key requirement create normative contradictions with a second encoding. The spec needs refactoring.

4. **Format auto-detection should use CBOR's self-describing tag.** (Data Format, Rust Engineer, Standards) If the first 3 bytes are `0xd9 0xd9 0xf7`, the file is CBOR. If the first non-whitespace byte is `{` (0x7B), it is JSON. No custom magic bytes needed.

5. **Hash computation must remain encoding-independent.** (Security, Standards, Graph Modeling) Boundary reference hashing (SPEC-004) and canonical identifier strings (SPEC-002) must operate on the logical data model, not serialized bytes, ensuring identical results regardless of encoding.

6. **JSON must remain a first-class citizen.** (ERP Integration, Open Source, Standards) ERP systems and integration middleware have excellent JSON support but poor/no CBOR support. JSON should remain the default and the minimum conformance requirement.

7. **The `convert` command is the natural home for format conversion.** (All experts referencing CLI) Extending the existing `omtsf convert` with `--to json` and `--to cbor` flags fits the existing architecture.

---

## Critical Issues

### C1: `serde_json::Value` pervasiveness blocks binary format support
**Flagged by:** Data Format Expert, Rust Engineer, Graph Modeling Expert

The `omtsf-core` crate uses `serde_json::Map<String, serde_json::Value>` for unknown-field catch-all (`extra` fields) and stores `geo`, `governance_structure`, and `control_type` as `serde_json::Value`. Ciborium's deserializer cannot produce `serde_json::Value` — it produces `ciborium::Value`. This means you cannot deserialize a CBOR file directly into the current `OmtsFile` type.

**Resolution:** Introduce a format-neutral value type (`ExtValue` enum) that both `serde_json::Value` and `ciborium::Value` can convert to/from. This is the largest refactor and must be completed before any binary format work.

### C2: Spec normatively couples data model to JSON serialization
**Flagged by:** Standards Expert, Data Format Expert, Security Expert

SPEC-001 Section 2 ("An .omts file is a JSON document"), Section 2.1 (first-key requirement), and Section 11 (conformance clauses) all assume JSON. Adding a binary format without refactoring creates internal contradictions.

**Resolution:** Refactor SPEC-001 into: (a) encoding-independent abstract data model sections, (b) a JSON serialization binding annex, (c) a CBOR serialization binding annex.

### C3: Deterministic encoding required for integrity operations
**Flagged by:** Security Expert, Standards Expert

Boundary reference hashing (SPEC-004 Section 4) depends on deterministic byte sequences. CBOR has multiple valid byte representations for the same logical value. Without mandating a specific deterministic profile (CBOR CDE or dCBOR), different implementations could produce different hashes.

**Resolution:** Either (a) mandate CBOR CDE/dCBOR for deterministic CBOR output, or (b) specify that all hash-dependent operations use the canonical identifier string format (SPEC-002 Section 4) as input, not serialized bytes. The panel recommends option (b) as it decouples integrity from encoding.

---

## Major Issues

### M1: Middleware ecosystem lacks CBOR support
**Flagged by:** ERP Integration Expert

MuleSoft DataWeave, SAP CPI, Dell Boomi, and Oracle Integration Cloud have no native CBOR support. Requiring CBOR output from ERP extractors imposes custom serialization code.

**Resolution:** Tier conformance requirements — JSON is MUST for all producers; CBOR is MAY for producers, MUST for the reference CLI and analytics consumers.

### M2: `#[serde(flatten)]` compatibility with ciborium
**Flagged by:** Rust Engineer, Data Format Expert

The `flatten` attribute drives unknown-field preservation. While CBOR is self-describing (so `deserialize_any` works), there are known edge cases with internally tagged enums and ciborium. Must be validated with actual struct hierarchy.

**Resolution:** Build a comprehensive integration test suite for the `OmtsFile` hierarchy with ciborium before committing to CBOR in the spec.

### M3: Conformance clause bifurcation undefined
**Flagged by:** Standards Expert, ERP Integration Expert, Open Source Strategist

Must a conformant consumer accept both encodings? May a producer emit only JSON? Current Section 11 does not address this.

**Resolution:** Define conformance profiles — "JSON Conformant" (minimum) and "Full Conformant" (JSON + CBOR + conversion).

### M4: Round-trip fidelity for numeric types
**Flagged by:** Rust Engineer, Graph Modeling Expert

JSON distinguishes integer `42` from float `42.0`, but CBOR encodes both as integer. A JSON → CBOR → JSON round-trip may lose this distinction. Ownership `percentage: 51.0` could become `51`.

**Resolution:** Spec must define canonical numeric representation, or round-trip tests must normalize numeric comparison.

### M5: Binary output on terminal is hostile
**Flagged by:** Rust Engineer

Commands that write to stdout (`convert`, `merge`, `subgraph`) would emit binary to a terminal. The CLI should refuse binary output to a TTY unless `--force` is given, mirroring `gzip`'s behavior.

### M6: Nesting depth unbounded for binary input
**Flagged by:** Security Expert

The `extra` catch-all can contain deeply nested structures. Binary parsers must enforce maximum recursion depth (recommend 64) to prevent stack exhaustion.

---

## Minor Issues

- **Compression should be orthogonal to encoding.** Binary and compression are independent layers. Consider `.omts.zst` (zstd-compressed JSON) as a pragmatic intermediate step. (Data Format, Open Source Strategist)
- **Endianness specification needed.** Standardize on little-endian. (Graph Modeling Expert)
- **File extension / MIME type strategy.** Need `application/vnd.omtsf+json` and `application/vnd.omtsf+cbor` media types, or decide if `.omts` covers both with magic-byte discrimination. (Standards Expert)
- **Floating-point width in CBOR.** CBOR has half/single/double float. Must specify which representation to use for OMTSF numeric fields. (Security Expert)
- **EDI coexistence.** Binary .omts should not be positioned as competing with EDI for transactional exchange. (ERP Integration Expert)
- **I/O path must skip UTF-8 validation for binary.** The current `read_input` pipeline assumes UTF-8. Binary input must bypass this. (Rust Engineer)

---

## Consolidated Recommendations

### P0 — Immediate (before binary format work begins)

| # | Recommendation | Source Expert(s) |
|---|---------------|-----------------|
| 1 | **Introduce a format-neutral value type in `omtsf-core`.** Replace `serde_json::Map<String, serde_json::Value>` with a custom `ExtValue` enum that both JSON and CBOR can serialize to/from. This unblocks all binary format work. | Data Format, Rust Engineer |
| 2 | **Refactor SPEC-001 into abstract data model + serialization bindings.** Extract encoding-independent sections. Move JSON-specific rules to a binding annex. Add a CBOR binding annex. | Standards, Data Format |
| 3 | **Choose CBOR (RFC 8949) as the binary encoding.** Use `ciborium` crate in Rust. Leverage self-describing tag 55799 (`0xd9d9f7`) for format detection. | All 7 experts |
| 4 | **Define format auto-detection in the spec.** First 3 bytes `0xd9 0xd9 0xf7` = CBOR. First non-whitespace byte `{` = JSON. | Data Format, Rust Engineer, Standards |
| 5 | **Specify that hash-dependent operations (boundary refs, canonical identifiers) are encoding-independent.** They operate on logical data model strings, not serialized bytes. | Security, Standards |

### P1 — Before v1.0

| # | Recommendation | Source Expert(s) |
|---|---------------|-----------------|
| 6 | **Define conformance profiles per encoding.** JSON-only = minimum conformance. JSON+CBOR = full conformance. Producers MAY emit JSON only. Reference CLI and validators MUST support both. | Standards, ERP Integration, Open Source |
| 7 | **Extend `omtsf convert` command.** Add `--to json` (default) and `--to cbor` flags. Auto-detect input format. Refuse binary output to TTY without `--force`. | Rust Engineer, Data Format, ERP Integration |
| 8 | **Refactor CLI I/O to return raw bytes.** Move UTF-8 validation into the JSON path only, allowing binary files through the same size-checked pipeline. | Rust Engineer |
| 9 | **Add round-trip property tests.** JSON → typed → CBOR → typed → JSON must preserve structural equality. Cover numeric edge cases, null vs absent, extension fields. | Rust Engineer, Graph Modeling |
| 10 | **Mandate deterministic CBOR encoding profile.** If content hashing over serialized bytes is ever needed, require CBOR CDE (RFC 8949 Section 4.2). For now, decouple hashing from encoding (see P0-5). | Security, Standards |
| 11 | **Add nesting depth and allocation limits to advisory limits.** Recommend max nesting depth of 64 for binary parsers. | Security |
| 12 | **Plan IANA media type registration.** `application/vnd.omtsf+json` and `application/vnd.omtsf+cbor`. | Standards |

### P2 — Future / post-1.0

| # | Recommendation | Source Expert(s) |
|---|---------------|-----------------|
| 13 | **Benchmark CBOR vs JSON at scale.** Test at 10K, 100K, 1M nodes for file size, parse time, memory. Publish results. | Data Format, ERP Integration |
| 14 | **Provide reference conversion libraries for enterprise middleware.** Java/Groovy helpers for SAP CPI and MuleSoft to handle CBOR. | ERP Integration |
| 15 | **Consider string interning for graph-local IDs in binary format.** Integer-indexed string table could reduce file size 20-30% for edge-heavy graphs. | Graph Modeling |
| 16 | **Consider zstd compression as a separate, orthogonal layer.** Define `.omts.zst` convention or a compression byte in the header. | Data Format, Open Source |
| 17 | **Fuzz the binary parser.** Use `cargo-fuzz` with 10^6+ iterations before any release. | Security, Rust Engineer |
| 18 | **Consider columnar variant (Arrow IPC) for analytical workloads.** Secondary format tier for very large graphs. | Graph Modeling |

---

## Cross-Domain Interactions

These interdependencies are the most valuable insights from the multi-expert review:

1. **Integrity ↔ Encoding (Security × Standards × Data Format):** The boundary reference hash computation (SPEC-004) is the most encoding-sensitive operation. The panel converged on keeping it encoding-independent (operating on canonical strings, not serialized bytes), which eliminates the largest cross-domain risk.

2. **Adoption ↔ Conformance (Open Source × ERP × Standards):** Tiering conformance (JSON = minimum, CBOR = optional) prevents the binary format from becoming an adoption barrier for ERP extractors while giving analytics platforms the compact encoding they need.

3. **Architecture ↔ Spec (Rust Engineer × Data Format × Standards):** The `serde_json::Value` refactor in `omtsf-core` is both an implementation task and a spec task. The spec must stop assuming JSON as the sole encoding; the implementation must stop using JSON-native types as the internal representation. These must proceed in lockstep.

4. **Security ↔ Implementation (Security × Rust Engineer):** Rust's memory safety eliminates buffer overflow risks, but algorithmic complexity attacks (deep nesting, huge arrays) remain. The advisory size limits must be extended to cover nesting depth, and the binary parser must be fuzzed.

5. **ERP Integration ↔ Ecosystem Timing (ERP × Open Source):** The compressed JSON path (`.omts.zst`) could deliver most of the size benefits without requiring any binary format work, buying time for the JSON format to stabilize before CBOR is introduced.

---

## Individual Expert Reports

### Data Format Expert (Data Format Architect)

#### Assessment

The proposal to add a binary encoding alongside JSON for `.omts` files is sound from a format design perspective and well-timed — doing this early (pre-1.0) avoids the pain of retrofitting it after the format has wide adoption. The advisory size limits of 1M nodes and 5M edges make binary encoding a near-necessity for the upper end of the use case: a JSON file at that scale could easily reach multiple gigabytes, where a schemaless binary encoding like CBOR would yield 25-40% smaller payloads before compression, and 60-80% smaller with zstd applied on top.

However, the current implementation has a deep structural coupling to `serde_json` that will require careful architectural work. The `extra` fields on `OmtsFile`, `Node`, `Edge`, and `EdgeProperties` are typed as `serde_json::Map<String, serde_json::Value>` — a type that is not directly serializable to non-JSON serde formats without a conversion layer. This means the binary format cannot be a simple "swap `serde_json::to_writer` for `ciborium::into_writer`" — the intermediate representation itself is JSON-native.

My recommended approach is CBOR (RFC 8949) as the binary encoding, with a fixed file header that distinguishes formats, optional zstd compression as a separate layer, and a phased refactoring of `omtsf-core` to use a format-neutral value type internally.

#### Strengths
- CBOR's data model is a superset of JSON's (RFC 8949 Section 6.1), making round-trip fidelity straightforward
- Existing serde ecosystem (`ciborium`) provides serde-compatible CBOR for Rust
- CBOR's self-describing tag 55799 (`0xd9d9f7`) serves as a built-in magic number
- The spec already anticipates multiple serializations (vision document)
- WASM compatibility: ciborium has no I/O dependencies

#### Concerns
- **[Critical]** `serde_json::Value` pervasion — 243 occurrences across 22 source files. Ciborium's serializer does not understand `serde_json::Value`'s internal representation.
- **[Major]** `#[serde(flatten)]` compatibility with CBOR — undertested edge cases with ciborium
- **[Major]** Header design — "first key must be `omtsf_version`" doesn't apply to binary
- **[Minor]** Compression should be orthogonal to encoding
- **[Minor]** MessagePack lacks IETF standardization; CBOR is preferred

#### Recommendations
1. (P0) Introduce format-neutral value type in omtsf-core
2. (P0) Define file header in spec: first bytes `0xd9d9f7` = CBOR, `{` = JSON
3. (P1) Add compression envelope (separate layer)
4. (P1) Extend convert command with `--format json|cbor`
5. (P1) Update conformance clauses for binary
6. (P2) Benchmark CBOR vs JSON vs MessagePack at scale

---

### Systems Engineering Expert (Senior Rust Engineer)

#### Assessment

From a systems engineering perspective, adding a binary format is technically achievable but involves a significant architectural challenge: the deep entanglement of `serde_json::Value` throughout `omtsf-core`. These are not incidental — they are structural. Every `Node`, `Edge`, `EdgeProperties`, and `OmtsFile` struct uses `#[serde(flatten)] pub extra: serde_json::Map<String, serde_json::Value>` for unknown-field preservation.

The good news is that CBOR is semantically a superset of JSON's data model and is self-describing — meaning serde's `#[serde(flatten)]` will work with `ciborium` since it relies on `deserialize_any`. CBOR also has an IETF-standardized magic number (tag 55799) making format auto-detection trivial. However, the `serde_json::Value` type in the flatten catch-all is a blocker: ciborium's deserializer will not produce `serde_json::Value`.

I recommend CBOR (ciborium) for its IETF pedigree, self-describing nature, and existing file-magic specification.

#### Strengths
- Serde-first architecture — all types already derive Serialize/Deserialize
- Clean I/O boundary — omtsf-core operates on &str/&[u8], CLI owns I/O
- Existing `convert` command is the natural extension point
- CBOR's self-describe tag provides unambiguous 3-byte detection
- ciborium is compatible with no_std+alloc for WASM

#### Concerns
- **[Critical]** `serde_json::Value` pervasiveness — cannot deserialize CBOR directly into current types
- **[Major]** `#[serde(flatten)]` + ciborium edge cases with tagged enums
- **[Major]** JSON integer/float distinction lost in CBOR round-trip
- **[Minor]** Binary output to terminal is hostile
- **[Minor]** `read_input` assumes UTF-8 — binary must bypass this path

#### Recommendations
1. (P0) Introduce format-neutral value type replacing serde_json::Map
2. (P0) Choose CBOR with ciborium 0.2.x
3. (P1) Define format detection in spec (3-byte CBOR tag vs `{`)
4. (P1) Refactor io::read_input to return Vec<u8>
5. (P1) Extend convert command with --to json/cbor
6. (P2) Add round-trip property tests with proptest
7. (P2) Benchmark size and speed delta

---

### Graph Modeling Expert (Graph Data Modeling Specialist)

#### Assessment

From a graph data modeling perspective, introducing a compact binary format is well-motivated. The flat adjacency list structure — nodes as a list, edges as a list with source/target references — maps cleanly to a record-oriented binary layout. Given OMTSF's advisory limits of 1M nodes and 5M edges, a JSON file at that scale could easily reach hundreds of megabytes, making binary encoding a practical necessity.

However, OMTSF's forward-compatibility requirement (unknown field preservation) is the dominant constraint on format choice. The flat adjacency-list model carries heterogeneous property schemas (7 node types, 16+ edge types), and unknown fields must survive round-trip. This eliminates schema-required formats (Protobuf, FlatBuffers) and points toward schemaless binary encodings (CBOR, MessagePack).

#### Strengths
- Flat adjacency list is binary-friendly — naturally record-oriented
- Graph-local IDs are compact (file-scoped, can be string-interned in binary)
- No nested graph structure (unlike GraphML/GEXF)
- Advisory size limits provide concrete design bounds

#### Concerns
- **[Critical]** Round-trip fidelity with unknown fields — must support generic extension map encoding
- **[Critical]** Property type heterogeneity across node/edge types — binary must handle union/tagged encoding
- **[Major]** Merge semantics depend on string identity predicates — interning must not affect merge
- **[Major]** Memory-mapped binary could skip deserialization but conflicts with petgraph's owned-data model
- **[Minor]** Endianness should be standardized (little-endian)
- **[Minor]** Binary-then-compress yields better ratios than JSON-then-compress

#### Recommendations
1. (P0) Choose schemaless binary encoding (CBOR or MessagePack)
2. (P0) Define magic-byte header for format discrimination
3. (P1) Implement string interning for graph-local IDs
4. (P1) Specify binary round-trip preserves JSON-level semantics
5. (P2) Treat compression as separate optional layer
6. (P2) Consider columnar variant (Arrow IPC) for analytical workloads

---

### Standards Expert (Standards & Interoperability Specialist)

#### Assessment

From a standards development perspective, introducing a binary encoding is a foreseeable and reasonable evolution. However, multi-encoding standards have a long and instructive history — some successful (GS1 EPCIS 2.0's parallel XML and JSON-LD bindings), many painful (ASN.1's proliferation of encoding rules that splintered tooling ecosystems). The critical question is how to structure the specification so the abstract data model remains encoding-independent while each serialization binding is rigorously specified.

What concerns me most is the current spec's tight coupling between the data model and JSON. SPEC-001 Section 2 flatly states "An .omts file is a JSON document." Adding a binary format without first refactoring into abstract model + binding layers will create normative contradictions.

#### Strengths
- Vision document foresight explicitly anticipated format evolution
- Flat adjacency-list model is inherently encoding-neutral
- Semver versioning provides evolution mechanism
- Self-contained file design supports format discriminators
- CBOR has existing IANA media type and structured syntax suffix

#### Concerns
- **[Critical]** Normative coupling of data model to JSON in SPEC-001 Sections 2, 2.1, and 11
- **[Critical]** Media type and file identification strategy undefined (need magic number + IANA types + extension guidance)
- **[Major]** Conformance clause bifurcation — must define per-encoding requirements
- **[Major]** Canonical form for integrity operations must be encoding-independent
- **[Minor]** Risk of ecosystem fragmentation if introduced before JSON stabilizes
- **[Minor]** Compression is transport-layer, not format-layer

#### Recommendations
1. (P0) Refactor SPEC-001 into abstract model + serialization bindings
2. (P0) Define media type registration strategy (application/vnd.omtsf+json and +cbor)
3. (P0) Define canonical encoding for hash computation (encoding-independent)
4. (P1) Define conformance profiles per encoding
5. (P1) Adopt CBOR (RFC 8949)
6. (P2) Defer binary encoding to informative until post-first-adopter
7. (P2) Specify compression as transport-layer concern

---

### Security & Privacy Expert (Data Security & Privacy Architect)

#### Assessment

From a security and privacy standpoint, introducing a binary format intersects directly with OMTSF's most security-critical operations: boundary reference hashing, selective disclosure enforcement, and privacy-preserving redaction. The current JSON format benefits from human inspectability — essential for AMLD-sensitive beneficial ownership data and GDPR-constrained person nodes.

My primary concern is the integrity chain. SPEC-004's boundary reference computation depends on canonical identifier strings being byte-deterministic. CBOR has multiple valid byte representations for the same logical value; only strict adherence to a deterministic encoding profile eliminates this ambiguity. From a parser security perspective, binary formats historically carry a larger attack surface, though Rust's memory safety mitigates the most dangerous class of vulnerabilities.

#### Strengths
- Rust's memory safety eliminates buffer overflow class of binary parser vulnerabilities
- Serde abstraction layer is format-agnostic
- `file_salt` is already raw bytes hex-encoded (binary can store as bytestring)
- WASM constraint limits I/O side channels
- Advisory size limits provide natural DoS defense

#### Concerns
- **[Critical]** Deterministic encoding required for boundary reference hashing — CBOR CDE or dCBOR must be mandated
- **[Critical]** Binary header must prevent format confusion attacks
- **[Major]** Unknown-field ordering in binary may break deterministic encoding
- **[Major]** Binary opacity reduces auditability of privacy-sensitive content
- **[Major]** Recursive nesting depth must be bounded (recommend max 64)
- **[Minor]** Floating-point representation divergence across CBOR widths
- **[Minor]** Compression before encryption can leak information (CRIME/BREACH)

#### Recommendations
1. (P0) Mandate deterministic encoding profile with byte-level test vectors
2. (P0) Define binary header with magic number, version, and profile identifier
3. (P1) Enforce recursion depth and allocation limits
4. (P1) Hash computation must operate on canonical identifier strings, not serialized bytes
5. (P1) Require `inspect` command to render binary files as human-readable text
6. (P2) Add content_hash field to header for integrity verification
7. (P2) Fuzz binary parser from day one

---

### Enterprise Integration Expert (Enterprise Systems Architect)

#### Assessment

From twenty years of ERP integration experience, the current JSON-only format is well-aligned with middleware reality. MuleSoft, SAP CPI, Dell Boomi, and Oracle Integration Cloud all have native JSON support. CBOR and MessagePack are not built-in DataWeave formats or standard SAP CPI message transformers.

That said, the legitimate need exists: 1M nodes and 5M edges can produce very large files where binary reduces size 3-5x. The key is ensuring JSON remains the canonical exchange format for ERP integrations, with binary positioned as an optimization for machine-to-machine and archival use cases.

#### Strengths
- JSON remains fully supported — ERP tools need no changes
- Header-based format discrimination is sound
- The `convert` command provides a clean bridge
- Size reduction matters for regulatory submission aggregation
- Round-trip fidelity aligns with enterprise data governance

#### Concerns
- **[Critical]** Middleware ecosystem gap — no native CBOR in MuleSoft/SAP CPI/Dell Boomi
- **[Major]** "Must support both for output" is too strong for ERP producers
- **[Major]** Testing and validation complexity doubles
- **[Minor]** EDI coexistence — binary should not target transactional exchange
- **[Minor]** Extension field preservation must be guaranteed in binary

#### Recommendations
1. (P0) Tier requirements: JSON MUST for producers, binary MAY; both MUST for CLI/consumers
2. (P0) Choose CBOR over MessagePack for IETF pedigree
3. (P1) Define binary as mechanical mapping from JSON data model
4. (P1) Extend convert with --to-cbor and --to-json
5. (P1) Specify header format precisely
6. (P2) Provide reference conversion scripts for middleware
7. (P2) Benchmark with realistic ERP data profiles

---

### Open Source Strategy Expert (Open Source Governance Lead)

#### Assessment

From an adoption strategy perspective, introducing a binary format is the right *eventual* move, but timing carries significant ecosystem risk. The project is pre-1.0, has zero third-party implementations, no production users, and a reference implementation still under active development. Every successful multi-format standard introduced the compact encoding *after* the human-readable format had proven itself.

The right strategy is to *design* the binary format now (so the data model does not accidentally become JSON-dependent) but *defer mandatory implementation* until after first production deployments.

#### Strengths
- Data model is serialization-agnostic by design
- `omtsf_version` enables format negotiation without major version bump
- WASM target benefits from smaller wire sizes
- CLI already has a `convert` command
- Extension mechanism (reverse-domain notation) translates to any encoding

#### Concerns
- **[Critical]** No third-party implementations exist — dual-format doubles adoption barrier
- **[Critical]** Conformance test suite not yet complete — testing neither format properly
- **[Major]** Contributor attention diverted from core features (merge, redaction, queries)
- **[Major]** ERP integration complexity increases prematurely
- **[Minor]** File extension / MIME type convention needed

#### Recommendations
1. (P0) Design binary encoding now; defer mandatory implementation to post-1.0
2. (P0) Define conformance levels separating format support
3. (P1) Complete JSON conformance test suite first
4. (P1) Add binary support as optional Cargo feature flag
5. (P2) Engage community on format selection via RFC process
6. (P2) Define conversion round-trip guarantees early

---

## Questions for the User

Before proceeding with spec changes, the panel recommends confirmation on these decisions:

1. **Binary format choice: CBOR?** All 7 experts recommend CBOR (RFC 8949). Do you agree, or do you want to evaluate alternatives?

2. **Timing: implement now or defer?** The panel split on this. The recommended synthesis is: specify now, implement in the reference CLI now, but make binary conformance optional for third-party producers. Does this approach work?

3. **Format detection: CBOR self-describing tag vs custom header?** The majority recommend using CBOR's built-in tag 55799 (`0xd9d9f7`) for detection rather than a custom magic number. Some experts suggested a custom `OMTS` header for more extensibility (compression flags, version). Which approach do you prefer?

4. **Compression: in-scope or deferred?** Should the spec define a compression layer now (e.g., zstd as a separate envelope), or defer compression to a future spec revision?

5. **Spec refactoring scope:** The Standards Expert recommends splitting SPEC-001 into abstract model + serialization bindings. This is a significant restructuring. Should this happen as part of the binary format work, or as a separate effort?
