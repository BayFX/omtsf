#![deny(clippy::print_stdout, clippy::print_stderr)]

pub mod canonical;
pub mod check_digits;
pub mod enums;
pub mod file;
pub mod graph;
pub mod identity;
pub mod newtypes;
pub mod rules_l1_eid;
pub mod serde_helpers;
pub mod structures;
pub mod types;
pub mod union_find;
pub mod validation;

pub use canonical::{CanonicalId, build_identifier_index};
pub use enums::{
    AttestationOutcome, AttestationStatus, AttestationType, Confidence, ConsolidationBasis,
    ControlType, DisclosureScope, EdgeType, EdgeTypeTag, EmissionFactorSource, EventType, NodeType,
    NodeTypeTag, OrganizationStatus, RiskLikelihood, RiskSeverity, Sensitivity, ServiceType,
    VerificationStatus,
};
pub use file::OmtsFile;
pub use graph::{
    DEFAULT_MAX_DEPTH, Direction, EdgeWeight, GraphBuildError, NodeWeight, OmtsGraph, QueryError,
    all_paths, build_graph, reachable_from, shortest_path,
};
pub use identity::{identifiers_match, is_lei_annulled, temporal_compatible};
pub use newtypes::{CalendarDate, CountryCode, EdgeId, FileSalt, NewtypeError, NodeId, SemVer};
pub use structures::{Edge, EdgeProperties, Node};
pub use types::{DataQuality, Geo, GeoParseError, Identifier, Label, parse_geo};
pub use union_find::UnionFind;
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
