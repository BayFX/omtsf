# OMTS: Open Multi-Tier Supply Format

A graph-based file format for encoding multi-tier supply chain networks. An `.omts` file stores organizations, facilities, goods, persons, attestations, and consignments as typed nodes, connected by typed edges representing supply relationships, corporate hierarchy, operations, and attestations. Files can be validated against a formal schema, merged across organizational boundaries using composite external identifiers, queried for upstream/downstream reachability, and selectively redacted before sharing.

OMTS is designed for tier-n supply chain visibility: store a complete supplier graph from tier 1 through tier n, query reachability and paths between any two entities, merge partial graphs from different parties into a unified view, and extract subgraphs by selector or neighborhood.

**Version:** 0.0.1 (draft)
**License:** Specs [CC-BY-4.0](spec/LICENSE) | Code [Apache-2.0](LICENSE)

### Example

An `.omts` file is a self-contained, JSON-compatible document (also serializable as CBOR). Nodes are entities, edges are relationships:

```json
{
  "omts_version": "0.0.1",
  "snapshot_date": "2026-02-19",
  "file_salt": "a1b2c3d4...64 hex chars...",
  "nodes": [
    { "id": "org-acme", "type": "organization", "name": "Acme Corp",
      "external_ids": [{ "scheme": "lei", "value": "5493006MHB84DD0ZWV18" }] },
    { "id": "org-bolt", "type": "organization", "name": "Bolt Fasteners Ltd" },
    { "id": "fac-bolt", "type": "facility", "name": "Bolt Sheffield Plant",
      "geo": { "lat": 53.38, "lon": -1.47 } }
  ],
  "edges": [
    { "id": "e-001", "type": "supplies", "source": "org-bolt", "target": "org-acme",
      "properties": { "commodity": "7318.15", "tier": 1 } },
    { "id": "e-002", "type": "operates", "source": "fac-bolt", "target": "org-bolt" }
  ]
}
```

The format is designed around five principles:

- **The file is a graph, stored flat.** Nodes and edges are flat lists with no nesting. Merging files from different parties is list concatenation and deduplication, not tree reconciliation.
- **Validation is not optional.** A valid `.omts` file passes structural and semantic checks. Edges must reference existing nodes. Identifiers must be well-formed. Recipients can trust the structure without inspecting every field.
- **The format is the contract.** If two systems both produce valid `.omts` files, those files are compatible. No bilateral mapping tables, no integration projects.
- **Data stays local.** Validation, analysis, and transformation all run locally. Supply chain data never needs to leave the machine.
- **The spec is open and vendor-neutral.** No single company owns the format. The specification and reference implementation are open source.

## The Data Model

### Node Types

| Type | Description |
|------|-------------|
| `organization` | A legal entity: company, NGO, government body |
| `facility` | A physical location: factory, warehouse, farm, mine, port |
| `good` | A product, material, commodity, or service |
| `person` | A natural person (beneficial owner, director) |
| `attestation` | A certificate, audit result, or due diligence statement |
| `consignment` | A batch, lot, or shipment of goods |
| `boundary_ref` | A placeholder replacing a redacted node (preserves graph structure) |

### Edge Types

| Category | Types |
|----------|-------|
| Corporate hierarchy | `ownership`, `operational_control`, `legal_parentage`, `former_identity`, `beneficial_ownership` |
| Supply relationships | `supplies`, `subcontracts`, `sells_to`, `distributes`, `brokers`, `tolls` |
| Operational links | `operates`, `produces`, `composed_of` |
| Attestation | `attested_by` |

Nodes carry external identifiers (LEI, DUNS, GLN, VAT, national registrations) that enable cross-file entity resolution during merge.

## Specifications

| Spec | Title | Scope |
|------|-------|-------|
| [SPEC-001](spec/graph-data-model.md) | Graph Data Model | File structure, node types, edge types, validation rules |
| [SPEC-002](spec/entity-identification.md) | Entity Identification | Identifier schemes, composite identifiers, check digit validation |
| [SPEC-003](spec/merge-semantics.md) | Merge Semantics | Multi-file merge procedure, identity predicates, `same_as` edges |
| [SPEC-004](spec/selective-disclosure.md) | Selective Disclosure | Sensitivity levels, redaction rules, boundary references |
| [SPEC-005](spec/erp-integration.md) | ERP Integration | Export mappings for SAP, Oracle, Dynamics 365 (informative) |
| [SPEC-006](spec/standards-mapping.md) | Standards Mapping | Regulatory alignment: EUDR, LkSG, CSDDD, CBAM, AMLD (informative) |
| [SPEC-007](spec/serialization-bindings.md) | Serialization Bindings | JSON and CBOR encoding, zstd compression, encoding detection |

## Rust Reference Implementation (`omts-rs`)

The canonical tooling for working with `.omts` files is a Rust library and CLI. It parses, validates, merges, redacts, diffs, and queries supply chain graphs. The core library compiles to WebAssembly for browser-based use.

### CLI Commands

| Command | Description |
|---------|-------------|
| `omts validate <file>` | Validate against the spec (L1/L2/L3) |
| `omts merge <file>...` | Merge two or more files into a single graph |
| `omts redact <file> --scope <s>` | Redact for a target disclosure scope |
| `omts diff <a> <b>` | Structural diff between two files |
| `omts inspect <file>` | Print summary statistics |
| `omts convert <file>` | Re-serialize, convert between JSON/CBOR, compress with zstd |
| `omts query <file>` | Search nodes/edges by type, label, identifier, jurisdiction, name |
| `omts reach <file> <node>` | List all reachable nodes from a source (upstream/downstream) |
| `omts path <file> <from> <to>` | Find paths between two nodes |
| `omts subgraph <file> [nodes]...` | Extract induced subgraph by node IDs and/or selectors |
| `omts import <file>` | Import from Excel (`.xlsx`) to `.omts` |
| `omts export <file> -o <out>` | Export from `.omts` to Excel (`.xlsx`) |
| `omts init` | Scaffold a new minimal `.omts` file |

All commands that read `.omts` files accept `-` to read from stdin. Use `-f json` for machine-readable output or `-f human` (default) for colored terminal output.

#### Import and Export

`import` reads an Excel workbook (auto-detecting the template variant) and produces a valid `.omts` file. `export` writes an `.omts` graph to Excel in either the full multi-sheet template (`--output-format excel`) or the simplified single-sheet supplier list (`--output-format excel-supplier-list`).

```bash
omts import suppliers.xlsx -o supply-chain.omts
omts export supply-chain.omts -o full-export.xlsx
omts export supply-chain.omts --output-format excel-supplier-list -o suppliers.xlsx
```

#### Query and Subgraph

`query` finds nodes and edges matching property predicates. `subgraph` extracts the matched nodes (plus their interconnecting edges) as a new `.omts` file. Both commands share a common selector syntax.

```bash
omts query supply-chain.omts --node-type organization --jurisdiction DE
omts query supply-chain.omts --label risk-tier=high --count
omts subgraph supply-chain.omts --identifier lei --expand 1 -o subset.omts
```

Selectors: `--node-type`, `--edge-type`, `--label KEY[=VALUE]`, `--identifier SCHEME[:VALUE]`, `--jurisdiction CC`, `--name PATTERN`.

#### Graph Traversal

`reach` lists all nodes reachable from a starting node (configurable direction and depth). `path` finds simple paths between two nodes (shortest first).

```bash
omts reach supply-chain.omts org-acme --direction both --depth 3
omts path supply-chain.omts org-acme fac-plant-01
```

#### Merge, Redact, and Convert

`merge` combines files from different sources using composite external identifiers for entity resolution. `redact` strips sensitive data for a target disclosure scope (`public`, `partner`, `internal`). `convert` normalizes or transcodes between JSON and CBOR with optional zstd compression.

```bash
omts merge supplier-a.omts supplier-b.omts > combined.omts
omts redact combined.omts --scope public > shareable.omts
omts convert combined.omts --to cbor --compress > compact.omts.cbor
```

### Excel Support

OMTS provides two Excel template variants for interoperability with spreadsheet-based workflows:

**Rich Template** (multi-sheet) — Full-fidelity round-trip for all node types, edge types, identifiers, and attestations. Suited for data engineers and tooling integration. Twelve sheets mirror the OMTS graph structure exactly.

**Simplified Template** ("Supplier List", single-sheet) — A flat table of tier-1/2/3 suppliers that auto-generates organization nodes and `supplies` edges on import. Suited for procurement teams filling out supplier data manually. Metadata is embedded in rows 1-2; data rows start at row 5.

On import, the template variant is auto-detected by inspecting sheet names. On export, use `--output-format excel` for the rich template or `--output-format excel-supplier-list` for the simplified template.

Template files and a detailed column reference are in [`templates/excel/`](templates/excel/README.md).

### Key Capabilities

- **Three-level validation.** L1 (structural integrity), L2 (semantic completeness), L3 (cross-reference enrichment including cycle detection).
- **Merge with entity resolution.** Combines files from different sources using composite external identifiers. Handles overlapping and disjoint graphs.
- **Selective redaction.** Replaces sensitive nodes with boundary references at `public` or `partner` disclosure scopes. Person nodes and confidential identifiers are stripped automatically.
- **Graph queries.** Reachability analysis, shortest path, all-paths enumeration, ego graphs, and induced subgraph extraction. Supports edge-type filtering and directional traversal.
- **Selector-based queries.** Query nodes and edges by type, label, identifier, jurisdiction, or name pattern. Extract subchains by selector match with configurable neighborhood expansion.
- **Structural diff.** Compares two files and reports added, removed, and modified nodes and edges. Supports type-based and field-based filtering.
- **Excel import/export.** Round-trip between `.omts` and Excel workbooks. Full multi-sheet template for data engineers; simplified single-sheet supplier list for procurement teams. Template variant is auto-detected on import.
- **CBOR and compression.** CBOR encoding produces files 21% smaller than JSON and decodes 26-36% faster. Zstd compression supported. Automatic encoding detection on load.
- **WASM-compatible core.** The `omts-core` library has no I/O dependencies and compiles to `wasm32-unknown-unknown` for client-side browser tooling.

### Performance

Benchmarked on supply chain graphs from 141 elements (28 KB) to 2.2 million elements (500 MB):

| Operation | Small (141 elem) | Large (5.9K elem) | Huge (2.2M elem) |
|-----------|------------------|--------------------|-------------------|
| JSON parse | 162 us | 11.4 ms | 4.53 s |
| CBOR decode | 163 us | 8.49 ms | 3.92 s |
| Graph build | 29 us | 1.40 ms | 1.59 s |
| Validate L1+L2+L3 | 59 us | 3.80 ms | 5.01 s |
| Merge (disjoint) | 1.12 ms | 82.6 ms | - |
| Structural diff | 316 us | 17.4 ms | - |
| Reachability | 4.5 us | 234 us | 455 ms |
| Selector match | 991 ns | 68.1 us | 82.5 ms |

CBOR files are 21% smaller than JSON and decode 26-36% faster. Full
validation of a 500 MB graph completes in roughly 5 seconds.

### Build

```
cd omts-rs
cargo build --release
```

See [omts-rs/README.md](omts-rs/README.md) for the full command reference.

## Repository Layout

```
spec/                 Normative and informative specifications
schema/               JSON Schema for .omts files
tests/fixtures/       Validation test fixtures (.omts)
templates/excel/      Excel import/export templates
omts-rs/             Rust reference implementation (CLI + library)
  crates/omts-core/    Core library (parsing, validation, merge, graph, WASM-safe)
  crates/omts-cli/     CLI binary
  crates/omts-excel/   Excel import/export library
  crates/omts-wasm/    WASM bindings
  crates/omts-bench/   Benchmarks and supply chain generator
docs/                 Vision, governance, reviews, roadmap
  usecases/            Use case documentation
```

## Background

Supply chain data is typically spread across proprietary formats, internal schemas, and spreadsheets, with each organization holding a partial view of the network. There is no standard way to export a supply network and hand it to another party such that both sides can read, validate, and merge it without manual translation.

Regulations like EUDR, LkSG/CSDDD, CBAM, and beneficial ownership directives require structured multi-tier visibility into upstream supply chains. OMTS provides a common file format to support this.

### Use Cases

- **EUDR due diligence.** Model origin cooperatives, plantations with geolocation, and Due Diligence Statements as attestation nodes.
- **LkSG/CSDDD multi-tier mapping.** Document supplier hierarchies across tiers with risk assessments attached as attestation nodes.
- **Multi-ERP consolidation.** Export supplier masters from SAP, Oracle, and Dynamics as `.omts` files. Merge them using composite identifiers (LEI, DUNS, VAT numbers) to produce a single deduplicated supplier graph.
- **Beneficial ownership transparency.** Map corporate structures with ownership percentages, legal parentage, and person nodes for UBOs, governed by privacy rules.
- **CBAM embedded emissions.** Track installation-level emissions data on consignment nodes linked to producing facilities.
- **Selective disclosure.** Share supply chain structure with auditors or partners while redacting commercially sensitive identities behind salted-hash boundary references.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

Specifications are licensed under [CC-BY-4.0](spec/LICENSE). Code is licensed under [Apache-2.0](LICENSE).
