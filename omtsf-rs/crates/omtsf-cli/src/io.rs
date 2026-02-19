/// File and stdin reading with size enforcement and UTF-8 validation.
///
/// This module is the single entry point for all input I/O in the `omtsf`
/// binary. `omtsf-core` never touches the filesystem; all reading happens here.
///
/// Key behaviours:
/// - Disk files: size checked via `std::fs::metadata` before any read.
/// - Stdin: buffered with a `Read::take` cap so allocation is bounded.
/// - UTF-8 validation via `std::str::from_utf8` with byte-offset reporting.
/// - All I/O errors are converted to [`CliError`] variants with exit code 2.
use std::io::Read as _;
use std::path::{Path, PathBuf};

use crate::PathOrStdin;
use crate::error::CliError;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Reads the entire contents of `source` into a `String`.
///
/// For disk files the file length is checked against `max_size` via
/// `std::fs::metadata` before any bytes are read. For stdin a capped reader
/// (`Read::take`) is used so that the allocation is bounded.
///
/// # Errors
///
/// Returns [`CliError`] (exit code 2) for:
/// - file not found
/// - permission denied
/// - file exceeds `max_size`
/// - stdin stream exceeds `max_size`
/// - any other I/O error
/// - invalid UTF-8 (includes byte offset of the first bad sequence)
pub fn read_input(source: &PathOrStdin, max_size: u64) -> Result<String, CliError> {
    match source {
        PathOrStdin::Path(path) => read_file(path, max_size),
        PathOrStdin::Stdin => read_stdin(max_size),
    }
}

// ---------------------------------------------------------------------------
// Disk file reading
// ---------------------------------------------------------------------------

/// Reads a disk file, enforcing the size limit and UTF-8 requirement.
fn read_file(path: &PathBuf, max_size: u64) -> Result<String, CliError> {
    // Size check via metadata — no allocation until we know it's within bounds.
    let file_size = match std::fs::metadata(path) {
        Ok(meta) => meta.len(),
        Err(e) => {
            return Err(io_error_to_cli(&e, path));
        }
    };

    if file_size > max_size {
        return Err(CliError::FileTooLarge {
            source: path.display().to_string(),
            limit: max_size,
            actual: Some(file_size),
        });
    }

    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            return Err(io_error_to_cli(&e, path));
        }
    };

    bytes_to_string(&bytes, &path.display().to_string())
}

/// Maps a `std::io::Error` arising from a disk-file operation to a [`CliError`].
fn io_error_to_cli(e: &std::io::Error, path: &Path) -> CliError {
    match e.kind() {
        std::io::ErrorKind::NotFound => CliError::FileNotFound {
            path: path.to_path_buf(),
        },
        std::io::ErrorKind::PermissionDenied => CliError::PermissionDenied {
            path: path.to_path_buf(),
        },
        // All other I/O error kinds are wrapped in the generic IoError variant.
        // We list a few common ones explicitly to silence the exhaustiveness
        // lint while still routing everything unknown to IoError.
        std::io::ErrorKind::ConnectionRefused
        | std::io::ErrorKind::ConnectionReset
        | std::io::ErrorKind::HostUnreachable
        | std::io::ErrorKind::NetworkUnreachable
        | std::io::ErrorKind::ConnectionAborted
        | std::io::ErrorKind::NotConnected
        | std::io::ErrorKind::AddrInUse
        | std::io::ErrorKind::AddrNotAvailable
        | std::io::ErrorKind::NetworkDown
        | std::io::ErrorKind::BrokenPipe
        | std::io::ErrorKind::AlreadyExists
        | std::io::ErrorKind::WouldBlock
        | std::io::ErrorKind::NotADirectory
        | std::io::ErrorKind::IsADirectory
        | std::io::ErrorKind::DirectoryNotEmpty
        | std::io::ErrorKind::ReadOnlyFilesystem
        | std::io::ErrorKind::StaleNetworkFileHandle
        | std::io::ErrorKind::InvalidInput
        | std::io::ErrorKind::InvalidData
        | std::io::ErrorKind::TimedOut
        | std::io::ErrorKind::WriteZero
        | std::io::ErrorKind::StorageFull
        | std::io::ErrorKind::NotSeekable
        | std::io::ErrorKind::QuotaExceeded
        | std::io::ErrorKind::FileTooLarge
        | std::io::ErrorKind::ResourceBusy
        | std::io::ErrorKind::ExecutableFileBusy
        | std::io::ErrorKind::Deadlock
        | std::io::ErrorKind::CrossesDevices
        | std::io::ErrorKind::TooManyLinks
        | std::io::ErrorKind::ArgumentListTooLong
        | std::io::ErrorKind::Interrupted
        | std::io::ErrorKind::Unsupported
        | std::io::ErrorKind::UnexpectedEof
        | std::io::ErrorKind::OutOfMemory
        | std::io::ErrorKind::Other
        | _ => CliError::IoError {
            source: path.display().to_string(),
            detail: e.to_string(),
        },
    }
}

// ---------------------------------------------------------------------------
// Stdin reading
// ---------------------------------------------------------------------------

/// Reads the entire stdin stream, capped at `max_size` bytes.
///
/// Uses `Read::take` so the buffer allocation is bounded. If the stream
/// produces exactly `max_size` bytes we perform one final byte read to
/// distinguish "exactly at the limit" from "over the limit".
fn read_stdin(max_size: u64) -> Result<String, CliError> {
    let stdin = std::io::stdin();
    let handle = stdin.lock();

    // Read at most max_size bytes; allocate no more.
    let mut limited = handle.take(max_size);
    let mut buf: Vec<u8> = Vec::new();

    limited
        .read_to_end(&mut buf)
        .map_err(|e| CliError::StdinReadError {
            detail: e.to_string(),
        })?;

    // If we read exactly max_size bytes the stream may still have more data.
    // Attempt to read one additional byte to detect overflow.
    if buf.len() as u64 == max_size {
        let stdin2 = std::io::stdin();
        let mut handle2 = stdin2.lock();
        let mut probe = [0u8; 1];
        let extra = handle2
            .read(&mut probe)
            .map_err(|e| CliError::StdinReadError {
                detail: e.to_string(),
            })?;
        if extra > 0 {
            return Err(CliError::FileTooLarge {
                source: "-".to_owned(),
                limit: max_size,
                actual: None,
            });
        }
    }

    bytes_to_string(&buf, "-")
}

// ---------------------------------------------------------------------------
// UTF-8 conversion
// ---------------------------------------------------------------------------

/// Converts a byte buffer to a `String`, returning a [`CliError`] with the
/// byte offset of the first invalid sequence on failure.
fn bytes_to_string(bytes: &[u8], source_label: &str) -> Result<String, CliError> {
    match std::str::from_utf8(bytes) {
        Ok(s) => Ok(s.to_owned()),
        Err(e) => Err(CliError::InvalidUtf8 {
            source: source_label.to_owned(),
            byte_offset: e.valid_up_to(),
        }),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    #![allow(clippy::panic)]
    #![allow(clippy::wildcard_enum_match_arm)]

    use std::io::Write as _;

    use super::*;
    use crate::PathOrStdin;

    // ── helpers ──────────────────────────────────────────────────────────────

    /// Creates a named temporary file with the given contents and returns its path.
    fn temp_file_with(contents: &[u8]) -> tempfile::NamedTempFile {
        let mut f = tempfile::NamedTempFile::new().expect("create temp file");
        f.write_all(contents).expect("write temp file");
        f
    }

    // ── disk file: happy path ────────────────────────────────────────────────

    #[test]
    fn read_valid_utf8_file() {
        let content = r#"{"hello":"world"}"#;
        let f = temp_file_with(content.as_bytes());
        let source = PathOrStdin::Path(f.path().to_path_buf());
        let result = read_input(&source, 1024).expect("should read file");
        assert_eq!(result, content);
    }

    #[test]
    fn read_empty_file() {
        let f = temp_file_with(b"");
        let source = PathOrStdin::Path(f.path().to_path_buf());
        let result = read_input(&source, 1024).expect("should read empty file");
        assert_eq!(result, "");
    }

    // ── disk file: size limit ────────────────────────────────────────────────

    #[test]
    fn read_file_exactly_at_limit_succeeds() {
        let content = b"hello";
        let f = temp_file_with(content);
        let source = PathOrStdin::Path(f.path().to_path_buf());
        // 5 bytes is exactly at the limit of 5.
        let result = read_input(&source, 5).expect("should succeed at limit");
        assert_eq!(result, "hello");
    }

    #[test]
    fn read_file_over_limit_returns_error() {
        let content = b"hello world";
        let f = temp_file_with(content);
        let source = PathOrStdin::Path(f.path().to_path_buf());
        let err = read_input(&source, 5).expect_err("should fail over limit");
        assert_eq!(err.exit_code(), 2);
        let msg = err.message();
        assert!(
            msg.contains("too large") || msg.contains("exceeded"),
            "message: {msg}"
        );
    }

    #[test]
    fn read_file_over_limit_reports_actual_size() {
        let content = b"hello world"; // 11 bytes
        let f = temp_file_with(content);
        let source = PathOrStdin::Path(f.path().to_path_buf());
        let err = read_input(&source, 4).expect_err("should fail");
        match err {
            CliError::FileTooLarge {
                actual: Some(n), ..
            } => {
                assert_eq!(n, 11, "actual size should be 11");
            }
            other => panic!("expected FileTooLarge, got {other:?}"),
        }
    }

    // ── disk file: UTF-8 validation ──────────────────────────────────────────

    #[test]
    fn read_invalid_utf8_returns_error_with_offset() {
        // Valid ASCII up to byte 5, then an invalid byte sequence.
        let mut data = b"hello".to_vec();
        data.push(0xFF); // invalid UTF-8 byte
        let f = temp_file_with(&data);
        let source = PathOrStdin::Path(f.path().to_path_buf());
        let err = read_input(&source, 1024).expect_err("should fail on bad UTF-8");
        assert_eq!(err.exit_code(), 2);
        match err {
            CliError::InvalidUtf8 { byte_offset, .. } => {
                assert_eq!(byte_offset, 5, "first valid bytes: 'hello' = 5 bytes");
            }
            other => panic!("expected InvalidUtf8, got {other:?}"),
        }
    }

    #[test]
    fn read_invalid_utf8_at_start_offset_is_zero() {
        let data = vec![0xFF, 0xFE]; // immediately invalid
        let f = temp_file_with(&data);
        let source = PathOrStdin::Path(f.path().to_path_buf());
        let err = read_input(&source, 1024).expect_err("should fail");
        match err {
            CliError::InvalidUtf8 { byte_offset, .. } => {
                assert_eq!(byte_offset, 0);
            }
            other => panic!("expected InvalidUtf8, got {other:?}"),
        }
    }

    // ── disk file: I/O errors ────────────────────────────────────────────────

    #[test]
    fn read_nonexistent_file_returns_file_not_found() {
        let source = PathOrStdin::Path(PathBuf::from("/no/such/file/ever.omts"));
        let err = read_input(&source, 1024).expect_err("should fail");
        assert_eq!(err.exit_code(), 2);
        assert!(matches!(err, CliError::FileNotFound { .. }));
    }
}
