# omtsf-cli Technical Specification: CLI Interface

**Status:** Draft
**Date:** 2026-02-19

---

## 1. Purpose

This document specifies the complete command-line interface for `omtsf-cli`: argument structure, flags, file I/O behavior, output formatting, and exit codes. It is the authoritative reference for the binary surface area. The library API (`omtsf-core`) is specified in separate documents; this document covers only the CLI layer.

All commands follow the form `omtsf <subcommand> [options] [arguments]`.

---

## 2. Global Flags

These flags apply to every subcommand and are defined on the root `Cli` struct.

| Flag | Short | Type | Default | Description |
|------|-------|------|---------|-------------|
| `--format <fmt>` | `-f` | `human` or `json` | `human` | Output format. `human` emits colored, tabular output to stderr and plain text to stdout. `json` emits structured JSON (NDJSON for diagnostics, single object for data). |
| `--quiet` | `-q` | bool | false | Suppress all stderr output except errors. Incompatible with `--verbose`. |
| `--verbose` | `-v` | bool | false | Increase stderr output: show timing, internal rule counts, file metadata. Incompatible with `--quiet`. |
| `--max-file-size <bytes>` | | u64 | 268435456 (256 MB) | Maximum file size in bytes. Also settable via `OMTSF_MAX_FILE_SIZE` env var. CLI flag takes precedence over env var. |
| `--no-color` | | bool | false | Disable ANSI color codes in human output. Also respects `NO_COLOR` env var per <https://no-color.org>. |
| `--help` | `-h` | | | Print help for the command or subcommand. |
| `--version` | `-V` | | | Print `omtsf <version>` and exit. |

### clap Derive Structure (Illustrative)

```rust
#[derive(Parser)]
#[command(name = "omtsf", version, about = "OMTSF reference CLI")]
struct Cli {
    #[command(subcommand)]
    command: Command,

    #[arg(long, short = 'f', default_value = "human", global = true)]
    format: OutputFormat,

    #[arg(long, short = 'q', global = true, conflicts_with = "verbose")]
    quiet: bool,

    #[arg(long, short = 'v', global = true, conflicts_with = "quiet")]
    verbose: bool,

    #[arg(long, global = true, env = "OMTSF_MAX_FILE_SIZE",
          default_value = "268435456")]
    max_file_size: u64,

    #[arg(long, global = true, env = "NO_COLOR")]
    no_color: bool,
}
```

---

## 3. Command Specifications

### 3.1 `omtsf validate <file>`

Validates a single `.omts` file against the OMTSF specification suite.

**Arguments:**
- `<file>` (required) -- Path to an `.omts` file, or `-` for stdin.

**Flags:**
- `--level <n>` -- Maximum validation level to run. `1` = L1 only, `2` = L1+L2 (default), `3` = L1+L2+L3.

**Behavior:** Parses the file, runs the validation engine at the requested level, and emits diagnostics to stderr. Produces no stdout output. Exit code reflects the worst finding severity.

**Exit codes:** 0 = valid (no L1 errors), 1 = validation errors (L1 violations), 2 = parse failure (not valid JSON or missing required fields).

**Examples:**
```
omtsf validate supply-chain.omts
omtsf validate --level 3 supply-chain.omts
cat supply-chain.omts | omtsf validate -
omtsf validate -f json supply-chain.omts 2> findings.ndjson
```

### 3.2 `omtsf merge <file>...`

Merges two or more `.omts` files into a single graph per SPEC-003.

**Arguments:**
- `<file>...` (required, minimum 2) -- Paths to `.omts` files, or `-` for stdin (at most one argument may be `-`).

**Flags:**
- `--strategy <s>` -- Merge strategy: `union` (default) or `intersect`. Controls how non-overlapping nodes are handled.

**Behavior:** Reads all input files, runs L1 validation on each (rejecting any that fail), executes the merge engine, and writes the merged `.omts` to stdout. Diagnostics (merge decisions, identity matches) go to stderr.

**Exit codes:** 0 = success, 1 = merge conflict (unresolvable property collision), 2 = parse/validation failure on any input file.

**Examples:**
```
omtsf merge file-a.omts file-b.omts > merged.omts
omtsf merge --strategy intersect a.omts b.omts c.omts > common.omts
cat remote.omts | omtsf merge - local.omts > combined.omts
```

### 3.3 `omtsf redact <file>`

Produces a redacted copy of a graph for a target disclosure scope per SPEC-004.

**Arguments:**
- `<file>` (required) -- Path to an `.omts` file, or `-` for stdin.

**Flags:**
- `--scope <scope>` (required) -- Target disclosure scope: `public`, `partner`, or `internal`.

**Behavior:** Parses the file, applies redaction rules for the target scope (stripping sensitive identifiers, replacing redacted nodes with `boundary_ref` stubs, omitting sensitive edge properties), sets `disclosure_scope` in the output header, and writes the redacted `.omts` to stdout. Reports redaction statistics (nodes redacted, identifiers stripped, boundary refs generated) to stderr.

**Exit codes:** 0 = success, 1 = redaction error (e.g., scope is less restrictive than existing `disclosure_scope`), 2 = parse/validation failure.

**Examples:**
```
omtsf redact --scope public supply-chain.omts > public.omts
omtsf redact --scope partner internal.omts | omtsf validate -
```

### 3.4 `omtsf inspect <file>`

Prints summary statistics for a graph.

**Arguments:**
- `<file>` (required) -- Path to an `.omts` file, or `-` for stdin.

**Behavior:** Parses the file and prints a summary to stdout including: node count by type, edge count by type, identifier count by scheme, disclosure scope, file version, snapshot date, and reporting entity. In `--format json` mode, emits a single JSON object with these fields.

**Exit codes:** 0 = success, 2 = parse failure.

**Examples:**
```
omtsf inspect supply-chain.omts
omtsf inspect -f json supply-chain.omts | jq .node_counts
```

### 3.5 `omtsf diff <a> <b>`

Computes a structural diff between two `.omts` files.

**Arguments:**
- `<a>` (required) -- Path to the base file, or `-` for stdin.
- `<b>` (required) -- Path to the comparison file (cannot be `-` if `<a>` is `-`).

**Flags:**
- `--ids-only` -- Only report added/removed/changed node and edge IDs, not property-level detail.

**Behavior:** Parses both files, matches nodes and edges by ID, and reports additions, removals, and property changes to stdout. In human mode, output uses `+`/`-` prefix lines (similar to unified diff). In JSON mode, emits a structured diff object.

**Exit codes:** 0 = files are identical, 1 = differences found, 2 = parse failure on either file.

**Examples:**
```
omtsf diff v1.omts v2.omts
omtsf diff --ids-only baseline.omts current.omts
omtsf diff -f json old.omts new.omts > changes.json
```

### 3.6 `omtsf convert <file>`

Re-serializes an `.omts` file. Useful for normalizing whitespace, key ordering, and verifying round-trip fidelity.

**Arguments:**
- `<file>` (required) -- Path to an `.omts` file, or `-` for stdin.

**Flags:**
- `--pretty` -- Pretty-print JSON output with 2-space indentation (default).
- `--compact` -- Emit minified JSON with no extraneous whitespace.

**Behavior:** Parses the file into the typed data model, re-serializes to JSON, and writes to stdout. Unknown fields captured via `serde(flatten)` are preserved. The default is pretty-printed output; `--compact` produces single-line JSON.

**Exit codes:** 0 = success, 2 = parse failure.

**Examples:**
```
omtsf convert messy.omts > clean.omts
omtsf convert --compact supply-chain.omts | wc -c
cat untrusted.omts | omtsf convert - > normalized.omts
```

### 3.7 `omtsf reach <file> <node-id>`

Lists all nodes reachable from a source node via directed edges.

**Arguments:**
- `<file>` (required) -- Path to an `.omts` file, or `-` for stdin.
- `<node-id>` (required) -- The starting node ID.

**Flags:**
- `--depth <n>` -- Maximum traversal depth (default: unlimited).
- `--direction <d>` -- Traversal direction: `outgoing` (default), `incoming`, or `both`.

**Behavior:** Builds the directed graph, performs a breadth-first traversal from the source node, and writes the set of reachable node IDs to stdout (one per line in human mode, JSON array in json mode). Reports traversal statistics (nodes visited, max depth reached) to stderr in verbose mode.

**Exit codes:** 0 = success, 1 = source node ID not found, 2 = parse failure.

**Examples:**
```
omtsf reach supply-chain.omts org-001
omtsf reach --depth 3 supply-chain.omts org-001
omtsf reach --direction both -f json graph.omts node-42
```

### 3.8 `omtsf path <file> <from> <to>`

Finds paths between two nodes.

**Arguments:**
- `<file>` (required) -- Path to an `.omts` file, or `-` for stdin.
- `<from>` (required) -- Source node ID.
- `<to>` (required) -- Target node ID.

**Flags:**
- `--max-paths <n>` -- Maximum number of paths to report (default: 10).
- `--max-depth <n>` -- Maximum path length in edges (default: 20).

**Behavior:** Builds the directed graph and finds paths from `<from>` to `<to>`. Reports paths to stdout: in human mode, each path is printed as a chain of node IDs separated by ` -> `; in JSON mode, emits an array of path arrays. Paths are ordered shortest-first.

**Exit codes:** 0 = at least one path found, 1 = no path exists or a node ID is not found, 2 = parse failure.

**Examples:**
```
omtsf path supply-chain.omts org-001 facility-099
omtsf path --max-paths 3 graph.omts src dst
```

### 3.9 `omtsf subgraph <file> <node-id>...`

Extracts the induced subgraph for a set of nodes.

**Arguments:**
- `<file>` (required) -- Path to an `.omts` file, or `-` for stdin.
- `<node-id>...` (required, minimum 1) -- One or more node IDs to include.

**Flags:**
- `--expand <n>` -- Include neighbors up to `n` hops from the specified nodes (default: 0, meaning only the listed nodes and edges between them).

**Behavior:** Builds the graph, selects the specified nodes (plus neighbors within `--expand` distance), collects all edges where both endpoints are in the selected set, and writes a valid `.omts` file to stdout. The output header is copied from the input with an updated `snapshot_date`.

**Exit codes:** 0 = success, 1 = one or more node IDs not found, 2 = parse failure.

**Examples:**
```
omtsf subgraph supply-chain.omts org-001 org-002 > pair.omts
omtsf subgraph --expand 2 graph.omts org-001 > neighborhood.omts
```

### 3.10 `omtsf init`

Scaffolds a new `.omts` file.

**Arguments:** None.

**Flags:**
- `--example` -- Generate a realistic example file instead of a minimal skeleton.

**Behavior:** Writes a valid `.omts` file to stdout. Without `--example`, the output is a minimal file: header with a freshly generated `file_salt`, today's date as `snapshot_date`, empty `nodes` and `edges` arrays. With `--example`, the output includes sample organization, facility, and product nodes with realistic identifiers and edges.

**Exit codes:** 0 = success (always succeeds unless stdout write fails).

**Examples:**
```
omtsf init > new-graph.omts
omtsf init --example > demo.omts
omtsf init --example | omtsf validate -
```

---

## 4. File I/O Module

File I/O is exclusively the CLI's concern. `omtsf-core` never touches the filesystem.

### 4.1 Path Resolution

All `<file>` arguments accept either an absolute path, a relative path (resolved against the current working directory), or the literal string `-` for stdin. When a command accepts multiple file arguments, at most one may be `-`.

### 4.2 Stdin Support

When `-` is provided, the CLI reads the entire stdin stream into a byte buffer before passing it to the parser. This is necessary because `omtsf-core` operates on `&str` / `&[u8]` inputs, not streaming readers. Stdin is not seekable, so the full contents must be buffered.

### 4.3 File Size Enforcement

Before reading any file (or stdin), the CLI checks the size against `--max-file-size`:
- **Disk files:** `std::fs::metadata` provides the file length before reading. Reject immediately if it exceeds the limit.
- **Stdin:** Read into a buffer with a capped allocation. If the buffer reaches the limit before EOF, abort with an error. Use `Read::take` to bound the allocation.

The limit applies per file. For multi-file commands like `merge`, each file is checked independently.

### 4.4 Encoding

All `.omts` files are UTF-8 JSON. The CLI validates UTF-8 encoding when converting from bytes to string (`std::str::from_utf8`). Invalid UTF-8 produces exit code 2 with a message identifying the byte offset of the first invalid sequence.

### 4.5 I/O Error Handling

| Condition | Behavior |
|-----------|----------|
| File not found | stderr message, exit 2 |
| Permission denied | stderr message, exit 2 |
| File exceeds size limit | stderr message with limit and actual size, exit 2 |
| Invalid UTF-8 | stderr message with byte offset, exit 2 |
| Stdout write failure (broken pipe) | Silently exit 0 (standard Unix behavior for piped output) |
| Stdin read error | stderr message, exit 2 |

Broken pipe handling: the CLI installs a handler for `SIGPIPE` (or equivalent) so that piping output through `head` or similar tools does not produce an error.

---

## 5. Output Formatting

### 5.1 Human Mode (Default)

**Diagnostics (stderr):** One finding per line, color-coded by severity.
```
[E] L1-GDM-03  edge "e-042": target "node-999" not found
[W] L2-EID-01  node "org-001": no external identifiers
[I] L3-EID-02  node "org-001": LEI status is LAPSED
```

Colors: `[E]` red, `[W]` yellow, `[I]` cyan. Disabled when `--no-color` is set or `NO_COLOR` env var is present, or when stderr is not a TTY.

**Data (stdout):** Command-specific. `inspect` uses aligned columns. `reach` and `path` use plain text, one entry per line. `diff` uses `+`/`-` prefixed lines.

**Quiet mode (`--quiet`):** Suppresses all stderr output except parse errors and I/O errors. Useful in scripts that only check exit codes.

**Verbose mode (`--verbose`):** Adds timing information (`parsed in 42ms, validated in 18ms`), rule execution counts, and file metadata to stderr.

### 5.2 JSON Mode (`--format json`)

**Diagnostics:** NDJSON (newline-delimited JSON) to stderr. Each finding is a single-line JSON object:
```json
{"rule_id":"L1-GDM-03","severity":"error","location":{"type":"edge","id":"e-042","field":"target"},"message":"target \"node-999\" not found"}
```

**Data:** Single JSON document to stdout. For commands that produce `.omts` files (`merge`, `redact`, `convert`, `subgraph`, `init`), the output is the JSON file itself regardless of `--format`. For `inspect`, `reach`, `path`, and `diff`, the output is a structured JSON object.

### 5.3 Summary Counts

The `validate` command, in human mode, ends with a summary line:
```
3 errors, 1 warning, 0 info (checked 142 nodes, 87 edges)
```

In quiet mode, the summary is suppressed. In JSON mode, the summary is a final JSON object with key `"summary"`.

---

## 6. Exit Code Table

| Code | Meaning | Used By |
|------|---------|---------|
| 0 | Success. No errors, or diff found no differences. | All commands |
| 1 | Logical failure: validation errors (L1), merge conflicts, no path found, node ID not found, diff found differences, redaction scope error. | `validate`, `merge`, `redact`, `reach`, `path`, `subgraph`, `diff` |
| 2 | Input failure: file not found, permission denied, size limit exceeded, invalid UTF-8, JSON parse error, missing required fields. | All commands |

Design rationale: two non-zero codes distinguish between "the tool worked correctly but the input has problems" (1) and "the tool could not process the input at all" (2). This is consistent with `grep` (0 = match, 1 = no match, 2 = error) and `diff` (0 = same, 1 = different, 2 = error). Scripts can branch on `$?` without parsing stderr.

---

## 7. clap Subcommand Dispatch (Illustrative)

```rust
#[derive(Subcommand)]
enum Command {
    /// Validate an .omts file against the OMTSF specification.
    Validate {
        #[arg(value_name = "FILE")]
        file: PathOrStdin,
        #[arg(long, default_value = "2", value_parser = clap::value_parser!(u8).range(1..=3))]
        level: u8,
    },
    /// Merge two or more .omts files.
    Merge {
        #[arg(value_name = "FILE", num_args = 2..)]
        files: Vec<PathOrStdin>,
        #[arg(long, default_value = "union")]
        strategy: MergeStrategy,
    },
    /// Redact a file for a target disclosure scope.
    Redact {
        #[arg(value_name = "FILE")]
        file: PathOrStdin,
        #[arg(long)]
        scope: DisclosureScope,
    },
    /// Print summary statistics.
    Inspect {
        #[arg(value_name = "FILE")]
        file: PathOrStdin,
    },
    /// Structural diff between two files.
    Diff {
        #[arg(value_name = "A")]
        a: PathOrStdin,
        #[arg(value_name = "B")]
        b: PathOrStdin,
        #[arg(long)]
        ids_only: bool,
    },
    /// Re-serialize a file.
    Convert {
        #[arg(value_name = "FILE")]
        file: PathOrStdin,
        #[arg(long, default_value = "true")]
        pretty: bool,
        #[arg(long, conflicts_with = "pretty")]
        compact: bool,
    },
    /// List reachable nodes from a source.
    Reach {
        #[arg(value_name = "FILE")]
        file: PathOrStdin,
        #[arg(value_name = "NODE_ID")]
        node_id: String,
        #[arg(long)]
        depth: Option<u32>,
        #[arg(long, default_value = "outgoing")]
        direction: Direction,
    },
    /// Find paths between two nodes.
    Path {
        #[arg(value_name = "FILE")]
        file: PathOrStdin,
        #[arg(value_name = "FROM")]
        from: String,
        #[arg(value_name = "TO")]
        to: String,
        #[arg(long, default_value = "10")]
        max_paths: usize,
        #[arg(long, default_value = "20")]
        max_depth: u32,
    },
    /// Extract an induced subgraph.
    Subgraph {
        #[arg(value_name = "FILE")]
        file: PathOrStdin,
        #[arg(value_name = "NODE_ID", num_args = 1..)]
        node_ids: Vec<String>,
        #[arg(long, default_value = "0")]
        expand: u32,
    },
    /// Scaffold a new .omts file.
    Init {
        #[arg(long)]
        example: bool,
    },
}
```

`PathOrStdin` is a small wrapper type implementing `clap::ValueParserFactory`. It parses `"-"` as a `Stdin` variant and anything else as a `PathBuf` via `Path(PathBuf)`. This avoids stringly-typed handling of the stdin sentinel throughout the codebase.

---

## 8. Environment Variables

| Variable | Purpose | Overridden By |
|----------|---------|---------------|
| `OMTSF_MAX_FILE_SIZE` | Default file size limit in bytes | `--max-file-size` flag |
| `NO_COLOR` | Disable ANSI color output | `--no-color` flag |

No other environment variables are read. In particular, no configuration files, no home-directory dotfiles, and no XDG paths. The CLI is stateless and fully driven by its arguments and these two env vars.
