# omtsf-core Technical Specification: Data Model

**Status:** Draft
**Date:** 2026-02-19

---

## 1. Scope

This document specifies the Rust type definitions in `omtsf-core` that represent the OMTSF graph data model. It covers the complete type hierarchy for files, nodes, edges, identifiers, and labels; the serde strategy for JSON round-trip fidelity; the modeling decisions for graph-local references; and WASM compatibility constraints.

All section references of the form `SPEC-001 Section N` refer to the OMTSF Graph Data Model specification. References of the form `SPEC-002 Section N` refer to the Entity Identification specification.

---

## 2. Top-Level File Type

The root type corresponds to the JSON top-level object (SPEC-001 Section 2).

```rust
pub struct OmtsFile {
    pub omtsf_version: SemVer,
    pub snapshot_date: CalendarDate,
    pub file_salt: FileSalt,
    pub disclosure_scope: Option<DisclosureScope>,
    pub previous_snapshot_ref: Option<String>,
    pub snapshot_sequence: Option<u64>,
    pub reporting_entity: Option<NodeId>,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,

    /// All JSON fields not recognized by this version of omtsf-core.
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}
```

The `extra` field is the mechanism for round-trip preservation of unknown fields (SPEC-001 Section 2.2). Every struct in the type hierarchy carries an analogous `#[serde(flatten)]` catch-all. This is the single most important serde decision in the crate: we MUST NOT use `#[serde(deny_unknown_fields)]` anywhere.

---

## 3. Newtype Wrappers

Validated strings are represented as newtypes. Construction goes through fallible `TryFrom<&str>` or a `parse`-style method; the inner value is not publicly mutable. This prevents invalid data from entering the type system after initial parsing.

```rust
/// Semantic version string: `MAJOR.MINOR.PATCH`.
pub struct SemVer(String);

/// ISO 8601 calendar date: `YYYY-MM-DD`. No week dates, no ordinal dates.
pub struct CalendarDate(String);

/// Exactly 64 lowercase hex characters. Regex: `^[0-9a-f]{64}$`.
pub struct FileSalt(String);

/// Non-empty, file-unique string. Used for node `id` and edge `id`.
pub struct NodeId(String);

/// Same type as NodeId but semantically distinct in documentation.
pub type EdgeId = NodeId;

/// ISO 3166-1 alpha-2 country code. Two uppercase ASCII letters.
pub struct CountryCode(String);
```

All newtypes implement `Deref<Target = str>` for ergonomic read access, `Display`, `Serialize`, and `Deserialize` (with validation in the `Deserialize` impl). They do not implement `DerefMut`.

**Why strings, not structured types?** `CalendarDate` wraps a `String`, not a `chrono::NaiveDate`. The spec mandates the `YYYY-MM-DD` format exactly, and round-trip fidelity requires emitting exactly what was parsed. A `chrono` type would normalize the representation and could silently alter values like `2026-02-01` vs. `2026-2-1`. Validation (confirming the string is a real date) happens in the validation engine, not in the type constructor. The type constructor only enforces the regex shape.

`SemVer` follows the same rationale. It validates the `MAJOR.MINOR.PATCH` shape but does not parse into three integers at construction time. A `fn major(&self) -> u32` accessor method parses on demand.

---

## 4. Enums

### 4.1 Disclosure Scope

```rust
#[serde(rename_all = "snake_case")]
pub enum DisclosureScope {
    Internal,
    Partner,
    Public,
}
```

### 4.2 Node Type Tag

Node types use an internally tagged enum (see Section 5). The tag values are snake_case strings matching the JSON `type` field.

```rust
#[serde(rename_all = "snake_case")]
pub enum NodeType {
    Organization,
    Facility,
    Good,
    Person,
    Attestation,
    Consignment,
    BoundaryRef,
}
```

Extension node types (reverse-domain notation, SPEC-001 Section 8.1) do not appear in this enum. They are handled by a catch-all variant (see Section 5).

### 4.3 Edge Type Tag

```rust
#[serde(rename_all = "snake_case")]
pub enum EdgeType {
    Ownership,
    OperationalControl,
    LegalParentage,
    FormerIdentity,
    BeneficialOwnership,
    Supplies,
    Subcontracts,
    Tolls,
    Distributes,
    Brokers,
    Operates,
    Produces,
    ComposedOf,
    SellsTo,
    AttestedBy,
    SameAs,
}
```

### 4.4 Other Enums

Each property-level enum follows the same `#[serde(rename_all = "snake_case")]` convention. Representative examples:

```rust
pub enum AttestationType {
    Certification, Audit, DueDiligenceStatement, SelfDeclaration, Other,
}

pub enum Confidence {
    Verified, Reported, Inferred, Estimated,
}

pub enum Sensitivity {
    Public, Restricted, Confidential,
}

pub enum VerificationStatus {
    Verified, Reported, Inferred, Unverified,
}

pub enum OrganizationStatus {
    Active, Dissolved, Merged, Suspended,
}

pub enum AttestationOutcome {
    Pass, ConditionalPass, Fail, Pending, NotApplicable,
}

pub enum AttestationStatus {
    Active, Suspended, Revoked, Expired, Withdrawn,
}

pub enum RiskSeverity {
    Critical, High, Medium, Low,
}

pub enum RiskLikelihood {
    VeryLikely, Likely, Possible, Unlikely,
}

pub enum EmissionFactorSource {
    Actual, DefaultEu, DefaultCountry,
}

pub enum ControlType {
    Franchise, Management, Tolling, LicensedManufacturing, Other,
}
```

---

## 5. Node Modeling

### 5.1 Decision: Flat Struct with Type-Specific Payload

Nodes in JSON are flat objects with a `type` discriminator and a variable set of fields depending on that type. Two Rust representations were considered:

1. **Internally tagged enum** (`#[serde(tag = "type")]`). Each variant is a distinct struct. This gives maximal type safety but makes the `extra` catch-all for unknown fields difficult: serde's internally tagged representation consumes the full JSON object per variant, and extension node types (arbitrary strings) require a fallback variant holding raw `serde_json::Value`.

2. **Flat struct with a type tag and optional fields.** One `Node` struct carries all possible properties as `Option<T>`. Simpler serde story, trivial `#[serde(flatten)]` for extras. Downside: no compile-time enforcement that `attestation_type` is present on attestation nodes.

We use approach (2). Validation that the correct fields are present for a given node type is the validation engine's responsibility, not the type system's. This matches the spec's layered validation model: deserialization is not validation. A file that deserializes successfully is not necessarily valid; it must still pass L1 checks.

```rust
pub struct Node {
    pub id: NodeId,

    #[serde(rename = "type")]
    pub node_type: NodeTypeTag,

    // Universal optional fields (SPEC-001 Section 8)
    pub identifiers: Option<Vec<Identifier>>,
    pub data_quality: Option<DataQuality>,
    pub labels: Option<Vec<Label>>,

    // organization
    pub name: Option<String>,
    pub jurisdiction: Option<CountryCode>,
    pub status: Option<OrganizationStatus>,
    pub governance_structure: Option<GovernanceStructure>,

    // facility
    pub operator: Option<NodeId>,
    pub address: Option<String>,
    pub geo: Option<serde_json::Value>,

    // good
    pub commodity_code: Option<String>,
    pub unit: Option<String>,

    // person
    pub role: Option<String>,

    // attestation
    pub attestation_type: Option<AttestationType>,
    pub standard: Option<String>,
    pub issuer: Option<String>,
    pub valid_from: Option<CalendarDate>,
    pub valid_to: Option<Option<CalendarDate>>,
    pub outcome: Option<AttestationOutcome>,
    pub attestation_status: Option<AttestationStatus>,
    pub reference: Option<String>,
    pub risk_severity: Option<RiskSeverity>,
    pub risk_likelihood: Option<RiskLikelihood>,

    // consignment
    pub lot_id: Option<String>,
    pub quantity: Option<f64>,
    // unit: already declared above (shared with good)
    pub production_date: Option<CalendarDate>,
    pub origin_country: Option<CountryCode>,
    pub direct_emissions_co2e: Option<f64>,
    pub indirect_emissions_co2e: Option<f64>,
    pub emission_factor_source: Option<EmissionFactorSource>,
    pub installation_id: Option<NodeId>,

    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}
```

### 5.2 The NodeTypeTag Type

The `type` field must accept both known enum variants and arbitrary extension strings.

```rust
pub enum NodeTypeTag {
    Known(NodeType),
    Extension(String),
}
```

A custom `Deserialize` impl attempts to match against `NodeType` variants first. If no variant matches and the string contains a dot (extension convention), it deserializes as `Extension`. If the string contains no dot and is not a known type, it still deserializes as `Extension` -- rejection is a validation concern, not a deserialization concern.

### 5.3 The `valid_to` Null vs. Absent Distinction

The spec assigns distinct meaning to `"valid_to": null` (no expiration, explicitly stated) versus the field being absent (not provided). The Rust type `Option<Option<CalendarDate>>` captures this:

- Field absent in JSON: outer `Option` is `None`
- Field present as `null`: outer is `Some`, inner is `None`
- Field present with a value: `Some(Some(date))`

A custom serde deserializer with `#[serde(default, deserialize_with = "nullable_field")]` handles this. The serializer skips the field entirely when the outer option is `None` and writes `null` when the inner option is `None`.

---

## 6. Edge Modeling

### 6.1 Structure

Edges have four structural fields at the top level and all other properties inside a `properties` wrapper (SPEC-001 Section 2.1).

```rust
pub struct Edge {
    pub id: EdgeId,

    #[serde(rename = "type")]
    pub edge_type: EdgeTypeTag,

    pub source: NodeId,
    pub target: NodeId,
    pub identifiers: Option<Vec<Identifier>>,
    pub properties: EdgeProperties,

    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}
```

### 6.2 EdgeProperties

Like nodes, edge properties use a flat struct rather than a per-edge-type enum. All type-specific properties are `Option<T>`. The `data_quality` and `labels` fields live here, not on the top-level `Edge`, matching the JSON serialization convention.

```rust
pub struct EdgeProperties {
    pub data_quality: Option<DataQuality>,
    pub labels: Option<Vec<Label>>,

    // Temporal (most edge types)
    pub valid_from: Option<CalendarDate>,
    pub valid_to: Option<Option<CalendarDate>>,

    // ownership, beneficial_ownership
    pub percentage: Option<f64>,
    pub direct: Option<bool>,

    // operational_control, beneficial_ownership
    pub control_type: Option<serde_json::Value>,

    // legal_parentage
    pub consolidation_basis: Option<ConsolidationBasis>,

    // former_identity
    pub event_type: Option<EventType>,
    pub effective_date: Option<CalendarDate>,
    pub description: Option<String>,

    // supplies, subcontracts, brokers, sells_to
    pub commodity: Option<String>,
    pub contract_ref: Option<String>,
    pub volume: Option<f64>,
    pub volume_unit: Option<String>,
    pub annual_value: Option<f64>,
    pub value_currency: Option<String>,
    pub tier: Option<u32>,
    pub share_of_buyer_demand: Option<f64>,

    // distributes
    pub service_type: Option<ServiceType>,

    // composed_of
    pub quantity: Option<f64>,
    pub unit: Option<String>,

    // attested_by
    pub scope: Option<String>,

    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}
```

### 6.3 EdgeTypeTag

Mirrors `NodeTypeTag`: known variants plus an extension fallback.

```rust
pub enum EdgeTypeTag {
    Known(EdgeType),
    Extension(String),
}
```

---

## 7. Shared Types

### 7.1 Identifier

Corresponds to the identifier record (SPEC-002 Section 3).

```rust
pub struct Identifier {
    pub scheme: String,
    pub value: String,
    pub authority: Option<String>,
    pub valid_from: Option<CalendarDate>,
    pub valid_to: Option<Option<CalendarDate>>,
    pub sensitivity: Option<Sensitivity>,
    pub verification_status: Option<VerificationStatus>,
    pub verification_date: Option<CalendarDate>,

    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}
```

The `scheme` field is a plain `String`, not an enum. The spec defines core schemes (`lei`, `duns`, `gln`, `nat-reg`, `vat`, `internal`) and permits arbitrary extension schemes (SPEC-002 Section 5.2). Scheme-specific format validation belongs in the validation engine.

### 7.2 DataQuality

```rust
pub struct DataQuality {
    pub confidence: Option<Confidence>,
    pub source: Option<String>,
    pub last_verified: Option<CalendarDate>,

    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}
```

### 7.3 Label

```rust
pub struct Label {
    pub key: String,
    pub value: Option<String>,

    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}
```

### 7.4 Geo

The `geo` field on facility nodes accepts either `{lat, lon}` or arbitrary GeoJSON (SPEC-001 Section 4.2). It is typed as `serde_json::Value` on the `Node` struct. A helper method provides typed access:

```rust
pub enum Geo {
    Point { lat: f64, lon: f64 },
    GeoJson(serde_json::Value),
}

impl Node {
    pub fn geo_parsed(&self) -> Option<Result<Geo, GeoParseError>> { .. }
}
```

This avoids forcing a schema on the `geo` field during deserialization while providing a typed API for consumers that need it.

---

## 8. Serde Strategy

### 8.1 Rename Convention

All structs use `#[serde(rename_all = "snake_case")]`. This is the default JSON key style in OMTSF (e.g., `omtsf_version`, `snapshot_date`, `file_salt`). The `type` field on nodes and edges uses `#[serde(rename = "type")]` since `type` is a Rust keyword.

### 8.2 Unknown Field Preservation

Every struct carries `#[serde(flatten)] pub extra: serde_json::Map<String, serde_json::Value>`. This ensures that fields added in future spec versions, or fields from extension types, survive a deserialize-serialize round trip without data loss.

The `#[serde(flatten)]` approach has a known performance cost: serde must buffer the entire JSON object to separate known from unknown fields. For files within the advisory size limits (SPEC-001 Section 9.4), this is acceptable. For pathological files (millions of nodes), profiling will determine if a streaming two-pass approach is needed.

### 8.3 Null vs. Absent

Three categories:

1. **Required fields** (e.g., `id`, `name`): typed directly, no `Option`. Deserialization fails if absent.
2. **Optional fields where null is not meaningful** (e.g., `jurisdiction`, `commodity_code`): `Option<T>`, serialized with `#[serde(skip_serializing_if = "Option::is_none")]`.
3. **Optional fields where null carries meaning** (e.g., `valid_to`): `Option<Option<T>>` with a custom deserializer. `None` = absent (skip on serialize), `Some(None)` = explicit null (serialize as `null`), `Some(Some(v))` = present with value.

### 8.4 Custom Deserializers

Custom `Deserialize` implementations are required for:

- `NodeTypeTag` / `EdgeTypeTag`: attempt known enum match, fall back to extension string.
- `SemVer`, `CalendarDate`, `FileSalt`, `CountryCode`: shape validation on deserialize.
- `Option<Option<T>>` fields: distinguish null from absent.

### 8.5 Serialization Order

The spec states that `omtsf_version` MUST be the first key in the top-level JSON object (SPEC-001 Section 2.1). `serde_json` serializes struct fields in declaration order, so the field order in `OmtsFile` is load-bearing. The `extra` flattened map is emitted last.

---

## 9. Graph-Local References

Nodes and edges reference each other by `NodeId` string, not by index or pointer. This matches the JSON representation directly and avoids any need for an index-building pass during deserialization.

After deserialization, the validation engine and graph engine build lookup structures:

- `HashMap<NodeId, usize>` mapping node IDs to their index in the `nodes` vec.
- A `petgraph::DiGraph` for traversal queries.

These derived structures live outside `OmtsFile`. The data model types are pure data; graph topology is a separate concern owned by the graph engine (specified in `graph-engine.md`).

This separation is deliberate. `OmtsFile` is a faithful representation of the JSON document. The graph engine is an interpretation of that document. Keeping them apart means `OmtsFile` can be constructed, serialized, and diffed without ever building a graph.

---

## 10. Owned Data

All types use owned data (`String`, `Vec<T>`, `serde_json::Map`). No lifetime parameters, no borrowed slices.

This is a conscious trade-off. Zero-copy deserialization (`&'de str` fields with `#[serde(borrow)]`) would reduce allocation pressure on large files but introduces lifetime constraints that make the types unusable across async boundaries and difficult to store in long-lived data structures like the graph engine's `petgraph` instance. It also prevents modification of the deserialized tree, which merge and redaction require.

If profiling reveals that allocation during deserialization is a bottleneck for multi-million-node files, the mitigation path is arena allocation (e.g., `bumpalo` with `serde`'s `borrow` feature), not lifetime-infected public types. The public API surface remains owned.

---

## 11. WASM Compatibility

### 11.1 No System Dependencies

Every type in this document is `Send + Sync` under standard Rust rules (all fields are owned, no `Rc`, no `Cell`). However, `Send + Sync` are not meaningful on `wasm32-unknown-unknown` since WASM is single-threaded. The relevant constraint is that nothing in the type definitions or their `Deserialize` / `Serialize` impls touches `std::fs`, `std::net`, `std::process`, or any OS-level API.

The types depend on:
- `serde` (no-std compatible with `alloc` feature)
- `serde_json` (requires `alloc`, no OS dependencies)

No other dependencies are permitted in the data model module.

### 11.2 wasm-bindgen Surface

The `OmtsFile` struct and its children are not directly `#[wasm_bindgen]`-annotated. wasm-bindgen cannot handle complex nested Rust types. Instead, the future `omtsf-wasm` crate will expose a thin JS-facing API that accepts JSON strings, deserializes into these types internally, and returns results as JSON or simple scalar values. The types defined here are the internal representation, not the FFI boundary.

### 11.3 Serialization to/from JS

For WASM consumers that need the full parsed tree on the JS side, `serde-wasm-bindgen` can convert `OmtsFile` to a `JsValue` without an intermediate JSON string. This is a `omtsf-wasm` crate concern and does not affect the type definitions here.

---

## 12. Summary of Key Decisions

| Decision | Rationale |
|----------|-----------|
| Flat struct per node/edge, not per-type enum | Extension types, `#[serde(flatten)]` compatibility, validation is not deserialization |
| `Option<Option<T>>` for null-vs-absent | Spec assigns distinct semantics (SPEC-001 Section 4.5: `valid_to: null` means no expiration) |
| String newtypes with validation-on-construct | Prevents invalid `FileSalt`, `CalendarDate`, etc. from propagating through the system |
| `serde_json::Value` for `geo` and `control_type` | Polymorphic JSON shapes that do not map to a single Rust type |
| `serde_json::Map` catch-all on every struct | Round-trip preservation of unknown fields (SPEC-001 Section 2.2, 11.2) |
| Owned data, no lifetimes | Merge/redaction require mutation; async/graph-engine storage requires `'static` |
| `NodeId` string references, not indices | Direct 1:1 mapping to JSON; index structures are a graph-engine concern |
| No `#[serde(deny_unknown_fields)]` anywhere | Spec requires forward-compatible consumers (SPEC-001 Section 11.2) |
