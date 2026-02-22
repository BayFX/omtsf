/// Parses the Identifiers sheet and merges with inline identifier columns.
///
/// The Identifiers sheet has one row per identifier record with columns:
/// `node_id`, `scheme`, `value`, `authority`, `sensitivity`, `valid_from`,
/// `valid_to`, `verification_status`.
///
/// Identifiers from the Identifiers sheet are merged with inline identifier
/// columns collected from node sheets, deduplicated per L1-EID-11 (same
/// scheme+value on same node → keep one).
use std::collections::{BTreeMap, HashMap};

use calamine::{Data, Range};

use omtsf_core::enums::{NodeType, NodeTypeTag, Sensitivity, VerificationStatus};
use omtsf_core::newtypes::CalendarDate;
use omtsf_core::structures::Node;
use omtsf_core::types::Identifier;

use crate::error::ImportError;
use crate::sheet::{
    build_header_index, cell_is_empty, cell_ref, cell_to_string, read_optional_string,
    require_column,
};

/// Parses the Identifiers sheet into a map of `node_id` → Vec<Identifier>.
pub fn parse_identifiers_sheet(
    sheet: &Range<Data>,
) -> Result<HashMap<String, Vec<Identifier>>, ImportError> {
    let sheet_name = "Identifiers";
    if sheet.is_empty() {
        return Ok(HashMap::new());
    }
    let headers = build_header_index(sheet, sheet_name)?;
    let node_id_col = require_column(&headers, sheet_name, "node_id")?;
    let scheme_col = require_column(&headers, sheet_name, "scheme")?;
    let value_col = require_column(&headers, sheet_name, "value")?;

    let mut map: HashMap<String, Vec<Identifier>> = HashMap::new();

    for (row_idx, row) in sheet.rows().skip(1).enumerate() {
        if row.iter().all(cell_is_empty) {
            continue;
        }

        let node_id_val = cell_to_string(row.get(node_id_col).unwrap_or(&Data::Empty));
        if node_id_val.is_empty() {
            continue;
        }

        let scheme = cell_to_string(row.get(scheme_col).unwrap_or(&Data::Empty));
        if scheme.is_empty() {
            return Err(ImportError::InvalidCell {
                cell_ref: cell_ref(sheet_name, scheme_col, row_idx),
                expected: "identifier scheme".to_owned(),
                got: String::new(),
            });
        }

        let value = cell_to_string(row.get(value_col).unwrap_or(&Data::Empty));
        if value.is_empty() {
            return Err(ImportError::InvalidCell {
                cell_ref: cell_ref(sheet_name, value_col, row_idx),
                expected: "identifier value".to_owned(),
                got: String::new(),
            });
        }

        let authority = read_optional_string(row, &headers, "authority");
        let sensitivity = read_optional_string(row, &headers, "sensitivity")
            .as_deref()
            .and_then(parse_sensitivity);
        let valid_from = read_optional_string(row, &headers, "valid_from")
            .and_then(|s| CalendarDate::try_from(s.as_str()).ok());
        let valid_to: Option<Option<CalendarDate>> =
            read_optional_string(row, &headers, "valid_to")
                .and_then(|s| CalendarDate::try_from(s.as_str()).ok())
                .map(Some);
        let verification_status = read_optional_string(row, &headers, "verification_status")
            .as_deref()
            .and_then(parse_verification_status)
            .or(Some(VerificationStatus::Reported));

        // Apply scheme-level default sensitivity when not explicitly set.
        let sensitivity = sensitivity.or_else(|| default_sensitivity_for_scheme(&scheme));

        let identifier = Identifier {
            scheme,
            value,
            authority,
            valid_from,
            valid_to,
            sensitivity,
            verification_status,
            verification_date: None,
            extra: BTreeMap::new(),
        };

        map.entry(node_id_val).or_default().push(identifier);
    }

    Ok(map)
}

/// Applies sensitivity defaults per SPEC-004 Section 2 for known schemes.
pub fn default_sensitivity_for_scheme(scheme: &str) -> Option<Sensitivity> {
    match scheme.to_lowercase().as_str() {
        "lei" | "duns" | "gln" | "org.gs1.gln" | "org.gs1.gtin" => Some(Sensitivity::Public),
        "nat-reg" | "vat" | "internal" => Some(Sensitivity::Restricted),
        _ => None,
    }
}

/// Merges inline identifiers (from node sheets) and Identifiers sheet identifiers
/// onto nodes, deduplicating per scheme+value per node.
pub fn merge_identifiers_onto_nodes(
    nodes: &mut [Node],
    inline_ids: &HashMap<String, Vec<Identifier>>,
    sheet_ids: &HashMap<String, Vec<Identifier>>,
) {
    for node in nodes.iter_mut() {
        let node_key = node.id.to_string();
        let mut combined: Vec<Identifier> = Vec::new();

        if let Some(ids) = inline_ids.get(&node_key) {
            combined.extend(ids.iter().cloned());
        }
        if let Some(ids) = sheet_ids.get(&node_key) {
            combined.extend(ids.iter().cloned());
        }

        if combined.is_empty() {
            continue;
        }

        // Deduplicate: keep first occurrence of each scheme+value pair.
        let mut seen: std::collections::HashSet<(String, String)> =
            std::collections::HashSet::new();
        let mut deduped: Vec<Identifier> = combined
            .into_iter()
            .filter(|id| seen.insert((id.scheme.clone(), id.value.clone())))
            .collect();

        // Per SPEC-004 Section 5, all identifiers on person nodes default to
        // confidential regardless of scheme-level defaults.
        if matches!(&node.node_type, NodeTypeTag::Known(NodeType::Person)) {
            for id in &mut deduped {
                if id.sensitivity.is_none() {
                    id.sensitivity = Some(Sensitivity::Confidential);
                }
            }
        }

        node.identifiers = Some(deduped);
    }
}

fn parse_sensitivity(s: &str) -> Option<Sensitivity> {
    match s.trim().to_lowercase().as_str() {
        "public" => Some(Sensitivity::Public),
        "restricted" => Some(Sensitivity::Restricted),
        "confidential" => Some(Sensitivity::Confidential),
        _ => None,
    }
}

fn parse_verification_status(s: &str) -> Option<VerificationStatus> {
    match s.trim().to_lowercase().as_str() {
        "verified" => Some(VerificationStatus::Verified),
        "reported" => Some(VerificationStatus::Reported),
        "inferred" => Some(VerificationStatus::Inferred),
        "unverified" => Some(VerificationStatus::Unverified),
        _ => None,
    }
}
