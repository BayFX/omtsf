//! Clap CLI definition: root struct, subcommands, and shared argument types.
use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

/// A CLI argument that is either a filesystem path or the stdin sentinel `"-"`.
///
/// Parsing `"-"` yields [`PathOrStdin::Stdin`]; anything else yields
/// [`PathOrStdin::Path`].  This avoids stringly-typed handling of the stdin
/// sentinel throughout the codebase.
#[derive(Clone, Debug)]
pub enum PathOrStdin {
    /// Read from standard input.
    Stdin,
    /// Read from the given filesystem path.
    Path(PathBuf),
}

impl std::str::FromStr for PathOrStdin {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "-" {
            Ok(PathOrStdin::Stdin)
        } else {
            Ok(PathOrStdin::Path(PathBuf::from(s)))
        }
    }
}

/// Output format for CLI commands.
///
/// `Human` emits colored, tabular output to stderr and plain text to stdout.
/// `Json` emits structured JSON (NDJSON for diagnostics, single object for
/// data).
#[derive(Clone, Debug, ValueEnum)]
pub enum OutputFormat {
    /// Human-readable, optionally colored output (default).
    Human,
    /// Structured JSON / NDJSON output.
    Json,
}

/// Strategy controlling how non-overlapping nodes are handled during a merge.
#[derive(Clone, Debug, ValueEnum)]
pub enum MergeStrategy {
    /// Include all nodes from all inputs (default).
    Union,
    /// Include only nodes present in all inputs.
    Intersect,
}

/// Target disclosure scope for a redaction operation.
#[derive(Clone, Debug, ValueEnum)]
pub enum DisclosureScope {
    /// Publicly shareable — most restrictive.
    Public,
    /// Shareable with vetted partners.
    Partner,
    /// Internal only — least restrictive.
    Internal,
}

/// Graph traversal direction for the `reach` subcommand.
#[derive(Clone, Debug, ValueEnum)]
pub enum Direction {
    /// Follow edges away from the source node (default).
    Outgoing,
    /// Follow edges toward the source node.
    Incoming,
    /// Follow edges in both directions.
    Both,
}

/// Target serialization encoding for the `convert` subcommand.
///
/// Only `json` and `cbor` are valid targets; zstd is the compression layer,
/// not an encoding, and is controlled separately via `--compress`.
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum TargetEncoding {
    /// JSON encoding (default).
    Json,
    /// CBOR encoding with self-describing tag 55799.
    Cbor,
}

/// All top-level subcommands exposed by the `omtsf` binary.
#[derive(Subcommand)]
pub enum Command {
    /// Validate an .omts file against the OMTSF specification.
    Validate {
        /// Path to an .omts file, or `-` for stdin.
        #[arg(value_name = "FILE")]
        file: PathOrStdin,
        /// Maximum validation level to run (1 = L1 only, 2 = L1+L2, 3 = all).
        #[arg(long, default_value = "2", value_parser = clap::value_parser!(u8).range(1..=3))]
        level: u8,
    },

    /// Merge two or more .omts files into a single graph.
    Merge {
        /// Paths to .omts files, or `-` for stdin (at most one may be `-`).
        #[arg(value_name = "FILE", num_args = 2..)]
        files: Vec<PathOrStdin>,
        /// Merge strategy: union (default) or intersect.
        #[arg(long, default_value = "union")]
        strategy: MergeStrategy,
    },

    /// Redact a file for a target disclosure scope.
    Redact {
        /// Path to an .omts file, or `-` for stdin.
        #[arg(value_name = "FILE")]
        file: PathOrStdin,
        /// Target disclosure scope (required).
        #[arg(long)]
        scope: DisclosureScope,
    },

    /// Print summary statistics for a graph.
    Inspect {
        /// Path to an .omts file, or `-` for stdin.
        #[arg(value_name = "FILE")]
        file: PathOrStdin,
    },

    /// Compute a structural diff between two .omts files.
    Diff {
        /// Path to the base file, or `-` for stdin.
        #[arg(value_name = "A")]
        a: PathOrStdin,
        /// Path to the comparison file (cannot be `-` if A is `-`).
        #[arg(value_name = "B")]
        b: PathOrStdin,
        /// Only report added/removed/changed IDs, not property-level detail.
        #[arg(long)]
        ids_only: bool,
        /// Only print the summary statistics line, no per-element details.
        #[arg(long)]
        summary_only: bool,
        /// Restrict diff to nodes of this type (repeatable).
        #[arg(long, value_name = "TYPE")]
        node_type: Vec<String>,
        /// Restrict diff to edges of this type (repeatable).
        #[arg(long, value_name = "TYPE")]
        edge_type: Vec<String>,
        /// Exclude this property from comparison (repeatable).
        #[arg(long, value_name = "FIELD")]
        ignore_field: Vec<String>,
    },

    /// Re-serialize an .omts file (normalize whitespace, key ordering).
    Convert {
        /// Path to an .omts file, or `-` for stdin.
        #[arg(value_name = "FILE")]
        file: PathOrStdin,
        /// Target encoding: json (default) or cbor.
        #[arg(long, default_value = "json", value_enum)]
        to: TargetEncoding,
        /// Pretty-print JSON output with 2-space indentation (default when --to json).
        ///
        /// Ignored when `--to cbor`.
        #[arg(long, default_value = "true")]
        pretty: bool,
        /// Emit minified JSON with no extraneous whitespace.
        ///
        /// Mutually exclusive with `--pretty`. Ignored when `--to cbor`.
        #[arg(long, conflicts_with = "pretty")]
        compact: bool,
        /// Compress output with zstd after serialization.
        #[arg(long)]
        compress: bool,
    },

    /// List all nodes reachable from a source node via directed edges.
    Reach {
        /// Path to an .omts file, or `-` for stdin.
        #[arg(value_name = "FILE")]
        file: PathOrStdin,
        /// The starting node ID.
        #[arg(value_name = "NODE_ID")]
        node_id: String,
        /// Maximum traversal depth (default: unlimited).
        #[arg(long)]
        depth: Option<u32>,
        /// Traversal direction: outgoing (default), incoming, or both.
        #[arg(long, default_value = "outgoing")]
        direction: Direction,
    },

    /// Find paths between two nodes.
    Path {
        /// Path to an .omts file, or `-` for stdin.
        #[arg(value_name = "FILE")]
        file: PathOrStdin,
        /// Source node ID.
        #[arg(value_name = "FROM")]
        from: String,
        /// Target node ID.
        #[arg(value_name = "TO")]
        to: String,
        /// Maximum number of paths to report (default: 10).
        #[arg(long, default_value = "10")]
        max_paths: usize,
        /// Maximum path length in edges (default: 20).
        #[arg(long, default_value = "20")]
        max_depth: u32,
    },

    /// Extract the induced subgraph for a set of nodes.
    Subgraph {
        /// Path to an .omts file, or `-` for stdin.
        #[arg(value_name = "FILE")]
        file: PathOrStdin,
        /// One or more node IDs to include.
        #[arg(value_name = "NODE_ID", num_args = 1.., required = true)]
        node_ids: Vec<String>,
        /// Include neighbors up to N hops from the specified nodes (default: 0).
        #[arg(long, default_value = "0")]
        expand: u32,
    },

    /// Scaffold a new minimal .omts file.
    Init {
        /// Generate a realistic example file instead of a minimal skeleton.
        #[arg(long)]
        example: bool,
    },

    /// Query nodes and edges by property predicates.
    Query {
        /// Path to an .omts file, or `-` for stdin.
        #[arg(value_name = "FILE")]
        file: PathOrStdin,
        /// Match nodes of this type (repeatable; e.g. organization, facility).
        #[arg(long, value_name = "TYPE")]
        node_type: Vec<String>,
        /// Match edges of this type (repeatable; e.g. supplies, ownership).
        #[arg(long, value_name = "TYPE")]
        edge_type: Vec<String>,
        /// Match elements with this label key, or key=value pair (repeatable).
        #[arg(long, value_name = "KEY[=VALUE]")]
        label: Vec<String>,
        /// Match nodes with this identifier scheme, or scheme:value pair (repeatable).
        #[arg(long, value_name = "SCHEME[:VALUE]")]
        identifier: Vec<String>,
        /// Match nodes whose jurisdiction equals this ISO 3166-1 alpha-2 code (repeatable).
        #[arg(long, value_name = "CC")]
        jurisdiction: Vec<String>,
        /// Match nodes whose name contains this pattern (case-insensitive substring, repeatable).
        #[arg(long, value_name = "PATTERN")]
        name: Vec<String>,
        /// Print only match counts (nodes: N, edges: M) without listing individual results.
        #[arg(long)]
        count: bool,
    },

    /// Extract a subgraph rooted at selector-matched nodes and edges.
    #[command(name = "extract-subchain")]
    ExtractSubchain {
        /// Path to an .omts file, or `-` for stdin.
        #[arg(value_name = "FILE")]
        file: PathOrStdin,
        /// Match nodes of this type (repeatable; e.g. organization, facility).
        #[arg(long, value_name = "TYPE")]
        node_type: Vec<String>,
        /// Match edges of this type (repeatable; e.g. supplies, ownership).
        #[arg(long, value_name = "TYPE")]
        edge_type: Vec<String>,
        /// Match elements with this label key, or key=value pair (repeatable).
        #[arg(long, value_name = "KEY[=VALUE]")]
        label: Vec<String>,
        /// Match nodes with this identifier scheme, or scheme:value pair (repeatable).
        #[arg(long, value_name = "SCHEME[:VALUE]")]
        identifier: Vec<String>,
        /// Match nodes whose jurisdiction equals this ISO 3166-1 alpha-2 code (repeatable).
        #[arg(long, value_name = "CC")]
        jurisdiction: Vec<String>,
        /// Match nodes whose name contains this pattern (case-insensitive substring, repeatable).
        #[arg(long, value_name = "PATTERN")]
        name: Vec<String>,
        /// BFS expansion hops from seed elements (default: 1).
        #[arg(long, default_value = "1")]
        expand: u32,
    },
}

/// Root CLI struct for the `omtsf` binary.
///
/// All global flags are defined here and marked `global = true` so that clap
/// propagates them to every subcommand.
#[derive(Parser)]
#[command(
    name = "omtsf",
    version,
    about = "OMTSF reference CLI",
    long_about = "Open Multi-Tier Supply-Chain Framework reference command-line tool.\n\
                  Validates, merges, redacts, inspects, diffs, converts, queries,\n\
                  and scaffolds .omts supply chain graph files."
)]
pub struct Cli {
    /// Active subcommand.
    #[command(subcommand)]
    pub command: Command,

    /// Output format: human (default) or json.
    #[arg(long, short = 'f', default_value = "human", global = true)]
    pub format: OutputFormat,

    /// Suppress all stderr output except errors (incompatible with `--verbose`).
    #[arg(long, short = 'q', global = true, conflicts_with = "verbose")]
    pub quiet: bool,

    /// Increase stderr verbosity: timing, rule counts, file metadata
    /// (incompatible with `--quiet`).
    #[arg(long, short = 'v', global = true, conflicts_with = "quiet")]
    pub verbose: bool,

    /// Maximum input file size in bytes.
    ///
    /// Can also be set via the `OMTSF_MAX_FILE_SIZE` environment variable.
    /// The CLI flag takes precedence over the environment variable.
    /// Default: 268435456 (256 MB).
    #[arg(
        long,
        global = true,
        env = "OMTSF_MAX_FILE_SIZE",
        default_value = "268435456"
    )]
    pub max_file_size: u64,

    /// Disable ANSI color codes in human output.
    ///
    /// Also respects the `NO_COLOR` environment variable per
    /// <https://no-color.org>.
    #[arg(long, global = true, env = "NO_COLOR")]
    pub no_color: bool,
}

#[cfg(test)]
mod tests;
