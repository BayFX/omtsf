/// Import logic for the simplified single-sheet "Supplier List" template.
///
/// Template layout:
/// - Row 1: `A1="Reporting Entity"`, `B1=<name>`, `C1="Snapshot Date"`, `D1=<YYYY-MM-DD>`
/// - Row 2: `A2="Disclosure Scope"`, `B2=<scope>`
/// - Row 3: blank separator
/// - Row 4: column headers
/// - Row 5+: data rows
use std::collections::{BTreeMap, HashMap};
use std::io::{Read, Seek};

use calamine::{Data, Range, Reader, Xlsx};

use omtsf_core::enums::{DisclosureScope, EdgeType, EdgeTypeTag, NodeType, NodeTypeTag};
use omtsf_core::file::OmtsFile;
use omtsf_core::generate_file_salt;
use omtsf_core::newtypes::{CalendarDate, CountryCode, EdgeId, NodeId, SemVer};
use omtsf_core::structures::{Edge, EdgeProperties, Node};
use omtsf_core::types::{Identifier, Label};

use crate::error::ImportError;
use crate::sheet::cell_to_string;
use crate::slug::{make_edge_slug, make_slug};

const OMTSF_VERSION: &str = "1.0.0";
const SHEET_NAME: &str = "Supplier List";

/// Parsed data from a single supplier row.
struct SupplierRow {
    supplier_name: String,
    supplier_id: Option<String>,
    jurisdiction: Option<String>,
    tier: u32,
    parent_supplier: Option<String>,
    business_unit: Option<String>,
    commodity: Option<String>,
    valid_from: Option<String>,
    annual_value: Option<f64>,
    value_currency: Option<String>,
    contract_ref: Option<String>,
    lei: Option<String>,
    duns: Option<String>,
    vat: Option<String>,
    vat_country: Option<String>,
    risk_tier: Option<String>,
    kraljic_quadrant: Option<String>,
    approval_status: Option<String>,
}

/// Imports a "Supplier List" single-sheet workbook into an [`OmtsFile`].
///
/// # Errors
///
/// Returns [`ImportError`] for missing required fields, invalid cell values,
/// unresolved parent supplier references, or L1 validation failures.
pub fn import_supplier_list<R: Read + Seek>(
    mut workbook: Xlsx<R>,
) -> Result<OmtsFile, ImportError> {
    let sheet = workbook
        .worksheet_range(SHEET_NAME)
        .map_err(|e| ImportError::ExcelRead {
            detail: e.to_string(),
        })?;

    let reporting_entity_name = read_metadata_cell(&sheet, 0, 1)?;
    if reporting_entity_name.is_empty() {
        return Err(ImportError::InvalidCell {
            cell_ref: "Supplier List!B1".to_owned(),
            expected: "reporting entity name".to_owned(),
            got: String::new(),
        });
    }

    let snapshot_date_str = read_metadata_cell(&sheet, 0, 3).unwrap_or_default();
    let snapshot_date = if snapshot_date_str.is_empty() {
        CalendarDate::try_from("2000-01-01").map_err(|e| ImportError::InvalidCell {
            cell_ref: "Supplier List!D1".to_owned(),
            expected: "YYYY-MM-DD date".to_owned(),
            got: format!("(default failed: {e})"),
        })?
    } else {
        CalendarDate::try_from(snapshot_date_str.as_str()).map_err(|e| {
            ImportError::InvalidCell {
                cell_ref: "Supplier List!D1".to_owned(),
                expected: "YYYY-MM-DD date".to_owned(),
                got: format!("{snapshot_date_str} ({e})"),
            }
        })?
    };

    let disclosure_scope_str = read_metadata_cell(&sheet, 1, 1).unwrap_or_default();
    let disclosure_scope = if disclosure_scope_str.is_empty() {
        Some(DisclosureScope::Partner)
    } else {
        Some(parse_disclosure_scope(&disclosure_scope_str)?)
    };

    let headers = build_header_index_at_row(&sheet, 3);
    let rows = parse_data_rows(&sheet, &headers)?;

    if rows.iter().all(|r| r.supplier_name.is_empty()) {
        return Err(ImportError::InvalidCell {
            cell_ref: format!("{SHEET_NAME}!A5"),
            expected: "at least one non-empty supplier_name".to_owned(),
            got: String::new(),
        });
    }

    validate_parent_refs(&rows)?;

    let file_salt = generate_file_salt().map_err(|e| ImportError::ExcelRead {
        detail: format!("CSPRNG failure: {e}"),
    })?;

    let omtsf_version = SemVer::try_from(OMTSF_VERSION).map_err(|e| ImportError::ExcelRead {
        detail: format!("internal: invalid version string: {e}"),
    })?;

    let reporting_entity_id = make_slug("org", &reporting_entity_name, 0);
    let reporting_entity_node_id =
        NodeId::try_from(reporting_entity_id.as_str()).map_err(|e| ImportError::ExcelRead {
            detail: format!("invalid reporting entity node id: {e}"),
        })?;

    let reporting_entity_node = Node {
        id: reporting_entity_node_id.clone(),
        node_type: NodeTypeTag::Known(NodeType::Organization),
        name: Some(reporting_entity_name.clone()),
        ..Default::default()
    };

    let supplier_result = build_supplier_nodes(&rows, &reporting_entity_id)?;
    let edges = build_edges(
        &rows,
        &supplier_result.name_to_node_id,
        &supplier_result.id_to_node_id,
        &reporting_entity_id,
    )?;

    let mut nodes = vec![reporting_entity_node];
    nodes.extend(supplier_result.nodes);

    let omts_file = OmtsFile {
        omtsf_version,
        snapshot_date,
        file_salt,
        disclosure_scope,
        previous_snapshot_ref: None,
        snapshot_sequence: None,
        reporting_entity: Some(reporting_entity_node_id),
        nodes,
        edges,
        extra: BTreeMap::new(),
    };

    let config = omtsf_core::validation::ValidationConfig {
        run_l1: true,
        run_l2: false,
        run_l3: false,
    };
    let result = omtsf_core::validation::validate(&omts_file, &config, None);
    let errors: Vec<String> = result
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, omtsf_core::validation::Severity::Error))
        .map(|d| d.message.clone())
        .collect();
    if !errors.is_empty() {
        return Err(ImportError::ValidationFailed {
            diagnostics: errors,
        });
    }

    Ok(omts_file)
}

/// Reads a metadata cell from the sheet at (`row_idx`, `col_idx`) (0-based).
///
/// Returns an empty string if the cell is absent or empty; returns error only
/// on a calamine read error.
fn read_metadata_cell(
    sheet: &Range<Data>,
    row_idx: u32,
    col_idx: u32,
) -> Result<String, ImportError> {
    let rows: Vec<_> = sheet.rows().collect();
    let row = rows.get(row_idx as usize);
    let cell = row
        .and_then(|r| r.get(col_idx as usize))
        .unwrap_or(&Data::Empty);
    Ok(cell_to_string(cell))
}

/// Builds a header-name → column-index map from the row at the given 0-based row index.
fn build_header_index_at_row(sheet: &Range<Data>, row_idx: usize) -> HashMap<String, usize> {
    let mut map = HashMap::new();
    let rows: Vec<_> = sheet.rows().collect();
    let Some(row) = rows.get(row_idx) else {
        return map;
    };
    for (col_idx, cell) in row.iter().enumerate() {
        let header = cell_to_string(cell).trim().to_lowercase();
        if !header.is_empty() {
            map.insert(header, col_idx);
        }
    }
    map
}

/// Parses data rows starting at row index 4 (0-based), which is row 5 in Excel.
fn parse_data_rows(
    sheet: &Range<Data>,
    headers: &HashMap<String, usize>,
) -> Result<Vec<SupplierRow>, ImportError> {
    let mut result = Vec::new();
    let all_rows: Vec<_> = sheet.rows().collect();
    for row in all_rows.iter().skip(4) {
        let supplier_name = get_cell_str(row, headers, "supplier_name");
        if supplier_name.is_empty() {
            continue;
        }

        let tier_str = get_cell_str(row, headers, "tier");
        let tier = if tier_str.is_empty() {
            1u32
        } else {
            tier_str.parse::<u32>().unwrap_or(1).clamp(1, 3)
        };

        let annual_value = get_cell_f64(row, headers, "annual_value");

        result.push(SupplierRow {
            supplier_name,
            supplier_id: non_empty(get_cell_str(row, headers, "supplier_id")),
            jurisdiction: non_empty(get_cell_str(row, headers, "jurisdiction")),
            tier,
            parent_supplier: non_empty(get_cell_str(row, headers, "parent_supplier")),
            business_unit: non_empty(get_cell_str(row, headers, "business_unit")),
            commodity: non_empty(get_cell_str(row, headers, "commodity")),
            valid_from: non_empty(get_cell_str(row, headers, "valid_from")),
            annual_value,
            value_currency: non_empty(get_cell_str(row, headers, "value_currency")),
            contract_ref: non_empty(get_cell_str(row, headers, "contract_ref")),
            lei: non_empty(get_cell_str(row, headers, "lei")),
            duns: non_empty(get_cell_str(row, headers, "duns")),
            vat: non_empty(get_cell_str(row, headers, "vat")),
            vat_country: non_empty(get_cell_str(row, headers, "vat_country")),
            risk_tier: non_empty(get_cell_str(row, headers, "risk_tier")),
            kraljic_quadrant: non_empty(get_cell_str(row, headers, "kraljic_quadrant")),
            approval_status: non_empty(get_cell_str(row, headers, "approval_status")),
        });
    }
    Ok(result)
}

fn get_cell_str(row: &[Data], headers: &HashMap<String, usize>, key: &str) -> String {
    let col = match headers.get(key) {
        Some(c) => *c,
        None => return String::new(),
    };
    let cell = row.get(col).unwrap_or(&Data::Empty);
    cell_to_string(cell)
}

fn get_cell_f64(row: &[Data], headers: &HashMap<String, usize>, key: &str) -> Option<f64> {
    let s = get_cell_str(row, headers, key);
    if s.is_empty() {
        return None;
    }
    s.parse::<f64>().ok()
}

fn non_empty(s: String) -> Option<String> {
    if s.is_empty() { None } else { Some(s) }
}

/// Validates that all tier 2/3 rows have a `parent_supplier` that exists as a
/// `supplier_name` or `supplier_id` in the file.
fn validate_parent_refs(rows: &[SupplierRow]) -> Result<(), ImportError> {
    let all_names: std::collections::HashSet<&str> =
        rows.iter().map(|r| r.supplier_name.as_str()).collect();
    let all_ids: std::collections::HashSet<&str> = rows
        .iter()
        .filter_map(|r| r.supplier_id.as_deref())
        .collect();

    for (i, row) in rows.iter().enumerate() {
        if row.tier >= 2 {
            match &row.parent_supplier {
                None => {
                    return Err(ImportError::InvalidCell {
                        cell_ref: format!("{SHEET_NAME}!E{}", i + 5),
                        expected: "parent_supplier for tier 2/3 row".to_owned(),
                        got: String::new(),
                    });
                }
                Some(parent) => {
                    if !all_names.contains(parent.as_str()) && !all_ids.contains(parent.as_str()) {
                        return Err(ImportError::UnresolvedReference {
                            cell_ref: format!("{SHEET_NAME}!E{}", i + 5),
                            node_id: parent.clone(),
                        });
                    }
                }
            }
        }
    }
    Ok(())
}

/// Result of [`build_supplier_nodes`]: org nodes plus two lookup maps.
struct SupplierNodeResult {
    nodes: Vec<Node>,
    name_to_node_id: HashMap<String, String>,
    id_to_node_id: HashMap<String, String>,
}

/// Builds a vec of supplier org nodes and lookup maps.
///
/// When `supplier_id` is present, rows sharing the same `supplier_id` collapse
/// to one node regardless of name. Otherwise, dedup falls back to
/// `supplier_name` (preserving existing behaviour).
fn build_supplier_nodes(
    rows: &[SupplierRow],
    reporting_entity_id: &str,
) -> Result<SupplierNodeResult, ImportError> {
    let mut name_to_node_id: HashMap<String, String> = HashMap::new();
    let mut id_to_node_id: HashMap<String, String> = HashMap::new();
    let mut node_map: HashMap<String, Node> = HashMap::new();
    let mut counter = 1usize;

    for row in rows {
        let name = &row.supplier_name;

        let existing_node_id = if let Some(sid) = &row.supplier_id {
            id_to_node_id.get(sid).cloned()
        } else {
            name_to_node_id.get(name).cloned()
        };

        let node_id_str = match existing_node_id {
            Some(nid) => {
                name_to_node_id.entry(name.clone()).or_insert(nid.clone());
                nid
            }
            None => {
                let id = make_slug("org", name, counter);
                counter += 1;
                let id = if id == reporting_entity_id {
                    format!("{id}-supplier")
                } else {
                    id
                };
                name_to_node_id.insert(name.clone(), id.clone());
                if let Some(sid) = &row.supplier_id {
                    id_to_node_id.insert(sid.clone(), id.clone());
                }
                id
            }
        };

        let node_id =
            NodeId::try_from(node_id_str.as_str()).map_err(|e| ImportError::ExcelRead {
                detail: format!("invalid node id {node_id_str:?}: {e}"),
            })?;

        let entry = node_map.entry(node_id_str).or_insert_with(|| {
            let jurisdiction = row
                .jurisdiction
                .as_deref()
                .and_then(|j| CountryCode::try_from(j).ok());
            Node {
                id: node_id.clone(),
                node_type: NodeTypeTag::Known(NodeType::Organization),
                name: Some(name.clone()),
                jurisdiction,
                ..Default::default()
            }
        });

        merge_identifiers(entry, row);
        if let Some(sid) = &row.supplier_id {
            merge_supplier_id_identifier(entry, sid);
        }
    }

    let nodes = node_map.into_values().collect();
    Ok(SupplierNodeResult {
        nodes,
        name_to_node_id,
        id_to_node_id,
    })
}

/// Merges identifiers from a row onto an existing node (deduplicating by scheme+value).
fn merge_identifiers(node: &mut Node, row: &SupplierRow) {
    let existing = node.identifiers.get_or_insert_with(Vec::new);

    let mut add_id = |scheme: &str, value: &str, authority: Option<String>| {
        if value.is_empty() {
            return;
        }
        let already_present = existing
            .iter()
            .any(|id| id.scheme == scheme && id.value == value);
        if !already_present {
            existing.push(Identifier {
                scheme: scheme.to_owned(),
                value: value.to_owned(),
                authority,
                valid_from: None,
                valid_to: None,
                sensitivity: None,
                verification_status: None,
                verification_date: None,
                extra: BTreeMap::new(),
            });
        }
    };

    if let Some(lei) = &row.lei {
        add_id("lei", lei, None);
    }
    if let Some(duns) = &row.duns {
        add_id("duns", duns, None);
    }
    if let Some(vat) = &row.vat {
        add_id("vat", vat, row.vat_country.clone());
    }
    if existing.is_empty() {
        node.identifiers = None;
    }
}

/// Stores `supplier_id` as an `internal` identifier with authority `supplier-list`.
fn merge_supplier_id_identifier(node: &mut Node, supplier_id: &str) {
    let existing = node.identifiers.get_or_insert_with(Vec::new);
    let already_present = existing.iter().any(|id| {
        id.scheme == "internal"
            && id.value == supplier_id
            && id.authority.as_deref() == Some("supplier-list")
    });
    if !already_present {
        existing.push(Identifier {
            scheme: "internal".to_owned(),
            value: supplier_id.to_owned(),
            authority: Some("supplier-list".to_owned()),
            valid_from: None,
            valid_to: None,
            sensitivity: None,
            verification_status: None,
            verification_date: None,
            extra: BTreeMap::new(),
        });
    }
}

/// Builds edges from the parsed supplier rows.
///
/// Edge direction: `supplies` goes FROM supplier TO buyer.
/// - Tier 1: supplier → reporting entity
/// - Tier 2/3: supplier → parent supplier
///
/// Relationship-specific labels (`risk-tier`, `kraljic-quadrant`,
/// `approval-status`, `business-unit`) are attached to edge properties.
fn build_edges(
    rows: &[SupplierRow],
    name_to_node_id: &HashMap<String, String>,
    id_to_node_id: &HashMap<String, String>,
    reporting_entity_id: &str,
) -> Result<Vec<Edge>, ImportError> {
    let mut edges = Vec::new();
    let mut counter = 1usize;

    for row in rows {
        let source_id_str =
            name_to_node_id
                .get(&row.supplier_name)
                .ok_or_else(|| ImportError::ExcelRead {
                    detail: format!("supplier not in id map: {}", row.supplier_name),
                })?;

        let target_id_str = if row.tier == 1 {
            reporting_entity_id.to_owned()
        } else {
            let parent = row
                .parent_supplier
                .as_deref()
                .ok_or_else(|| ImportError::ExcelRead {
                    detail: format!(
                        "tier {} row for {:?} has no parent_supplier",
                        row.tier, row.supplier_name
                    ),
                })?;
            id_to_node_id
                .get(parent)
                .or_else(|| name_to_node_id.get(parent))
                .ok_or_else(|| ImportError::UnresolvedReference {
                    cell_ref: format!("{SHEET_NAME}!parent_supplier"),
                    node_id: parent.to_owned(),
                })?
                .clone()
        };

        let source_id =
            NodeId::try_from(source_id_str.as_str()).map_err(|e| ImportError::ExcelRead {
                detail: format!("invalid source id {source_id_str:?}: {e}"),
            })?;
        let target_id =
            NodeId::try_from(target_id_str.as_str()).map_err(|e| ImportError::ExcelRead {
                detail: format!("invalid target id {target_id_str:?}: {e}"),
            })?;

        let edge_id_str = make_edge_slug("supplies", source_id_str, &target_id_str, counter);
        let edge_id =
            EdgeId::try_from(edge_id_str.as_str()).map_err(|e| ImportError::ExcelRead {
                detail: format!("invalid edge id {edge_id_str:?}: {e}"),
            })?;
        counter += 1;

        let valid_from = row
            .valid_from
            .as_deref()
            .and_then(|s| CalendarDate::try_from(s).ok());

        let mut labels = Vec::new();
        if let Some(rt) = &row.risk_tier {
            labels.push(Label {
                key: "risk-tier".to_owned(),
                value: Some(rt.clone()),
                extra: BTreeMap::new(),
            });
        }
        if let Some(kq) = &row.kraljic_quadrant {
            labels.push(Label {
                key: "kraljic-quadrant".to_owned(),
                value: Some(kq.clone()),
                extra: BTreeMap::new(),
            });
        }
        if let Some(ap) = &row.approval_status {
            labels.push(Label {
                key: "approval-status".to_owned(),
                value: Some(ap.clone()),
                extra: BTreeMap::new(),
            });
        }
        if let Some(bu) = &row.business_unit {
            labels.push(Label {
                key: "business-unit".to_owned(),
                value: Some(bu.clone()),
                extra: BTreeMap::new(),
            });
        }

        let properties = EdgeProperties {
            commodity: row.commodity.clone(),
            valid_from,
            annual_value: row.annual_value,
            value_currency: row.value_currency.clone(),
            contract_ref: row.contract_ref.clone(),
            tier: Some(row.tier),
            labels: if labels.is_empty() {
                None
            } else {
                Some(labels)
            },
            ..Default::default()
        };

        edges.push(Edge {
            id: edge_id,
            edge_type: EdgeTypeTag::Known(EdgeType::Supplies),
            source: source_id,
            target: target_id,
            identifiers: None,
            properties,
            extra: BTreeMap::new(),
        });
    }

    Ok(edges)
}

fn parse_disclosure_scope(s: &str) -> Result<DisclosureScope, ImportError> {
    match s.trim().to_lowercase().as_str() {
        "internal" => Ok(DisclosureScope::Internal),
        "partner" => Ok(DisclosureScope::Partner),
        "public" => Ok(DisclosureScope::Public),
        other => Err(ImportError::InvalidCell {
            cell_ref: format!("{SHEET_NAME}!B2"),
            expected: "internal, partner, or public".to_owned(),
            got: other.to_owned(),
        }),
    }
}
