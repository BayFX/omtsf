/// Implementation of `omtsf import`.
///
/// Reads an external format file (currently only Excel `.xlsx`) and writes a
/// valid `.omts` file to stdout or a specified output path.
///
/// Exit codes:
/// - 0 = success
/// - 1 = import failed (validation errors, person+public scope conflict)
/// - 2 = file not found, I/O error, or unknown format
use std::fs;
use std::io::{self, Write as _};
use std::path::Path;

use omtsf_excel::ImportError;

use crate::ImportFormat;
use crate::error::CliError;

/// Runs the `import` command.
///
/// Reads `file` in the specified `format`, writes the resulting `.omts` JSON
/// to `output` (or stdout when `output` is `None`).
///
/// # Errors
///
/// Returns [`CliError`] on I/O failures, unknown format, or import errors.
pub fn run(file: &Path, format: &ImportFormat, output: Option<&Path>) -> Result<(), CliError> {
    match format {
        ImportFormat::Excel => run_excel(file, output),
    }
}

fn run_excel(file: &Path, output: Option<&Path>) -> Result<(), CliError> {
    let reader = fs::File::open(file).map_err(|e| {
        use std::io::ErrorKind;
        match e.kind() {
            ErrorKind::NotFound => CliError::FileNotFound {
                path: file.to_path_buf(),
            },
            ErrorKind::PermissionDenied => CliError::PermissionDenied {
                path: file.to_path_buf(),
            },
            ErrorKind::ConnectionRefused
            | ErrorKind::ConnectionReset
            | ErrorKind::HostUnreachable
            | ErrorKind::NetworkUnreachable
            | ErrorKind::ConnectionAborted
            | ErrorKind::NotConnected
            | ErrorKind::AddrInUse
            | ErrorKind::AddrNotAvailable
            | ErrorKind::NetworkDown
            | ErrorKind::BrokenPipe
            | ErrorKind::AlreadyExists
            | ErrorKind::WouldBlock
            | ErrorKind::NotADirectory
            | ErrorKind::IsADirectory
            | ErrorKind::DirectoryNotEmpty
            | ErrorKind::ReadOnlyFilesystem
            | ErrorKind::StaleNetworkFileHandle
            | ErrorKind::InvalidInput
            | ErrorKind::InvalidData
            | ErrorKind::TimedOut
            | ErrorKind::WriteZero
            | ErrorKind::StorageFull
            | ErrorKind::NotSeekable
            | ErrorKind::QuotaExceeded
            | ErrorKind::FileTooLarge
            | ErrorKind::ResourceBusy
            | ErrorKind::ExecutableFileBusy
            | ErrorKind::Deadlock
            | ErrorKind::CrossesDevices
            | ErrorKind::TooManyLinks
            | ErrorKind::ArgumentListTooLong
            | ErrorKind::Interrupted
            | ErrorKind::Unsupported
            | ErrorKind::UnexpectedEof
            | ErrorKind::OutOfMemory
            | ErrorKind::Other
            | _ => CliError::IoError {
                source: file.display().to_string(),
                detail: e.to_string(),
            },
        }
    })?;

    let omts_file = omtsf_excel::import_excel(reader).map_err(|e| map_import_error(e, file))?;

    let json = serde_json::to_string_pretty(&omts_file).map_err(|e| CliError::IoError {
        source: "import".to_owned(),
        detail: format!("JSON serialization failed: {e}"),
    })?;

    match output {
        Some(out_path) => {
            fs::write(out_path, json.as_bytes()).map_err(|e| CliError::IoError {
                source: out_path.display().to_string(),
                detail: e.to_string(),
            })?;
        }
        None => {
            let stdout = io::stdout();
            let mut handle = stdout.lock();
            handle
                .write_all(json.as_bytes())
                .map_err(|e| CliError::IoError {
                    source: "stdout".to_owned(),
                    detail: e.to_string(),
                })?;
            handle.write_all(b"\n").map_err(|e| CliError::IoError {
                source: "stdout".to_owned(),
                detail: e.to_string(),
            })?;
        }
    }

    Ok(())
}

/// Maps an [`ImportError`] to a [`CliError`].
fn map_import_error(e: ImportError, _file: &Path) -> CliError {
    match e {
        ImportError::ExcelRead { detail } => CliError::ParseFailed { detail },
        ImportError::MissingSheet { .. }
        | ImportError::MissingColumn { .. }
        | ImportError::InvalidCell { .. }
        | ImportError::UnresolvedReference { .. } => CliError::ParseFailed {
            detail: e.to_string(),
        },
        ImportError::ValidationFailed { .. } => CliError::InvalidArgument {
            detail: e.to_string(),
        },
        ImportError::PersonNodesWithPublicScope => CliError::InvalidArgument {
            detail: e.to_string(),
        },
    }
}
