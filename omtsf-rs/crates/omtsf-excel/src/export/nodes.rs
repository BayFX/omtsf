/// Writes node sheets: Organizations, Facilities, Goods, Persons,
/// Attestations, Consignments.
///
/// Organization nodes include inline identifier columns (lei, duns, `nat_reg`,
/// vat, internal) mirroring the import-side layout. Boundary ref nodes are
/// omitted as they are import-time artifacts.
///
/// Every sheet includes a `labels` column (`key=value;key2=value2` encoding)
/// and a `_conflicts` column (JSON array, only populated on merged files).
use rust_xlsxwriter::{Worksheet, XlsxError};

use omtsf_core::enums::{NodeType, NodeTypeTag};
use omtsf_core::structures::Node;
use omtsf_core::types::{Identifier, Label};

use crate::error::ExportError;
use crate::export::style::{set_column_widths, write_header_row};

fn ws(ws: &mut Worksheet, row: u32, col: u16, val: &str) -> Result<(), ExportError> {
    ws.write(row, col, val)
        .map(|_| ())
        .map_err(|e: XlsxError| ExportError::ExcelWrite {
            detail: e.to_string(),
        })
}

fn wf64(ws: &mut Worksheet, row: u32, col: u16, val: f64) -> Result<(), ExportError> {
    ws.write(row, col, val)
        .map(|_| ())
        .map_err(|e: XlsxError| ExportError::ExcelWrite {
            detail: e.to_string(),
        })
}

/// Serialises a `labels` vec as `key=value;key2` (value omitted for flag labels).
fn labels_to_str(labels: &[Label]) -> String {
    labels
        .iter()
        .map(|l| match &l.value {
            Some(v) => format!("{}={}", l.key, v),
            None => l.key.clone(),
        })
        .collect::<Vec<_>>()
        .join(";")
}

/// Returns the JSON-serialised `_conflicts` array from the node's `extra` map,
/// or an empty string if absent.
fn conflicts_str(node: &Node) -> String {
    node.extra
        .get("_conflicts")
        .and_then(|v| serde_json::to_string(v).ok())
        .unwrap_or_default()
}

/// Sets up the Organizations sheet headers and column widths, then writes all
/// organization rows from `nodes`.
///
/// Returns the number of data rows written.
pub fn write_organizations(ws: &mut Worksheet, nodes: &[Node]) -> Result<u32, ExportError> {
    write_header_row(
        ws,
        &[
            "id",
            "name",
            "jurisdiction",
            "status",
            "governance_structure",
            "lei",
            "duns",
            "nat_reg_value",
            "nat_reg_authority",
            "vat_value",
            "vat_country",
            "internal_id",
            "internal_system",
            "labels",
            "_conflicts",
        ],
    )?;
    set_column_widths(
        ws,
        &[
            (0, 24.0),
            (1, 30.0),
            (2, 14.0),
            (3, 12.0),
            (4, 24.0),
            (5, 22.0),
            (6, 14.0),
            (7, 20.0),
            (8, 22.0),
            (9, 20.0),
            (10, 14.0),
            (11, 20.0),
            (12, 22.0),
            (13, 30.0),
            (14, 30.0),
        ],
    )?;

    let mut row: u32 = 1;
    for node in nodes {
        if matches!(&node.node_type, NodeTypeTag::Known(NodeType::Organization)) {
            write_org_row(ws, row, node)?;
            row += 1;
        }
    }
    Ok(row - 1)
}

fn write_org_row(worksheet: &mut Worksheet, row: u32, node: &Node) -> Result<(), ExportError> {
    ws(worksheet, row, 0, &node.id.to_string())?;
    ws(worksheet, row, 1, node.name.as_deref().unwrap_or(""))?;
    ws(
        worksheet,
        row,
        2,
        node.jurisdiction.as_ref().map(|c| c.as_ref()).unwrap_or(""),
    )?;
    ws(
        worksheet,
        row,
        3,
        node.status.as_ref().map(org_status_str).unwrap_or(""),
    )?;

    // governance_structure: serialise DynValue to JSON string if present.
    let gov_str = node
        .governance_structure
        .as_ref()
        .and_then(|v| {
            v.as_str()
                .map(str::to_owned)
                .or_else(|| serde_json::to_string(v).ok())
        })
        .unwrap_or_default();
    ws(worksheet, row, 4, &gov_str)?;

    if let Some(ids) = &node.identifiers {
        write_org_inline_ids(worksheet, row, ids)?;
    }

    let labels_str = node
        .labels
        .as_deref()
        .map(labels_to_str)
        .unwrap_or_default();
    ws(worksheet, row, 13, &labels_str)?;
    ws(worksheet, row, 14, &conflicts_str(node))?;

    Ok(())
}

fn write_org_inline_ids(
    worksheet: &mut Worksheet,
    row: u32,
    ids: &[Identifier],
) -> Result<(), ExportError> {
    let find = |scheme: &str| ids.iter().find(|id| id.scheme.to_lowercase() == scheme);

    if let Some(lei) = find("lei") {
        ws(worksheet, row, 5, &lei.value)?;
    }
    if let Some(duns) = find("duns") {
        ws(worksheet, row, 6, &duns.value)?;
    }
    if let Some(nat) = find("nat-reg") {
        ws(worksheet, row, 7, &nat.value)?;
        ws(worksheet, row, 8, nat.authority.as_deref().unwrap_or(""))?;
    }
    if let Some(vat) = find("vat") {
        ws(worksheet, row, 9, &vat.value)?;
        ws(worksheet, row, 10, vat.authority.as_deref().unwrap_or(""))?;
    }
    if let Some(int) = find("internal") {
        ws(worksheet, row, 11, &int.value)?;
        ws(worksheet, row, 12, int.authority.as_deref().unwrap_or(""))?;
    }

    Ok(())
}

/// Sets up the Facilities sheet headers and writes all facility rows.
///
/// Returns the number of data rows written.
pub fn write_facilities(worksheet: &mut Worksheet, nodes: &[Node]) -> Result<u32, ExportError> {
    write_header_row(
        worksheet,
        &[
            "id",
            "name",
            "operator_id",
            "address",
            "latitude",
            "longitude",
            "labels",
            "_conflicts",
        ],
    )?;
    set_column_widths(
        worksheet,
        &[
            (0, 24.0),
            (1, 30.0),
            (2, 24.0),
            (3, 40.0),
            (4, 12.0),
            (5, 12.0),
            (6, 30.0),
            (7, 30.0),
        ],
    )?;

    let mut row: u32 = 1;
    for node in nodes {
        if matches!(&node.node_type, NodeTypeTag::Known(NodeType::Facility)) {
            write_facility_row(worksheet, row, node)?;
            row += 1;
        }
    }
    Ok(row - 1)
}

fn write_facility_row(worksheet: &mut Worksheet, row: u32, node: &Node) -> Result<(), ExportError> {
    ws(worksheet, row, 0, &node.id.to_string())?;
    ws(worksheet, row, 1, node.name.as_deref().unwrap_or(""))?;
    ws(
        worksheet,
        row,
        2,
        node.operator.as_ref().map(|id| id.as_ref()).unwrap_or(""),
    )?;
    ws(worksheet, row, 3, node.address.as_deref().unwrap_or(""))?;

    if let Some(geo_val) = &node.geo {
        if let Some(obj) = geo_val.as_object() {
            if let Some(lat) = obj.get("lat").and_then(omtsf_core::DynValue::as_f64) {
                wf64(worksheet, row, 4, lat)?;
            }
            if let Some(lon) = obj.get("lon").and_then(omtsf_core::DynValue::as_f64) {
                wf64(worksheet, row, 5, lon)?;
            }
        }
    }

    let labels_str = node
        .labels
        .as_deref()
        .map(labels_to_str)
        .unwrap_or_default();
    ws(worksheet, row, 6, &labels_str)?;
    ws(worksheet, row, 7, &conflicts_str(node))?;

    Ok(())
}

/// Sets up the Goods sheet headers and writes all good rows.
///
/// Returns the number of data rows written.
pub fn write_goods(worksheet: &mut Worksheet, nodes: &[Node]) -> Result<u32, ExportError> {
    write_header_row(
        worksheet,
        &[
            "id",
            "name",
            "commodity_code",
            "unit",
            "labels",
            "_conflicts",
        ],
    )?;
    set_column_widths(
        worksheet,
        &[
            (0, 24.0),
            (1, 30.0),
            (2, 20.0),
            (3, 12.0),
            (4, 30.0),
            (5, 30.0),
        ],
    )?;

    let mut row: u32 = 1;
    for node in nodes {
        if matches!(&node.node_type, NodeTypeTag::Known(NodeType::Good)) {
            ws(worksheet, row, 0, &node.id.to_string())?;
            ws(worksheet, row, 1, node.name.as_deref().unwrap_or(""))?;
            ws(
                worksheet,
                row,
                2,
                node.commodity_code.as_deref().unwrap_or(""),
            )?;
            ws(worksheet, row, 3, node.unit.as_deref().unwrap_or(""))?;
            let labels_str = node
                .labels
                .as_deref()
                .map(labels_to_str)
                .unwrap_or_default();
            ws(worksheet, row, 4, &labels_str)?;
            ws(worksheet, row, 5, &conflicts_str(node))?;
            row += 1;
        }
    }
    Ok(row - 1)
}

/// Sets up the Persons sheet headers and writes all person rows.
///
/// Returns the number of data rows written.
pub fn write_persons(worksheet: &mut Worksheet, nodes: &[Node]) -> Result<u32, ExportError> {
    write_header_row(
        worksheet,
        &["id", "name", "jurisdiction", "role", "labels", "_conflicts"],
    )?;
    set_column_widths(
        worksheet,
        &[
            (0, 24.0),
            (1, 30.0),
            (2, 14.0),
            (3, 20.0),
            (4, 30.0),
            (5, 30.0),
        ],
    )?;

    let mut row: u32 = 1;
    for node in nodes {
        if matches!(&node.node_type, NodeTypeTag::Known(NodeType::Person)) {
            ws(worksheet, row, 0, &node.id.to_string())?;
            ws(worksheet, row, 1, node.name.as_deref().unwrap_or(""))?;
            ws(
                worksheet,
                row,
                2,
                node.jurisdiction.as_ref().map(|c| c.as_ref()).unwrap_or(""),
            )?;
            ws(worksheet, row, 3, node.role.as_deref().unwrap_or(""))?;
            let labels_str = node
                .labels
                .as_deref()
                .map(labels_to_str)
                .unwrap_or_default();
            ws(worksheet, row, 4, &labels_str)?;
            ws(worksheet, row, 5, &conflicts_str(node))?;
            row += 1;
        }
    }
    Ok(row - 1)
}

/// Sets up the Attestations sheet headers and writes all attestation rows.
///
/// Note: `attested_by` edge data (columns 10 and 11) is written in a separate
/// pass by the edge export module.
///
/// Returns the number of data rows written.
pub fn write_attestations(worksheet: &mut Worksheet, nodes: &[Node]) -> Result<u32, ExportError> {
    write_header_row(
        worksheet,
        &[
            "id",
            "name",
            "attestation_type",
            "standard",
            "issuer",
            "valid_from",
            "valid_to",
            "outcome",
            "status",
            "reference",
            "attested_entity_id",
            "scope",
            "risk_severity",
            "risk_likelihood",
            "labels",
            "_conflicts",
        ],
    )?;
    set_column_widths(
        worksheet,
        &[
            (0, 24.0),
            (1, 30.0),
            (2, 24.0),
            (3, 30.0),
            (4, 24.0),
            (5, 14.0),
            (6, 14.0),
            (7, 18.0),
            (8, 14.0),
            (9, 30.0),
            (10, 24.0),
            (11, 24.0),
            (12, 16.0),
            (13, 18.0),
            (14, 30.0),
            (15, 30.0),
        ],
    )?;

    let mut row: u32 = 1;
    for node in nodes {
        if matches!(&node.node_type, NodeTypeTag::Known(NodeType::Attestation)) {
            write_attestation_row(worksheet, row, node)?;
            row += 1;
        }
    }
    Ok(row - 1)
}

fn write_attestation_row(
    worksheet: &mut Worksheet,
    row: u32,
    node: &Node,
) -> Result<(), ExportError> {
    ws(worksheet, row, 0, &node.id.to_string())?;
    ws(worksheet, row, 1, node.name.as_deref().unwrap_or(""))?;
    ws(
        worksheet,
        row,
        2,
        node.attestation_type
            .as_ref()
            .map(attestation_type_str)
            .unwrap_or(""),
    )?;
    ws(worksheet, row, 3, node.standard.as_deref().unwrap_or(""))?;
    ws(worksheet, row, 4, node.issuer.as_deref().unwrap_or(""))?;
    ws(
        worksheet,
        row,
        5,
        node.valid_from
            .as_ref()
            .map(std::string::ToString::to_string)
            .as_deref()
            .unwrap_or(""),
    )?;

    let valid_to_str: String = match &node.valid_to {
        Some(Some(d)) => d.to_string(),
        _ => String::new(),
    };
    ws(worksheet, row, 6, &valid_to_str)?;
    ws(
        worksheet,
        row,
        7,
        node.outcome
            .as_ref()
            .map(attestation_outcome_str)
            .unwrap_or(""),
    )?;
    ws(
        worksheet,
        row,
        8,
        node.attestation_status
            .as_ref()
            .map(attestation_status_str)
            .unwrap_or(""),
    )?;
    ws(worksheet, row, 9, node.reference.as_deref().unwrap_or(""))?;

    // Columns 10 and 11 (attested_entity_id, scope) are filled by the
    // attested_by back-fill pass in the edge module.

    ws(
        worksheet,
        row,
        12,
        node.risk_severity
            .as_ref()
            .map(risk_severity_str)
            .unwrap_or(""),
    )?;
    ws(
        worksheet,
        row,
        13,
        node.risk_likelihood
            .as_ref()
            .map(risk_likelihood_str)
            .unwrap_or(""),
    )?;

    let labels_str = node
        .labels
        .as_deref()
        .map(labels_to_str)
        .unwrap_or_default();
    ws(worksheet, row, 14, &labels_str)?;
    ws(worksheet, row, 15, &conflicts_str(node))?;

    Ok(())
}

/// Sets up the Consignments sheet headers and writes all consignment rows.
///
/// Returns the number of data rows written.
pub fn write_consignments(worksheet: &mut Worksheet, nodes: &[Node]) -> Result<u32, ExportError> {
    write_header_row(
        worksheet,
        &[
            "id",
            "name",
            "lot_id",
            "unit",
            "quantity",
            "production_date",
            "origin_country",
            "direct_emissions_co2e",
            "indirect_emissions_co2e",
            "installation_id",
            "labels",
            "_conflicts",
        ],
    )?;
    set_column_widths(
        worksheet,
        &[
            (0, 24.0),
            (1, 30.0),
            (2, 16.0),
            (3, 10.0),
            (4, 12.0),
            (5, 16.0),
            (6, 16.0),
            (7, 22.0),
            (8, 24.0),
            (9, 24.0),
            (10, 30.0),
            (11, 30.0),
        ],
    )?;

    let mut row: u32 = 1;
    for node in nodes {
        if matches!(&node.node_type, NodeTypeTag::Known(NodeType::Consignment)) {
            write_consignment_row(worksheet, row, node)?;
            row += 1;
        }
    }
    Ok(row - 1)
}

fn write_consignment_row(
    worksheet: &mut Worksheet,
    row: u32,
    node: &Node,
) -> Result<(), ExportError> {
    ws(worksheet, row, 0, &node.id.to_string())?;
    ws(worksheet, row, 1, node.name.as_deref().unwrap_or(""))?;
    ws(worksheet, row, 2, node.lot_id.as_deref().unwrap_or(""))?;
    ws(worksheet, row, 3, node.unit.as_deref().unwrap_or(""))?;

    if let Some(qty) = node.quantity {
        wf64(worksheet, row, 4, qty)?;
    }

    ws(
        worksheet,
        row,
        5,
        node.production_date
            .as_ref()
            .map(std::string::ToString::to_string)
            .as_deref()
            .unwrap_or(""),
    )?;
    ws(
        worksheet,
        row,
        6,
        node.origin_country
            .as_ref()
            .map(|c| c.as_ref())
            .unwrap_or(""),
    )?;

    if let Some(de) = node.direct_emissions_co2e {
        wf64(worksheet, row, 7, de)?;
    }
    if let Some(ie) = node.indirect_emissions_co2e {
        wf64(worksheet, row, 8, ie)?;
    }

    ws(
        worksheet,
        row,
        9,
        node.installation_id
            .as_ref()
            .map(|id| id.as_ref())
            .unwrap_or(""),
    )?;

    let labels_str = node
        .labels
        .as_deref()
        .map(labels_to_str)
        .unwrap_or_default();
    ws(worksheet, row, 10, &labels_str)?;
    ws(worksheet, row, 11, &conflicts_str(node))?;

    Ok(())
}

fn org_status_str(s: &omtsf_core::enums::OrganizationStatus) -> &'static str {
    match s {
        omtsf_core::enums::OrganizationStatus::Active => "active",
        omtsf_core::enums::OrganizationStatus::Dissolved => "dissolved",
        omtsf_core::enums::OrganizationStatus::Merged => "merged",
        omtsf_core::enums::OrganizationStatus::Suspended => "suspended",
    }
}

fn attestation_type_str(s: &omtsf_core::enums::AttestationType) -> &'static str {
    match s {
        omtsf_core::enums::AttestationType::Certification => "certification",
        omtsf_core::enums::AttestationType::Audit => "audit",
        omtsf_core::enums::AttestationType::DueDiligenceStatement => "due_diligence_statement",
        omtsf_core::enums::AttestationType::SelfDeclaration => "self_declaration",
        omtsf_core::enums::AttestationType::Other => "other",
    }
}

fn attestation_outcome_str(s: &omtsf_core::enums::AttestationOutcome) -> &'static str {
    match s {
        omtsf_core::enums::AttestationOutcome::Pass => "pass",
        omtsf_core::enums::AttestationOutcome::ConditionalPass => "conditional_pass",
        omtsf_core::enums::AttestationOutcome::Fail => "fail",
        omtsf_core::enums::AttestationOutcome::Pending => "pending",
        omtsf_core::enums::AttestationOutcome::NotApplicable => "not_applicable",
    }
}

fn attestation_status_str(s: &omtsf_core::enums::AttestationStatus) -> &'static str {
    match s {
        omtsf_core::enums::AttestationStatus::Active => "active",
        omtsf_core::enums::AttestationStatus::Suspended => "suspended",
        omtsf_core::enums::AttestationStatus::Revoked => "revoked",
        omtsf_core::enums::AttestationStatus::Expired => "expired",
        omtsf_core::enums::AttestationStatus::Withdrawn => "withdrawn",
    }
}

fn risk_severity_str(s: &omtsf_core::enums::RiskSeverity) -> &'static str {
    match s {
        omtsf_core::enums::RiskSeverity::Critical => "critical",
        omtsf_core::enums::RiskSeverity::High => "high",
        omtsf_core::enums::RiskSeverity::Medium => "medium",
        omtsf_core::enums::RiskSeverity::Low => "low",
    }
}

fn risk_likelihood_str(s: &omtsf_core::enums::RiskLikelihood) -> &'static str {
    match s {
        omtsf_core::enums::RiskLikelihood::VeryLikely => "very_likely",
        omtsf_core::enums::RiskLikelihood::Likely => "likely",
        omtsf_core::enums::RiskLikelihood::Possible => "possible",
        omtsf_core::enums::RiskLikelihood::Unlikely => "unlikely",
    }
}
