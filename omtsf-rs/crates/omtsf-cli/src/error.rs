/// CLI error types with associated exit codes.
///
/// [`CliError`] is the top-level error type for the `omtsf` binary. Every
/// variant maps to a stable exit code (1 or 2) via [`CliError::exit_code`]:
///
/// - Exit code **2** — input failure: the tool could not read or parse the
///   input at all. These errors terminate early before any domain logic runs.
/// - Exit code **1** — logical failure: the tool ran to completion but the
///   result is a well-defined failure (validation errors, merge conflict, etc.).
use std::fmt;
use std::path::PathBuf;

/// All error conditions that the `omtsf` CLI can produce.
///
/// Use [`CliError::exit_code`] to obtain the exit code associated with each
/// variant. [`CliError::message`] returns the human-readable error string
/// that should be printed to stderr before exiting.
#[derive(Debug)]
pub enum CliError {
    /// A file argument could not be found on the filesystem.
    FileNotFound {
        /// The path that was not found.
        path: PathBuf,
    },

    /// The process lacks permission to read a file.
    PermissionDenied {
        /// The path that could not be read.
        path: PathBuf,
    },

    /// The input exceeds the configured [`--max-file-size`] limit.
    FileTooLarge {
        /// A human-readable label for the source (`"-"` for stdin, or the
        /// filesystem path).
        source: String,
        /// The configured size limit in bytes.
        limit: u64,
        /// The actual size in bytes, if known (disk files only; `None` for
        /// stdin where the exact size is unknown).
        actual: Option<u64>,
    },

    /// The input bytes are not valid UTF-8.
    InvalidUtf8 {
        /// A human-readable label for the source.
        source: String,
        /// The byte offset of the first invalid byte sequence.
        byte_offset: usize,
    },

    /// An I/O error occurred while reading from stdin.
    StdinReadError {
        /// The underlying I/O error message.
        detail: String,
    },

    /// A generic I/O error not covered by the more specific variants above.
    IoError {
        /// A human-readable label for the source.
        source: String,
        /// The underlying I/O error message.
        detail: String,
    },

    /// The decompressed size of a zstd input exceeded the configured limit.
    ///
    /// This guard prevents decompression bombs from exhausting available memory.
    DecompressedTooLarge {
        /// A human-readable label for the source.
        source: String,
        /// The configured limit in bytes (`4 * max_file_size`).
        limit: usize,
    },

    /// The initial bytes of the input do not match any known encoding.
    EncodingDetectionFailed {
        /// A human-readable label for the source.
        source: String,
        /// The first bytes that were inspected (up to 4 bytes), hex-formatted.
        first_bytes_hex: String,
    },

    /// The input is not a valid OMTSF file (not valid JSON or missing required
    /// fields).
    ///
    /// This is distinct from I/O errors: the bytes were read successfully but
    /// could not be parsed as an [`omtsf_core::OmtsFile`].
    ParseFailed {
        /// A human-readable description of the parse failure.
        detail: String,
    },

    /// A validation pass found one or more L1 errors.
    ///
    /// The diagnostics have already been printed; this variant exists so
    /// `main` can call `process::exit(1)` cleanly.
    ValidationErrors,

    /// A merge operation produced an unresolvable conflict.
    MergeConflict {
        /// A description of the conflict.
        detail: String,
    },

    /// A node ID supplied to a graph query was not found in the graph.
    NodeNotFound {
        /// The node ID that could not be resolved.
        node_id: String,
    },

    /// A graph query returned no results (e.g. no path found).
    NoResults {
        /// A human-readable description of what was not found.
        detail: String,
    },

    /// The graph could not be built from the input file.
    GraphBuildError {
        /// A description of the construction error.
        detail: String,
    },

    /// A diff was computed successfully and found at least one difference.
    ///
    /// The diff output has already been written to stdout; this variant exists
    /// so `main` can call `process::exit(1)` cleanly (following the `diff(1)`
    /// convention: exit 1 = differences found, not an error).
    DiffHasDifferences,

    /// A redaction operation failed due to a scope or engine error.
    ///
    /// This covers the case where the target scope is less restrictive than
    /// the existing `disclosure_scope` of the input file, or when the
    /// redaction engine produces invalid output.
    RedactionError {
        /// A description of the redaction error.
        detail: String,
    },

    /// An invalid command-line argument value was provided.
    ///
    /// This covers cases where clap accepted the raw string but the value
    /// failed domain validation (e.g. a malformed `--jurisdiction` code or
    /// an empty selector set on `query`/`extract-subchain`).
    InvalidArgument {
        /// A human-readable description of what was wrong.
        detail: String,
    },

    /// An internal error that indicates a bug in omtsf.
    ///
    /// These should never occur under normal operation. If one appears it
    /// means a condition that is logically impossible was reached, and the
    /// user should file a bug report.
    InternalError {
        /// A description of the internal failure.
        detail: String,
    },
}

impl CliError {
    /// Returns the process exit code for this error.
    ///
    /// - `2` — input failure (file not found, parse error, etc.).
    /// - `1` — logical failure (validation errors, merge conflict, etc.).
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::FileNotFound { .. }
            | Self::PermissionDenied { .. }
            | Self::FileTooLarge { .. }
            | Self::InvalidUtf8 { .. }
            | Self::StdinReadError { .. }
            | Self::IoError { .. }
            | Self::DecompressedTooLarge { .. }
            | Self::EncodingDetectionFailed { .. }
            | Self::ParseFailed { .. } => 2,

            Self::ValidationErrors
            | Self::MergeConflict { .. }
            | Self::NodeNotFound { .. }
            | Self::NoResults { .. }
            | Self::DiffHasDifferences
            | Self::RedactionError { .. } => 1,

            Self::GraphBuildError { .. }
            | Self::InternalError { .. }
            | Self::InvalidArgument { .. } => 2,
        }
    }

    /// Returns a human-readable error message suitable for printing to stderr.
    ///
    /// Every message includes: what went wrong, which file/node/edge is
    /// affected (where applicable), and guidance on how to fix it.
    pub fn message(&self) -> String {
        match self {
            Self::FileNotFound { path } => {
                format!(
                    "error: file not found: {}\n\
                     hint: check that the path is correct and the file exists",
                    path.display()
                )
            }
            Self::PermissionDenied { path } => {
                format!(
                    "error: permission denied reading {}\n\
                     hint: check that you have read access to this file",
                    path.display()
                )
            }
            Self::FileTooLarge {
                source,
                limit,
                actual: Some(actual),
            } => {
                format!(
                    "error: file too large: {source} is {actual} bytes, limit is {limit} bytes\n\
                     hint: use --max-file-size to raise the limit, or split the file into smaller inputs"
                )
            }
            Self::FileTooLarge {
                source,
                limit,
                actual: None,
            } => {
                format!(
                    "error: file too large: {source} exceeded limit of {limit} bytes\n\
                     hint: use --max-file-size to raise the limit, or split the input into smaller chunks"
                )
            }
            Self::InvalidUtf8 {
                source,
                byte_offset,
            } => {
                format!(
                    "error: invalid UTF-8 in {source}: first invalid byte at offset {byte_offset}\n\
                     hint: ensure the file is saved as UTF-8; re-encode it with `iconv -t UTF-8` if needed"
                )
            }
            Self::StdinReadError { detail } => {
                format!(
                    "error: failed to read stdin: {detail}\n\
                     hint: ensure the piped input is not truncated and the source process exited cleanly"
                )
            }
            Self::IoError { source, detail } => {
                format!("error: I/O error reading {source}: {detail}")
            }
            Self::DecompressedTooLarge { source, limit } => {
                format!(
                    "error: decompressed size of {source} exceeds limit of {limit} bytes\n\
                     hint: this may be a decompression bomb; use --max-file-size to raise the \
                     limit only if the source is trusted"
                )
            }
            Self::EncodingDetectionFailed {
                source,
                first_bytes_hex,
            } => {
                format!(
                    "error: unrecognized encoding in {source}: first bytes are {first_bytes_hex}\n\
                     hint: ensure the file is a valid .omts file in JSON, CBOR, or zstd-compressed format"
                )
            }
            Self::ParseFailed { detail } => {
                format!(
                    "error: failed to parse input as an OMTSF file: {detail}\n\
                     hint: validate the JSON syntax and ensure all required fields \
                     (omtsf_version, snapshot_date, file_salt, nodes, edges) are present"
                )
            }
            Self::ValidationErrors => "error: validation failed with one or more errors\n\
                 hint: review the diagnostics above and correct the reported fields"
                .to_owned(),
            Self::MergeConflict { detail } => {
                format!(
                    "error: merge conflict: {detail}\n\
                     hint: ensure each input file is individually valid before merging"
                )
            }
            Self::NodeNotFound { node_id } => {
                format!(
                    "error: node not found: {node_id:?}\n\
                     hint: run `omtsf inspect <file>` to list node IDs present in the graph"
                )
            }
            Self::NoResults { detail } => {
                format!(
                    "error: {detail}\n\
                     hint: verify the node IDs are correct and the graph contains the expected edges"
                )
            }
            Self::GraphBuildError { detail } => {
                format!(
                    "error: could not build graph from input: {detail}\n\
                     hint: run `omtsf validate <file>` to check for structural errors first"
                )
            }
            Self::DiffHasDifferences => "diff: files differ".to_owned(),
            Self::RedactionError { detail } => {
                format!(
                    "error: redaction failed: {detail}\n\
                     hint: the target --scope must be at least as restrictive as the \
                     file's existing disclosure_scope"
                )
            }
            Self::InvalidArgument { detail } => {
                format!(
                    "error: invalid argument: {detail}\n\
                     hint: run `omtsf <subcommand> --help` for usage information"
                )
            }
            Self::InternalError { detail } => internal_error_message(detail),
        }
    }
}

/// Formats a message for an internal error (bug) that should be reported.
///
/// Call this for error paths that indicate a bug in omtsf rather than a
/// user input problem.
pub fn internal_error_message(detail: &str) -> String {
    format!(
        "error: internal error: {detail}\n\
         This is a bug in omtsf. Please report it at \
         https://github.com/omtsf/omtsf-rs/issues"
    )
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message())
    }
}

impl std::error::Error for CliError {}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use std::path::PathBuf;

    use super::*;

    #[test]
    fn file_not_found_is_exit_2() {
        let e = CliError::FileNotFound {
            path: PathBuf::from("foo.omts"),
        };
        assert_eq!(e.exit_code(), 2);
    }

    #[test]
    fn permission_denied_is_exit_2() {
        let e = CliError::PermissionDenied {
            path: PathBuf::from("/root/secret.omts"),
        };
        assert_eq!(e.exit_code(), 2);
    }

    #[test]
    fn file_too_large_is_exit_2() {
        let e = CliError::FileTooLarge {
            source: "big.omts".to_owned(),
            limit: 1024,
            actual: Some(2048),
        };
        assert_eq!(e.exit_code(), 2);
    }

    #[test]
    fn invalid_utf8_is_exit_2() {
        let e = CliError::InvalidUtf8 {
            source: "bad.omts".to_owned(),
            byte_offset: 42,
        };
        assert_eq!(e.exit_code(), 2);
    }

    #[test]
    fn stdin_read_error_is_exit_2() {
        let e = CliError::StdinReadError {
            detail: "broken pipe".to_owned(),
        };
        assert_eq!(e.exit_code(), 2);
    }

    #[test]
    fn io_error_is_exit_2() {
        let e = CliError::IoError {
            source: "file.omts".to_owned(),
            detail: "device full".to_owned(),
        };
        assert_eq!(e.exit_code(), 2);
    }

    #[test]
    fn validation_errors_is_exit_1() {
        assert_eq!(CliError::ValidationErrors.exit_code(), 1);
    }

    #[test]
    fn merge_conflict_is_exit_1() {
        let e = CliError::MergeConflict {
            detail: "property collision on org-001".to_owned(),
        };
        assert_eq!(e.exit_code(), 1);
    }

    #[test]
    fn file_not_found_message_contains_path() {
        let e = CliError::FileNotFound {
            path: PathBuf::from("supply-chain.omts"),
        };
        let msg = e.message();
        assert!(msg.contains("supply-chain.omts"), "message: {msg}");
        assert!(msg.contains("not found"), "message: {msg}");
    }

    #[test]
    fn permission_denied_message_contains_path() {
        let e = CliError::PermissionDenied {
            path: PathBuf::from("/etc/shadow"),
        };
        let msg = e.message();
        assert!(msg.contains("/etc/shadow"), "message: {msg}");
        assert!(msg.contains("permission denied"), "message: {msg}");
    }

    #[test]
    fn file_too_large_with_actual_mentions_sizes() {
        let e = CliError::FileTooLarge {
            source: "big.omts".to_owned(),
            limit: 1_000_000,
            actual: Some(2_000_000),
        };
        let msg = e.message();
        assert!(msg.contains("2000000"), "message: {msg}");
        assert!(msg.contains("1000000"), "message: {msg}");
    }

    #[test]
    fn file_too_large_without_actual_mentions_limit() {
        let e = CliError::FileTooLarge {
            source: "-".to_owned(),
            limit: 512,
            actual: None,
        };
        let msg = e.message();
        assert!(msg.contains("512"), "message: {msg}");
    }

    #[test]
    fn invalid_utf8_message_contains_offset() {
        let e = CliError::InvalidUtf8 {
            source: "corrupt.omts".to_owned(),
            byte_offset: 99,
        };
        let msg = e.message();
        assert!(msg.contains("99"), "message: {msg}");
        assert!(msg.contains("corrupt.omts"), "message: {msg}");
    }

    #[test]
    fn display_matches_message() {
        let e = CliError::FileNotFound {
            path: PathBuf::from("x.omts"),
        };
        assert_eq!(format!("{e}"), e.message());
    }

    #[test]
    fn error_trait_is_implemented() {
        let e: Box<dyn std::error::Error> = Box::new(CliError::ValidationErrors);
        assert!(!e.to_string().is_empty());
    }

    #[test]
    fn internal_error_is_exit_2() {
        let e = CliError::InternalError {
            detail: "unexpected None in resolved id".to_owned(),
        };
        assert_eq!(e.exit_code(), 2);
    }

    #[test]
    fn internal_error_message_contains_bug_report_url() {
        let e = CliError::InternalError {
            detail: "unreachable branch".to_owned(),
        };
        let msg = e.message();
        assert!(
            msg.contains("https://github.com/omtsf/omtsf-rs/issues"),
            "message should contain bug report URL: {msg}"
        );
    }

    #[test]
    fn internal_error_message_contains_detail() {
        let e = CliError::InternalError {
            detail: "my specific detail".to_owned(),
        };
        let msg = e.message();
        assert!(
            msg.contains("my specific detail"),
            "message should contain detail: {msg}"
        );
    }

    #[test]
    fn internal_error_message_fn_contains_url() {
        let msg = super::internal_error_message("test detail");
        assert!(
            msg.contains("https://github.com/omtsf/omtsf-rs/issues"),
            "helper should contain bug report URL: {msg}"
        );
    }

    #[test]
    fn parse_failed_message_contains_detail() {
        let e = CliError::ParseFailed {
            detail: "line 3, column 5: expected value".to_owned(),
        };
        let msg = e.message();
        assert!(msg.contains("line 3"), "message should contain line: {msg}");
        assert!(
            msg.contains("column 5"),
            "message should contain column: {msg}"
        );
    }

    #[test]
    fn parse_failed_message_contains_guidance() {
        let e = CliError::ParseFailed {
            detail: "some error".to_owned(),
        };
        let msg = e.message();
        assert!(
            msg.contains("omtsf_version") || msg.contains("required fields"),
            "message should mention required fields: {msg}"
        );
    }

    #[test]
    fn file_not_found_message_contains_hint() {
        let e = CliError::FileNotFound {
            path: PathBuf::from("missing.omts"),
        };
        let msg = e.message();
        assert!(
            msg.contains("hint") || msg.contains("check"),
            "message should contain guidance: {msg}"
        );
    }

    #[test]
    fn node_not_found_message_contains_guidance() {
        let e = CliError::NodeNotFound {
            node_id: "org-999".to_owned(),
        };
        let msg = e.message();
        assert!(
            msg.contains("org-999"),
            "message should contain node ID: {msg}"
        );
        assert!(
            msg.contains("hint") || msg.contains("inspect"),
            "message should contain guidance: {msg}"
        );
    }

    #[test]
    fn graph_build_error_message_contains_guidance() {
        let e = CliError::GraphBuildError {
            detail: "duplicate node ID org-001".to_owned(),
        };
        let msg = e.message();
        assert!(
            msg.contains("validate") || msg.contains("hint"),
            "message should suggest validate: {msg}"
        );
    }
}
