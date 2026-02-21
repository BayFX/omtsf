//! Implementation of `omtsf inspect <file>`.
//!
//! Parses an `.omts` file and prints summary statistics to stdout:
//! - node count by type
//! - edge count by type
//! - identifier count by scheme (across all nodes and edges)
//! - disclosure scope (if present)
//! - OMTSF version
//! - snapshot date
//! - reporting entity (if present)
//!
//! In `--format json` mode a single JSON object is emitted to stdout.
//! In human mode, aligned key/value lines are printed.
//!
//! Exit codes: 0 = success, 2 = parse failure.
use std::collections::BTreeMap;

use omtsf_core::OmtsFile;
use omtsf_core::enums::{EdgeType, EdgeTypeTag, NodeType, NodeTypeTag};
use serde_json;

use crate::OutputFormat;
use crate::error::CliError;

/// Statistics gathered from a parsed [`OmtsFile`].
pub struct InspectStats {
    /// Total number of nodes.
    pub node_count: usize,
    /// Node count grouped by type string (e.g. "organization", "facility").
    pub node_counts: BTreeMap<String, usize>,
    /// Total number of edges.
    pub edge_count: usize,
    /// Edge count grouped by type string (e.g. "supplies", "owns").
    pub edge_counts: BTreeMap<String, usize>,
    /// Identifier count grouped by scheme (across all nodes and edges).
    pub identifier_counts: BTreeMap<String, usize>,
    /// Total identifier count.
    pub identifier_count: usize,
    /// OMTSF specification version string.
    pub version: String,
    /// Snapshot date string.
    pub snapshot_date: String,
    /// Disclosure scope string, or `None` if not set.
    pub disclosure_scope: Option<String>,
    /// Reporting entity node ID, or `None` if not set.
    pub reporting_entity: Option<String>,
}

impl InspectStats {
    /// Computes statistics from a parsed [`OmtsFile`].
    pub fn from_file(file: &OmtsFile) -> Self {
        let mut node_counts: BTreeMap<String, usize> = BTreeMap::new();
        let mut edge_counts: BTreeMap<String, usize> = BTreeMap::new();
        let mut identifier_counts: BTreeMap<String, usize> = BTreeMap::new();

        for node in &file.nodes {
            let type_str = node_type_tag_to_str(&node.node_type);
            *node_counts.entry(type_str).or_insert(0) += 1;

            if let Some(ids) = &node.identifiers {
                for id in ids {
                    *identifier_counts.entry(id.scheme.clone()).or_insert(0) += 1;
                }
            }
        }

        for edge in &file.edges {
            let type_str = edge_type_tag_to_str(&edge.edge_type);
            *edge_counts.entry(type_str).or_insert(0) += 1;

            if let Some(ids) = &edge.identifiers {
                for id in ids {
                    *identifier_counts.entry(id.scheme.clone()).or_insert(0) += 1;
                }
            }
        }

        let node_count = file.nodes.len();
        let edge_count = file.edges.len();
        let identifier_count = identifier_counts.values().sum();

        let disclosure_scope = file.disclosure_scope.as_ref().map(|s| {
            serde_json::to_value(s)
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_else(|| format!("{s:?}").to_lowercase())
        });

        let reporting_entity = file
            .reporting_entity
            .as_ref()
            .map(std::string::ToString::to_string);

        Self {
            node_count,
            node_counts,
            edge_count,
            edge_counts,
            identifier_counts,
            identifier_count,
            version: file.omtsf_version.to_string(),
            snapshot_date: file.snapshot_date.to_string(),
            disclosure_scope,
            reporting_entity,
        }
    }
}

/// Runs the `inspect` command.
///
/// Parses `content` as an OMTSF file, computes statistics, and writes them
/// to `stdout` in the requested format. Returns the exit code (0 or 2).
///
/// # Errors
///
/// Returns [`CliError`] with exit code 2 if the content cannot be parsed.
pub fn run(content: &str, format: &OutputFormat) -> Result<(), CliError> {
    let file: OmtsFile = serde_json::from_str(content).map_err(|e| CliError::ParseFailed {
        detail: format!("line {}, column {}: {e}", e.line(), e.column()),
    })?;

    let stats = InspectStats::from_file(&file);

    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    match format {
        OutputFormat::Human => print_human(&mut out, &stats),
        OutputFormat::Json => print_json(&mut out, &stats),
    }
    .map_err(|e| CliError::IoError {
        source: "stdout".to_owned(),
        detail: e.to_string(),
    })
}

/// Writes inspect statistics in human-readable aligned format.
fn print_human<W: std::io::Write>(w: &mut W, stats: &InspectStats) -> std::io::Result<()> {
    writeln!(w, "version:        {}", stats.version)?;
    writeln!(w, "snapshot_date:  {}", stats.snapshot_date)?;
    if let Some(scope) = &stats.disclosure_scope {
        writeln!(w, "disclosure:     {scope}")?;
    }
    if let Some(entity) = &stats.reporting_entity {
        writeln!(w, "reporting:      {entity}")?;
    }
    writeln!(w, "nodes:          {}", stats.node_count)?;
    for (type_str, count) in &stats.node_counts {
        writeln!(w, "  {type_str}: {count}")?;
    }
    writeln!(w, "edges:          {}", stats.edge_count)?;
    for (type_str, count) in &stats.edge_counts {
        writeln!(w, "  {type_str}: {count}")?;
    }
    writeln!(w, "identifiers:    {}", stats.identifier_count)?;
    for (scheme, count) in &stats.identifier_counts {
        writeln!(w, "  {scheme}: {count}")?;
    }
    Ok(())
}

/// Writes inspect statistics as a single JSON object to stdout.
fn print_json<W: std::io::Write>(w: &mut W, stats: &InspectStats) -> std::io::Result<()> {
    let mut obj = serde_json::Map::new();

    obj.insert(
        "version".to_owned(),
        serde_json::Value::String(stats.version.clone()),
    );
    obj.insert(
        "snapshot_date".to_owned(),
        serde_json::Value::String(stats.snapshot_date.clone()),
    );

    if let Some(scope) = &stats.disclosure_scope {
        obj.insert(
            "disclosure_scope".to_owned(),
            serde_json::Value::String(scope.clone()),
        );
    }
    if let Some(entity) = &stats.reporting_entity {
        obj.insert(
            "reporting_entity".to_owned(),
            serde_json::Value::String(entity.clone()),
        );
    }

    obj.insert(
        "node_count".to_owned(),
        serde_json::Value::Number(stats.node_count.into()),
    );

    let node_counts_obj: serde_json::Map<String, serde_json::Value> = stats
        .node_counts
        .iter()
        .map(|(k, v)| (k.clone(), serde_json::Value::Number((*v).into())))
        .collect();
    obj.insert(
        "node_counts".to_owned(),
        serde_json::Value::Object(node_counts_obj),
    );

    obj.insert(
        "edge_count".to_owned(),
        serde_json::Value::Number(stats.edge_count.into()),
    );

    let edge_counts_obj: serde_json::Map<String, serde_json::Value> = stats
        .edge_counts
        .iter()
        .map(|(k, v)| (k.clone(), serde_json::Value::Number((*v).into())))
        .collect();
    obj.insert(
        "edge_counts".to_owned(),
        serde_json::Value::Object(edge_counts_obj),
    );

    obj.insert(
        "identifier_count".to_owned(),
        serde_json::Value::Number(stats.identifier_count.into()),
    );

    let id_counts_obj: serde_json::Map<String, serde_json::Value> = stats
        .identifier_counts
        .iter()
        .map(|(k, v)| (k.clone(), serde_json::Value::Number((*v).into())))
        .collect();
    obj.insert(
        "identifier_counts".to_owned(),
        serde_json::Value::Object(id_counts_obj),
    );

    let json = serde_json::to_string_pretty(&serde_json::Value::Object(obj))
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    writeln!(w, "{json}")
}

/// Returns the JSON string representation of a [`NodeTypeTag`].
fn node_type_tag_to_str(tag: &NodeTypeTag) -> String {
    match tag {
        NodeTypeTag::Known(nt) => match nt {
            NodeType::Organization => "organization".to_owned(),
            NodeType::Facility => "facility".to_owned(),
            NodeType::Good => "good".to_owned(),
            NodeType::Person => "person".to_owned(),
            NodeType::Attestation => "attestation".to_owned(),
            NodeType::Consignment => "consignment".to_owned(),
            NodeType::BoundaryRef => "boundary_ref".to_owned(),
        },
        NodeTypeTag::Extension(s) => s.clone(),
    }
}

/// Returns the JSON string representation of an [`EdgeTypeTag`].
fn edge_type_tag_to_str(tag: &EdgeTypeTag) -> String {
    match tag {
        EdgeTypeTag::Known(et) => match et {
            EdgeType::Ownership => "ownership".to_owned(),
            EdgeType::OperationalControl => "operational_control".to_owned(),
            EdgeType::LegalParentage => "legal_parentage".to_owned(),
            EdgeType::FormerIdentity => "former_identity".to_owned(),
            EdgeType::BeneficialOwnership => "beneficial_ownership".to_owned(),
            EdgeType::Supplies => "supplies".to_owned(),
            EdgeType::Subcontracts => "subcontracts".to_owned(),
            EdgeType::Tolls => "tolls".to_owned(),
            EdgeType::Distributes => "distributes".to_owned(),
            EdgeType::Brokers => "brokers".to_owned(),
            EdgeType::Operates => "operates".to_owned(),
            EdgeType::Produces => "produces".to_owned(),
            EdgeType::ComposedOf => "composed_of".to_owned(),
            EdgeType::SellsTo => "sells_to".to_owned(),
            EdgeType::AttestedBy => "attested_by".to_owned(),
            EdgeType::SameAs => "same_as".to_owned(),
        },
        EdgeTypeTag::Extension(s) => s.clone(),
    }
}
