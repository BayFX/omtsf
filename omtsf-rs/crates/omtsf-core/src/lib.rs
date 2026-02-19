#![deny(clippy::print_stdout, clippy::print_stderr)]

pub mod canonical;
pub mod enums;
pub mod file;
pub mod newtypes;
pub mod serde_helpers;
pub mod structures;
pub mod types;
pub mod validation;

pub use canonical::{CanonicalId, build_identifier_index};
pub use enums::{
    AttestationOutcome, AttestationStatus, AttestationType, Confidence, ConsolidationBasis,
    ControlType, DisclosureScope, EdgeType, EdgeTypeTag, EmissionFactorSource, EventType, NodeType,
    NodeTypeTag, OrganizationStatus, RiskLikelihood, RiskSeverity, Sensitivity, ServiceType,
    VerificationStatus,
};
pub use file::OmtsFile;
pub use newtypes::{CalendarDate, CountryCode, EdgeId, FileSalt, NewtypeError, NodeId, SemVer};
pub use structures::{Edge, EdgeProperties, Node};
pub use types::{DataQuality, Geo, GeoParseError, Identifier, Label, parse_geo};
pub use validation::{
    Diagnostic, Location, ParseError, RuleId, Severity, ValidateOutput, ValidationResult,
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
