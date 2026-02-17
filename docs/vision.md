# OMTSF Vision

Open Multi-Tier Supply Format.

## Problem

Supply chain data is trapped. Every organization holds a partial view of a shared network, encoded in proprietary formats, internal schemas, and spreadsheets. There is no common way to export a supply network and hand it to another party such that both sides can read, validate, and merge it without manual translation.

Regulations increasingly require multi-tier visibility. Companies cannot comply with what they cannot see. The tooling exists to analyze supply chains, but the data never reaches it in a usable form.

The missing piece is not another platform. It is a file format.

## What OMTSF Is

OMTSF is an open exchange format for supply chain data. It represents a supply network as a directed graph of typed nodes and typed edges. The exact node and edge types are spec decisions, but the model must be expressive enough to represent the actors in a supply chain (organizations, facilities), the goods that flow through it, and the relationships between them. Goods are first-class objects in the graph, not attributes on edges. This allows the same network to be segmented by what flows through it — a single factory may supply cocoa to one buyer and sugar to another, and those are distinct supply chains that share infrastructure.

An `.omts` file is a self-contained document. It carries its own metadata, its own schema version, and enough structure to be validated without external context. It is designed to be exported from one system, handed to another party, and imported into a different system without data loss or ambiguity.

OMTSF is not a database, not an API, and not a platform. It is a file you can put in an email attachment, store in version control, or drop into a regulatory submission.

## Design Principles

These are the invariants of the project. They apply to the spec, the tooling, and any future work.

**The file is a graph, stored flat.** The data model is a directed graph, but the serialization is a flat adjacency list: a list of nodes and a list of edges. No nesting. This makes merging files from different parties a matter of concatenating and deduplicating lists rather than reconciling trees.

**Validation is not optional.** A valid `.omts` file must pass structural and semantic checks. Edges must reference existing nodes. Identifiers must be well-formed. The format is strict by design because the entire point is that a recipient can trust the structure without inspecting every field.

**The format is the contract.** If two systems both produce valid `.omts` files, those files are compatible. No out-of-band agreements, no bilateral mapping tables, no integration projects. Conformance to the spec is sufficient for interoperability.

**Data stays local.** The tooling must never require sending supply chain data to a remote service. Validation, visualization, and analysis all run locally. This is a hard requirement because supply chain data is commercially sensitive and often subject to confidentiality obligations.

**The spec is open and vendor-neutral.** No single company owns the format. The specification and reference implementation are open source. Adoption depends on trust, and trust requires neutrality.

## Technical Direction

**Rust reference implementation.** The canonical library for reading, writing, and validating `.omts` files is written in Rust. Rust is chosen because the library must parse untrusted input safely, run efficiently on large graphs, and compile to WebAssembly for browser-based tooling.

**Serialization format is a spec concern.** The vision defines the data model (flat adjacency list of nodes and edges), not the encoding. The serialization format will be chosen during spec work, weighing human readability, disk efficiency, tooling support, and compression. The reference implementation must support whatever formats the spec defines.

**CLI as the primary interface.** The first tool is a command-line validator. It reads an `.omts` file, runs all validation checks, and reports errors. This is the minimum viable tool and the foundation for CI/CD integration, scripting, and automated pipelines.

**WASM for the browser.** The same Rust library compiles to WebAssembly, enabling browser-based tools that process files entirely on the client. No server, no uploads, no data leaving the user's machine.

**Graph analysis in the runtime.** The file format is flat, but the library loads it into a directed graph in memory. This enables queries like reachability (does a disruption at node X affect node Y?), path enumeration, and n-tier visibility. The graph is a runtime concern, not a file format concern.

## Scope Boundaries

The following are explicitly out of scope for the format itself. They may become separate projects or specifications later.

- **Real-time data exchange.** OMTSF is a document format, not a streaming protocol.
- **Access control and permissions.** The format does not encode who is allowed to see what. That is a concern for the systems that produce and consume the files.
- **Domain-specific field definitions.** The spec will define the structural schema (what a node is, what an edge is, what metadata a file carries). The exact set of domain fields (sustainability scores, risk ratings, compliance certifications) will be defined in separate specifications that extend the core schema.
- **Merge conflict resolution.** The flat structure makes merging easy, but conflict resolution (two parties disagree about the same node) is a tooling problem, not a format problem.

## Project Structure

The project has two deliverables that evolve together:

1. **The specification.** A versioned document that defines the `.omts` file format. It is the source of truth for what constitutes a valid file.
2. **The reference implementation (`omtsf-rs`).** A Rust library and CLI that reads, writes, validates, and analyzes `.omts` files. It serves as both a usable tool and a conformance reference for other implementations.

## What Comes Next

The immediate next steps are detailed specifications for:

- The serialization format (encoding, compression, human-readable vs binary tradeoffs)
- The `.omts` file schema (header, node types, edge types, metadata, extensibility model)
- The validation rule set (structural integrity, identifier formats, semantic constraints)
- The merge semantics (how two files describing overlapping networks combine)
- The versioning and migration strategy (how the schema evolves without breaking consumers)
