//! Implementation of `omtsf query <file> [selector flags]`.
//!
//! Parses an `.omts` file, applies selector predicates from the CLI flags, and
//! prints matching nodes and edges.
//!
//! Flags:
//! - `--node-type`, `--edge-type`, `--label`, `--identifier`, `--jurisdiction`,
//!   `--name` (repeatable, see [`crate::cmd::selectors`] for parsing rules)
//! - `--count`: print only match counts, not individual results.
//!
//! Output (human mode, default): tab-separated table with columns
//! `KIND`, `ID`, `TYPE`, `NAME/ENDPOINT`.
//! Output (JSON mode): `{"nodes": [...], "edges": [...]}`.
//! Output (count mode): just match counts on stdout.
//!
//! Diagnostic match counts are always emitted to stderr.
//!
//! Exit codes: 0 = at least one match found, 1 = no matches, 2 = parse error.
use omtsf_core::OmtsFile;
use omtsf_core::graph::selector_match;

use crate::OutputFormat;
use crate::cmd::selectors::build_selector_set;
use crate::error::CliError;

/// Runs the `query` command.
///
/// Builds a `SelectorSet` from the supplied flag vectors and calls
/// `selector_match` against the pre-parsed `file` to find matching nodes
/// and edges.
///
/// # Output
///
/// - `--count`: one line per kind (`nodes: N`, `edges: M`) on stdout.
/// - Human mode: a tab-separated table on stdout; match counts on stderr.
/// - JSON mode: `{"nodes": [...], "edges": [...]}` on stdout; counts on stderr.
///
/// # Errors
///
/// - [`CliError`] exit code 2 if no selector flags were provided.
/// - [`CliError`] exit code 1 if no elements match the selectors.
#[allow(clippy::too_many_arguments)]
pub fn run(
    file: &OmtsFile,
    node_types: &[String],
    edge_types: &[String],
    labels: &[String],
    identifiers: &[String],
    jurisdictions: &[String],
    names: &[String],
    count: bool,
    format: &OutputFormat,
) -> Result<(), CliError> {
    let selector_set = build_selector_set(
        node_types,
        edge_types,
        labels,
        identifiers,
        jurisdictions,
        names,
    )?;

    let result = selector_match(file, &selector_set);

    let matched_nodes: Vec<&omtsf_core::Node> = result
        .node_indices
        .iter()
        .map(|&i| &file.nodes[i])
        .collect();

    let matched_edges: Vec<&omtsf_core::Edge> = result
        .edge_indices
        .iter()
        .map(|&i| &file.edges[i])
        .collect();

    let node_count = matched_nodes.len();
    let edge_count = matched_edges.len();

    if node_count == 0 && edge_count == 0 {
        return Err(CliError::NoResults {
            detail: "no nodes or edges matched the given selectors".to_owned(),
        });
    }

    eprintln!("matched: {node_count} node(s), {edge_count} edge(s)");

    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    if count {
        use std::io::Write as _;
        writeln!(out, "nodes: {node_count}").map_err(|e| CliError::IoError {
            source: "stdout".to_owned(),
            detail: e.to_string(),
        })?;
        writeln!(out, "edges: {edge_count}").map_err(|e| CliError::IoError {
            source: "stdout".to_owned(),
            detail: e.to_string(),
        })?;
        return Ok(());
    }

    match format {
        OutputFormat::Human => print_human(&mut out, &matched_nodes, &matched_edges),
        OutputFormat::Json => print_json(&mut out, &matched_nodes, &matched_edges),
    }
    .map_err(|e| CliError::IoError {
        source: "stdout".to_owned(),
        detail: e.to_string(),
    })
}

/// Writes matched nodes and edges as a tab-separated table.
///
/// Columns: `KIND`, `ID`, `TYPE`, `NAME/ENDPOINT`
fn print_human<W: std::io::Write>(
    w: &mut W,
    nodes: &[&omtsf_core::Node],
    edges: &[&omtsf_core::Edge],
) -> std::io::Result<()> {
    writeln!(w, "KIND\tID\tTYPE\tNAME/ENDPOINT")?;

    for node in nodes {
        let kind = "node";
        let id = node.id.to_string();
        let type_str = node_type_display(&node.node_type);
        let name = node.name.as_deref().unwrap_or("-");
        writeln!(w, "{kind}\t{id}\t{type_str}\t{name}")?;
    }

    for edge in edges {
        let kind = "edge";
        let id = edge.id.to_string();
        let type_str = edge_type_display(&edge.edge_type);
        let endpoint = format!("{}→{}", edge.source, edge.target);
        writeln!(w, "{kind}\t{id}\t{type_str}\t{endpoint}")?;
    }

    Ok(())
}

/// Writes matched nodes and edges as a JSON object.
///
/// Output: `{"nodes": [...], "edges": [...]}` where each element is the full
/// serialized JSON of the matching node or edge.
fn print_json<W: std::io::Write>(
    w: &mut W,
    nodes: &[&omtsf_core::Node],
    edges: &[&omtsf_core::Edge],
) -> std::io::Result<()> {
    let nodes_value: Vec<serde_json::Value> = nodes
        .iter()
        .map(serde_json::to_value)
        .collect::<Result<_, _>>()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

    let edges_value: Vec<serde_json::Value> = edges
        .iter()
        .map(serde_json::to_value)
        .collect::<Result<_, _>>()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

    let mut obj = serde_json::Map::new();
    obj.insert("nodes".to_owned(), serde_json::Value::Array(nodes_value));
    obj.insert("edges".to_owned(), serde_json::Value::Array(edges_value));

    let json = serde_json::to_string_pretty(&serde_json::Value::Object(obj))
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    writeln!(w, "{json}")
}

/// Returns a display string for a [`omtsf_core::NodeTypeTag`].
fn node_type_display(tag: &omtsf_core::NodeTypeTag) -> String {
    match tag {
        omtsf_core::NodeTypeTag::Known(t) => serde_json::to_value(t)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_else(|| format!("{t:?}")),
        omtsf_core::NodeTypeTag::Extension(s) => s.clone(),
    }
}

/// Returns a display string for a [`omtsf_core::EdgeTypeTag`].
fn edge_type_display(tag: &omtsf_core::EdgeTypeTag) -> String {
    match tag {
        omtsf_core::EdgeTypeTag::Known(t) => serde_json::to_value(t)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_else(|| format!("{t:?}")),
        omtsf_core::EdgeTypeTag::Extension(s) => s.clone(),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    #![allow(clippy::panic)]

    use super::*;
    use crate::OutputFormat;
    const SAMPLE_FILE: &str = r#"{
        "omtsf_version": "1.0.0",
        "snapshot_date": "2026-02-19",
        "file_salt": "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef",
        "nodes": [
            {"id": "org-1", "type": "organization", "name": "Acme Corp"},
            {"id": "org-2", "type": "facility"}
        ],
        "edges": [
            {"id": "e-1", "type": "supplies", "source": "org-1", "target": "org-2"}
        ]
    }"#;

    fn empty() -> Vec<String> {
        vec![]
    }

    fn strs(v: &[&str]) -> Vec<String> {
        v.iter().map(std::string::ToString::to_string).collect()
    }

    fn parse(s: &str) -> OmtsFile {
        serde_json::from_str(s).expect("valid OMTS JSON")
    }

    /// Matching by node type returns exit code 0.
    #[test]
    fn test_query_organization_succeeds() {
        let file = parse(SAMPLE_FILE);
        let result = run(
            &file,
            &strs(&["organization"]),
            &empty(),
            &empty(),
            &empty(),
            &empty(),
            &empty(),
            false,
            &OutputFormat::Human,
        );
        assert!(
            result.is_ok(),
            "should match organization nodes: {result:?}"
        );
    }

    /// Matching by edge type returns exit code 0.
    #[test]
    fn test_query_edge_type_supplies_succeeds() {
        let file = parse(SAMPLE_FILE);
        let result = run(
            &file,
            &empty(),
            &strs(&["supplies"]),
            &empty(),
            &empty(),
            &empty(),
            &empty(),
            false,
            &OutputFormat::Human,
        );
        assert!(result.is_ok(), "should match supplies edges: {result:?}");
    }

    /// No match returns `NoResults` error (exit code 1).
    #[test]
    fn test_query_no_match_returns_exit_1() {
        let file = parse(SAMPLE_FILE);
        let result = run(
            &file,
            &strs(&["good"]),
            &empty(),
            &empty(),
            &empty(),
            &empty(),
            &empty(),
            false,
            &OutputFormat::Human,
        );
        let err = result.expect_err("no good nodes → NoResults");
        assert_eq!(err.exit_code(), 1);
    }

    /// `--count` mode returns exit code 0 and does not error.
    #[test]
    fn test_query_count_mode() {
        let file = parse(SAMPLE_FILE);
        let result = run(
            &file,
            &strs(&["organization"]),
            &empty(),
            &empty(),
            &empty(),
            &empty(),
            &empty(),
            true,
            &OutputFormat::Human,
        );
        assert!(result.is_ok(), "count mode should succeed: {result:?}");
    }

    /// JSON mode returns exit code 0 for a valid query.
    #[test]
    fn test_query_json_mode() {
        let file = parse(SAMPLE_FILE);
        let result = run(
            &file,
            &strs(&["organization"]),
            &empty(),
            &empty(),
            &empty(),
            &empty(),
            &empty(),
            false,
            &OutputFormat::Json,
        );
        assert!(result.is_ok(), "JSON mode should succeed: {result:?}");
    }

    /// Empty selectors → `InvalidArgument` (exit code 2).
    #[test]
    fn test_query_no_selectors_returns_exit_2() {
        let file = parse(SAMPLE_FILE);
        let result = run(
            &file,
            &empty(),
            &empty(),
            &empty(),
            &empty(),
            &empty(),
            &empty(),
            false,
            &OutputFormat::Human,
        );
        let err = result.expect_err("no selectors → error");
        assert_eq!(err.exit_code(), 2);
    }

    /// Human output table includes the KIND, ID, TYPE, NAME/ENDPOINT header.
    #[test]
    fn test_human_output_contains_header() {
        let file: OmtsFile = serde_json::from_str(SAMPLE_FILE).expect("parse");
        let mut buf = Vec::new();
        let nodes: Vec<&omtsf_core::Node> = file.nodes.iter().collect();
        let edges: Vec<&omtsf_core::Edge> = vec![];
        print_human(&mut buf, &nodes, &edges).expect("write");
        let output = String::from_utf8(buf).expect("utf8");
        assert!(output.contains("KIND"), "should contain KIND header");
        assert!(output.contains("ID"), "should contain ID header");
        assert!(output.contains("TYPE"), "should contain TYPE header");
    }

    /// Human output includes node IDs and types.
    #[test]
    fn test_human_output_contains_node_data() {
        let file: OmtsFile = serde_json::from_str(SAMPLE_FILE).expect("parse");
        let mut buf = Vec::new();
        let nodes: Vec<&omtsf_core::Node> = file.nodes.iter().collect();
        let edges: Vec<&omtsf_core::Edge> = vec![];
        print_human(&mut buf, &nodes, &edges).expect("write");
        let output = String::from_utf8(buf).expect("utf8");
        assert!(output.contains("org-1"), "should contain node ID org-1");
        assert!(
            output.contains("organization"),
            "should contain type organization"
        );
        assert!(output.contains("Acme Corp"), "should contain name");
    }

    /// Human output includes edge source→target notation.
    #[test]
    fn test_human_output_contains_edge_data() {
        let file: OmtsFile = serde_json::from_str(SAMPLE_FILE).expect("parse");
        let mut buf = Vec::new();
        let nodes: Vec<&omtsf_core::Node> = vec![];
        let edges: Vec<&omtsf_core::Edge> = file.edges.iter().collect();
        print_human(&mut buf, &nodes, &edges).expect("write");
        let output = String::from_utf8(buf).expect("utf8");
        assert!(output.contains("e-1"), "should contain edge ID");
        assert!(output.contains("org-1"), "should contain source");
        assert!(output.contains("org-2"), "should contain target");
    }

    /// JSON output is a valid JSON object with `nodes` and `edges` arrays.
    #[test]
    fn test_json_output_structure() {
        let file: OmtsFile = serde_json::from_str(SAMPLE_FILE).expect("parse");
        let mut buf = Vec::new();
        let nodes: Vec<&omtsf_core::Node> = file.nodes.iter().collect();
        let edges: Vec<&omtsf_core::Edge> = file.edges.iter().collect();
        print_json(&mut buf, &nodes, &edges).expect("write");
        let output = String::from_utf8(buf).expect("utf8");
        let parsed: serde_json::Value = serde_json::from_str(&output).expect("valid JSON");
        assert!(parsed.get("nodes").is_some(), "should have nodes key");
        assert!(parsed.get("edges").is_some(), "should have edges key");
        let nodes_arr = parsed["nodes"].as_array().expect("nodes is array");
        assert_eq!(nodes_arr.len(), 2);
        let edges_arr = parsed["edges"].as_array().expect("edges is array");
        assert_eq!(edges_arr.len(), 1);
    }
}
