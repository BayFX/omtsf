# omtsf-cli Technical Specification: Overview

**Status:** Draft
**Date:** 2026-02-19

---

## 1. Purpose

`omtsf-cli` is the reference command-line tool for working with `.omts` files. It validates, merges, redacts, inspects, diffs, converts, queries, and scaffolds supply chain graph files as defined by the OMTSF specification suite (SPEC-001 through SPEC-006).

The CLI is the primary interface for integrating OMTSF into automated pipelines, CI/CD workflows, and developer toolchains. It is designed for implementors building on or contributing to the OMTSF ecosystem.

---

## 2. Design Goals

1. **Correctness over speed.** The CLI is the reference implementation. Its validation output is authoritative. If the CLI says a file is valid, it is valid.
2. **Local-only processing.** No network calls unless explicitly requested (e.g., L3 enrichment checks against external registries). All core operations work offline.
3. **Composable output.** Human-readable by default. Exit code 0 on success, non-zero on failure. Diagnostics to stderr, data to stdout. Suitable for piping and scripting.
4. **WASM-ready architecture.** The library layer must not depend on filesystem or OS primitives. I/O is a CLI concern, not a library concern. This ensures the core logic can compile to WebAssembly for browser-based tooling without a separate implementation.

---

## 3. Workspace Structure

The project is a Cargo workspace with three crates:

```
omtsf-rs/
├── Cargo.toml              # workspace root
├── crates/
│   ├── omtsf-core/         # library: data model, parsing, validation, merge, graph engine
│   │   ├── Cargo.toml
│   │   └── src/
│   ├── omtsf-cli/          # binary: argument parsing, I/O, formatting, exit codes
│   │   ├── Cargo.toml
│   │   └── src/
│   └── omtsf-wasm/         # future: wasm-bindgen surface (not specced yet)
│       └── Cargo.toml
├── docs/                   # technical specification documents
└── tests/                  # integration tests, fixture .omts files
```

**`omtsf-core`** is the library. It owns:
- The Rust type model for `.omts` files (nodes, edges, header, identifiers, labels)
- JSON deserialization and serialization (serde + serde_json)
- Three-level validation engine (L1/L2/L3)
- Merge engine (SPEC-003)
- Selective disclosure / redaction engine (SPEC-004)
- Graph construction and query engine (petgraph-backed)
- Diff engine

`omtsf-core` has **no** dependency on `std::fs`, `std::io::Read` from files, or any OS-level I/O. It operates on `&str`, `&[u8]`, or `serde_json::Value` inputs. This is the WASM boundary constraint.

**`omtsf-cli`** is the binary. It owns:
- Argument parsing (clap)
- File I/O: reading `.omts` files from disk, writing to stdout or files
- Output formatting: human-readable diagnostics, summary tables
- Exit code mapping
- Configurable file size limits

**`omtsf-wasm`** is a future crate. The current spec ensures `omtsf-core` is compatible with wasm32 targets but does not define the wasm-bindgen surface.

---

## 4. Commands

### 4.1 Core Commands

| Command | Input | Output | Spec Reference |
|---------|-------|--------|----------------|
| `omtsf validate <file>` | One `.omts` file | Diagnostics (errors, warnings, info) | SPEC-001 §9, SPEC-002 §6, SPEC-004 §6 |
| `omtsf merge <file>...` | Two or more `.omts` files | Merged `.omts` to stdout | SPEC-003 |
| `omtsf redact <file>` | One `.omts` file + scope flag | Redacted `.omts` to stdout | SPEC-004 |
| `omtsf inspect <file>` | One `.omts` file | Summary statistics to stdout | — |
| `omtsf diff <a> <b>` | Two `.omts` files | Structural diff to stdout | — |
| `omtsf convert <file>` | One `.omts` file | Re-serialized `.omts` to stdout | — |

### 4.2 Graph Query Commands

| Command | Input | Output | Description |
|---------|-------|--------|-------------|
| `omtsf reach <file> <node-id>` | File + source node | Reachable node set | All nodes reachable from the given node via directed edges |
| `omtsf path <file> <from> <to>` | File + two node IDs | Paths | All or shortest paths between two nodes |
| `omtsf subgraph <file> <node-id>...` | File + node set | `.omts` subgraph to stdout | Extract the induced subgraph containing the specified nodes and all edges between them |

### 4.3 Scaffolding Commands

| Command | Input | Output | Description |
|---------|-------|--------|-------------|
| `omtsf init` | None | Minimal `.omts` to stdout | Minimal valid file: header with generated `file_salt`, empty `nodes`/`edges` arrays |
| `omtsf init --example` | None | Example `.omts` to stdout | Realistic sample file for learning and testing |

---

## 5. Key Dependencies

| Crate | Purpose | Used By |
|-------|---------|---------|
| `serde`, `serde_json` | JSON serialization/deserialization of `.omts` files | `omtsf-core` |
| `clap` | CLI argument parsing, subcommand dispatch, help generation | `omtsf-cli` |
| `petgraph` | Directed graph construction, traversal, path queries | `omtsf-core` |

Additional dependencies (e.g., `chrono` for date validation, `rand` for `file_salt` generation) will be specified per-module.

---

## 6. File Size Limits

The CLI loads `.omts` files entirely into memory. To prevent resource exhaustion from untrusted input, the CLI enforces a configurable maximum file size.

- **Default limit:** 256 MB
- **Override:** `--max-file-size <bytes>` flag or `OMTSF_MAX_FILE_SIZE` environment variable
- **Behavior on exceed:** reject with a clear error message before parsing begins

This is a CLI concern, not a library concern. `omtsf-core` accepts parsed data structures and does not enforce file-level size limits.

---

## 7. Conventions

- **stdout:** Data output (merged files, subgraphs, inspect summaries, init scaffolds).
- **stderr:** Diagnostics (validation findings, progress messages, errors).
- **Exit codes:** `0` = success. Non-zero = failure. Specific non-zero codes to be defined per-command.
- **File arguments:** All commands that accept files accept `-` for stdin.

---

## 8. Document Roadmap

This overview is the first of several specification documents for the CLI and library:

| Document | Scope |
|----------|-------|
| `overview.md` | This document. High-level architecture, commands, workspace layout. |
| `data-model.md` | Rust type definitions for nodes, edges, header, identifiers, labels. Serde mapping from JSON. |
| `validation.md` | Validation engine architecture. Rule registry. L1/L2/L3 implementation. Diagnostic types. |
| `merge.md` | Merge engine. Identity predicates, candidate detection, property resolution, `same_as` handling. |
| `redaction.md` | Selective disclosure engine. Scope filtering, boundary ref generation, sensitivity enforcement. |
| `graph-engine.md` | Graph construction from flat adjacency list. petgraph integration. Query algorithms (reach, path, subgraph). |
| `diff.md` | Structural diff algorithm. Node/edge matching strategy. Output format. |
| `cli-interface.md` | Detailed command specifications: all flags, argument parsing, output formatting, exit codes. |
