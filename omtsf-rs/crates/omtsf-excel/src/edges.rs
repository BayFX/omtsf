/// Parses edge sheets: Supply Relationships, Corporate Structure, Same As.
///
/// Also generates `attested_by` edges from the Attestations sheet's
/// `attested_entity_id` and `scope` columns.
///
/// Supply Relationships uses domain-friendly columns: `supplier_id`/`buyer_id`
/// (maps to source/target for `supplies`-type edges).
/// Corporate Structure uses: `subsidiary_id`/`parent_id` (maps to
/// source=subsidiary, target=parent for `ownership`-type edges).
use std::collections::{BTreeMap, HashMap, HashSet};

use calamine::{Data, Range};

use omtsf_core::enums::{ConsolidationBasis, EdgeType, EdgeTypeTag};
use omtsf_core::newtypes::{CalendarDate, EdgeId, NodeId};
use omtsf_core::structures::{Edge, EdgeProperties};

use crate::error::ImportError;
use crate::sheet::{
    build_header_index, cell_is_empty, cell_ref, cell_to_string, read_optional_float,
    read_optional_string, read_optional_u32, require_column,
};
use crate::slug::make_edge_slug;

const SUPPLY_REL_SHEET: &str = "Supply Relationships";
const CORP_STRUCT_SHEET: &str = "Corporate Structure";
const SAME_AS_SHEET: &str = "Same As";

/// Parses all edge sheets and generates `attested_by` edges.
///
/// `node_ids` is the set of all node IDs known from the node sheets.
/// Edges referencing unknown nodes produce [`ImportError::UnresolvedReference`].
pub fn parse_all_edges(
    supply_rel: &Range<Data>,
    corp_struct: &Range<Data>,
    same_as: &Range<Data>,
    attestations_sheet: &Range<Data>,
    node_ids: &HashSet<String>,
) -> Result<Vec<Edge>, ImportError> {
    let mut edges = Vec::new();
    edges.extend(parse_supply_relationships(supply_rel, node_ids)?);
    edges.extend(parse_corporate_structure(corp_struct, node_ids)?);
    edges.extend(parse_same_as(same_as, node_ids)?);
    edges.extend(generate_attested_by_edges(
        &edges,
        attestations_sheet,
        node_ids,
    )?);
    Ok(edges)
}

/// Parses the Supply Relationships sheet.
///
/// Domain columns: `supplier_id` → source, `buyer_id` → target.
/// Edge type is read from the `type` column; defaults to `supplies`.
fn parse_supply_relationships(
    sheet: &Range<Data>,
    node_ids: &HashSet<String>,
) -> Result<Vec<Edge>, ImportError> {
    let sheet_name = SUPPLY_REL_SHEET;
    if sheet.is_empty() {
        return Ok(vec![]);
    }
    let headers = build_header_index(sheet, sheet_name)?;
    let mut edges = Vec::new();
    let mut counter = 0usize;

    for (row_idx, row) in sheet.rows().skip(1).enumerate() {
        if row.iter().all(cell_is_empty) {
            continue;
        }
        counter += 1;

        let raw_id = read_optional_string(row, &headers, "id");
        let edge_type_str =
            read_optional_string(row, &headers, "type").unwrap_or_else(|| "supplies".to_owned());
        let edge_type = parse_supply_edge_type(&edge_type_str);

        let source_id = read_required_id(row, &headers, sheet_name, "supplier_id", row_idx)?;
        let target_id = read_required_id(row, &headers, sheet_name, "buyer_id", row_idx)?;

        check_ref(
            node_ids,
            &source_id,
            sheet_name,
            headers.get("supplier_id").copied().unwrap_or(2),
            row_idx,
        )?;
        check_ref(
            node_ids,
            &target_id,
            sheet_name,
            headers.get("buyer_id").copied().unwrap_or(3),
            row_idx,
        )?;

        let edge_id_str = raw_id
            .unwrap_or_else(|| make_edge_slug(&edge_type_str, &source_id, &target_id, counter));
        let edge_id =
            EdgeId::try_from(edge_id_str.as_str()).map_err(|e| ImportError::InvalidCell {
                cell_ref: format!("{sheet_name}!A{}", row_idx + 2),
                expected: "non-empty edge ID".to_owned(),
                got: format!("{edge_id_str} ({e})"),
            })?;

        let source =
            NodeId::try_from(source_id.as_str()).map_err(|_| ImportError::InvalidCell {
                cell_ref: format!("{sheet_name}!C{}", row_idx + 2),
                expected: "valid node ID".to_owned(),
                got: source_id.clone(),
            })?;
        let target =
            NodeId::try_from(target_id.as_str()).map_err(|_| ImportError::InvalidCell {
                cell_ref: format!("{sheet_name}!D{}", row_idx + 2),
                expected: "valid node ID".to_owned(),
                got: target_id.clone(),
            })?;

        let valid_from = read_optional_string(row, &headers, "valid_from")
            .and_then(|s| CalendarDate::try_from(s.as_str()).ok());
        let valid_to: Option<Option<CalendarDate>> =
            read_optional_string(row, &headers, "valid_to")
                .and_then(|s| CalendarDate::try_from(s.as_str()).ok())
                .map(Some);
        let commodity = read_optional_string(row, &headers, "commodity");
        let tier = read_optional_u32(row, &headers, "tier", sheet_name, row_idx)?;
        let volume = read_optional_float(row, &headers, "volume", sheet_name, row_idx)?;
        let volume_unit = read_optional_string(row, &headers, "volume_unit");
        let annual_value = read_optional_float(row, &headers, "annual_value", sheet_name, row_idx)?;
        let value_currency = read_optional_string(row, &headers, "value_currency");
        let contract_ref = read_optional_string(row, &headers, "contract_ref");
        let share_of_buyer_demand =
            read_optional_float(row, &headers, "share_of_buyer_demand", sheet_name, row_idx)?;

        let properties = EdgeProperties {
            valid_from,
            valid_to,
            commodity,
            tier,
            volume,
            volume_unit,
            annual_value,
            value_currency,
            contract_ref,
            share_of_buyer_demand,
            ..Default::default()
        };

        edges.push(Edge {
            id: edge_id,
            edge_type: EdgeTypeTag::Known(edge_type),
            source,
            target,
            identifiers: None,
            properties,
            extra: BTreeMap::new(),
        });
    }
    Ok(edges)
}

/// Parses the Corporate Structure sheet.
///
/// Domain columns: `subsidiary_id` → source, `parent_id` → target.
/// Edge type defaults to `ownership`.
fn parse_corporate_structure(
    sheet: &Range<Data>,
    node_ids: &HashSet<String>,
) -> Result<Vec<Edge>, ImportError> {
    let sheet_name = CORP_STRUCT_SHEET;
    if sheet.is_empty() {
        return Ok(vec![]);
    }
    let headers = build_header_index(sheet, sheet_name)?;
    let mut edges = Vec::new();
    let mut counter = 0usize;

    for (row_idx, row) in sheet.rows().skip(1).enumerate() {
        if row.iter().all(cell_is_empty) {
            continue;
        }
        counter += 1;

        let raw_id = read_optional_string(row, &headers, "id");
        let edge_type_str =
            read_optional_string(row, &headers, "type").unwrap_or_else(|| "ownership".to_owned());
        let edge_type = parse_corporate_edge_type(&edge_type_str);

        let source_id = read_required_id(row, &headers, sheet_name, "subsidiary_id", row_idx)?;
        let target_id = read_required_id(row, &headers, sheet_name, "parent_id", row_idx)?;

        check_ref(
            node_ids,
            &source_id,
            sheet_name,
            headers.get("subsidiary_id").copied().unwrap_or(2),
            row_idx,
        )?;
        check_ref(
            node_ids,
            &target_id,
            sheet_name,
            headers.get("parent_id").copied().unwrap_or(3),
            row_idx,
        )?;

        let edge_id_str = raw_id
            .unwrap_or_else(|| make_edge_slug(&edge_type_str, &source_id, &target_id, counter));
        let edge_id =
            EdgeId::try_from(edge_id_str.as_str()).map_err(|e| ImportError::InvalidCell {
                cell_ref: format!("{sheet_name}!A{}", row_idx + 2),
                expected: "non-empty edge ID".to_owned(),
                got: format!("{edge_id_str} ({e})"),
            })?;

        let source =
            NodeId::try_from(source_id.as_str()).map_err(|_| ImportError::InvalidCell {
                cell_ref: format!("{sheet_name}!C{}", row_idx + 2),
                expected: "valid node ID".to_owned(),
                got: source_id.clone(),
            })?;
        let target =
            NodeId::try_from(target_id.as_str()).map_err(|_| ImportError::InvalidCell {
                cell_ref: format!("{sheet_name}!D{}", row_idx + 2),
                expected: "valid node ID".to_owned(),
                got: target_id.clone(),
            })?;

        let valid_from = read_optional_string(row, &headers, "valid_from")
            .and_then(|s| CalendarDate::try_from(s.as_str()).ok());
        let valid_to: Option<Option<CalendarDate>> =
            read_optional_string(row, &headers, "valid_to")
                .and_then(|s| CalendarDate::try_from(s.as_str()).ok())
                .map(Some);
        let percentage = read_optional_float(row, &headers, "percentage", sheet_name, row_idx)?;
        let direct_raw = read_optional_string(row, &headers, "direct");
        let direct = direct_raw
            .as_deref()
            .map(|s| matches!(s.to_lowercase().as_str(), "true" | "yes" | "1"));
        let consolidation_basis = read_optional_string(row, &headers, "consolidation_basis")
            .as_deref()
            .and_then(parse_consolidation_basis);

        let properties = EdgeProperties {
            valid_from,
            valid_to,
            percentage,
            direct,
            consolidation_basis,
            ..Default::default()
        };

        edges.push(Edge {
            id: edge_id,
            edge_type: EdgeTypeTag::Known(edge_type),
            source,
            target,
            identifiers: None,
            properties,
            extra: BTreeMap::new(),
        });
    }
    Ok(edges)
}

/// Parses the Same As sheet into `same_as` edges.
fn parse_same_as(
    sheet: &Range<Data>,
    node_ids: &HashSet<String>,
) -> Result<Vec<Edge>, ImportError> {
    let sheet_name = SAME_AS_SHEET;
    if sheet.is_empty() {
        return Ok(vec![]);
    }
    let headers = build_header_index(sheet, sheet_name)?;
    let a_col = require_column(&headers, sheet_name, "entity_a")?;
    let b_col = require_column(&headers, sheet_name, "entity_b")?;
    let mut edges = Vec::new();
    let mut counter = 0usize;

    for (row_idx, row) in sheet.rows().skip(1).enumerate() {
        if row.iter().all(cell_is_empty) {
            continue;
        }
        counter += 1;

        let a_id = cell_to_string(row.get(a_col).unwrap_or(&Data::Empty));
        let b_id = cell_to_string(row.get(b_col).unwrap_or(&Data::Empty));

        if a_id.is_empty() || b_id.is_empty() {
            continue;
        }

        check_ref(node_ids, &a_id, sheet_name, a_col, row_idx)?;
        check_ref(node_ids, &b_id, sheet_name, b_col, row_idx)?;

        let edge_id_str = make_edge_slug("same-as", &a_id, &b_id, counter);
        let edge_id =
            EdgeId::try_from(edge_id_str.as_str()).map_err(|_| ImportError::InvalidCell {
                cell_ref: format!("{sheet_name}!A{}", row_idx + 2),
                expected: "valid edge ID".to_owned(),
                got: edge_id_str.clone(),
            })?;
        let source = NodeId::try_from(a_id.as_str()).map_err(|_| ImportError::InvalidCell {
            cell_ref: cell_ref(sheet_name, a_col, row_idx),
            expected: "valid node ID".to_owned(),
            got: a_id.clone(),
        })?;
        let target = NodeId::try_from(b_id.as_str()).map_err(|_| ImportError::InvalidCell {
            cell_ref: cell_ref(sheet_name, b_col, row_idx),
            expected: "valid node ID".to_owned(),
            got: b_id.clone(),
        })?;

        let basis = read_optional_string(row, &headers, "basis");

        let mut extra: BTreeMap<String, omtsf_core::dynvalue::DynValue> = BTreeMap::new();
        if let Some(b) = &basis {
            extra.insert(
                "basis".to_owned(),
                omtsf_core::dynvalue::DynValue::from(serde_json::json!(b)),
            );
        }
        if let Some(conf_str) = read_optional_string(row, &headers, "confidence") {
            extra.insert(
                "confidence".to_owned(),
                omtsf_core::dynvalue::DynValue::from(serde_json::json!(conf_str)),
            );
        }

        edges.push(Edge {
            id: edge_id,
            edge_type: EdgeTypeTag::Known(EdgeType::SameAs),
            source,
            target,
            identifiers: None,
            properties: EdgeProperties::default(),
            extra,
        });
    }
    Ok(edges)
}

/// Generates `attested_by` edges from Attestations sheet rows that have
/// `attested_entity_id` set.
fn generate_attested_by_edges(
    existing_edges: &[Edge],
    attestations: &Range<Data>,
    node_ids: &HashSet<String>,
) -> Result<Vec<Edge>, ImportError> {
    let sheet_name = "Attestations";
    if attestations.is_empty() {
        return Ok(vec![]);
    }
    let headers = build_header_index(attestations, sheet_name)?;
    let id_col = match headers.get("id") {
        Some(c) => *c,
        None => return Ok(vec![]),
    };
    let entity_col = match headers.get("attested_entity_id") {
        Some(c) => *c,
        None => return Ok(vec![]),
    };

    let mut edges = Vec::new();
    let mut counter = existing_edges.len();

    for (row_idx, row) in attestations.rows().skip(1).enumerate() {
        if row.iter().all(cell_is_empty) {
            continue;
        }
        counter += 1;

        let att_id = cell_to_string(row.get(id_col).unwrap_or(&Data::Empty));
        let entity_id = cell_to_string(row.get(entity_col).unwrap_or(&Data::Empty));

        if att_id.is_empty() || entity_id.is_empty() {
            continue;
        }

        // att_id must be known (it's a node on this sheet).
        // entity_id must reference an existing node.
        check_ref(node_ids, &att_id, sheet_name, id_col, row_idx)?;
        check_ref(node_ids, &entity_id, sheet_name, entity_col, row_idx)?;

        let edge_id_str = format!("edge-attested-by-{counter}");
        let edge_id =
            EdgeId::try_from(edge_id_str.as_str()).map_err(|_| ImportError::InvalidCell {
                cell_ref: format!("{sheet_name}!A{}", row_idx + 2),
                expected: "valid edge ID".to_owned(),
                got: edge_id_str.clone(),
            })?;

        let source =
            NodeId::try_from(entity_id.as_str()).map_err(|_| ImportError::InvalidCell {
                cell_ref: cell_ref(sheet_name, entity_col, row_idx),
                expected: "valid node ID".to_owned(),
                got: entity_id.clone(),
            })?;
        let target = NodeId::try_from(att_id.as_str()).map_err(|_| ImportError::InvalidCell {
            cell_ref: cell_ref(sheet_name, id_col, row_idx),
            expected: "valid node ID".to_owned(),
            got: att_id.clone(),
        })?;

        let scope = read_optional_string(row, &headers, "scope");
        let properties = EdgeProperties {
            scope,
            ..Default::default()
        };

        edges.push(Edge {
            id: edge_id,
            edge_type: EdgeTypeTag::Known(EdgeType::AttestedBy),
            source,
            target,
            identifiers: None,
            properties,
            extra: BTreeMap::new(),
        });
    }
    Ok(edges)
}

fn check_ref(
    node_ids: &HashSet<String>,
    id: &str,
    sheet_name: &str,
    col_idx: usize,
    row_idx: usize,
) -> Result<(), ImportError> {
    if !node_ids.contains(id) {
        return Err(ImportError::UnresolvedReference {
            cell_ref: crate::sheet::cell_ref(sheet_name, col_idx, row_idx),
            node_id: id.to_owned(),
        });
    }
    Ok(())
}

fn read_required_id(
    row: &[Data],
    headers: &HashMap<String, usize>,
    sheet_name: &str,
    header: &str,
    row_idx: usize,
) -> Result<String, ImportError> {
    let col = headers
        .get(&header.to_lowercase())
        .copied()
        .ok_or_else(|| ImportError::MissingColumn {
            sheet: sheet_name.to_owned(),
            column: header.to_owned(),
        })?;
    let val = cell_to_string(row.get(col).unwrap_or(&Data::Empty));
    if val.is_empty() {
        Err(ImportError::InvalidCell {
            cell_ref: crate::sheet::cell_ref(sheet_name, col, row_idx),
            expected: "node ID reference".to_owned(),
            got: String::new(),
        })
    } else {
        Ok(val)
    }
}

fn parse_supply_edge_type(s: &str) -> EdgeType {
    match s.trim().to_lowercase().as_str() {
        "supplies" => EdgeType::Supplies,
        "subcontracts" => EdgeType::Subcontracts,
        "tolls" => EdgeType::Tolls,
        "distributes" => EdgeType::Distributes,
        "brokers" => EdgeType::Brokers,
        "operates" => EdgeType::Operates,
        "produces" => EdgeType::Produces,
        "sells_to" | "sells to" => EdgeType::SellsTo,
        _ => EdgeType::Supplies,
    }
}

fn parse_corporate_edge_type(s: &str) -> EdgeType {
    match s.trim().to_lowercase().as_str() {
        "ownership" => EdgeType::Ownership,
        "legal_parentage" | "legal parentage" => EdgeType::LegalParentage,
        "operational_control" | "operational control" => EdgeType::OperationalControl,
        "beneficial_ownership" | "beneficial ownership" => EdgeType::BeneficialOwnership,
        "former_identity" | "former identity" => EdgeType::FormerIdentity,
        _ => EdgeType::Ownership,
    }
}

fn parse_consolidation_basis(s: &str) -> Option<ConsolidationBasis> {
    match s.trim().to_lowercase().as_str() {
        "ifrs10" | "ifrs 10" => Some(ConsolidationBasis::Ifrs10),
        "us_gaap_asc810" | "us gaap asc810" | "gaap" => Some(ConsolidationBasis::UsGaapAsc810),
        "other" => Some(ConsolidationBasis::Other),
        "unknown" => Some(ConsolidationBasis::Unknown),
        _ => None,
    }
}
