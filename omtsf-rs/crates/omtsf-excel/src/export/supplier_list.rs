/// Exports an [`OmtsFile`] to the simplified "Supplier List" single-sheet format.
///
/// Layout mirrors the template produced by `generate_template.py`:
/// - Row 1: metadata (Reporting Entity, Snapshot Date)
/// - Row 2: metadata (Disclosure Scope)
/// - Row 3: blank separator
/// - Row 4: column headers
/// - Row 5+: data rows, sorted by tier ASC then name ASC
use std::collections::HashMap;
use std::io::Write;

use rust_xlsxwriter::{Workbook, Worksheet, XlsxError};

use omtsf_core::enums::{EdgeType, EdgeTypeTag, NodeType, NodeTypeTag};
use omtsf_core::file::OmtsFile;
use omtsf_core::structures::{Edge, Node};

use crate::error::ExportError;

/// Exports an [`OmtsFile`] to the simplified "Supplier List" single-sheet format.
///
/// The workbook bytes are written to `writer`. All organization nodes (except
/// the reporting entity) are written as supplier rows. Supply-chain tier is
/// computed from `supplies` edge topology.
///
/// # Errors
///
/// Returns [`ExportError`] if the workbook cannot be built or written.
pub fn export_supplier_list<W: Write>(file: &OmtsFile, mut writer: W) -> Result<(), ExportError> {
    let mut wb = Workbook::new();
    wb.add_worksheet()
        .set_name("Supplier List")
        .map_err(|e: XlsxError| ExportError::ExcelWrite {
            detail: e.to_string(),
        })?;

    let ws = wb
        .worksheet_from_name("Supplier List")
        .map_err(|e: XlsxError| ExportError::ExcelWrite {
            detail: e.to_string(),
        })?;

    write_sheet(ws, file)?;

    let xlsx_bytes = wb
        .save_to_buffer()
        .map_err(|e: XlsxError| ExportError::ExcelWrite {
            detail: e.to_string(),
        })?;

    writer.write_all(&xlsx_bytes).map_err(|e| ExportError::Io {
        detail: e.to_string(),
    })?;
    writer.flush().map_err(|e| ExportError::Io {
        detail: e.to_string(),
    })?;

    Ok(())
}

fn write_sheet(ws: &mut Worksheet, file: &OmtsFile) -> Result<(), ExportError> {
    let reporting_entity_id = file
        .reporting_entity
        .as_ref()
        .map(ToString::to_string)
        .or_else(|| {
            file.nodes
                .iter()
                .find(|n| matches!(&n.node_type, NodeTypeTag::Known(NodeType::Organization)))
                .map(|n| n.id.to_string())
        })
        .unwrap_or_default();

    let reporting_entity_name = file
        .nodes
        .iter()
        .find(|n| n.id.to_string() == reporting_entity_id)
        .and_then(|n| n.name.as_deref())
        .unwrap_or("");

    let snapshot_date = file.snapshot_date.to_string();

    let disclosure_scope = file
        .disclosure_scope
        .as_ref()
        .map(|s| format!("{s:?}").to_lowercase())
        .unwrap_or_default();

    ws_write(ws, 0, 0, "Reporting Entity")?;
    ws_write(ws, 0, 1, reporting_entity_name)?;
    ws_write(ws, 0, 2, "Snapshot Date")?;
    ws_write(ws, 0, 3, &snapshot_date)?;
    ws_write(ws, 1, 0, "Disclosure Scope")?;
    ws_write(ws, 1, 1, &disclosure_scope)?;

    let headers = [
        "supplier_name",
        "supplier_id",
        "jurisdiction",
        "tier",
        "parent_supplier",
        "business_unit",
        "commodity",
        "valid_from",
        "annual_value",
        "value_currency",
        "contract_ref",
        "lei",
        "duns",
        "vat",
        "vat_country",
        "internal_id",
        "risk_tier",
        "kraljic_quadrant",
        "approval_status",
        "notes",
    ];
    for (col, header) in headers.iter().enumerate() {
        ws_write(ws, 3, col as u16, header)?;
    }

    let suppliers: Vec<&Node> = file
        .nodes
        .iter()
        .filter(|n| {
            matches!(&n.node_type, NodeTypeTag::Known(NodeType::Organization))
                && n.id.to_string() != reporting_entity_id
        })
        .collect();

    let supplies_edges: Vec<&Edge> = file
        .edges
        .iter()
        .filter(|e| matches!(&e.edge_type, EdgeTypeTag::Known(EdgeType::Supplies)))
        .collect();

    let tier_map = compute_tiers(&suppliers, &supplies_edges, &reporting_entity_id);
    let parent_map = build_parent_map(&suppliers, &supplies_edges);

    let mut rows: Vec<ExportRow> =
        build_rows(&suppliers, &supplies_edges, &tier_map, &parent_map, file);
    rows.sort_by(|a, b| {
        a.tier
            .cmp(&b.tier)
            .then(a.supplier_name.cmp(&b.supplier_name))
            .then(a.business_unit.cmp(&b.business_unit))
    });

    for (i, row) in rows.iter().enumerate() {
        let excel_row = (4 + i) as u32;
        ws_write(ws, excel_row, 0, &row.supplier_name)?;
        ws_write(ws, excel_row, 1, &row.supplier_id)?;
        ws_write(ws, excel_row, 2, &row.jurisdiction)?;
        ws_write_u32(ws, excel_row, 3, row.tier)?;
        ws_write(ws, excel_row, 4, &row.parent_supplier)?;
        ws_write(ws, excel_row, 5, &row.business_unit)?;
        ws_write(ws, excel_row, 6, &row.commodity)?;
        ws_write(ws, excel_row, 7, &row.valid_from)?;
        if let Some(av) = row.annual_value {
            ws_write_f64(ws, excel_row, 8, av)?;
        }
        ws_write(ws, excel_row, 9, &row.value_currency)?;
        ws_write(ws, excel_row, 10, &row.contract_ref)?;
        ws_write(ws, excel_row, 11, &row.lei)?;
        ws_write(ws, excel_row, 12, &row.duns)?;
        ws_write(ws, excel_row, 13, &row.vat)?;
        ws_write(ws, excel_row, 14, &row.vat_country)?;
        ws_write(ws, excel_row, 15, &row.internal_id)?;
        ws_write(ws, excel_row, 16, &row.risk_tier)?;
        ws_write(ws, excel_row, 17, &row.kraljic_quadrant)?;
        ws_write(ws, excel_row, 18, &row.approval_status)?;
    }

    Ok(())
}

struct ExportRow {
    supplier_name: String,
    supplier_id: String,
    jurisdiction: String,
    tier: u32,
    parent_supplier: String,
    business_unit: String,
    commodity: String,
    valid_from: String,
    annual_value: Option<f64>,
    value_currency: String,
    contract_ref: String,
    lei: String,
    duns: String,
    vat: String,
    vat_country: String,
    internal_id: String,
    risk_tier: String,
    kraljic_quadrant: String,
    approval_status: String,
}

/// Computes tier (1, 2, 3) for each supplier node by BFS from reporting entity.
fn compute_tiers<'a>(
    suppliers: &[&'a Node],
    edges: &[&'a Edge],
    reporting_entity_id: &str,
) -> HashMap<String, u32> {
    let mut tier_map: HashMap<String, u32> = HashMap::new();

    // Index: target_id → list of source_ids (suppliers pointing to target)
    let mut by_target: HashMap<String, Vec<String>> = HashMap::new();
    for edge in edges {
        by_target
            .entry(edge.target.to_string())
            .or_default()
            .push(edge.source.to_string());
    }

    // BFS from reporting entity
    let mut queue: std::collections::VecDeque<(String, u32)> = std::collections::VecDeque::new();
    queue.push_back((reporting_entity_id.to_owned(), 0));

    while let Some((node_id, depth)) = queue.pop_front() {
        let supplier_depth = depth + 1;
        if let Some(sources) = by_target.get(&node_id) {
            for src in sources {
                if !tier_map.contains_key(src) {
                    tier_map.insert(src.clone(), supplier_depth);
                    queue.push_back((src.clone(), supplier_depth));
                }
            }
        }
    }

    // Default any unvisited supplier nodes to tier 1
    for node in suppliers {
        let id = node.id.to_string();
        tier_map.entry(id).or_insert(1);
    }

    tier_map
}

/// Maps supplier node ID → parent supplier node ID (for tier 2/3).
fn build_parent_map<'a>(_suppliers: &[&'a Node], edges: &[&'a Edge]) -> HashMap<String, String> {
    let mut map: HashMap<String, String> = HashMap::new();
    for edge in edges {
        map.insert(edge.source.to_string(), edge.target.to_string());
    }
    map
}

/// Builds export rows from supplier nodes and their supply edges.
///
/// A supplier may appear in multiple edges (different commodities or business
/// units). Each edge generates one row. Relationship-specific labels
/// (`risk_tier`, `kraljic_quadrant`, `approval_status`, `business_unit`) are
/// read from edge properties.
fn build_rows<'a>(
    suppliers: &[&'a Node],
    edges: &[&'a Edge],
    tier_map: &HashMap<String, u32>,
    parent_map: &HashMap<String, String>,
    file: &OmtsFile,
) -> Vec<ExportRow> {
    let node_by_id: HashMap<String, &Node> =
        file.nodes.iter().map(|n| (n.id.to_string(), n)).collect();

    let mut rows = Vec::new();

    for node in suppliers {
        let node_id = node.id.to_string();
        let tier = *tier_map.get(&node_id).unwrap_or(&1);

        let parent_supplier = parent_map
            .get(&node_id)
            .and_then(|pid| node_by_id.get(pid))
            .map(|pn| {
                let parent_sid = find_supplier_id(pn);
                if parent_sid.is_empty() {
                    pn.name.as_deref().unwrap_or("").to_owned()
                } else {
                    parent_sid
                }
            })
            .unwrap_or_default();

        let node_edges: Vec<&&Edge> = edges
            .iter()
            .filter(|e| e.source.to_string() == node_id)
            .collect();

        let jurisdiction = node
            .jurisdiction
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_default();

        let supplier_id = find_supplier_id(node);
        let lei = find_identifier(node, "lei");
        let duns = find_identifier(node, "duns");
        let vat_id = find_identifier_with_authority(node, "vat");
        let vat = vat_id.0;
        let vat_country = vat_id.1;
        let internal_id = find_internal_id(node);

        if node_edges.is_empty() {
            rows.push(ExportRow {
                supplier_name: node.name.as_deref().unwrap_or("").to_owned(),
                supplier_id: supplier_id.clone(),
                jurisdiction: jurisdiction.clone(),
                tier,
                parent_supplier: parent_supplier.clone(),
                business_unit: String::new(),
                commodity: String::new(),
                valid_from: String::new(),
                annual_value: None,
                value_currency: String::new(),
                contract_ref: String::new(),
                lei: lei.clone(),
                duns: duns.clone(),
                vat: vat.clone(),
                vat_country: vat_country.clone(),
                internal_id: internal_id.clone(),
                risk_tier: String::new(),
                kraljic_quadrant: String::new(),
                approval_status: String::new(),
            });
        } else {
            for edge in &node_edges {
                let props = &edge.properties;
                let risk_tier = find_edge_label(edge, "risk_tier");
                let kraljic_quadrant = find_edge_label(edge, "kraljic_quadrant");
                let approval_status = find_edge_label(edge, "approval_status");
                let business_unit = find_edge_label(edge, "business_unit");

                rows.push(ExportRow {
                    supplier_name: node.name.as_deref().unwrap_or("").to_owned(),
                    supplier_id: supplier_id.clone(),
                    jurisdiction: jurisdiction.clone(),
                    tier,
                    parent_supplier: parent_supplier.clone(),
                    business_unit,
                    commodity: props.commodity.as_deref().unwrap_or("").to_owned(),
                    valid_from: props
                        .valid_from
                        .as_ref()
                        .map(ToString::to_string)
                        .unwrap_or_default(),
                    annual_value: props.annual_value,
                    value_currency: props.value_currency.as_deref().unwrap_or("").to_owned(),
                    contract_ref: props.contract_ref.as_deref().unwrap_or("").to_owned(),
                    lei: lei.clone(),
                    duns: duns.clone(),
                    vat: vat.clone(),
                    vat_country: vat_country.clone(),
                    internal_id: internal_id.clone(),
                    risk_tier,
                    kraljic_quadrant,
                    approval_status,
                });
            }
        }
    }

    rows
}

fn find_identifier(node: &Node, scheme: &str) -> String {
    node.identifiers
        .as_deref()
        .and_then(|ids| ids.iter().find(|id| id.scheme == scheme))
        .map(|id| id.value.as_str())
        .unwrap_or("")
        .to_owned()
}

fn find_identifier_with_authority(node: &Node, scheme: &str) -> (String, String) {
    let id = node
        .identifiers
        .as_deref()
        .and_then(|ids| ids.iter().find(|id| id.scheme == scheme));
    match id {
        Some(id) => (
            id.value.clone(),
            id.authority.as_deref().unwrap_or("").to_owned(),
        ),
        None => (String::new(), String::new()),
    }
}

/// Extracts a label value from an edge's properties.
fn find_edge_label(edge: &Edge, key: &str) -> String {
    edge.properties
        .labels
        .as_deref()
        .and_then(|labels| labels.iter().find(|l| l.key == key))
        .and_then(|l| l.value.as_deref())
        .unwrap_or("")
        .to_owned()
}

/// Extracts the `supplier_id` from a node's identifiers (scheme=`internal`,
/// authority=`supplier-list`).
fn find_supplier_id(node: &Node) -> String {
    node.identifiers
        .as_deref()
        .and_then(|ids| {
            ids.iter().find(|id| {
                id.scheme == "internal" && id.authority.as_deref() == Some("supplier-list")
            })
        })
        .map(|id| id.value.as_str())
        .unwrap_or("")
        .to_owned()
}

/// Extracts a generic `internal` identifier, excluding `supplier-list` ones.
fn find_internal_id(node: &Node) -> String {
    node.identifiers
        .as_deref()
        .and_then(|ids| {
            ids.iter().find(|id| {
                id.scheme == "internal" && id.authority.as_deref() != Some("supplier-list")
            })
        })
        .map(|id| id.value.as_str())
        .unwrap_or("")
        .to_owned()
}

fn ws_write(ws: &mut Worksheet, row: u32, col: u16, val: &str) -> Result<(), ExportError> {
    if val.is_empty() {
        return Ok(());
    }
    ws.write(row, col, val)
        .map(|_| ())
        .map_err(|e: XlsxError| ExportError::ExcelWrite {
            detail: e.to_string(),
        })
}

fn ws_write_f64(ws: &mut Worksheet, row: u32, col: u16, val: f64) -> Result<(), ExportError> {
    ws.write(row, col, val)
        .map(|_| ())
        .map_err(|e: XlsxError| ExportError::ExcelWrite {
            detail: e.to_string(),
        })
}

fn ws_write_u32(ws: &mut Worksheet, row: u32, col: u16, val: u32) -> Result<(), ExportError> {
    ws.write(row, col, val)
        .map(|_| ())
        .map_err(|e: XlsxError| ExportError::ExcelWrite {
            detail: e.to_string(),
        })
}
