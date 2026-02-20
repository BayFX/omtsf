pub mod cmd;
pub mod error;
pub mod format;
pub mod io;

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

// ── PathOrStdin ──────────────────────────────────────────────────────────────

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

// ── OutputFormat ─────────────────────────────────────────────────────────────

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

// ── MergeStrategy ────────────────────────────────────────────────────────────

/// Strategy controlling how non-overlapping nodes are handled during a merge.
#[derive(Clone, Debug, ValueEnum)]
pub enum MergeStrategy {
    /// Include all nodes from all inputs (default).
    Union,
    /// Include only nodes present in all inputs.
    Intersect,
}

// ── DisclosureScope ──────────────────────────────────────────────────────────

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

// ── Direction ────────────────────────────────────────────────────────────────

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

// ── Command enum ─────────────────────────────────────────────────────────────

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
        /// Pretty-print JSON output with 2-space indentation (default).
        #[arg(long, default_value = "true")]
        pretty: bool,
        /// Emit minified JSON with no extraneous whitespace.
        #[arg(long, conflicts_with = "pretty")]
        compact: bool,
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
}

// ── Root Cli struct ──────────────────────────────────────────────────────────

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

// ── Entry point ──────────────────────────────────────────────────────────────

fn main() {
    // On Unix, reset SIGPIPE to its default disposition so that writing to a
    // closed pipe (e.g. `omtsf validate file.omts | head`) silently exits 0
    // rather than crashing with a Broken Pipe error message.
    #[cfg(unix)]
    install_sigpipe_default();

    let cli = Cli::parse();

    let result = dispatch(&cli);

    if let Err(e) = result {
        eprintln!("{}", e.message());
        std::process::exit(e.exit_code());
    }
}

/// Dispatches the parsed CLI arguments to the appropriate command handler.
///
/// Returns `Ok(())` on success or a [`error::CliError`] on failure. The
/// caller is responsible for printing the error message and exiting with the
/// appropriate exit code.
fn dispatch(cli: &Cli) -> Result<(), error::CliError> {
    match &cli.command {
        Command::Validate { file, level } => {
            let content = io::read_input(file, cli.max_file_size)?;
            cmd::validate::run(
                &content,
                *level,
                &cli.format,
                cli.quiet,
                cli.verbose,
                cli.no_color,
            )
        }

        Command::Inspect { file } => {
            let content = io::read_input(file, cli.max_file_size)?;
            cmd::inspect::run(&content, &cli.format)
        }

        Command::Convert { file, compact, .. } => {
            let content = io::read_input(file, cli.max_file_size)?;
            cmd::convert::run(&content, *compact)
        }

        Command::Init { example } => cmd::init::run(*example),

        Command::Reach {
            file,
            node_id,
            depth,
            direction,
        } => {
            let content = io::read_input(file, cli.max_file_size)?;
            cmd::reach::run(&content, node_id, *depth, direction, &cli.format)
        }

        Command::Path {
            file,
            from,
            to,
            max_paths,
            max_depth,
        } => {
            let content = io::read_input(file, cli.max_file_size)?;
            cmd::path::run(&content, from, to, *max_paths, *max_depth, &cli.format)
        }

        Command::Subgraph {
            file,
            node_ids,
            expand,
        } => {
            let content = io::read_input(file, cli.max_file_size)?;
            cmd::subgraph::run(&content, node_ids, *expand)
        }

        Command::Diff {
            a,
            b,
            ids_only,
            summary_only,
            node_type,
            edge_type,
            ignore_field,
        } => {
            let content_a = io::read_input(a, cli.max_file_size)?;
            let content_b = io::read_input(b, cli.max_file_size)?;
            cmd::diff::run(
                &content_a,
                &content_b,
                *ids_only,
                *summary_only,
                node_type,
                edge_type,
                ignore_field,
                &cli.format,
            )
        }

        // Commands not yet implemented — exit 2 to indicate input failure.
        Command::Merge { .. } | Command::Redact { .. } => {
            eprintln!("not yet implemented");
            std::process::exit(2);
        }
    }
}

// ── SIGPIPE handler ───────────────────────────────────────────────────────────

/// Resets `SIGPIPE` to its default disposition (`SIG_DFL`).
///
/// Rust's runtime ignores `SIGPIPE` by default, which causes programs that
/// write to a closed pipe (e.g. `omtsf validate file.omts | head`) to receive
/// an `Err(BrokenPipe)` from a write call rather than being terminated silently.
/// By restoring the default disposition, the kernel will terminate the process
/// with exit code 0 (consistent with standard Unix behavior) when a write to a
/// closed pipe occurs.
///
/// This function uses `libc::signal` which requires the `libc` crate. It is
/// only compiled on Unix targets via `#[cfg(unix)]` at the call site.
#[cfg(unix)]
fn install_sigpipe_default() {
    // SAFETY: signal() is safe to call during single-threaded program
    // initialization before any other threads are spawned. SIG_DFL is a valid
    // handler for SIGPIPE. The return value (previous handler) is discarded.
    //
    // The workspace denies `unsafe_code` globally, but this is the minimal
    // unavoidable use of libc required for SIGPIPE handling on Unix. There is
    // no safe Rust equivalent in the standard library.
    //
    // We use an inline allow rather than a workspace-level exception so the
    // scope of the unsafe block is as narrow as possible.
    #[allow(unsafe_code)]
    {
        // SAFETY: See above.
        unsafe {
            libc::signal(libc::SIGPIPE, libc::SIG_DFL);
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    #![allow(clippy::panic)]
    #![allow(clippy::wildcard_enum_match_arm)]

    use clap::CommandFactory;

    use super::*;

    // ── Help structure ──────────────────────────────────────────────────────

    /// The root help output must contain all top-level subcommand names.
    #[test]
    fn test_root_help_lists_all_subcommands() {
        let mut cmd = Cli::command();
        let help = format!("{}", cmd.render_help());

        let expected_subcommands = [
            "validate", "merge", "redact", "inspect", "diff", "convert", "reach", "path",
            "subgraph", "init",
        ];
        for name in &expected_subcommands {
            assert!(
                help.contains(name),
                "root help should mention subcommand '{name}'"
            );
        }
    }

    /// The root help output must describe every global flag.
    #[test]
    fn test_root_help_lists_global_flags() {
        let mut cmd = Cli::command();
        let help = format!("{}", cmd.render_help());

        let expected_flags = [
            "--format",
            "--quiet",
            "--verbose",
            "--max-file-size",
            "--no-color",
            "--help",
            "--version",
        ];
        for flag in &expected_flags {
            assert!(
                help.contains(flag),
                "root help should mention flag '{flag}'"
            );
        }
    }

    // ── Subcommand help ─────────────────────────────────────────────────────

    /// `omtsf validate --help` must mention `--level` and `FILE`.
    #[test]
    fn test_validate_help() {
        let mut cmd = Cli::command();
        let sub = cmd
            .find_subcommand_mut("validate")
            .expect("validate subcommand should exist");
        let help = format!("{}", sub.render_help());
        assert!(
            help.contains("--level"),
            "validate help should mention --level"
        );
        assert!(help.contains("FILE"), "validate help should mention FILE");
    }

    /// `omtsf merge --help` must mention `--strategy`.
    #[test]
    fn test_merge_help() {
        let mut cmd = Cli::command();
        let sub = cmd
            .find_subcommand_mut("merge")
            .expect("merge subcommand should exist");
        let help = format!("{}", sub.render_help());
        assert!(
            help.contains("--strategy"),
            "merge help should mention --strategy"
        );
    }

    /// `omtsf redact --help` must mention `--scope`.
    #[test]
    fn test_redact_help() {
        let mut cmd = Cli::command();
        let sub = cmd
            .find_subcommand_mut("redact")
            .expect("redact subcommand should exist");
        let help = format!("{}", sub.render_help());
        assert!(
            help.contains("--scope"),
            "redact help should mention --scope"
        );
    }

    /// `omtsf inspect --help` must mention `FILE`.
    #[test]
    fn test_inspect_help() {
        let mut cmd = Cli::command();
        let sub = cmd
            .find_subcommand_mut("inspect")
            .expect("inspect subcommand should exist");
        let help = format!("{}", sub.render_help());
        assert!(help.contains("FILE"), "inspect help should mention FILE");
    }

    /// `omtsf diff --help` must mention `--ids-only`.
    #[test]
    fn test_diff_help() {
        let mut cmd = Cli::command();
        let sub = cmd
            .find_subcommand_mut("diff")
            .expect("diff subcommand should exist");
        let help = format!("{}", sub.render_help());
        assert!(
            help.contains("--ids-only"),
            "diff help should mention --ids-only"
        );
    }

    /// `omtsf convert --help` must mention both `--compact` and `--pretty`.
    #[test]
    fn test_convert_help() {
        let mut cmd = Cli::command();
        let sub = cmd
            .find_subcommand_mut("convert")
            .expect("convert subcommand should exist");
        let help = format!("{}", sub.render_help());
        assert!(
            help.contains("--compact"),
            "convert help should mention --compact"
        );
        assert!(
            help.contains("--pretty"),
            "convert help should mention --pretty"
        );
    }

    /// `omtsf reach --help` must mention `--depth`, `--direction`, and `NODE_ID`.
    #[test]
    fn test_reach_help() {
        let mut cmd = Cli::command();
        let sub = cmd
            .find_subcommand_mut("reach")
            .expect("reach subcommand should exist");
        let help = format!("{}", sub.render_help());
        assert!(
            help.contains("--depth"),
            "reach help should mention --depth"
        );
        assert!(
            help.contains("--direction"),
            "reach help should mention --direction"
        );
        assert!(
            help.contains("NODE_ID"),
            "reach help should mention NODE_ID"
        );
    }

    /// `omtsf path --help` must mention `--max-paths` and `--max-depth`.
    #[test]
    fn test_path_help() {
        let mut cmd = Cli::command();
        let sub = cmd
            .find_subcommand_mut("path")
            .expect("path subcommand should exist");
        let help = format!("{}", sub.render_help());
        assert!(
            help.contains("--max-paths"),
            "path help should mention --max-paths"
        );
        assert!(
            help.contains("--max-depth"),
            "path help should mention --max-depth"
        );
    }

    /// `omtsf subgraph --help` must mention `--expand`.
    #[test]
    fn test_subgraph_help() {
        let mut cmd = Cli::command();
        let sub = cmd
            .find_subcommand_mut("subgraph")
            .expect("subgraph subcommand should exist");
        let help = format!("{}", sub.render_help());
        assert!(
            help.contains("--expand"),
            "subgraph help should mention --expand"
        );
    }

    /// `omtsf init --help` must mention `--example`.
    #[test]
    fn test_init_help() {
        let mut cmd = Cli::command();
        let sub = cmd
            .find_subcommand_mut("init")
            .expect("init subcommand should exist");
        let help = format!("{}", sub.render_help());
        assert!(
            help.contains("--example"),
            "init help should mention --example"
        );
    }

    // ── Argument parsing ────────────────────────────────────────────────────

    /// Parsing `validate -` should produce `PathOrStdin::Stdin`.
    #[test]
    fn test_path_or_stdin_parses_dash_as_stdin() {
        let cli = Cli::try_parse_from(["omtsf", "validate", "-"]).expect("should parse validate -");
        match cli.command {
            Command::Validate { file, .. } => match file {
                PathOrStdin::Stdin => {}
                PathOrStdin::Path(p) => panic!("expected Stdin, got Path({p:?})"),
            },
            _ => panic!("expected Validate subcommand"),
        }
    }

    /// Parsing a real path should produce `PathOrStdin::Path`.
    #[test]
    fn test_path_or_stdin_parses_real_path() {
        let cli = Cli::try_parse_from(["omtsf", "validate", "supply-chain.omts"])
            .expect("should parse validate <path>");
        match cli.command {
            Command::Validate { file, .. } => match file {
                PathOrStdin::Path(p) => {
                    assert_eq!(p.to_string_lossy(), "supply-chain.omts");
                }
                PathOrStdin::Stdin => panic!("expected Path, got Stdin"),
            },
            _ => panic!("expected Validate subcommand"),
        }
    }

    /// `--quiet` and `--verbose` must conflict with each other.
    #[test]
    fn test_quiet_verbose_conflict() {
        let result = Cli::try_parse_from(["omtsf", "--quiet", "--verbose", "validate", "-"]);
        assert!(
            result.is_err(),
            "--quiet and --verbose should conflict; parse should fail"
        );
    }

    /// `--max-file-size` should default to 256 MB (268435456 bytes).
    #[test]
    fn test_max_file_size_default() {
        let cli = Cli::try_parse_from(["omtsf", "validate", "-"])
            .expect("should parse without --max-file-size");
        assert_eq!(
            cli.max_file_size, 268_435_456,
            "default max_file_size should be 256 MB"
        );
    }

    /// `--max-file-size` CLI flag overrides the default.
    #[test]
    fn test_max_file_size_cli_override() {
        let cli = Cli::try_parse_from(["omtsf", "--max-file-size", "1048576", "validate", "-"])
            .expect("should parse with --max-file-size");
        assert_eq!(cli.max_file_size, 1_048_576);
    }

    /// `--format json` should parse to `OutputFormat::Json`.
    #[test]
    fn test_format_flag_json() {
        let cli = Cli::try_parse_from(["omtsf", "--format", "json", "validate", "-"])
            .expect("should parse --format json");
        assert!(
            matches!(cli.format, OutputFormat::Json),
            "format should be Json"
        );
    }

    /// The default `--format` is `human`.
    #[test]
    fn test_format_flag_default_is_human() {
        let cli =
            Cli::try_parse_from(["omtsf", "validate", "-"]).expect("should parse without --format");
        assert!(
            matches!(cli.format, OutputFormat::Human),
            "default format should be Human"
        );
    }

    /// `merge` requires at least two file arguments.
    #[test]
    fn test_merge_requires_two_files() {
        let result = Cli::try_parse_from(["omtsf", "merge", "only-one.omts"]);
        assert!(
            result.is_err(),
            "merge with fewer than 2 files should fail to parse"
        );
    }

    /// `merge` accepts two or more files.
    #[test]
    fn test_merge_accepts_two_files() {
        let cli = Cli::try_parse_from(["omtsf", "merge", "a.omts", "b.omts"])
            .expect("should parse merge with 2 files");
        match cli.command {
            Command::Merge { files, .. } => {
                assert_eq!(files.len(), 2);
            }
            _ => panic!("expected Merge subcommand"),
        }
    }

    /// `subgraph` requires at least one node ID.
    #[test]
    fn test_subgraph_requires_at_least_one_node_id() {
        let result = Cli::try_parse_from(["omtsf", "subgraph", "graph.omts"]);
        assert!(
            result.is_err(),
            "subgraph with no node IDs should fail to parse"
        );
    }

    /// `validate --level` range: values 1, 2, 3 are valid.
    #[test]
    fn test_validate_level_range_valid() {
        for level in ["1", "2", "3"] {
            let cli =
                Cli::try_parse_from(["omtsf", "validate", "--level", level, "supply-chain.omts"])
                    .unwrap_or_else(|e| panic!("--level {level} should be valid: {e}"));
            match cli.command {
                Command::Validate { level: l, .. } => {
                    let expected: u8 = level.parse().expect("test level parses");
                    assert_eq!(l, expected);
                }
                _ => panic!("expected Validate"),
            }
        }
    }

    /// `validate --level 0` and `--level 4` are out of range and must be rejected.
    #[test]
    fn test_validate_level_range_out_of_bounds() {
        let result =
            Cli::try_parse_from(["omtsf", "validate", "--level", "0", "supply-chain.omts"]);
        assert!(result.is_err(), "--level 0 should be rejected");

        let result =
            Cli::try_parse_from(["omtsf", "validate", "--level", "4", "supply-chain.omts"]);
        assert!(result.is_err(), "--level 4 should be rejected");
    }

    /// `redact --scope` accepts all three variants.
    #[test]
    fn test_redact_scope_variants() {
        for scope in ["public", "partner", "internal"] {
            let cli =
                Cli::try_parse_from(["omtsf", "redact", "--scope", scope, "supply-chain.omts"])
                    .unwrap_or_else(|e| panic!("--scope {scope} should be valid: {e}"));
            match cli.command {
                Command::Redact { scope: s, .. } => {
                    let actual = format!("{s:?}").to_lowercase();
                    assert!(
                        actual.contains(scope),
                        "scope should be {scope}, got {actual}"
                    );
                }
                _ => panic!("expected Redact"),
            }
        }
    }

    /// `reach --direction` accepts all three direction variants.
    #[test]
    fn test_reach_direction_variants() {
        for direction in ["outgoing", "incoming", "both"] {
            let cli = Cli::try_parse_from([
                "omtsf",
                "reach",
                "graph.omts",
                "node-001",
                "--direction",
                direction,
            ])
            .unwrap_or_else(|e| panic!("--direction {direction} should be valid: {e}"));
            match cli.command {
                Command::Reach { direction: d, .. } => {
                    let actual = format!("{d:?}").to_lowercase();
                    assert!(
                        actual.contains(direction),
                        "direction should be {direction}, got {actual}"
                    );
                }
                _ => panic!("expected Reach"),
            }
        }
    }

    /// `convert --compact` conflicts with `--pretty`.
    #[test]
    fn test_convert_compact_conflicts_with_pretty() {
        let result =
            Cli::try_parse_from(["omtsf", "convert", "--compact", "--pretty", "file.omts"]);
        assert!(
            result.is_err(),
            "--compact and --pretty should conflict; parse should fail"
        );
    }

    /// clap's internal consistency check must pass for the full command tree.
    #[test]
    fn test_cli_debug_assert() {
        Cli::command().debug_assert();
    }
}
