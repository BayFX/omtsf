//! Implementation of `omtsf diff <a> <b>`.
//!
//! Parses two `.omts` files, runs the structural diff engine, and writes the
//! result to stdout.
//!
//! Flags:
//! - `--ids-only`: Only show IDs of changed elements, no property detail.
//! - `--summary-only`: Only show the summary statistics line.
//! - `--node-type <TYPE>` (repeatable): Restrict diff to nodes of this type.
//! - `--edge-type <TYPE>` (repeatable): Restrict diff to edges of this type.
//! - `--ignore-field <FIELD>` (repeatable): Exclude this property from comparison.
//!
//! Exit codes:
//! - 0 = files are identical
//! - 1 = differences found
//! - 2 = parse failure on either file
use std::collections::HashSet;

use omtsf_core::{
    DiffFilter, DiffResult, DiffSummary, EdgeDiff, EdgeRef, EdgesDiff, IdentifierFieldDiff,
    IdentifierSetDiff, LabelSetDiff, NodeDiff, NodeRef, NodesDiff, OmtsFile, PropertyChange,
    diff_filtered,
};

use crate::OutputFormat;
use crate::error::CliError;

// ---------------------------------------------------------------------------
// run
// ---------------------------------------------------------------------------

/// Runs the `diff` command.
///
/// Parses `content_a` and `content_b` as OMTSF files, constructs the
/// [`DiffFilter`] from CLI flags, calls [`diff_filtered`], and writes the
/// result to stdout in the requested format.
///
/// Returns `Ok(())` when the files are identical (exit 0).
/// Returns [`CliError::DiffHasDifferences`] (exit 1) when differences are found.
/// Returns [`CliError::ParseFailed`] (exit 2) when either file cannot be parsed.
///
/// # Errors
///
/// - [`CliError::ParseFailed`] — either file is not a valid OMTSF file.
/// - [`CliError::DiffHasDifferences`] — the diff is non-empty.
/// - [`CliError::IoError`] — stdout write failed.
#[allow(clippy::too_many_arguments)]
pub fn run(
    content_a: &str,
    content_b: &str,
    ids_only: bool,
    summary_only: bool,
    node_types: &[String],
    edge_types: &[String],
    ignore_fields: &[String],
    format: &OutputFormat,
) -> Result<(), CliError> {
    // --- Parse both files ---
    let file_a: OmtsFile = serde_json::from_str(content_a).map_err(|e| CliError::ParseFailed {
        detail: format!("file A: {e}"),
    })?;
    let file_b: OmtsFile = serde_json::from_str(content_b).map_err(|e| CliError::ParseFailed {
        detail: format!("file B: {e}"),
    })?;

    // --- Build DiffFilter ---
    let filter = DiffFilter {
        node_types: if node_types.is_empty() {
            None
        } else {
            Some(node_types.iter().cloned().collect::<HashSet<_>>())
        },
        edge_types: if edge_types.is_empty() {
            None
        } else {
            Some(edge_types.iter().cloned().collect::<HashSet<_>>())
        },
        ignore_fields: ignore_fields.iter().cloned().collect::<HashSet<_>>(),
    };

    // --- Compute diff ---
    let result = diff_filtered(&file_a, &file_b, Some(&filter));

    // --- Write output ---
    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    match format {
        OutputFormat::Human => write_human(&mut out, &result, ids_only, summary_only),
        OutputFormat::Json => write_json(&mut out, &result),
    }
    .map_err(|e| CliError::IoError {
        source: "stdout".to_owned(),
        detail: e.to_string(),
    })?;

    // --- Exit code ---
    if result.is_empty() {
        Ok(())
    } else {
        Err(CliError::DiffHasDifferences)
    }
}

// ---------------------------------------------------------------------------
// Human output
// ---------------------------------------------------------------------------

/// Writes the diff result in unified-diff-inspired human-readable format.
fn write_human<W: std::io::Write>(
    w: &mut W,
    result: &DiffResult,
    ids_only: bool,
    summary_only: bool,
) -> std::io::Result<()> {
    let summary = result.summary();

    if summary_only {
        return write_summary_line(w, &summary);
    }

    write_nodes_human(w, &result.nodes, ids_only)?;
    write_edges_human(w, &result.edges, ids_only)?;

    // Warnings from the diff engine (e.g. ambiguous match groups)
    for warning in &result.warnings {
        writeln!(w, "! {warning}")?;
    }

    write_summary_line(w, &summary)
}

/// Writes node differences in human-readable format.
fn write_nodes_human<W: std::io::Write>(
    w: &mut W,
    nodes: &NodesDiff,
    ids_only: bool,
) -> std::io::Result<()> {
    if nodes.added.is_empty() && nodes.removed.is_empty() && nodes.modified.is_empty() {
        return Ok(());
    }

    writeln!(w, "Nodes:")?;

    for node in &nodes.added {
        write_node_ref_human(w, node, "+")?;
    }
    for node in &nodes.removed {
        write_node_ref_human(w, node, "-")?;
    }
    for node_diff in &nodes.modified {
        write_node_diff_human(w, node_diff, ids_only)?;
    }

    writeln!(w)
}

/// Writes a single [`NodeRef`] line with the given prefix (`+` or `-`).
fn write_node_ref_human<W: std::io::Write>(
    w: &mut W,
    node: &NodeRef,
    prefix: &str,
) -> std::io::Result<()> {
    if let Some(name) = &node.name {
        writeln!(
            w,
            "  {prefix} {} ({}) \"{}\"",
            node.id, node.node_type, name
        )
    } else {
        writeln!(w, "  {prefix} {} ({})", node.id, node.node_type)
    }
}

/// Writes a modified node's diff block.
fn write_node_diff_human<W: std::io::Write>(
    w: &mut W,
    node_diff: &NodeDiff,
    ids_only: bool,
) -> std::io::Result<()> {
    // Header: show both IDs when they differ
    if node_diff.id_a == node_diff.id_b {
        writeln!(w, "  ~ {} ({})", node_diff.id_a, node_diff.node_type)?;
    } else {
        writeln!(
            w,
            "  ~ {}/{} ({})",
            node_diff.id_a, node_diff.id_b, node_diff.node_type
        )?;
    }

    if ids_only {
        return Ok(());
    }

    if !node_diff.matched_by.is_empty() {
        writeln!(w, "    matched by: {}", node_diff.matched_by.join(", "))?;
    }

    write_property_changes_human(w, &node_diff.property_changes)?;
    write_identifier_set_diff_human(w, &node_diff.identifier_changes)?;
    write_label_set_diff_human(w, &node_diff.label_changes)?;

    Ok(())
}

/// Writes edge differences in human-readable format.
fn write_edges_human<W: std::io::Write>(
    w: &mut W,
    edges: &EdgesDiff,
    ids_only: bool,
) -> std::io::Result<()> {
    if edges.added.is_empty() && edges.removed.is_empty() && edges.modified.is_empty() {
        return Ok(());
    }

    writeln!(w, "Edges:")?;

    for edge in &edges.added {
        write_edge_ref_human(w, edge, "+")?;
    }
    for edge in &edges.removed {
        write_edge_ref_human(w, edge, "-")?;
    }
    for edge_diff in &edges.modified {
        write_edge_diff_human(w, edge_diff, ids_only)?;
    }

    writeln!(w)
}

/// Writes a single [`EdgeRef`] line with the given prefix.
fn write_edge_ref_human<W: std::io::Write>(
    w: &mut W,
    edge: &EdgeRef,
    prefix: &str,
) -> std::io::Result<()> {
    writeln!(
        w,
        "  {prefix} {} ({}) {} -> {}",
        edge.id, edge.edge_type, edge.source, edge.target
    )
}

/// Writes a modified edge's diff block.
fn write_edge_diff_human<W: std::io::Write>(
    w: &mut W,
    edge_diff: &EdgeDiff,
    ids_only: bool,
) -> std::io::Result<()> {
    if edge_diff.id_a == edge_diff.id_b {
        writeln!(w, "  ~ {} ({})", edge_diff.id_a, edge_diff.edge_type)?;
    } else {
        writeln!(
            w,
            "  ~ {}/{} ({})",
            edge_diff.id_a, edge_diff.id_b, edge_diff.edge_type
        )?;
    }

    if ids_only {
        return Ok(());
    }

    write_property_changes_human(w, &edge_diff.property_changes)?;
    write_identifier_set_diff_human(w, &edge_diff.identifier_changes)?;
    write_label_set_diff_human(w, &edge_diff.label_changes)?;

    Ok(())
}

/// Writes scalar property changes indented under a modified element.
fn write_property_changes_human<W: std::io::Write>(
    w: &mut W,
    changes: &[PropertyChange],
) -> std::io::Result<()> {
    for change in changes {
        match (&change.old_value, &change.new_value) {
            (None, Some(new)) => writeln!(w, "    + {}: {new}", change.field)?,
            (Some(old), None) => writeln!(w, "    - {}: {old}", change.field)?,
            (Some(old), Some(new)) => {
                writeln!(w, "    ~ {}: {old} -> {new}", change.field)?;
            }
            (None, None) => {}
        }
    }
    Ok(())
}

/// Writes identifier set differences.
fn write_identifier_set_diff_human<W: std::io::Write>(
    w: &mut W,
    diff: &IdentifierSetDiff,
) -> std::io::Result<()> {
    for id in &diff.added {
        writeln!(w, "    + identifier: {}:{}", id.scheme, id.value)?;
    }
    for id in &diff.removed {
        writeln!(w, "    - identifier: {}:{}", id.scheme, id.value)?;
    }
    for id_diff in &diff.modified {
        writeln!(w, "    ~ identifier: {}", id_diff.canonical_key)?;
        write_property_changes_human(w, &id_diff.field_changes)?;
    }
    Ok(())
}

/// Writes label set differences.
fn write_label_set_diff_human<W: std::io::Write>(
    w: &mut W,
    diff: &LabelSetDiff,
) -> std::io::Result<()> {
    for label in &diff.added {
        if let Some(val) = &label.value {
            writeln!(w, "    + label: {{{}:{val}}}", label.key)?;
        } else {
            writeln!(w, "    + label: {{{}}}", label.key)?;
        }
    }
    for label in &diff.removed {
        if let Some(val) = &label.value {
            writeln!(w, "    - label: {{{}:{val}}}", label.key)?;
        } else {
            writeln!(w, "    - label: {{{}}}", label.key)?;
        }
    }
    Ok(())
}

/// Writes the summary line.
fn write_summary_line<W: std::io::Write>(w: &mut W, summary: &DiffSummary) -> std::io::Result<()> {
    writeln!(
        w,
        "Summary: {} added, {} removed, {} modified, {} unchanged nodes; \
         {} added, {} removed, {} modified, {} unchanged edges",
        summary.nodes_added,
        summary.nodes_removed,
        summary.nodes_modified,
        summary.nodes_unchanged,
        summary.edges_added,
        summary.edges_removed,
        summary.edges_modified,
        summary.edges_unchanged,
    )
}

// ---------------------------------------------------------------------------
// JSON output
// ---------------------------------------------------------------------------

/// Writes the diff result as a single JSON object to stdout.
///
/// The structure mirrors the diff spec Section 5.2.
fn write_json<W: std::io::Write>(w: &mut W, result: &DiffResult) -> std::io::Result<()> {
    let summary = result.summary();

    let summary_obj = serde_json::json!({
        "nodes_added":     summary.nodes_added,
        "nodes_removed":   summary.nodes_removed,
        "nodes_modified":  summary.nodes_modified,
        "nodes_unchanged": summary.nodes_unchanged,
        "edges_added":     summary.edges_added,
        "edges_removed":   summary.edges_removed,
        "edges_modified":  summary.edges_modified,
        "edges_unchanged": summary.edges_unchanged,
    });

    let nodes_obj = serde_json::json!({
        "added":    node_refs_to_json(&result.nodes.added),
        "removed":  node_refs_to_json(&result.nodes.removed),
        "modified": node_diffs_to_json(&result.nodes.modified),
    });

    let edges_obj = serde_json::json!({
        "added":    edge_refs_to_json(&result.edges.added),
        "removed":  edge_refs_to_json(&result.edges.removed),
        "modified": edge_diffs_to_json(&result.edges.modified),
    });

    let output = serde_json::json!({
        "summary": summary_obj,
        "nodes": nodes_obj,
        "edges": edges_obj,
        "warnings": result.warnings,
    });

    let json = serde_json::to_string_pretty(&output)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    writeln!(w, "{json}")
}

/// Converts a slice of [`NodeRef`] values to a JSON array value.
fn node_refs_to_json(refs: &[NodeRef]) -> serde_json::Value {
    serde_json::Value::Array(
        refs.iter()
            .map(|n| {
                let mut obj = serde_json::Map::new();
                obj.insert("id".to_owned(), serde_json::Value::String(n.id.to_string()));
                obj.insert(
                    "node_type".to_owned(),
                    serde_json::Value::String(n.node_type.clone()),
                );
                if let Some(name) = &n.name {
                    obj.insert("name".to_owned(), serde_json::Value::String(name.clone()));
                }
                serde_json::Value::Object(obj)
            })
            .collect(),
    )
}

/// Converts a slice of [`EdgeRef`] values to a JSON array value.
fn edge_refs_to_json(refs: &[EdgeRef]) -> serde_json::Value {
    serde_json::Value::Array(
        refs.iter()
            .map(|e| {
                serde_json::json!({
                    "id":        e.id.to_string(),
                    "edge_type": e.edge_type,
                    "source":    e.source.to_string(),
                    "target":    e.target.to_string(),
                })
            })
            .collect(),
    )
}

/// Converts a slice of [`NodeDiff`] values to a JSON array value.
fn node_diffs_to_json(diffs: &[NodeDiff]) -> serde_json::Value {
    serde_json::Value::Array(
        diffs
            .iter()
            .map(|d| {
                serde_json::json!({
                    "id_a":               d.id_a,
                    "id_b":               d.id_b,
                    "node_type":          d.node_type,
                    "matched_by":         d.matched_by,
                    "property_changes":   property_changes_to_json(&d.property_changes),
                    "identifier_changes": identifier_set_diff_to_json(&d.identifier_changes),
                    "label_changes":      label_set_diff_to_json(&d.label_changes),
                })
            })
            .collect(),
    )
}

/// Converts a slice of [`EdgeDiff`] values to a JSON array value.
fn edge_diffs_to_json(diffs: &[EdgeDiff]) -> serde_json::Value {
    serde_json::Value::Array(
        diffs
            .iter()
            .map(|d| {
                serde_json::json!({
                    "id_a":               d.id_a,
                    "id_b":               d.id_b,
                    "edge_type":          d.edge_type,
                    "property_changes":   property_changes_to_json(&d.property_changes),
                    "identifier_changes": identifier_set_diff_to_json(&d.identifier_changes),
                    "label_changes":      label_set_diff_to_json(&d.label_changes),
                })
            })
            .collect(),
    )
}

/// Converts a slice of [`PropertyChange`] values to a JSON array.
fn property_changes_to_json(changes: &[PropertyChange]) -> serde_json::Value {
    serde_json::Value::Array(
        changes
            .iter()
            .map(|c| {
                serde_json::json!({
                    "field":     c.field,
                    "old_value": c.old_value,
                    "new_value": c.new_value,
                })
            })
            .collect(),
    )
}

/// Converts an [`IdentifierSetDiff`] to a JSON object.
fn identifier_set_diff_to_json(diff: &IdentifierSetDiff) -> serde_json::Value {
    let modified: Vec<serde_json::Value> = diff
        .modified
        .iter()
        .map(identifier_field_diff_to_json)
        .collect();

    let added: Vec<serde_json::Value> = diff
        .added
        .iter()
        .map(|id| {
            serde_json::json!({
                "scheme": id.scheme,
                "value":  id.value,
            })
        })
        .collect();

    let removed: Vec<serde_json::Value> = diff
        .removed
        .iter()
        .map(|id| {
            serde_json::json!({
                "scheme": id.scheme,
                "value":  id.value,
            })
        })
        .collect();

    serde_json::json!({
        "added":    added,
        "removed":  removed,
        "modified": modified,
    })
}

/// Converts an [`IdentifierFieldDiff`] to a JSON object.
fn identifier_field_diff_to_json(diff: &IdentifierFieldDiff) -> serde_json::Value {
    serde_json::json!({
        "canonical_key":  diff.canonical_key.to_string(),
        "field_changes":  property_changes_to_json(&diff.field_changes),
    })
}

/// Converts a [`LabelSetDiff`] to a JSON object.
fn label_set_diff_to_json(diff: &LabelSetDiff) -> serde_json::Value {
    let added: Vec<serde_json::Value> = diff
        .added
        .iter()
        .map(|l| serde_json::json!({"key": l.key, "value": l.value}))
        .collect();
    let removed: Vec<serde_json::Value> = diff
        .removed
        .iter()
        .map(|l| serde_json::json!({"key": l.key, "value": l.value}))
        .collect();
    serde_json::json!({
        "added":   added,
        "removed": removed,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    #![allow(clippy::panic)]

    use super::*;

    // Minimal valid OMTS file — no nodes, no edges.
    const EMPTY: &str = r#"{
        "omtsf_version": "1.0.0",
        "snapshot_date": "2026-02-19",
        "file_salt": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "nodes": [],
        "edges": []
    }"#;

    // File with one organization node (no external identifiers, so it cannot
    // be matched across files; it will appear as added or removed depending on
    // which side it is on).
    const WITH_ORG: &str = r#"{
        "omtsf_version": "1.0.0",
        "snapshot_date": "2026-02-19",
        "file_salt": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        "nodes": [
            {"id": "org-001", "type": "organization", "name": "Acme Corp"}
        ],
        "edges": []
    }"#;

    // File identical to WITH_ORG but with a different name.
    // To be matched the node needs an external identifier.
    const WITH_ORG_MODIFIED: &str = r#"{
        "omtsf_version": "1.0.0",
        "snapshot_date": "2026-02-19",
        "file_salt": "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
        "nodes": [
            {
                "id": "org-001",
                "type": "organization",
                "name": "Acme Corporation",
                "identifiers": [{"scheme": "duns", "value": "123456789"}]
            }
        ],
        "edges": []
    }"#;

    const WITH_ORG_SAME_EXT_ID: &str = r#"{
        "omtsf_version": "1.0.0",
        "snapshot_date": "2026-02-19",
        "file_salt": "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
        "nodes": [
            {
                "id": "org-001",
                "type": "organization",
                "name": "Acme Corp",
                "identifiers": [{"scheme": "duns", "value": "123456789"}]
            }
        ],
        "edges": []
    }"#;

    const NOT_JSON: &str = "this is not json";

    // ── run: parse failures ───────────────────────────────────────────────────

    #[test]
    fn run_bad_file_a_returns_parse_failed() {
        let result = run(
            NOT_JSON,
            EMPTY,
            false,
            false,
            &[],
            &[],
            &[],
            &OutputFormat::Human,
        );
        match result {
            Err(CliError::ParseFailed { detail }) => {
                assert!(detail.contains("file A"), "detail: {detail}");
            }
            other => panic!("expected ParseFailed, got {other:?}"),
        }
    }

    #[test]
    fn run_bad_file_b_returns_parse_failed() {
        let result = run(
            EMPTY,
            NOT_JSON,
            false,
            false,
            &[],
            &[],
            &[],
            &OutputFormat::Human,
        );
        match result {
            Err(CliError::ParseFailed { detail }) => {
                assert!(detail.contains("file B"), "detail: {detail}");
            }
            other => panic!("expected ParseFailed, got {other:?}"),
        }
    }

    #[test]
    fn run_parse_failure_exit_code_is_2() {
        let err = run(
            NOT_JSON,
            EMPTY,
            false,
            false,
            &[],
            &[],
            &[],
            &OutputFormat::Human,
        )
        .expect_err("should fail");
        assert_eq!(err.exit_code(), 2);
    }

    // ── run: identical files (exit 0) ─────────────────────────────────────────

    #[test]
    fn run_identical_empty_files_returns_ok() {
        let result = run(
            EMPTY,
            EMPTY,
            false,
            false,
            &[],
            &[],
            &[],
            &OutputFormat::Human,
        );
        assert!(
            result.is_ok(),
            "expected Ok for identical files: {result:?}"
        );
    }

    #[test]
    fn run_identical_files_exit_code_is_0() {
        // Use the matched-node fixture: both files have org-001 with the same
        // DUNS identifier so the diff engine can match them across files and
        // finds no changes.
        let result = run(
            WITH_ORG_SAME_EXT_ID,
            WITH_ORG_SAME_EXT_ID,
            false,
            false,
            &[],
            &[],
            &[],
            &OutputFormat::Human,
        );
        assert!(result.is_ok(), "identical files should exit 0: {result:?}");
    }

    // ── run: differing files (exit 1) ─────────────────────────────────────────

    #[test]
    fn run_different_files_returns_diff_has_differences() {
        let result = run(
            EMPTY,
            WITH_ORG,
            false,
            false,
            &[],
            &[],
            &[],
            &OutputFormat::Human,
        );
        match result {
            Err(CliError::DiffHasDifferences) => {}
            other => panic!("expected DiffHasDifferences, got {other:?}"),
        }
    }

    #[test]
    fn run_different_files_exit_code_is_1() {
        let err = run(
            EMPTY,
            WITH_ORG,
            false,
            false,
            &[],
            &[],
            &[],
            &OutputFormat::Human,
        )
        .expect_err("should fail with differences");
        assert_eq!(err.exit_code(), 1);
    }

    // ── run: --ids-only ───────────────────────────────────────────────────────

    #[test]
    fn run_ids_only_still_exits_1_for_differences() {
        let err = run(
            EMPTY,
            WITH_ORG,
            true,
            false,
            &[],
            &[],
            &[],
            &OutputFormat::Human,
        )
        .expect_err("should still exit 1");
        assert_eq!(err.exit_code(), 1);
    }

    // ── run: --summary-only ───────────────────────────────────────────────────

    #[test]
    fn run_summary_only_exits_1_for_differences() {
        let err = run(
            EMPTY,
            WITH_ORG,
            false,
            true,
            &[],
            &[],
            &[],
            &OutputFormat::Human,
        )
        .expect_err("should exit 1");
        assert_eq!(err.exit_code(), 1);
    }

    // ── run: JSON format ──────────────────────────────────────────────────────

    #[test]
    fn run_json_identical_files_returns_ok() {
        let result = run(
            EMPTY,
            EMPTY,
            false,
            false,
            &[],
            &[],
            &[],
            &OutputFormat::Json,
        );
        assert!(result.is_ok(), "expected Ok: {result:?}");
    }

    #[test]
    fn run_json_different_files_returns_diff_has_differences() {
        let result = run(
            EMPTY,
            WITH_ORG,
            false,
            false,
            &[],
            &[],
            &[],
            &OutputFormat::Json,
        );
        assert!(
            matches!(result, Err(CliError::DiffHasDifferences)),
            "expected DiffHasDifferences: {result:?}"
        );
    }

    // ── write_human: output content checks ───────────────────────────────────

    fn capture_human(
        content_a: &str,
        content_b: &str,
        ids_only: bool,
        summary_only: bool,
    ) -> String {
        let file_a: OmtsFile = serde_json::from_str(content_a).expect("parse A");
        let file_b: OmtsFile = serde_json::from_str(content_b).expect("parse B");
        let filter = DiffFilter::default();
        let result = diff_filtered(&file_a, &file_b, Some(&filter));
        let mut buf: Vec<u8> = Vec::new();
        write_human(&mut buf, &result, ids_only, summary_only).expect("write");
        String::from_utf8(buf).expect("utf8")
    }

    #[test]
    fn human_output_contains_summary_line() {
        let s = capture_human(EMPTY, WITH_ORG, false, false);
        assert!(s.contains("Summary:"), "output: {s}");
    }

    #[test]
    fn human_output_summary_only_contains_summary() {
        let s = capture_human(EMPTY, WITH_ORG, false, true);
        assert!(s.contains("Summary:"), "output: {s}");
    }

    #[test]
    fn human_output_added_node_has_plus_prefix() {
        let s = capture_human(EMPTY, WITH_ORG, false, false);
        assert!(s.contains("  + org-001"), "output: {s}");
    }

    #[test]
    fn human_output_removed_node_has_minus_prefix() {
        let s = capture_human(WITH_ORG, EMPTY, false, false);
        assert!(s.contains("  - org-001"), "output: {s}");
    }

    #[test]
    fn human_output_modified_node_has_tilde_prefix() {
        // Both files have org-001 with the same DUNS identifier so the node
        // is matched; the name differs so it appears as modified.
        let s = capture_human(WITH_ORG_SAME_EXT_ID, WITH_ORG_MODIFIED, false, false);
        assert!(s.contains("  ~ org-001"), "output: {s}");
    }

    #[test]
    fn human_output_modified_node_shows_property_change() {
        let s = capture_human(WITH_ORG_SAME_EXT_ID, WITH_ORG_MODIFIED, false, false);
        // Should contain "name" change information
        assert!(s.contains("name"), "output should mention 'name': {s}");
    }

    #[test]
    fn human_output_ids_only_omits_property_details() {
        let s = capture_human(WITH_ORG_SAME_EXT_ID, WITH_ORG_MODIFIED, true, false);
        // Should still have the ~ header but not the property detail
        assert!(s.contains("  ~ org-001"), "output: {s}");
        // Property changes indented under the node should not appear
        assert!(
            !s.contains("    ~"),
            "ids_only should omit property detail: {s}"
        );
    }

    // ── write_json: output content checks ────────────────────────────────────

    fn capture_json(content_a: &str, content_b: &str) -> serde_json::Value {
        let file_a: OmtsFile = serde_json::from_str(content_a).expect("parse A");
        let file_b: OmtsFile = serde_json::from_str(content_b).expect("parse B");
        let filter = DiffFilter::default();
        let result = diff_filtered(&file_a, &file_b, Some(&filter));
        let mut buf: Vec<u8> = Vec::new();
        write_json(&mut buf, &result).expect("write");
        serde_json::from_str(&String::from_utf8(buf).expect("utf8")).expect("json")
    }

    #[test]
    fn json_output_has_summary_field() {
        let v = capture_json(EMPTY, WITH_ORG);
        assert!(v.get("summary").is_some(), "missing 'summary': {v}");
    }

    #[test]
    fn json_output_has_nodes_field() {
        let v = capture_json(EMPTY, WITH_ORG);
        assert!(v.get("nodes").is_some(), "missing 'nodes': {v}");
    }

    #[test]
    fn json_output_has_edges_field() {
        let v = capture_json(EMPTY, WITH_ORG);
        assert!(v.get("edges").is_some(), "missing 'edges': {v}");
    }

    #[test]
    fn json_output_summary_counts_added_node() {
        let v = capture_json(EMPTY, WITH_ORG);
        let added = v["summary"]["nodes_added"].as_u64().expect("nodes_added");
        assert_eq!(added, 1, "expected 1 added node");
    }

    #[test]
    fn json_output_nodes_added_contains_id() {
        let v = capture_json(EMPTY, WITH_ORG);
        let added = v["nodes"]["added"].as_array().expect("nodes.added array");
        let ids: Vec<&str> = added.iter().filter_map(|n| n["id"].as_str()).collect();
        assert!(ids.contains(&"org-001"), "expected org-001 in added: {v}");
    }
}
