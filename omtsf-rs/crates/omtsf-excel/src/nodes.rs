/// Parses node sheets: Organizations, Facilities, Goods, Persons, Attestations, Consignments.
///
/// Each row becomes a node of the appropriate type. Inline identifier columns
/// (lei, duns, `nat_reg`, vat, internal) on the Organizations sheet are converted
/// to `Identifier` records. Returns the list of nodes and a map of inline
/// identifiers for later merging with the Identifiers sheet.
use std::collections::{BTreeMap, HashMap};

use calamine::{Data, Range};

use omtsf_core::enums::{
    AttestationOutcome, AttestationStatus, AttestationType, NodeType, NodeTypeTag,
    OrganizationStatus, Sensitivity, VerificationStatus,
};
use omtsf_core::newtypes::{CalendarDate, CountryCode, NodeId};
use omtsf_core::structures::Node;
use omtsf_core::types::{DataQuality, Identifier};

use crate::error::ImportError;
use crate::metadata::FileMetadata;
use crate::sheet::{
    build_header_index, cell_is_empty, cell_ref, cell_to_string, read_optional_float,
    read_optional_string, require_column,
};
use crate::slug::make_slug;

/// Inline identifier columns on the Organizations sheet.
struct InlineIdCol {
    scheme: &'static str,
    value_col: &'static str,
    authority_col: Option<&'static str>,
    country_col: Option<&'static str>,
    default_sensitivity: Sensitivity,
}

const ORG_INLINE_IDS: &[InlineIdCol] = &[
    InlineIdCol {
        scheme: "lei",
        value_col: "lei",
        authority_col: None,
        country_col: None,
        default_sensitivity: Sensitivity::Public,
    },
    InlineIdCol {
        scheme: "duns",
        value_col: "duns",
        authority_col: None,
        country_col: None,
        default_sensitivity: Sensitivity::Public,
    },
    InlineIdCol {
        scheme: "nat-reg",
        value_col: "nat_reg_value",
        authority_col: Some("nat_reg_authority"),
        country_col: None,
        default_sensitivity: Sensitivity::Restricted,
    },
    InlineIdCol {
        scheme: "vat",
        value_col: "vat_value",
        authority_col: None,
        country_col: Some("vat_country"),
        default_sensitivity: Sensitivity::Restricted,
    },
    InlineIdCol {
        scheme: "internal",
        value_col: "internal_id",
        authority_col: Some("internal_system"),
        country_col: None,
        default_sensitivity: Sensitivity::Restricted,
    },
];

/// Parses all node sheets and returns the combined node list.
///
/// `inline_identifiers` accumulates identifiers found in inline columns on
/// node sheets so they can be merged with the Identifiers sheet later.
#[allow(clippy::too_many_arguments)]
pub fn parse_all_nodes(
    orgs: &Range<Data>,
    facilities: &Range<Data>,
    goods: &Range<Data>,
    persons: &Range<Data>,
    attestations: &Range<Data>,
    consignments: &Range<Data>,
    meta: &FileMetadata,
    inline_identifiers: &mut HashMap<String, Vec<Identifier>>,
) -> Result<Vec<Node>, ImportError> {
    let mut nodes = Vec::new();
    nodes.extend(parse_organizations(orgs, meta, inline_identifiers)?);
    nodes.extend(parse_facilities(facilities, meta)?);
    nodes.extend(parse_goods(goods, meta)?);
    nodes.extend(parse_persons(persons, meta)?);
    nodes.extend(parse_attestations(attestations, meta)?);
    nodes.extend(parse_consignments(consignments, meta)?);
    Ok(nodes)
}

fn make_data_quality(meta: &FileMetadata) -> Option<DataQuality> {
    if meta.default_confidence.is_none() && meta.default_source.is_none() {
        return None;
    }
    Some(DataQuality {
        confidence: meta.default_confidence.clone(),
        source: meta.default_source.clone(),
        last_verified: None,
        extra: BTreeMap::new(),
    })
}

fn parse_organizations(
    sheet: &Range<Data>,
    meta: &FileMetadata,
    inline_identifiers: &mut HashMap<String, Vec<Identifier>>,
) -> Result<Vec<Node>, ImportError> {
    let sheet_name = "Organizations";
    if sheet.is_empty() {
        return Ok(vec![]);
    }
    let headers = build_header_index(sheet, sheet_name)?;
    let id_col = require_column(&headers, sheet_name, "id")?;
    let mut nodes = Vec::new();
    let mut counter = 0usize;

    for (row_idx, row) in sheet.rows().skip(1).enumerate() {
        if row.iter().all(cell_is_empty) {
            continue;
        }
        counter += 1;
        let raw_id = cell_to_string(row.get(id_col).unwrap_or(&Data::Empty));
        let name = read_optional_string(row, &headers, "name");
        let node_id_str = if raw_id.is_empty() {
            make_slug("org", name.as_deref().unwrap_or(""), counter)
        } else {
            raw_id
        };
        let node_id =
            NodeId::try_from(node_id_str.as_str()).map_err(|e| ImportError::InvalidCell {
                cell_ref: cell_ref(sheet_name, id_col, row_idx),
                expected: "non-empty node ID".to_owned(),
                got: format!("{node_id_str} ({e})"),
            })?;

        let jurisdiction = read_optional_string(row, &headers, "jurisdiction")
            .map(|s| CountryCode::try_from(s.as_str()))
            .transpose()
            .ok()
            .flatten();

        let status = read_optional_string(row, &headers, "status")
            .as_deref()
            .and_then(parse_org_status);

        // Collect inline identifiers.
        let mut ids = Vec::new();
        for id_col_def in ORG_INLINE_IDS {
            if let Some(val) = read_optional_string(row, &headers, id_col_def.value_col) {
                let authority = id_col_def
                    .authority_col
                    .and_then(|ac| read_optional_string(row, &headers, ac));
                let authority = if authority.is_none() {
                    id_col_def
                        .country_col
                        .and_then(|cc| read_optional_string(row, &headers, cc))
                } else {
                    authority
                };
                ids.push(Identifier {
                    scheme: id_col_def.scheme.to_owned(),
                    value: val,
                    authority,
                    valid_from: None,
                    valid_to: None,
                    sensitivity: Some(id_col_def.default_sensitivity.clone()),
                    verification_status: Some(VerificationStatus::Reported),
                    verification_date: None,
                    extra: BTreeMap::new(),
                });
            }
        }

        if !ids.is_empty() {
            inline_identifiers
                .entry(node_id.to_string())
                .or_default()
                .extend(ids);
        }

        let node = Node {
            id: node_id,
            node_type: NodeTypeTag::Known(NodeType::Organization),
            name,
            jurisdiction,
            status,
            data_quality: make_data_quality(meta),
            ..Default::default()
        };
        nodes.push(node);
    }
    Ok(nodes)
}

fn parse_facilities(sheet: &Range<Data>, meta: &FileMetadata) -> Result<Vec<Node>, ImportError> {
    let sheet_name = "Facilities";
    if sheet.is_empty() {
        return Ok(vec![]);
    }
    let headers = build_header_index(sheet, sheet_name)?;
    let id_col = require_column(&headers, sheet_name, "id")?;
    let mut nodes = Vec::new();
    let mut counter = 0usize;

    for (row_idx, row) in sheet.rows().skip(1).enumerate() {
        if row.iter().all(cell_is_empty) {
            continue;
        }
        counter += 1;
        let raw_id = cell_to_string(row.get(id_col).unwrap_or(&Data::Empty));
        let name = read_optional_string(row, &headers, "name");
        let node_id_str = if raw_id.is_empty() {
            make_slug("fac", name.as_deref().unwrap_or(""), counter)
        } else {
            raw_id
        };
        let node_id =
            NodeId::try_from(node_id_str.as_str()).map_err(|e| ImportError::InvalidCell {
                cell_ref: cell_ref(sheet_name, id_col, row_idx),
                expected: "non-empty node ID".to_owned(),
                got: format!("{node_id_str} ({e})"),
            })?;

        let operator = read_optional_string(row, &headers, "operator_id")
            .or_else(|| read_optional_string(row, &headers, "operator"))
            .and_then(|s| NodeId::try_from(s.as_str()).ok());

        let address = read_optional_string(row, &headers, "address");

        // Geo: lat/lon columns → point.
        let geo = build_geo_value(row, &headers, sheet_name, row_idx)?;

        let node = Node {
            id: node_id,
            node_type: NodeTypeTag::Known(NodeType::Facility),
            name,
            operator,
            address,
            geo,
            data_quality: make_data_quality(meta),
            ..Default::default()
        };
        nodes.push(node);
    }
    Ok(nodes)
}

fn parse_goods(sheet: &Range<Data>, meta: &FileMetadata) -> Result<Vec<Node>, ImportError> {
    let sheet_name = "Goods";
    if sheet.is_empty() {
        return Ok(vec![]);
    }
    let headers = build_header_index(sheet, sheet_name)?;
    let id_col = require_column(&headers, sheet_name, "id")?;
    let mut nodes = Vec::new();
    let mut counter = 0usize;

    for (row_idx, row) in sheet.rows().skip(1).enumerate() {
        if row.iter().all(cell_is_empty) {
            continue;
        }
        counter += 1;
        let raw_id = cell_to_string(row.get(id_col).unwrap_or(&Data::Empty));
        let name = read_optional_string(row, &headers, "name");
        let node_id_str = if raw_id.is_empty() {
            make_slug("good", name.as_deref().unwrap_or(""), counter)
        } else {
            raw_id
        };
        let node_id =
            NodeId::try_from(node_id_str.as_str()).map_err(|e| ImportError::InvalidCell {
                cell_ref: cell_ref(sheet_name, id_col, row_idx),
                expected: "non-empty node ID".to_owned(),
                got: format!("{node_id_str} ({e})"),
            })?;

        let commodity_code = read_optional_string(row, &headers, "commodity_code");
        let unit = read_optional_string(row, &headers, "unit");

        let node = Node {
            id: node_id,
            node_type: NodeTypeTag::Known(NodeType::Good),
            name,
            commodity_code,
            unit,
            data_quality: make_data_quality(meta),
            ..Default::default()
        };
        nodes.push(node);
    }
    Ok(nodes)
}

fn parse_persons(sheet: &Range<Data>, meta: &FileMetadata) -> Result<Vec<Node>, ImportError> {
    let sheet_name = "Persons";
    if sheet.is_empty() {
        return Ok(vec![]);
    }
    let headers = build_header_index(sheet, sheet_name)?;
    let id_col = require_column(&headers, sheet_name, "id")?;
    let mut nodes = Vec::new();
    let mut counter = 0usize;

    for (row_idx, row) in sheet.rows().skip(1).enumerate() {
        if row.iter().all(cell_is_empty) {
            continue;
        }
        counter += 1;
        let raw_id = cell_to_string(row.get(id_col).unwrap_or(&Data::Empty));
        let name = read_optional_string(row, &headers, "name");
        let node_id_str = if raw_id.is_empty() {
            make_slug("person", name.as_deref().unwrap_or(""), counter)
        } else {
            raw_id
        };
        let node_id =
            NodeId::try_from(node_id_str.as_str()).map_err(|e| ImportError::InvalidCell {
                cell_ref: cell_ref(sheet_name, id_col, row_idx),
                expected: "non-empty node ID".to_owned(),
                got: format!("{node_id_str} ({e})"),
            })?;

        let jurisdiction = read_optional_string(row, &headers, "jurisdiction")
            .or_else(|| read_optional_string(row, &headers, "nationality"))
            .map(|s| CountryCode::try_from(s.as_str()))
            .transpose()
            .ok()
            .flatten();

        let role = read_optional_string(row, &headers, "role");

        let node = Node {
            id: node_id,
            node_type: NodeTypeTag::Known(NodeType::Person),
            name,
            jurisdiction,
            role,
            data_quality: make_data_quality(meta),
            ..Default::default()
        };
        nodes.push(node);
    }
    Ok(nodes)
}

fn parse_attestations(sheet: &Range<Data>, meta: &FileMetadata) -> Result<Vec<Node>, ImportError> {
    let sheet_name = "Attestations";
    if sheet.is_empty() {
        return Ok(vec![]);
    }
    let headers = build_header_index(sheet, sheet_name)?;
    let id_col = require_column(&headers, sheet_name, "id")?;
    let mut nodes = Vec::new();
    let mut counter = 0usize;

    for (row_idx, row) in sheet.rows().skip(1).enumerate() {
        if row.iter().all(cell_is_empty) {
            continue;
        }
        counter += 1;
        let raw_id = cell_to_string(row.get(id_col).unwrap_or(&Data::Empty));
        let name = read_optional_string(row, &headers, "name");
        let node_id_str = if raw_id.is_empty() {
            make_slug("att", name.as_deref().unwrap_or(""), counter)
        } else {
            raw_id
        };
        let node_id =
            NodeId::try_from(node_id_str.as_str()).map_err(|e| ImportError::InvalidCell {
                cell_ref: cell_ref(sheet_name, id_col, row_idx),
                expected: "non-empty node ID".to_owned(),
                got: format!("{node_id_str} ({e})"),
            })?;

        let attestation_type = read_optional_string(row, &headers, "attestation_type")
            .as_deref()
            .and_then(parse_attestation_type);
        let standard = read_optional_string(row, &headers, "standard");
        let issuer = read_optional_string(row, &headers, "issuer");
        let valid_from = read_optional_string(row, &headers, "valid_from")
            .and_then(|s| CalendarDate::try_from(s.as_str()).ok());
        let valid_to: Option<Option<CalendarDate>> =
            read_optional_string(row, &headers, "valid_to")
                .and_then(|s| CalendarDate::try_from(s.as_str()).ok())
                .map(Some);
        let outcome = read_optional_string(row, &headers, "outcome")
            .as_deref()
            .and_then(parse_attestation_outcome);
        let attestation_status = read_optional_string(row, &headers, "status")
            .as_deref()
            .and_then(parse_attestation_status);
        let reference = read_optional_string(row, &headers, "reference");

        let node = Node {
            id: node_id,
            node_type: NodeTypeTag::Known(NodeType::Attestation),
            name,
            attestation_type,
            standard,
            issuer,
            valid_from,
            valid_to,
            outcome,
            attestation_status,
            reference,
            data_quality: make_data_quality(meta),
            ..Default::default()
        };
        nodes.push(node);
    }
    Ok(nodes)
}

fn parse_consignments(sheet: &Range<Data>, meta: &FileMetadata) -> Result<Vec<Node>, ImportError> {
    let sheet_name = "Consignments";
    if sheet.is_empty() {
        return Ok(vec![]);
    }
    let headers = build_header_index(sheet, sheet_name)?;
    let id_col = require_column(&headers, sheet_name, "id")?;
    let mut nodes = Vec::new();
    let mut counter = 0usize;

    for (row_idx, row) in sheet.rows().skip(1).enumerate() {
        if row.iter().all(cell_is_empty) {
            continue;
        }
        counter += 1;
        let raw_id = cell_to_string(row.get(id_col).unwrap_or(&Data::Empty));
        let name = read_optional_string(row, &headers, "name");
        let node_id_str = if raw_id.is_empty() {
            make_slug("con", name.as_deref().unwrap_or(""), counter)
        } else {
            raw_id
        };
        let node_id =
            NodeId::try_from(node_id_str.as_str()).map_err(|e| ImportError::InvalidCell {
                cell_ref: cell_ref(sheet_name, id_col, row_idx),
                expected: "non-empty node ID".to_owned(),
                got: format!("{node_id_str} ({e})"),
            })?;

        let lot_id = read_optional_string(row, &headers, "lot_id");
        let unit = read_optional_string(row, &headers, "unit");
        let quantity = read_optional_float(row, &headers, "quantity", sheet_name, row_idx)?;
        let production_date = read_optional_string(row, &headers, "production_date")
            .and_then(|s| CalendarDate::try_from(s.as_str()).ok());
        let origin_country = read_optional_string(row, &headers, "origin_country")
            .and_then(|s| CountryCode::try_from(s.as_str()).ok());
        let direct_emissions =
            read_optional_float(row, &headers, "direct_emissions_co2e", sheet_name, row_idx)?;
        let indirect_emissions = read_optional_float(
            row,
            &headers,
            "indirect_emissions_co2e",
            sheet_name,
            row_idx,
        )?;
        let installation_id = read_optional_string(row, &headers, "installation_id")
            .and_then(|s| NodeId::try_from(s.as_str()).ok());

        let node = Node {
            id: node_id,
            node_type: NodeTypeTag::Known(NodeType::Consignment),
            name,
            lot_id,
            unit,
            quantity,
            production_date,
            origin_country,
            direct_emissions_co2e: direct_emissions,
            indirect_emissions_co2e: indirect_emissions,
            installation_id,
            data_quality: make_data_quality(meta),
            ..Default::default()
        };
        nodes.push(node);
    }
    Ok(nodes)
}

/// Builds a geo `DynValue` from latitude/longitude columns, if present.
fn build_geo_value(
    row: &[Data],
    headers: &HashMap<String, usize>,
    sheet_name: &str,
    row_idx: usize,
) -> Result<Option<omtsf_core::dynvalue::DynValue>, ImportError> {
    let lat = read_optional_float(row, headers, "latitude", sheet_name, row_idx)?;
    let lon = read_optional_float(row, headers, "longitude", sheet_name, row_idx)?;
    match (lat, lon) {
        (Some(lat), Some(lon)) => Ok(Some(omtsf_core::dynvalue::DynValue::from(
            serde_json::json!({"lat": lat, "lon": lon}),
        ))),
        (None, None) => Ok(None),
        _ => Ok(None),
    }
}

fn parse_org_status(s: &str) -> Option<OrganizationStatus> {
    match s.trim().to_lowercase().as_str() {
        "active" => Some(OrganizationStatus::Active),
        "dissolved" => Some(OrganizationStatus::Dissolved),
        "merged" => Some(OrganizationStatus::Merged),
        "suspended" => Some(OrganizationStatus::Suspended),
        _ => None,
    }
}

fn parse_attestation_type(s: &str) -> Option<AttestationType> {
    match s.trim().to_lowercase().as_str() {
        "certification" => Some(AttestationType::Certification),
        "audit" => Some(AttestationType::Audit),
        "due_diligence_statement" | "due diligence statement" => {
            Some(AttestationType::DueDiligenceStatement)
        }
        "self_declaration" | "self declaration" => Some(AttestationType::SelfDeclaration),
        "other" => Some(AttestationType::Other),
        _ => None,
    }
}

fn parse_attestation_outcome(s: &str) -> Option<AttestationOutcome> {
    match s.trim().to_lowercase().as_str() {
        "pass" => Some(AttestationOutcome::Pass),
        "conditional_pass" | "conditional pass" => Some(AttestationOutcome::ConditionalPass),
        "fail" => Some(AttestationOutcome::Fail),
        "pending" => Some(AttestationOutcome::Pending),
        "not_applicable" | "not applicable" | "n/a" => Some(AttestationOutcome::NotApplicable),
        _ => None,
    }
}

fn parse_attestation_status(s: &str) -> Option<AttestationStatus> {
    match s.trim().to_lowercase().as_str() {
        "active" => Some(AttestationStatus::Active),
        "suspended" => Some(AttestationStatus::Suspended),
        "revoked" => Some(AttestationStatus::Revoked),
        "expired" => Some(AttestationStatus::Expired),
        "withdrawn" => Some(AttestationStatus::Withdrawn),
        _ => None,
    }
}
