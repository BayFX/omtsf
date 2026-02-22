/// Errors produced during Excel import.
use std::fmt;

/// All error conditions that can occur while importing an Excel workbook.
///
/// Structural errors (missing sheets, missing columns, invalid cell values,
/// unresolved references) fail fast. Validation errors from L1 rules are
/// collected and returned as [`ImportError::ValidationFailed`].
#[derive(Debug)]
pub enum ImportError {
    /// A required sheet is missing from the workbook.
    MissingSheet {
        /// Name of the required sheet.
        sheet: String,
    },

    /// A required column header is missing from a sheet.
    MissingColumn {
        /// Name of the sheet.
        sheet: String,
        /// Name of the missing column.
        column: String,
    },

    /// A cell value could not be parsed into the expected type.
    InvalidCell {
        /// Cell reference in `{Sheet}!{Column}{Row}` format.
        cell_ref: String,
        /// Human-readable description of the expected format.
        expected: String,
        /// The raw value that was rejected.
        got: String,
    },

    /// An edge references a node ID that was not found in any node sheet.
    UnresolvedReference {
        /// Cell reference where the dangling reference appears.
        cell_ref: String,
        /// The node ID that could not be resolved.
        node_id: String,
    },

    /// L1 validation rules found errors in the constructed graph.
    ValidationFailed {
        /// Human-readable summary of all L1 errors found.
        diagnostics: Vec<String>,
    },

    /// Person nodes are present but `disclosure_scope` is public.
    PersonNodesWithPublicScope,

    /// An I/O or parsing error from the calamine library.
    ExcelRead {
        /// Human-readable description of the error.
        detail: String,
    },
}

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingSheet { sheet } => {
                write!(f, "missing required sheet: {sheet:?}")
            }
            Self::MissingColumn { sheet, column } => {
                write!(f, "missing required column {column:?} in sheet {sheet:?}")
            }
            Self::InvalidCell {
                cell_ref,
                expected,
                got,
            } => {
                write!(f, "{cell_ref}: expected {expected}, got {got:?}")
            }
            Self::UnresolvedReference { cell_ref, node_id } => {
                write!(f, "{cell_ref}: unresolved node reference {node_id:?}")
            }
            Self::ValidationFailed { diagnostics } => {
                write!(
                    f,
                    "L1 validation failed with {} error(s):\n{}",
                    diagnostics.len(),
                    diagnostics.join("\n")
                )
            }
            Self::PersonNodesWithPublicScope => {
                write!(
                    f,
                    "person nodes are present but disclosure_scope is \"public\"; \
                     per SPEC-004 Section 5 person nodes must be omitted from public files"
                )
            }
            Self::ExcelRead { detail } => {
                write!(f, "Excel read error: {detail}")
            }
        }
    }
}

impl std::error::Error for ImportError {}
