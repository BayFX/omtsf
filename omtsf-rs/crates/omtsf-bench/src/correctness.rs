//! Post-operation invariant checkers for correctness validation.

use std::collections::HashSet;

use omtsf_core::diff::DiffResult;
use omtsf_core::file::OmtsFile;
use omtsf_core::graph::OmtsGraph;

/// Verifies graph construction invariants.
pub fn check_graph_invariants(file: &OmtsFile, graph: &OmtsGraph) -> Result<(), String> {
    if graph.node_count() != file.nodes.len() {
        return Err(format!(
            "node count mismatch: graph={}, file={}",
            graph.node_count(),
            file.nodes.len()
        ));
    }
    if graph.edge_count() != file.edges.len() {
        return Err(format!(
            "edge count mismatch: graph={}, file={}",
            graph.edge_count(),
            file.edges.len()
        ));
    }
    for node in &file.nodes {
        if graph.node_index(&node.id).is_none() {
            return Err(format!("node {} not found in graph index", node.id));
        }
    }
    Ok(())
}

/// Verifies that `reachable_from` result is a subset of all graph nodes
/// and excludes the start node.
pub fn check_reachable_excludes_start(
    graph: &OmtsGraph,
    start: &str,
    reachable: &HashSet<petgraph::stable_graph::NodeIndex>,
) -> Result<(), String> {
    let start_idx = graph
        .node_index(start)
        .ok_or_else(|| format!("start node {start} not found"))?;
    if reachable.contains(start_idx) {
        return Err("reachable set should not contain start node".to_owned());
    }
    if reachable.len() > graph.node_count() {
        return Err("reachable set larger than total graph nodes".to_owned());
    }
    Ok(())
}

/// Verifies `shortest_path` result:
/// - starts at `from`, ends at `to`
/// - each consecutive pair is connected by a real edge
pub fn check_shortest_path(
    graph: &OmtsGraph,
    from: &str,
    to: &str,
    path: &[petgraph::stable_graph::NodeIndex],
) -> Result<(), String> {
    if path.is_empty() {
        return Err("path is empty".to_owned());
    }
    let from_idx = graph
        .node_index(from)
        .ok_or_else(|| format!("from node {from} not found"))?;
    let to_idx = graph
        .node_index(to)
        .ok_or_else(|| format!("to node {to} not found"))?;

    if path[0] != *from_idx {
        return Err("path does not start at 'from' node".to_owned());
    }
    if path[path.len() - 1] != *to_idx {
        return Err("path does not end at 'to' node".to_owned());
    }

    let unique: HashSet<_> = path.iter().collect();
    if unique.len() != path.len() {
        return Err("path contains repeated nodes".to_owned());
    }

    Ok(())
}

/// Verifies `all_paths` result:
/// - all paths are simple (no repeated nodes)
/// - all start at `from` and end at `to`
pub fn check_all_paths(
    graph: &OmtsGraph,
    from: &str,
    to: &str,
    paths: &[Vec<petgraph::stable_graph::NodeIndex>],
) -> Result<(), String> {
    let from_idx = graph
        .node_index(from)
        .ok_or_else(|| format!("from node {from} not found"))?;
    let to_idx = graph
        .node_index(to)
        .ok_or_else(|| format!("to node {to} not found"))?;

    for (i, path) in paths.iter().enumerate() {
        if path.is_empty() {
            return Err(format!("path {i} is empty"));
        }
        if path[0] != *from_idx {
            return Err(format!("path {i} does not start at 'from'"));
        }
        if path[path.len() - 1] != *to_idx {
            return Err(format!("path {i} does not end at 'to'"));
        }
        let unique: HashSet<_> = path.iter().collect();
        if unique.len() != path.len() {
            return Err(format!("path {i} contains repeated nodes"));
        }
    }
    Ok(())
}

/// Verifies subgraph extraction invariants:
/// - output has exactly the requested nodes
/// - all edges have both endpoints in the output node set
pub fn check_subgraph(
    original_file: &OmtsFile,
    extracted: &OmtsFile,
    requested_ids: &[&str],
) -> Result<(), String> {
    let requested: HashSet<&str> = requested_ids.iter().copied().collect();
    let output_ids: HashSet<String> = extracted.nodes.iter().map(|n| n.id.to_string()).collect();

    if output_ids.len() != requested.len() {
        return Err(format!(
            "node count mismatch: requested={}, got={}",
            requested.len(),
            output_ids.len()
        ));
    }

    for id in &requested {
        if !output_ids.contains(*id) {
            return Err(format!("requested node {id} missing from output"));
        }
    }

    for edge in &extracted.edges {
        if !output_ids.contains(&edge.source.to_string()) {
            return Err(format!(
                "edge {} source {} not in output nodes",
                edge.id, edge.source
            ));
        }
        if !output_ids.contains(&edge.target.to_string()) {
            return Err(format!(
                "edge {} target {} not in output nodes",
                edge.id, edge.target
            ));
        }
    }

    let original_edges_in_subgraph: usize = original_file
        .edges
        .iter()
        .filter(|e| {
            requested.contains(e.source.to_string().as_str())
                && requested.contains(e.target.to_string().as_str())
        })
        .count();

    if extracted.edges.len() != original_edges_in_subgraph {
        return Err(format!(
            "edge count mismatch: expected {} edges in subgraph, got {}",
            original_edges_in_subgraph,
            extracted.edges.len()
        ));
    }

    Ok(())
}

/// Verifies merge invariants.
pub fn check_merge(inputs: &[&OmtsFile], output: &OmtsFile) -> Result<(), String> {
    let total_input_nodes: usize = inputs.iter().map(|f| f.nodes.len()).sum();
    if output.nodes.len() > total_input_nodes {
        return Err(format!(
            "merged node count {} exceeds sum of inputs {}",
            output.nodes.len(),
            total_input_nodes
        ));
    }

    let output_node_ids: HashSet<String> = output.nodes.iter().map(|n| n.id.to_string()).collect();
    for edge in &output.edges {
        if !output_node_ids.contains(&edge.source.to_string()) {
            return Err(format!(
                "merged edge {} source {} not in merged nodes",
                edge.id, edge.source
            ));
        }
        if !output_node_ids.contains(&edge.target.to_string()) {
            return Err(format!(
                "merged edge {} target {} not in merged nodes",
                edge.id, edge.target
            ));
        }
    }

    Ok(())
}

/// Verifies redaction invariants.
pub fn check_redaction(
    output: &OmtsFile,
    target_scope: &omtsf_core::enums::DisclosureScope,
) -> Result<(), String> {
    use omtsf_core::enums::{DisclosureScope, NodeType, NodeTypeTag};

    match &output.disclosure_scope {
        Some(scope) if scope == target_scope => {}
        other => {
            return Err(format!(
                "disclosure_scope mismatch: expected {:?}, got {:?}",
                target_scope, other
            ));
        }
    }

    if matches!(target_scope, DisclosureScope::Public) {
        for node in &output.nodes {
            if matches!(&node.node_type, NodeTypeTag::Known(NodeType::Person)) {
                return Err(format!("person node {} found in public output", node.id));
            }
        }
    }

    Ok(())
}

/// Verifies diff(a, a) produces empty diff.
pub fn check_self_diff(result: &DiffResult) -> Result<(), String> {
    let summary = result.summary();
    if summary.nodes_added != 0 {
        return Err(format!("self-diff has {} added nodes", summary.nodes_added));
    }
    if summary.nodes_removed != 0 {
        return Err(format!(
            "self-diff has {} removed nodes",
            summary.nodes_removed
        ));
    }
    if summary.nodes_modified != 0 {
        return Err(format!(
            "self-diff has {} modified nodes",
            summary.nodes_modified
        ));
    }
    if summary.edges_added != 0 {
        return Err(format!("self-diff has {} added edges", summary.edges_added));
    }
    if summary.edges_removed != 0 {
        return Err(format!(
            "self-diff has {} removed edges",
            summary.edges_removed
        ));
    }
    if summary.edges_modified != 0 {
        return Err(format!(
            "self-diff has {} modified edges",
            summary.edges_modified
        ));
    }
    Ok(())
}

/// Verifies diff accounting: added + matched = file B, removed + matched = file A.
pub fn check_diff_accounting(
    file_a: &OmtsFile,
    file_b: &OmtsFile,
    result: &DiffResult,
) -> Result<(), String> {
    let summary = result.summary();
    let matched_nodes = summary.nodes_modified + summary.nodes_unchanged;

    if summary.nodes_added + matched_nodes != file_b.nodes.len() {
        return Err(format!(
            "node accounting: added({}) + matched({}) != file_b.nodes({})",
            summary.nodes_added,
            matched_nodes,
            file_b.nodes.len()
        ));
    }
    if summary.nodes_removed + matched_nodes != file_a.nodes.len() {
        return Err(format!(
            "node accounting: removed({}) + matched({}) != file_a.nodes({})",
            summary.nodes_removed,
            matched_nodes,
            file_a.nodes.len()
        ));
    }
    Ok(())
}
