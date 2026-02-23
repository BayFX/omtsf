# OMTSF Specification: Serialization Bindings

**Spec:** OMTSF-SPEC-007
**Status:** Draft
**Date:** 2026-02-21
**Revision:** 1
**License:** [CC-BY-4.0](LICENSE)
---

## Related Specifications

| Spec | Relationship |
|------|-------------|
| OMTSF-SPEC-001 (Graph Data Model) | **Prerequisite.** Defines the abstract graph model serialized by the bindings in this spec. |
| OMTSF-SPEC-002 (Entity Identification) | Canonical string format (Section 4) is used for hashing operations; encoding-independent by design. |
| OMTSF-SPEC-004 (Selective Disclosure) | Boundary reference hashing (Section 4) operates on logical strings, not serialized bytes. |

---

## 1. Overview

This specification defines two normative serialization encodings for `.omts` files:

- **JSON** (ECMA-404 / RFC 8259) -- the text-based encoding
- **CBOR** (RFC 8949) -- the binary encoding

Both encodings carry the same abstract data model defined in OMTSF-SPEC-001 with identical semantics. A valid `.omts` file MUST be encoded in exactly one of these formats, optionally wrapped in a compression layer (Section 6).

Implementations MUST support at least the JSON encoding (Section 3). CBOR support (Section 4) and compression (Section 6) are defined for implementations that need compact binary transport.

---

## 2. File Encoding Detection

Implementations that accept both encodings MUST detect the format by inspecting the initial bytes of the file:

| First Bytes | Format | Action |
|-------------|--------|--------|
| `0x28 0xB5 0x2F 0xFD` | zstd compressed | Decompress, then re-detect encoding of the decompressed payload |
| `0xD9 0xD9 0xF7` | CBOR (self-describing tag 55799) | Parse as CBOR |
| First non-whitespace byte is `{` (0x7B) | JSON | Parse as JSON |

Detection order matters: check for zstd first, then CBOR, then JSON. If the initial bytes match none of these patterns, the file MUST be rejected with an encoding detection error.

**Whitespace for JSON detection.** The JSON check skips leading whitespace bytes (0x09, 0x0A, 0x0D, 0x20) before testing for `{`.

---

## 3. JSON Serialization Binding

### 3.1 Character Encoding

JSON `.omts` files MUST be encoded as UTF-8 (RFC 3629). A byte order mark (BOM) MUST NOT be present.

### 3.2 Edge Property Wrapper

Edge properties defined in OMTSF-SPEC-001, Sections 5, 6, and 7 MUST be nested inside a `"properties"` object on the edge. The structural fields `id`, `type`, `source`, and `target` are top-level fields on the edge object; all other fields are inside `properties`. Example:

```json
{
  "id": "edge-001",
  "type": "supplies",
  "source": "org-bolt",
  "target": "org-acme",
  "properties": {
    "valid_from": "2023-01-15",
    "commodity": "7318.15"
  }
}
```

The `data_quality` and `labels` fields follow the same placement convention: top-level on nodes, inside `properties` on edges.

### 3.3 First Key Requirement

The first key in the top-level JSON object MUST be `"omtsf_version"`. Consumers MUST NOT reject files where `omtsf_version` is present but not the first key.

### 3.4 Date Representation

All date fields MUST be serialized as JSON strings in `YYYY-MM-DD` format, as required by OMTSF-SPEC-001, Section 2.2.

### 3.5 Null vs. Absent

JSON distinguishes between a key with value `null` and the absence of a key. In `.omts` files:

- An optional field set to `null` (e.g., `"valid_to": null`) means the value is explicitly absent or open-ended.
- An optional field omitted entirely means the producer did not supply the value.

Consumers MUST treat `null` and absent identically for optional fields unless the field's definition specifies distinct semantics (e.g., `valid_to: null` means "no expiration").

### 3.6 Unknown Field Preservation

Consumers performing round-trip processing MUST preserve unknown fields and their values. Unknown fields MUST NOT cause validation failure. This supports forward compatibility (OMTSF-SPEC-001, Section 2.3).

---

## 4. CBOR Serialization Binding

### 4.1 CBOR Profile

`.omts` files encoded in CBOR MUST conform to the following profile of RFC 8949:

- **Self-describing tag.** Encoders SHOULD prepend CBOR tag 55799 (`0xD9 0xD9 0xF7`) to enable format detection (Section 2). Decoders MUST accept files with or without this tag.
- **String keys only.** All map keys MUST be text strings (CBOR major type 3). Integer keys are not permitted.
- **No byte strings for data fields.** All data values that are strings in the abstract model (names, identifiers, dates, hex-encoded values) MUST be encoded as CBOR text strings (major type 3), not byte strings (major type 2). This ensures lossless conversion to JSON.
- **Deterministic key ordering is not required.** Map key order is implementation-defined. Consumers MUST NOT depend on key order.

### 4.2 Type Mapping

| OMTSF Abstract Type | CBOR Encoding |
|---------------------|---------------|
| string | Text string (major type 3) |
| integer | Integer (major type 0 or 1) |
| number (floating-point) | Float (major type 7, IEEE 754) |
| boolean | Simple value true/false (major type 7) |
| null | Simple value null (major type 7, value 22) |
| date (YYYY-MM-DD) | Text string (major type 3) |
| array | Array (major type 4) |
| object / map | Map (major type 5) |

**Dates.** Date fields MUST be encoded as text strings containing the `YYYY-MM-DD` value, NOT as CBOR tag 0 (date/time string) or tag 1 (epoch-based date/time). This avoids ambiguity and ensures round-trip fidelity with JSON.

### 4.3 Edge Property Wrapper

The edge property wrapper applies to CBOR identically to JSON (Section 3.2). Edge logical properties MUST be nested inside a `"properties"` key in the edge map. Structural fields (`id`, `type`, `source`, `target`) are top-level keys in the edge map.

The `data_quality` and `labels` fields follow the same placement convention: top-level on node maps, inside the `"properties"` map on edge maps.

### 4.4 Null vs. Absent

CBOR distinguishes between a key mapped to `null` (simple value 22) and the absence of a key, with the same semantics as JSON (Section 3.5).

### 4.5 Unknown Field Preservation

Consumers performing round-trip processing of CBOR files MUST preserve unknown keys and their values, identical to the JSON requirement (Section 3.6).

### 4.6 First Key Requirement

The JSON first-key requirement (Section 3.3) does not apply to CBOR. Since CBOR map key order is not significant, there is no requirement on key ordering. The `"omtsf_version"` key MUST be present but MAY appear at any position in the top-level map.

---

## 5. Cross-Encoding Conversion Rules

Lossless conversion between JSON and CBOR MUST preserve:

1. **All field names** (map keys), including unknown fields.
2. **All values**, with type mapping per Section 4.2.
3. **Null vs. absent distinction** (Sections 3.5, 4.4).
4. **Array element order** for `nodes`, `edges`, `identifiers`, and `labels` arrays.

Conversion MUST NOT preserve:

- JSON key ordering (CBOR does not guarantee it; JSON output key order is implementation-defined after conversion).
- JSON whitespace or formatting.
- The CBOR self-describing tag 55799 (added or removed per encoder convention).

**Round-trip equivalence.** Two `.omts` files are **logically equivalent** if, after parsing into the abstract model, they contain the same nodes, edges, identifiers, and property values. Implementations that convert between encodings MUST produce logically equivalent output.

---

## 6. Compression Layer

### 6.1 Algorithm

The only supported compression algorithm is **zstd** (Zstandard, RFC 8878).

### 6.2 Layer Architecture

Compression is an outer layer applied after serialization:

```
Abstract model → Serialize (JSON or CBOR) → Compress (zstd)
```

Decompression reverses the process:

```
Decompress (zstd) → Detect encoding (Section 2) → Parse (JSON or CBOR)
```

### 6.3 Detection

Zstd-compressed files are identified by the zstd magic number: `0x28 0xB5 0x2F 0xFD` in the first four bytes. After decompression, the encoding detection procedure (Section 2) is applied to the decompressed payload.

### 6.4 Applicability

Compression is OPTIONAL. Producers MAY compress `.omts` files with zstd. Consumers that support the Extended conformance profile (Section 7) MUST detect and decompress zstd-wrapped files transparently.

### 6.5 Compression Parameters

This specification does not mandate a specific zstd compression level or window size. Producers SHOULD use compression parameters that balance size reduction with decompression speed for their use case. The default zstd compression level is a reasonable starting point.

---

## 7. Conformance Profiles

This specification defines three conformance profiles. Each profile builds on the previous one.

| Profile | Requirements |
|---------|-------------|
| **Minimum** | MUST support JSON encoding (Section 3). MUST detect JSON via the procedure in Section 2. |
| **Full** | Minimum, plus MUST support CBOR encoding (Section 4). MUST implement full encoding detection (Section 2). MUST support lossless JSON-to-CBOR and CBOR-to-JSON conversion (Section 5). |
| **Extended** | Full, plus MUST support zstd compression and decompression (Section 6). |

Implementations MUST declare which profile they conform to. An implementation MAY support a higher profile than required by its use case.

**Interoperability.** When exchanging files between implementations with different conformance profiles, the sender MUST use an encoding supported by the receiver. If the receiver's profile is unknown, JSON (uncompressed) is the safe default.

---

## 8. Hash Operations

### 8.1 Identifier and Boundary Reference Hashing

Hash operations defined in other OMTSF specifications operate on **logical strings**, not on serialized bytes:

- **Boundary reference hashing** (OMTSF-SPEC-004, Section 4) uses the canonical string form of identifiers (OMTSF-SPEC-002, Section 4) concatenated with the file salt. The hash input is independent of whether the file is serialized as JSON or CBOR.
- **Canonical identifier strings** (OMTSF-SPEC-002, Section 4) are defined as text, not as encoding-specific byte sequences.

Implementations MUST produce identical hash values regardless of the serialization encoding used for the file. There is no "canonical serialized form" for hashing purposes.

### 8.2 Content Hash (`file_integrity.content_hash`)

The `file_integrity.content_hash` field, when present, contains a SHA-256 hash (hex-encoded, lowercase) computed over the **canonical content bytes** of the file. The canonical content bytes are defined as follows:

1. Parse the file into the abstract data model (OMTSF-SPEC-001).
2. Re-serialize the abstract model to **JSON** using the following canonical rules:
   - Keys sorted lexicographically (UTF-8 byte order) at every nesting level.
   - No whitespace between tokens (compact serialization).
   - Numbers serialized without trailing zeros (e.g., `51` not `51.0`, but `51.5` not `51.50`).
   - The `file_integrity` object itself MUST be excluded from the serialization before hashing.
3. Compute `SHA-256` over the UTF-8 bytes of the resulting JSON string.

This definition ensures that two logically equivalent files (regardless of original encoding — JSON, CBOR, or compressed) produce the same content hash. Implementations MUST use this canonical JSON form for content hash computation, even when the file is stored as CBOR.

### 8.3 Opt-In Deterministic CBOR Profile

For use cases that require byte-level reproducibility of CBOR output (e.g., content-addressed storage, signature verification), implementations MAY support an opt-in deterministic CBOR encoding profile:

- Map keys sorted lexicographically by UTF-8 byte order (RFC 8949, Section 4.2.1, "length-first" deterministic encoding).
- Preferred serialization of integer and float values (shortest encoding).
- Arrays preserve element order as defined in the abstract model.

This profile is OPTIONAL. Implementations that support deterministic CBOR SHOULD document it in their conformance statement. The default CBOR profile (Section 4.1) does not require deterministic key ordering.
