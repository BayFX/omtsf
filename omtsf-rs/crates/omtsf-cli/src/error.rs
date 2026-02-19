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

// ---------------------------------------------------------------------------
// CliError
// ---------------------------------------------------------------------------

/// All error conditions that the `omtsf` CLI can produce.
///
/// Use [`CliError::exit_code`] to obtain the exit code associated with each
/// variant. [`CliError::message`] returns the human-readable error string
/// that should be printed to stderr before exiting.
#[derive(Debug)]
pub enum CliError {
    // --- Exit code 2: input failures ---
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

    // --- Exit code 1: logical failures ---
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
            | Self::IoError { .. } => 2,

            Self::ValidationErrors | Self::MergeConflict { .. } => 1,
        }
    }

    /// Returns a human-readable error message suitable for printing to stderr.
    pub fn message(&self) -> String {
        match self {
            Self::FileNotFound { path } => {
                format!("error: file not found: {}", path.display())
            }
            Self::PermissionDenied { path } => {
                format!("error: permission denied: {}", path.display())
            }
            Self::FileTooLarge {
                source,
                limit,
                actual: Some(actual),
            } => {
                format!("error: file too large: {source} is {actual} bytes, limit is {limit} bytes")
            }
            Self::FileTooLarge {
                source,
                limit,
                actual: None,
            } => {
                format!("error: file too large: {source} exceeded limit of {limit} bytes")
            }
            Self::InvalidUtf8 {
                source,
                byte_offset,
            } => {
                format!(
                    "error: invalid UTF-8 in {source}: first invalid byte at offset {byte_offset}"
                )
            }
            Self::StdinReadError { detail } => {
                format!("error: failed to read stdin: {detail}")
            }
            Self::IoError { source, detail } => {
                format!("error: I/O error reading {source}: {detail}")
            }
            Self::ValidationErrors => "error: validation failed with one or more errors".to_owned(),
            Self::MergeConflict { detail } => {
                format!("error: merge conflict: {detail}")
            }
        }
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message())
    }
}

impl std::error::Error for CliError {}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use std::path::PathBuf;

    use super::*;

    // ── exit_code ────────────────────────────────────────────────────────────

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

    // ── message content ──────────────────────────────────────────────────────

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
}
