# Project Guidelines

## Repository Structure

- `/spec/` — Normative and informative specification documents (main artifact)
  - `graph-data-model.md` (SPEC-001, foundation)
  - `entity-identification.md` (SPEC-002)
  - `merge-semantics.md` (SPEC-003)
  - `selective-disclosure.md` (SPEC-004)
  - `erp-integration.md` (SPEC-005, informative)
  - `standards-mapping.md` (SPEC-006, informative)
- `/omtsf-rs/` — Rust reference implementation (CLI + library)
  - `docs/` — Technical specification for the implementation
  - `README.md` — Command overview and build instructions
- `/docs/` — Reviews, vision, and auxiliary project documentation
  - `vision.md`
  - `reviews/` — Expert panel reviews

## Git Commits

- No `Co-Authored-By` trailer
- Short commit messages, senior engineer style (e.g., "add expert-panel skill", "fix validation edge case")

## Rust Development (omtsf-rs/)

### Quick Reference

| Command | What it does |
|---------|-------------|
| `just` | Type-check the workspace (default) |
| `just fmt` | Format all code |
| `just fmt-check` | Check formatting without changes |
| `just lint` | Run clippy with `-D warnings` |
| `just test` | Run all tests |
| `just build` | Build debug binaries |
| `just wasm-check` | Verify omtsf-core compiles to WASM |
| `just deny` | Run cargo-deny license/advisory checks |
| `just ci` | Full pipeline: fmt-check, lint, test, doc, wasm-check, deny |
| `just pre-commit` | Fast subset: fmt-check, lint, test |

### Workspace Rules

- **No `unsafe`** — `unsafe_code` is denied workspace-wide
- **No `unwrap()` / `expect()` / `panic!()` / `todo!()` / `unimplemented!()`** in production code — use `Result` + `?` instead
- **Exhaustive matches required** — `wildcard_enum_match_arm` is denied; always match every variant
- **No `dbg!()` macro** — remove before committing
- **WASM safety** — `omtsf-core` additionally denies `print_stdout` and `print_stderr`; all I/O belongs in `omtsf-cli`
- Test files may use `#![allow(clippy::expect_used)]` at the file level

### Where to Put Code

| Crate | Purpose | Notes |
|-------|---------|-------|
| `crates/omtsf-core` | Graph model, parsing, validation, merge logic | No I/O, no stdout/stderr, WASM-compatible |
| `crates/omtsf-cli` | CLI interface (`omtsf` binary) | Uses clap, handles all I/O |
| `crates/omtsf-wasm` | WASM bindings | Thin wrapper around omtsf-core |
| `crates/omtsf-core/tests/` | Integration tests for core | `#![allow(clippy::expect_used)]` permitted |
| `tests/fixtures/` | `.omts` test fixture files | JSON format, shared across crates |

### Code Style

- **File size limit**: keep `.rs` files under 800 lines — if longer, split into modules
- **Public interfaces**: `///` doc comments on all public types, traits, functions, methods, and enum variants — write for `cargo doc` readers
- **Inline comments**: only when explaining *why*, never *what* — if the code needs a comment to explain what it does, rewrite the code
- **No commented-out code** — delete it, git has history
- **No section-separator comments** (e.g., `// --- helpers ---`) — use modules instead

### Before Submitting

1. `just pre-commit` passes (fmt-check + lint + test)
2. New public types/functions have doc comments
3. New enum variants are handled in all match arms
4. Error types use `Result<T, E>` — never panic on bad input

### Error Handling

- Define error enums in the module that produces them
- Implement `std::fmt::Display` and `std::error::Error`
- Use `?` for propagation; callers decide how to handle errors
- CLI maps errors to user-friendly messages and exit codes

### Newtype Pattern

Prefer newtypes for domain identifiers to prevent mixing up plain strings:

```rust
pub struct NodeId(String);
pub struct EdgeId(String);
```
