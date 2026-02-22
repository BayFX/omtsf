//! Implementation of `omtsf init`.
//!
//! Generates a minimal valid `.omts` file and writes it to stdout.
//!
//! Flags:
//! - `--example`: generate a realistic example file with sample nodes and edges
//!   instead of the minimal skeleton.
//!
//! The generated file always contains:
//! - A freshly generated CSPRNG `file_salt` (32 random bytes, hex-encoded).
//! - Today's date as `snapshot_date` (derived from the system clock via the
//!   Unix epoch; no external date library is required).
//!
//! Exit codes: 0 = always succeeds unless stdout write fails.
use std::collections::BTreeMap;

use omtsf_core::BoundaryHashError;
use omtsf_core::enums::{EdgeType, EdgeTypeTag, NodeType, NodeTypeTag};
use omtsf_core::file::OmtsFile;
use omtsf_core::generate_file_salt;
use omtsf_core::newtypes::{CalendarDate, EdgeId, NodeId, SemVer};
use omtsf_core::structures::{Edge, EdgeProperties, Node};

use crate::error::CliError;

/// Runs the `init` command.
///
/// Generates and prints a valid `.omts` file to stdout. When `example` is
/// `true`, realistic sample content is included; otherwise only the minimal
/// required fields are set.
///
/// # Errors
///
/// Returns [`CliError`] if the CSPRNG fails or if stdout cannot be written.
pub fn run(example: bool) -> Result<(), CliError> {
    let salt = generate_file_salt().map_err(|e| csprng_to_cli_error(&e))?;
    let today = today_string().map_err(|e| CliError::IoError {
        source: "system clock".to_owned(),
        detail: e,
    })?;
    let snapshot_date = CalendarDate::try_from(today.as_str()).map_err(|e| CliError::IoError {
        source: "system clock".to_owned(),
        detail: format!("generated date is invalid: {e}"),
    })?;
    let version = SemVer::try_from("1.0.0").map_err(|e| CliError::InternalError {
        detail: format!("hardcoded SemVer is invalid: {e}"),
    })?;

    let file = if example {
        build_example_file(version, snapshot_date, salt)
    } else {
        build_minimal_file(version, snapshot_date, salt)
    }
    .map_err(|e| CliError::InternalError {
        detail: format!("failed to construct init file: {e}"),
    })?;

    let json = serde_json::to_string_pretty(&file).map_err(|e| CliError::IoError {
        source: "init".to_owned(),
        detail: format!("JSON serialization failed: {e}"),
    })?;

    println!("{json}");
    Ok(())
}

/// Builds a minimal valid [`OmtsFile`] with empty nodes and edges.
fn build_minimal_file(
    version: SemVer,
    snapshot_date: CalendarDate,
    salt: omtsf_core::newtypes::FileSalt,
) -> Result<OmtsFile, String> {
    Ok(OmtsFile {
        omtsf_version: version,
        snapshot_date,
        file_salt: salt,
        disclosure_scope: None,
        previous_snapshot_ref: None,
        snapshot_sequence: None,
        reporting_entity: None,
        nodes: vec![],
        edges: vec![],
        extra: BTreeMap::new(),
    })
}

/// Builds a realistic example [`OmtsFile`] with sample nodes and edges.
///
/// Includes one organization, one facility, and one product node, plus
/// supply and operates edges connecting them.
fn build_example_file(
    version: SemVer,
    snapshot_date: CalendarDate,
    salt: omtsf_core::newtypes::FileSalt,
) -> Result<OmtsFile, String> {
    let org_id = NodeId::try_from("org-example-corp").map_err(|e| e.to_string())?;
    let fac_id = NodeId::try_from("fac-example-plant").map_err(|e| e.to_string())?;
    let good_id = NodeId::try_from("good-example-product").map_err(|e| e.to_string())?;
    let edge_op_id = EdgeId::try_from("edge-operates-001").map_err(|e| e.to_string())?;
    let edge_prod_id = EdgeId::try_from("edge-produces-001").map_err(|e| e.to_string())?;

    let org_node = Node {
        id: org_id.clone(),
        node_type: NodeTypeTag::Known(NodeType::Organization),
        identifiers: None,
        data_quality: None,
        labels: None,
        name: Some("Example Corporation Ltd".to_owned()),
        jurisdiction: None,
        status: None,
        governance_structure: None,
        operator: None,
        address: None,
        geo: None,
        commodity_code: None,
        unit: None,
        role: None,
        attestation_type: None,
        standard: None,
        issuer: None,
        valid_from: None,
        valid_to: None,
        outcome: None,
        attestation_status: None,
        reference: None,
        risk_severity: None,
        risk_likelihood: None,
        lot_id: None,
        quantity: None,
        production_date: None,
        origin_country: None,
        direct_emissions_co2e: None,
        indirect_emissions_co2e: None,
        emission_factor_source: None,
        installation_id: None,
        extra: BTreeMap::new(),
    };

    let fac_node = Node {
        id: fac_id.clone(),
        node_type: NodeTypeTag::Known(NodeType::Facility),
        identifiers: None,
        data_quality: None,
        labels: None,
        name: Some("Example Manufacturing Plant".to_owned()),
        jurisdiction: None,
        status: None,
        governance_structure: None,
        operator: Some(org_id.clone()),
        address: Some("123 Industrial Way, Springfield".to_owned()),
        geo: None,
        commodity_code: None,
        unit: None,
        role: None,
        attestation_type: None,
        standard: None,
        issuer: None,
        valid_from: None,
        valid_to: None,
        outcome: None,
        attestation_status: None,
        reference: None,
        risk_severity: None,
        risk_likelihood: None,
        lot_id: None,
        quantity: None,
        production_date: None,
        origin_country: None,
        direct_emissions_co2e: None,
        indirect_emissions_co2e: None,
        emission_factor_source: None,
        installation_id: None,
        extra: BTreeMap::new(),
    };

    let good_node = Node {
        id: good_id.clone(),
        node_type: NodeTypeTag::Known(NodeType::Good),
        identifiers: None,
        data_quality: None,
        labels: None,
        name: Some("Example Widget".to_owned()),
        jurisdiction: None,
        status: None,
        governance_structure: None,
        operator: None,
        address: None,
        geo: None,
        commodity_code: Some("8479.89".to_owned()),
        unit: Some("pcs".to_owned()),
        role: None,
        attestation_type: None,
        standard: None,
        issuer: None,
        valid_from: None,
        valid_to: None,
        outcome: None,
        attestation_status: None,
        reference: None,
        risk_severity: None,
        risk_likelihood: None,
        lot_id: None,
        quantity: None,
        production_date: None,
        origin_country: None,
        direct_emissions_co2e: None,
        indirect_emissions_co2e: None,
        emission_factor_source: None,
        installation_id: None,
        extra: BTreeMap::new(),
    };

    let operates_edge = Edge {
        id: edge_op_id,
        edge_type: EdgeTypeTag::Known(EdgeType::Operates),
        source: org_id,
        target: fac_id.clone(),
        identifiers: None,
        properties: EdgeProperties::default(),
        extra: BTreeMap::new(),
    };

    let produces_edge = Edge {
        id: edge_prod_id,
        edge_type: EdgeTypeTag::Known(EdgeType::Produces),
        source: fac_id,
        target: good_id,
        identifiers: None,
        properties: EdgeProperties::default(),
        extra: BTreeMap::new(),
    };

    Ok(OmtsFile {
        omtsf_version: version,
        snapshot_date,
        file_salt: salt,
        disclosure_scope: None,
        previous_snapshot_ref: None,
        snapshot_sequence: None,
        reporting_entity: None,
        nodes: vec![org_node, fac_node, good_node],
        edges: vec![operates_edge, produces_edge],
        extra: BTreeMap::new(),
    })
}

/// Returns today's date as a `YYYY-MM-DD` string derived from the system clock.
///
/// Uses `std::time::SystemTime` and Unix epoch arithmetic to avoid a dependency
/// on `chrono` or `time`. Accurate for any date in the Gregorian calendar from
/// 1970 onwards.
pub(crate) fn today_string() -> Result<String, String> {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| format!("system clock before Unix epoch: {e}"))?
        .as_secs();

    let (year, month, day) = epoch_secs_to_ymd(secs);
    Ok(format!("{year:04}-{month:02}-{day:02}"))
}

/// Converts Unix timestamp in seconds to `(year, month, day)`.
///
/// Uses the proleptic Gregorian calendar algorithm. Accurate for all dates
/// from 1970-01-01 onwards.
fn epoch_secs_to_ymd(secs: u64) -> (u32, u32, u32) {
    let days = (secs / 86_400) as u32;

    // Algorithm from http://howardhinnant.github.io/date_algorithms.html
    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };

    (y, m, d)
}

/// Maps a [`BoundaryHashError`] to a [`CliError`] with exit code 2.
fn csprng_to_cli_error(e: &BoundaryHashError) -> CliError {
    CliError::IoError {
        source: "CSPRNG".to_owned(),
        detail: e.to_string(),
    }
}
