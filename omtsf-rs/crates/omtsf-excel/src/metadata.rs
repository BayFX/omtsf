/// Parses the Metadata sheet into top-level file fields.
///
/// The Metadata sheet uses a key/value layout (column A = field name,
/// column B = value). Required field: `snapshot_date`. Optional fields:
/// `reporting_entity`, `disclosure_scope`, `default_confidence`,
/// `default_source`.
use std::collections::HashMap;

use calamine::{Data, Range};

use omtsf_core::enums::{Confidence, DisclosureScope};
use omtsf_core::newtypes::CalendarDate;

use crate::error::ImportError;
use crate::sheet::cell_to_string;

/// Parsed values from the Metadata sheet.
#[derive(Debug)]
pub struct FileMetadata {
    /// Required snapshot date in `YYYY-MM-DD` format.
    pub snapshot_date: CalendarDate,
    /// Optional reporting entity node ID.
    pub reporting_entity: Option<String>,
    /// Optional disclosure scope.
    pub disclosure_scope: Option<DisclosureScope>,
    /// Default data quality confidence for all imported records.
    pub default_confidence: Option<Confidence>,
    /// Default data quality source string.
    pub default_source: Option<String>,
}

/// Parses the Metadata sheet.
///
/// # Errors
///
/// Returns [`ImportError::MissingSheet`] if the sheet is absent.
/// Returns [`ImportError::InvalidCell`] if `snapshot_date` is missing or malformed.
pub fn parse_metadata(sheet: &Range<Data>) -> Result<FileMetadata, ImportError> {
    let kv = build_kv_map(sheet);

    let snapshot_date_raw = kv.get("snapshot_date").cloned().unwrap_or_default();
    if snapshot_date_raw.is_empty() {
        return Err(ImportError::InvalidCell {
            cell_ref: "Metadata!B2".to_owned(),
            expected: "YYYY-MM-DD date".to_owned(),
            got: String::new(),
        });
    }
    let snapshot_date = CalendarDate::try_from(snapshot_date_raw.as_str()).map_err(|e| {
        ImportError::InvalidCell {
            cell_ref: "Metadata!B2".to_owned(),
            expected: "YYYY-MM-DD date".to_owned(),
            got: format!("{snapshot_date_raw} ({e})"),
        }
    })?;

    let reporting_entity = kv
        .get("reporting_entity")
        .filter(|s| !s.is_empty())
        .cloned();

    let disclosure_scope = kv
        .get("disclosure_scope")
        .filter(|s| !s.is_empty())
        .map(|s| parse_disclosure_scope(s))
        .transpose()?;

    let default_confidence = kv
        .get("default_confidence")
        .filter(|s| !s.is_empty())
        .map(|s| parse_confidence(s))
        .transpose()?;

    let default_source = kv.get("default_source").filter(|s| !s.is_empty()).cloned();

    Ok(FileMetadata {
        snapshot_date,
        reporting_entity,
        disclosure_scope,
        default_confidence,
        default_source,
    })
}

/// Builds a lowercase key → value map from the key/value sheet layout.
fn build_kv_map(sheet: &Range<Data>) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for row in sheet.rows() {
        let key = row.first().map(cell_to_string).unwrap_or_default();
        let val = row.get(1).map(cell_to_string).unwrap_or_default();
        let key = key.trim().to_lowercase();
        if !key.is_empty() && key != "field" {
            map.insert(key, val.trim().to_owned());
        }
    }
    map
}

fn parse_disclosure_scope(s: &str) -> Result<DisclosureScope, ImportError> {
    match s.trim().to_lowercase().as_str() {
        "internal" => Ok(DisclosureScope::Internal),
        "partner" => Ok(DisclosureScope::Partner),
        "public" => Ok(DisclosureScope::Public),
        other => Err(ImportError::InvalidCell {
            cell_ref: "Metadata!B4".to_owned(),
            expected: "internal, partner, or public".to_owned(),
            got: other.to_owned(),
        }),
    }
}

fn parse_confidence(s: &str) -> Result<Confidence, ImportError> {
    match s.trim().to_lowercase().as_str() {
        "verified" => Ok(Confidence::Verified),
        "reported" => Ok(Confidence::Reported),
        "inferred" => Ok(Confidence::Inferred),
        "estimated" => Ok(Confidence::Estimated),
        other => Err(ImportError::InvalidCell {
            cell_ref: "Metadata!B5".to_owned(),
            expected: "verified, reported, inferred, or estimated".to_owned(),
            got: other.to_owned(),
        }),
    }
}
