# OMTSF -- Open Multi-Tier Supply Format

An open exchange format for supply chain graph data. OMTSF represents supply networks as directed graphs of typed nodes and typed edges, serialized as self-contained `.omts` (JSON) files that can be validated, merged, and shared across organizational boundaries.

**Version:** 0.0.1 (draft)
**License:** Specs [CC-BY-4.0](spec/LICENSE) | Code [Apache-2.0](LICENSE)

## Repository Layout

```
spec/                 Normative and informative specifications
  graph-data-model.md   SPEC-001  Graph structure, node types, edge types, validation
  entity-identification.md  SPEC-002  Identifier schemes and composite identifiers
  merge-semantics.md   SPEC-003  Multi-file merge procedure and same_as edges
  selective-disclosure.md   SPEC-004  Sensitivity levels, redaction, boundary references
  erp-integration.md   SPEC-005  ERP export mappings (informative)
  standards-mapping.md SPEC-006  Regulatory and standards alignment (informative)
schema/               JSON Schema for .omts files
tests/fixtures/       Validation test fixtures
usecases/             Example use case descriptions
omtsf-rs/             Rust CLI for validation, merge, and redaction
docs/                 Vision, governance, reviews, roadmap
```

## Quick Start

An `.omts` file is a JSON document containing a graph of nodes (organizations, facilities, goods, persons, consignments, attestations) and typed edges (supply relationships, corporate hierarchy, attestations). See [SPEC-001](spec/graph-data-model.md) for the full schema.

```json
{
  "omtsf_version": "0.0.1",
  "snapshot_date": "2026-02-19",
  "file_salt": "a1b2c3d4e5f67890a1b2c3d4e5f67890a1b2c3d4e5f67890a1b2c3d4e5f67890",
  "nodes": [
    {"id": "org-acme", "type": "organization", "name": "Acme Corp",
     "external_ids": [{"scheme": "lei", "value": "5493006MHB84DD0ZWV18"}]}
  ],
  "edges": [
    {"id": "edge-001", "type": "supplies", "source": "org-bolt", "target": "org-acme",
     "properties": {"commodity": "7318.15"}}
  ]
}
```

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).
