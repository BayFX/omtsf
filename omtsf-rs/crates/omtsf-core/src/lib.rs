#![deny(clippy::print_stdout, clippy::print_stderr)]

pub mod boundary_hash;
pub mod canonical;
pub mod check_digits;
pub mod diff;
pub mod enums;
pub mod file;
pub mod graph;
pub mod identity;
pub mod merge;
pub mod merge_pipeline;
pub mod newtypes;
pub mod redaction;
pub mod rules_l1_eid;
pub mod sensitivity;
pub mod serde_helpers;
pub mod structures;
pub mod types;
pub mod union_find;
pub mod validation;

pub use boundary_hash::{BoundaryHashError, boundary_ref_value, decode_salt, generate_file_salt};
pub use canonical::{CanonicalId, build_identifier_index};
pub use diff::{
    DiffFilter, DiffResult, DiffSummary, EdgeDiff, EdgeRef, EdgesDiff, IdentifierFieldDiff,
    IdentifierSetDiff, LabelSetDiff, NodeDiff, NodeRef, NodesDiff, PropertyChange, diff,
    diff_filtered,
};
pub use enums::{
    AttestationOutcome, AttestationStatus, AttestationType, Confidence, ConsolidationBasis,
    ControlType, DisclosureScope, EdgeType, EdgeTypeTag, EmissionFactorSource, EventType, NodeType,
    NodeTypeTag, OrganizationStatus, RiskLikelihood, RiskSeverity, Sensitivity, ServiceType,
    VerificationStatus,
};
pub use file::OmtsFile;
pub use graph::{
    DEFAULT_MAX_DEPTH, Direction, EdgeWeight, GraphBuildError, NodeWeight, OmtsGraph, QueryError,
    all_paths, build_graph, detect_cycles, ego_graph, induced_subgraph, reachable_from,
    shortest_path,
};
pub use identity::{
    EdgeCompositeKey, build_edge_candidate_index, edge_composite_key,
    edge_identity_properties_match, edges_match, identifiers_match, is_lei_annulled,
    temporal_compatible,
};
pub use merge::{
    Conflict, ConflictEntry, MergeMetadata, SameAsThreshold, ScalarMergeResult,
    apply_same_as_edges, build_conflicts_value, merge_identifiers, merge_labels, merge_scalars,
};
pub use merge_pipeline::{
    MergeConfig, MergeError, MergeOutput, MergeWarning, merge, merge_with_config,
};
pub use newtypes::{CalendarDate, CountryCode, EdgeId, FileSalt, NewtypeError, NodeId, SemVer};
pub use redaction::{
    EdgeAction, NodeAction, classify_edge, classify_node, filter_edge_properties,
    filter_identifiers,
};
pub use sensitivity::{effective_property_sensitivity, effective_sensitivity};
pub use structures::{Edge, EdgeProperties, Node};
pub use types::{DataQuality, Geo, GeoParseError, Identifier, Label, parse_geo};
pub use union_find::UnionFind;
pub use validation::external::{ExternalDataSource, LeiRecord, NatRegRecord};
pub use validation::{
    Diagnostic, Level, Location, ParseError, RuleId, Severity, ValidateOutput, ValidationConfig,
    ValidationResult, ValidationRule, build_registry, validate,
};

/// Returns the current version of the omtsf-core library.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    #[test]
    fn version_is_semver() {
        let v = version();
        let parts: Vec<&str> = v.split('.').collect();
        assert_eq!(parts.len(), 3, "version should have 3 parts: {v}");
        for part in parts {
            part.parse::<u32>().expect("each part should be a number");
        }
    }
}
