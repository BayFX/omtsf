# omtsf-cli

Rust command-line tool for working with `.omts` files.

## Planned Commands

```
omtsf validate <file>          Validate an .omts file (L1/L2/L3)
omtsf merge <file>...          Merge multiple .omts files
omtsf redact <file>            Project a redacted subgraph for a given disclosure scope
omtsf inspect <file>           Print summary statistics (node/edge counts, types, identifier schemes)
omtsf diff <a> <b>             Show structural differences between two .omts files
omtsf convert <file>           Convert between serialization formats (JSON, future formats)
```

### `validate`

Runs the three-level validation defined in SPEC-001 Section 9:

- **L1 (Structural Integrity)** -- JSON schema conformance, referential integrity, identifier format.
- **L2 (Completeness)** -- Recommended fields present, external identifiers populated.
- **L3 (Enrichment)** -- Cross-reference checks against external registries (LEI, GLEIF RA list).

Exit code 0 on success, non-zero on failure. Diagnostics to stderr.

### `merge`

Implements the merge procedure from SPEC-003. Accepts two or more `.omts` files, resolves node identity via composite external identifiers, and writes a merged graph to stdout. Honors `same_as` edges and merge-group safety limits.

### `redact`

Applies the selective disclosure rules from SPEC-004. Given a target `disclosure_scope` (`internal`, `partner`, `public`), replaces nodes and edge properties that exceed the scope's sensitivity threshold with `boundary_ref` placeholders.

### `inspect`

Prints a human-readable summary: node counts by type, edge counts by type, identifier scheme coverage, disclosure scope, snapshot date.

### `diff`

Compares two `.omts` files structurally. Reports added/removed/modified nodes and edges by graph-local ID.

### `convert`

Round-trip serialization. Currently JSON-only; placeholder for future format support.

## Build

```
cargo build --release
```

Binary is at `target/release/omtsf`.

## License

Apache-2.0
