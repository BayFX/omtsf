/// File and stdin reading with size enforcement and multi-encoding parse support.
///
/// This module is the single entry point for all input I/O in the `omtsf`
/// binary. `omtsf-core` never touches the filesystem; all reading happens here.
///
/// Key behaviours:
/// - Disk files: size checked via `std::fs::metadata` before any read.
/// - Stdin: buffered with a `Read::take` cap so allocation is bounded.
/// - UTF-8 validation via `std::str::from_utf8` with byte-offset reporting.
/// - Multi-encoding parse via `omtsf_core::parse_omts` (JSON, CBOR, zstd).
/// - Decompression bomb guard: `max_decompressed = 4 * max_file_size`.
/// - All I/O errors are converted to [`CliError`] variants with exit code 2.
use std::io::Read as _;
use std::path::{Path, PathBuf};

use omtsf_core::{Encoding, OmtsDecodeError, OmtsFile, parse_omts};

use crate::PathOrStdin;
use crate::error::CliError;

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

/// Reads raw bytes from `source`, enforcing the size limit but without UTF-8
/// validation.
///
/// This is the correct low-level reader for binary formats (CBOR, zstd).
/// The returned `Vec<u8>` is passed directly to [`parse_omts`].
///
/// # Errors
///
/// Returns [`CliError`] (exit code 2) for:
/// - file not found
/// - permission denied
/// - file/stdin exceeds `max_size`
/// - any other I/O error
pub fn read_input_bytes(source: &PathOrStdin, max_size: u64) -> Result<Vec<u8>, CliError> {
    match source {
        PathOrStdin::Path(path) => read_file_bytes(path, max_size),
        PathOrStdin::Stdin => read_stdin_bytes(max_size),
    }
}

/// Reads raw bytes from `source` and parses them as an OMTSF file.
///
/// The complete read pipeline per SPEC-007 Section 4.6:
/// 1. Read bytes (size-checked).
/// 2. Call [`parse_omts`] to auto-detect encoding, decompress if zstd, and
///    parse as JSON or CBOR.
/// 3. If `verbose`, print `encoding: <name>` to stderr.
///
/// The decompression bomb guard applies a limit of `4 * max_file_size` on the
/// decompressed size of any zstd-wrapped input.
///
/// Returns the parsed [`OmtsFile`] and the detected innermost [`Encoding`].
///
/// # Errors
///
/// Returns [`CliError`] (exit code 2) for any read or parse failure.
pub fn read_and_parse(
    source: &PathOrStdin,
    max_file_size: u64,
    verbose: bool,
) -> Result<(OmtsFile, Encoding), CliError> {
    let source_label = source_label(source);
    let bytes = read_input_bytes(source, max_file_size)?;

    let max_decompressed = max_decompressed_limit(max_file_size);

    let (file, encoding) =
        parse_omts(&bytes, max_decompressed).map_err(|e| decode_error_to_cli(e, &source_label))?;

    if verbose {
        let enc_name = match encoding {
            Encoding::Json => "json",
            Encoding::Cbor => "cbor",
            Encoding::Zstd => "zstd",
        };
        eprintln!("encoding: {enc_name}");
    }

    Ok((file, encoding))
}

/// Computes the decompressed-size limit from `max_file_size`.
///
/// The limit is `4 * max_file_size`, capped at `usize::MAX` to avoid
/// overflow on 32-bit targets.
fn max_decompressed_limit(max_file_size: u64) -> usize {
    let four_x = max_file_size.saturating_mul(4);
    if four_x > usize::MAX as u64 {
        usize::MAX
    } else {
        four_x as usize
    }
}

/// Returns a human-readable label for the source.
fn source_label(source: &PathOrStdin) -> String {
    match source {
        PathOrStdin::Path(path) => path.display().to_string(),
        PathOrStdin::Stdin => "-".to_owned(),
    }
}

/// Maps an [`OmtsDecodeError`] to a [`CliError`].
fn decode_error_to_cli(e: OmtsDecodeError, source: &str) -> CliError {
    match e {
        OmtsDecodeError::EncodingDetection(inner) => CliError::EncodingDetectionFailed {
            source: source.to_owned(),
            first_bytes_hex: format!("{:02X?}", inner.first_bytes),
        },
        OmtsDecodeError::Cbor(inner) => CliError::ParseFailed {
            detail: format!("CBOR decode failed: {inner}"),
        },
        OmtsDecodeError::Json(inner) => CliError::ParseFailed {
            detail: format!("line {}, column {}: {inner}", inner.line(), inner.column()),
        },
        OmtsDecodeError::Compression(inner) => {
            use omtsf_core::CompressionError;
            match inner {
                CompressionError::SizeLimitExceeded { max_size } => {
                    CliError::DecompressedTooLarge {
                        source: source.to_owned(),
                        limit: max_size,
                    }
                }
                CompressionError::CompressionFailed(e) => CliError::ParseFailed {
                    detail: format!("compression failed: {e}"),
                },
                CompressionError::DecompressionFailed(e) => CliError::ParseFailed {
                    detail: format!("decompression failed: {e}"),
                },
            }
        }
        OmtsDecodeError::NestedCompression => CliError::ParseFailed {
            detail: "nested zstd compression is not supported".to_owned(),
        },
    }
}

/// Reads a disk file, enforcing the size limit and UTF-8 requirement.
fn read_file(path: &PathBuf, max_size: u64) -> Result<String, CliError> {
    let bytes = read_file_bytes(path, max_size)?;
    bytes_to_string(&bytes, &path.display().to_string())
}

/// Reads a disk file as raw bytes, enforcing the size limit.
fn read_file_bytes(path: &PathBuf, max_size: u64) -> Result<Vec<u8>, CliError> {
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

    match std::fs::read(path) {
        Ok(b) => Ok(b),
        Err(e) => Err(io_error_to_cli(&e, path)),
    }
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
        // Exhaustive match: all remaining kinds route to IoError.
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

/// Reads the entire stdin stream, capped at `max_size` bytes.
///
/// Uses `Read::take` so the buffer allocation is bounded. If the stream
/// produces exactly `max_size` bytes we perform one final byte read to
/// distinguish "exactly at the limit" from "over the limit".
fn read_stdin(max_size: u64) -> Result<String, CliError> {
    let buf = read_stdin_bytes(max_size)?;
    bytes_to_string(&buf, "-")
}

/// Reads the entire stdin stream as raw bytes, capped at `max_size`.
///
/// Uses `Read::take` so the buffer allocation is bounded. If the stream
/// produces exactly `max_size` bytes we perform one final byte read to
/// distinguish "exactly at the limit" from "over the limit".
fn read_stdin_bytes(max_size: u64) -> Result<Vec<u8>, CliError> {
    let stdin = std::io::stdin();
    let handle = stdin.lock();

    let mut limited = handle.take(max_size);
    let mut buf: Vec<u8> = Vec::new();

    limited
        .read_to_end(&mut buf)
        .map_err(|e| CliError::StdinReadError {
            detail: e.to_string(),
        })?;

    // Probe for overflow: if we got exactly max_size bytes, the stream may
    // have more data beyond the limit.
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

    Ok(buf)
}

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

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    #![allow(clippy::panic)]
    #![allow(clippy::wildcard_enum_match_arm)]

    use std::io::Write as _;

    use super::*;
    use crate::PathOrStdin;

    /// Creates a named temporary file with the given contents and returns its path.
    fn temp_file_with(contents: &[u8]) -> tempfile::NamedTempFile {
        let mut f = tempfile::NamedTempFile::new().expect("create temp file");
        f.write_all(contents).expect("write temp file");
        f
    }

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

    #[test]
    fn read_file_exactly_at_limit_succeeds() {
        let content = b"hello";
        let f = temp_file_with(content);
        let source = PathOrStdin::Path(f.path().to_path_buf());
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
        let content = b"hello world";
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

    #[test]
    fn read_invalid_utf8_returns_error_with_offset() {
        let mut data = b"hello".to_vec();
        data.push(0xFF);
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
        let data = vec![0xFF, 0xFE];
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

    #[test]
    fn read_nonexistent_file_returns_file_not_found() {
        let source = PathOrStdin::Path(PathBuf::from("/no/such/file/ever.omts"));
        let err = read_input(&source, 1024).expect_err("should fail");
        assert_eq!(err.exit_code(), 2);
        assert!(matches!(err, CliError::FileNotFound { .. }));
    }
}
