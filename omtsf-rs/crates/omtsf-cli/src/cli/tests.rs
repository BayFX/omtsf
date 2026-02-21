#![allow(clippy::expect_used)]
#![allow(clippy::panic)]
#![allow(clippy::wildcard_enum_match_arm)]

use clap::CommandFactory;

use super::*;

/// The root help output must contain all top-level subcommand names.
#[test]
fn test_root_help_lists_all_subcommands() {
    let mut cmd = Cli::command();
    let help = format!("{}", cmd.render_help());

    let expected_subcommands = [
        "validate",
        "merge",
        "redact",
        "inspect",
        "diff",
        "convert",
        "reach",
        "path",
        "subgraph",
        "init",
        "query",
        "extract-subchain",
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

/// `omtsf convert --help` must mention `--compact`, `--pretty`, `--to`, and `--compress`.
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
    assert!(help.contains("--to"), "convert help should mention --to");
    assert!(
        help.contains("--compress"),
        "convert help should mention --compress"
    );
}

/// `omtsf convert --to json` parses correctly and defaults to pretty output.
#[test]
fn test_convert_to_json_flag() {
    let cli = Cli::try_parse_from(["omtsf", "convert", "--to", "json", "supply-chain.omts"])
        .expect("should parse --to json");
    match cli.command {
        Command::Convert {
            to,
            pretty,
            compact,
            compress,
            ..
        } => {
            assert!(
                matches!(to, super::TargetEncoding::Json),
                "to should be Json"
            );
            assert!(pretty, "default pretty should be true");
            assert!(!compact, "compact should default to false");
            assert!(!compress, "compress should default to false");
        }
        _ => panic!("expected Convert subcommand"),
    }
}

/// `omtsf convert --to cbor` parses correctly.
#[test]
fn test_convert_to_cbor_flag() {
    let cli = Cli::try_parse_from(["omtsf", "convert", "--to", "cbor", "supply-chain.omts"])
        .expect("should parse --to cbor");
    match cli.command {
        Command::Convert { to, .. } => {
            assert!(
                matches!(to, super::TargetEncoding::Cbor),
                "to should be Cbor"
            );
        }
        _ => panic!("expected Convert subcommand"),
    }
}

/// `omtsf convert --compress` parses correctly.
#[test]
fn test_convert_compress_flag() {
    let cli = Cli::try_parse_from(["omtsf", "convert", "--compress", "supply-chain.omts"])
        .expect("should parse --compress");
    match cli.command {
        Command::Convert { compress, .. } => {
            assert!(compress, "compress should be true");
        }
        _ => panic!("expected Convert subcommand"),
    }
}

/// `omtsf convert --to cbor --compress` is accepted.
#[test]
fn test_convert_cbor_with_compress() {
    let cli = Cli::try_parse_from([
        "omtsf",
        "convert",
        "--to",
        "cbor",
        "--compress",
        "supply-chain.omts",
    ])
    .expect("should parse --to cbor --compress");
    match cli.command {
        Command::Convert { to, compress, .. } => {
            assert!(
                matches!(to, super::TargetEncoding::Cbor),
                "to should be Cbor"
            );
            assert!(compress, "compress should be true");
        }
        _ => panic!("expected Convert subcommand"),
    }
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
        let cli = Cli::try_parse_from(["omtsf", "validate", "--level", level, "supply-chain.omts"])
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
    let result = Cli::try_parse_from(["omtsf", "validate", "--level", "0", "supply-chain.omts"]);
    assert!(result.is_err(), "--level 0 should be rejected");

    let result = Cli::try_parse_from(["omtsf", "validate", "--level", "4", "supply-chain.omts"]);
    assert!(result.is_err(), "--level 4 should be rejected");
}

/// `redact --scope` accepts all three variants.
#[test]
fn test_redact_scope_variants() {
    for scope in ["public", "partner", "internal"] {
        let cli = Cli::try_parse_from(["omtsf", "redact", "--scope", scope, "supply-chain.omts"])
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
    let result = Cli::try_parse_from(["omtsf", "convert", "--compact", "--pretty", "file.omts"]);
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

/// `omtsf query --help` must mention `--node-type`, `--edge-type`, and `--count`.
#[test]
fn test_query_help() {
    let mut cmd = Cli::command();
    let sub = cmd
        .find_subcommand_mut("query")
        .expect("query subcommand should exist");
    let help = format!("{}", sub.render_help());
    assert!(
        help.contains("--node-type"),
        "query help should mention --node-type"
    );
    assert!(
        help.contains("--edge-type"),
        "query help should mention --edge-type"
    );
    assert!(
        help.contains("--count"),
        "query help should mention --count"
    );
    assert!(help.contains("FILE"), "query help should mention FILE");
}

/// `omtsf query` with `--count` flag parses correctly.
#[test]
fn test_query_count_flag_parses() {
    let cli = Cli::try_parse_from([
        "omtsf",
        "query",
        "--node-type",
        "organization",
        "--count",
        "graph.omts",
    ])
    .expect("should parse query --count");
    match cli.command {
        Command::Query { count, .. } => {
            assert!(count, "--count should be true");
        }
        _ => panic!("expected Query subcommand"),
    }
}

/// `omtsf query` with multiple `--node-type` flags parses all values.
#[test]
fn test_query_multiple_node_types_parse() {
    let cli = Cli::try_parse_from([
        "omtsf",
        "query",
        "--node-type",
        "organization",
        "--node-type",
        "facility",
        "graph.omts",
    ])
    .expect("should parse multiple --node-type");
    match cli.command {
        Command::Query { node_type, .. } => {
            assert_eq!(node_type.len(), 2);
            assert!(node_type.contains(&"organization".to_owned()));
            assert!(node_type.contains(&"facility".to_owned()));
        }
        _ => panic!("expected Query subcommand"),
    }
}

/// `omtsf query` with `--label key=value` parses correctly.
#[test]
fn test_query_label_flag_parses() {
    let cli = Cli::try_parse_from(["omtsf", "query", "--label", "tier=1", "graph.omts"])
        .expect("should parse --label tier=1");
    match cli.command {
        Command::Query { label, .. } => {
            assert_eq!(label, vec!["tier=1"]);
        }
        _ => panic!("expected Query subcommand"),
    }
}

/// `omtsf query` with `--identifier scheme:value` parses correctly.
#[test]
fn test_query_identifier_flag_parses() {
    let cli = Cli::try_parse_from([
        "omtsf",
        "query",
        "--identifier",
        "lei:529900T8BM49AURSDO55",
        "graph.omts",
    ])
    .expect("should parse --identifier scheme:value");
    match cli.command {
        Command::Query { identifier, .. } => {
            assert_eq!(identifier, vec!["lei:529900T8BM49AURSDO55"]);
        }
        _ => panic!("expected Query subcommand"),
    }
}

/// `omtsf extract-subchain --help` must mention `--expand` and selector flags.
#[test]
fn test_extract_subchain_help() {
    let mut cmd = Cli::command();
    let sub = cmd
        .find_subcommand_mut("extract-subchain")
        .expect("extract-subchain subcommand should exist");
    let help = format!("{}", sub.render_help());
    assert!(
        help.contains("--expand"),
        "extract-subchain help should mention --expand"
    );
    assert!(
        help.contains("--node-type"),
        "extract-subchain help should mention --node-type"
    );
    assert!(
        help.contains("FILE"),
        "extract-subchain help should mention FILE"
    );
}

/// `omtsf extract-subchain` default `--expand` is 1.
#[test]
fn test_extract_subchain_expand_default_is_1() {
    let cli = Cli::try_parse_from([
        "omtsf",
        "extract-subchain",
        "--node-type",
        "organization",
        "graph.omts",
    ])
    .expect("should parse extract-subchain without --expand");
    match cli.command {
        Command::ExtractSubchain { expand, .. } => {
            assert_eq!(expand, 1, "default --expand should be 1");
        }
        _ => panic!("expected ExtractSubchain subcommand"),
    }
}

/// `omtsf extract-subchain --expand 3` parses correctly.
#[test]
fn test_extract_subchain_expand_override() {
    let cli = Cli::try_parse_from([
        "omtsf",
        "extract-subchain",
        "--node-type",
        "facility",
        "--expand",
        "3",
        "graph.omts",
    ])
    .expect("should parse --expand 3");
    match cli.command {
        Command::ExtractSubchain { expand, .. } => {
            assert_eq!(expand, 3);
        }
        _ => panic!("expected ExtractSubchain subcommand"),
    }
}

/// `omtsf extract-subchain` with stdin sentinel `-` parses correctly.
#[test]
fn test_extract_subchain_stdin_sentinel() {
    let cli = Cli::try_parse_from([
        "omtsf",
        "extract-subchain",
        "--node-type",
        "organization",
        "-",
    ])
    .expect("should parse stdin sentinel");
    match cli.command {
        Command::ExtractSubchain { file, .. } => match file {
            PathOrStdin::Stdin => {}
            PathOrStdin::Path(p) => panic!("expected Stdin, got Path({p:?})"),
        },
        _ => panic!("expected ExtractSubchain"),
    }
}

/// `omtsf query` with `--jurisdiction CC` parses correctly.
#[test]
fn test_query_jurisdiction_flag_parses() {
    let cli = Cli::try_parse_from(["omtsf", "query", "--jurisdiction", "DE", "graph.omts"])
        .expect("should parse --jurisdiction DE");
    match cli.command {
        Command::Query { jurisdiction, .. } => {
            assert_eq!(jurisdiction, vec!["DE"]);
        }
        _ => panic!("expected Query subcommand"),
    }
}

/// `omtsf query` with `--name pattern` parses correctly.
#[test]
fn test_query_name_flag_parses() {
    let cli = Cli::try_parse_from(["omtsf", "query", "--name", "acme", "graph.omts"])
        .expect("should parse --name");
    match cli.command {
        Command::Query { name, .. } => {
            assert_eq!(name, vec!["acme"]);
        }
        _ => panic!("expected Query subcommand"),
    }
}
