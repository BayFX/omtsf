/// Diagnostic types for the OMTSF validation engine.
///
/// This module defines [`Diagnostic`], [`Severity`], [`RuleId`], [`Location`],
/// [`ValidationResult`], and [`ValidateOutput`] — the types that represent every
/// finding produced by the three-level validation engine described in
/// `omtsf-rs/docs/validation.md` Section 2.
use std::fmt;

// ---------------------------------------------------------------------------
// Severity
// ---------------------------------------------------------------------------

/// The severity level of a validation finding.
///
/// Maps directly to the three validation levels defined in the OMTSF spec:
/// L1 rules produce [`Severity::Error`], L2 rules produce [`Severity::Warning`],
/// and L3 rules produce [`Severity::Info`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Severity {
    /// L1 — structural violation; the file is non-conformant.
    Error,
    /// L2 — semantic concern; the file is conformant but suspect.
    Warning,
    /// L3 — enrichment observation derived from external data.
    Info,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Error => f.write_str("Error"),
            Self::Warning => f.write_str("Warning"),
            Self::Info => f.write_str("Info"),
        }
    }
}

// ---------------------------------------------------------------------------
// RuleId
// ---------------------------------------------------------------------------

/// Machine-readable identifier for a validation rule.
///
/// Each variant corresponds to exactly one rule defined in the OMTSF
/// specification. [`RuleId::code`] returns the canonical hyphenated form used
/// in serialised output (e.g. `"L1-GDM-03"`).
///
/// Extension rules from third-party validators use [`RuleId::Extension`].
/// Internal validator bugs use [`RuleId::Internal`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum RuleId {
    // --- L1: Graph Data Model (SPEC-001) ---
    /// L1-GDM-01: Every node has a non-empty `id`, unique within the file.
    L1Gdm01,
    /// L1-GDM-02: Every edge has a non-empty `id`, unique within the file.
    L1Gdm02,
    /// L1-GDM-03: Every edge `source` and `target` references an existing node `id`.
    L1Gdm03,
    /// L1-GDM-04: Edge `type` is a recognised core type, `same_as`, or reverse-domain extension.
    L1Gdm04,
    /// L1-GDM-05: `reporting_entity` if present references an existing organisation node `id`.
    L1Gdm05,
    /// L1-GDM-06: Edge source/target node types match the permitted types table. Extension edges are exempt.
    L1Gdm06,

    // --- L1: Entity Identification (SPEC-002) ---
    /// L1-EID-01: Every identifier has a non-empty `scheme`.
    L1Eid01,
    /// L1-EID-02: Every identifier has a non-empty `value`.
    L1Eid02,
    /// L1-EID-03: `authority` is present when scheme is `nat-reg`, `vat`, or `internal`.
    L1Eid03,
    /// L1-EID-04: `scheme` is a core scheme or reverse-domain extension.
    L1Eid04,
    /// L1-EID-05: LEI matches `^[A-Z0-9]{18}[0-9]{2}$` and passes MOD 97-10.
    L1Eid05,
    /// L1-EID-06: DUNS matches `^[0-9]{9}$`.
    L1Eid06,
    /// L1-EID-07: GLN matches `^[0-9]{13}$` and passes GS1 mod-10.
    L1Eid07,
    /// L1-EID-08: `valid_from` / `valid_to` if present are valid ISO 8601 dates.
    L1Eid08,
    /// L1-EID-09: `valid_from` <= `valid_to` when both present.
    L1Eid09,
    /// L1-EID-10: `sensitivity` if present is `public`, `restricted`, or `confidential`.
    L1Eid10,
    /// L1-EID-11: No duplicate `{scheme, value, authority}` tuple on the same node.
    L1Eid11,

    // --- L1: Selective Disclosure (SPEC-004) ---
    /// L1-SDI-01: `boundary_ref` nodes have exactly one identifier with scheme `opaque`.
    L1Sdi01,
    /// L1-SDI-02: If `disclosure_scope` is declared, sensitivity constraints are satisfied.
    L1Sdi02,

    // --- L2: Graph Data Model ---
    /// L2-GDM-01: A facility with no edge connecting it to an organisation.
    L2Gdm01,
    /// L2-GDM-02: An ownership edge missing `valid_from`.
    L2Gdm02,
    /// L2-GDM-03: L2 graph data model rule 03.
    L2Gdm03,
    /// L2-GDM-04: L2 graph data model rule 04.
    L2Gdm04,

    // --- L2: Entity Identification ---
    /// L2-EID-01: An organisation node with no external identifiers.
    L2Eid01,
    /// L2-EID-02: L2 entity identification rule 02.
    L2Eid02,
    /// L2-EID-03: L2 entity identification rule 03.
    L2Eid03,
    /// L2-EID-04: A country code that is not a valid ISO 3166-1 alpha-2.
    L2Eid04,
    /// L2-EID-05: L2 entity identification rule 05.
    L2Eid05,
    /// L2-EID-06: L2 entity identification rule 06.
    L2Eid06,
    /// L2-EID-07: L2 entity identification rule 07.
    L2Eid07,
    /// L2-EID-08: L2 entity identification rule 08.
    L2Eid08,

    // --- L3: Entity Identification (registry verification) ---
    /// L3-EID-01: L3 registry verification rule 01.
    L3Eid01,
    /// L3-EID-02: L3 registry verification rule 02.
    L3Eid02,
    /// L3-EID-03: L3 registry verification rule 03.
    L3Eid03,
    /// L3-EID-04: L3 registry verification rule 04.
    L3Eid04,
    /// L3-EID-05: L3 registry verification rule 05.
    L3Eid05,

    // --- L3: Merge Semantics ---
    /// L3-MRG-01: Ownership percentage sum verification.
    L3Mrg01,
    /// L3-MRG-02: Legal parentage cycle detection via topological sort.
    L3Mrg02,

    // --- Special variants ---
    /// An extension rule defined outside the core spec. Must not use `L1-*`, `L2-*`, or `L3-*` prefixes.
    Extension(String),
    /// An internal validator bug. Indicates a logic error in the validator itself.
    Internal,
}

impl RuleId {
    /// Returns the canonical hyphenated rule code string.
    ///
    /// For spec-defined rules the returned string matches the form used in the
    /// OMTSF specification (e.g. `"L1-GDM-03"`, `"L2-EID-04"`).
    /// For [`RuleId::Extension`] the inner string is returned as-is.
    /// For [`RuleId::Internal`] the string `"internal"` is returned.
    pub fn code(&self) -> &str {
        match self {
            Self::L1Gdm01 => "L1-GDM-01",
            Self::L1Gdm02 => "L1-GDM-02",
            Self::L1Gdm03 => "L1-GDM-03",
            Self::L1Gdm04 => "L1-GDM-04",
            Self::L1Gdm05 => "L1-GDM-05",
            Self::L1Gdm06 => "L1-GDM-06",
            Self::L1Eid01 => "L1-EID-01",
            Self::L1Eid02 => "L1-EID-02",
            Self::L1Eid03 => "L1-EID-03",
            Self::L1Eid04 => "L1-EID-04",
            Self::L1Eid05 => "L1-EID-05",
            Self::L1Eid06 => "L1-EID-06",
            Self::L1Eid07 => "L1-EID-07",
            Self::L1Eid08 => "L1-EID-08",
            Self::L1Eid09 => "L1-EID-09",
            Self::L1Eid10 => "L1-EID-10",
            Self::L1Eid11 => "L1-EID-11",
            Self::L1Sdi01 => "L1-SDI-01",
            Self::L1Sdi02 => "L1-SDI-02",
            Self::L2Gdm01 => "L2-GDM-01",
            Self::L2Gdm02 => "L2-GDM-02",
            Self::L2Gdm03 => "L2-GDM-03",
            Self::L2Gdm04 => "L2-GDM-04",
            Self::L2Eid01 => "L2-EID-01",
            Self::L2Eid02 => "L2-EID-02",
            Self::L2Eid03 => "L2-EID-03",
            Self::L2Eid04 => "L2-EID-04",
            Self::L2Eid05 => "L2-EID-05",
            Self::L2Eid06 => "L2-EID-06",
            Self::L2Eid07 => "L2-EID-07",
            Self::L2Eid08 => "L2-EID-08",
            Self::L3Eid01 => "L3-EID-01",
            Self::L3Eid02 => "L3-EID-02",
            Self::L3Eid03 => "L3-EID-03",
            Self::L3Eid04 => "L3-EID-04",
            Self::L3Eid05 => "L3-EID-05",
            Self::L3Mrg01 => "L3-MRG-01",
            Self::L3Mrg02 => "L3-MRG-02",
            Self::Extension(s) => s.as_str(),
            Self::Internal => "internal",
        }
    }
}

impl fmt::Display for RuleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

// ---------------------------------------------------------------------------
// Location
// ---------------------------------------------------------------------------

/// The location within the graph where a diagnostic finding was detected.
///
/// Every [`Diagnostic`] carries a `location` that points to the specific
/// node, edge, identifier entry, or header field responsible for the finding.
/// The `node_id` and `edge_id` values are the graph-local `id` strings from
/// the file, not internal indices.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Location {
    /// A field in the file header (e.g. `"spec_version"`, `"reporting_entity"`).
    Header {
        /// The name of the header field.
        field: &'static str,
    },
    /// A node property, or the node itself.
    Node {
        /// The graph-local `id` of the node.
        node_id: String,
        /// The specific property within the node, if applicable.
        field: Option<String>,
    },
    /// An edge property, or the edge itself.
    Edge {
        /// The graph-local `id` of the edge.
        edge_id: String,
        /// The specific property within the edge, if applicable.
        field: Option<String>,
    },
    /// An entry in a node's `identifiers` array.
    Identifier {
        /// The graph-local `id` of the node owning the identifier.
        node_id: String,
        /// The zero-based index within the node's `identifiers` array.
        index: usize,
        /// The specific property within the identifier entry, if applicable.
        field: Option<String>,
    },
    /// A file-level finding not attributable to a specific node or edge.
    Global,
}

impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Header { field } => write!(f, "header.{field}"),
            Self::Node {
                node_id,
                field: None,
            } => write!(f, "node \"{node_id}\""),
            Self::Node {
                node_id,
                field: Some(field),
            } => write!(f, "node \"{node_id}\" field \"{field}\""),
            Self::Edge {
                edge_id,
                field: None,
            } => write!(f, "edge \"{edge_id}\""),
            Self::Edge {
                edge_id,
                field: Some(field),
            } => write!(f, "edge \"{edge_id}\" field \"{field}\""),
            Self::Identifier {
                node_id,
                index,
                field: None,
            } => write!(f, "node \"{node_id}\" identifiers[{index}]"),
            Self::Identifier {
                node_id,
                index,
                field: Some(field),
            } => write!(f, "node \"{node_id}\" identifiers[{index}].{field}"),
            Self::Global => f.write_str("(global)"),
        }
    }
}

// ---------------------------------------------------------------------------
// Diagnostic
// ---------------------------------------------------------------------------

/// A single validation finding produced by the OMTSF validation engine.
///
/// Diagnostics are collected across all applicable rules and returned in a
/// [`ValidationResult`]. The engine never fails fast — all diagnostics for a
/// given file are collected before results are returned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// The rule that produced this finding.
    pub rule_id: RuleId,
    /// The severity of this finding.
    pub severity: Severity,
    /// Where in the graph the problem was detected.
    pub location: Location,
    /// A human-readable explanation of the problem.
    pub message: String,
}

impl Diagnostic {
    /// Constructs a new [`Diagnostic`].
    pub fn new(
        rule_id: RuleId,
        severity: Severity,
        location: Location,
        message: impl Into<String>,
    ) -> Self {
        Self {
            rule_id,
            severity,
            location,
            message: message.into(),
        }
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let level_char = match self.severity {
            Severity::Error => 'E',
            Severity::Warning => 'W',
            Severity::Info => 'I',
        };
        write!(
            f,
            "[{level_char}] {} {}: {}",
            self.rule_id, self.location, self.message
        )
    }
}

// ---------------------------------------------------------------------------
// ValidationResult
// ---------------------------------------------------------------------------

/// The collected output of a validation pass on a parsed OMTSF graph.
///
/// Always contains all diagnostics found — the engine never fails fast. Use
/// [`has_errors`][ValidationResult::has_errors] or
/// [`is_conformant`][ValidationResult::is_conformant] to determine overall
/// status, and the filtering iterators to inspect specific findings.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ValidationResult {
    /// All diagnostics produced during the validation pass.
    pub diagnostics: Vec<Diagnostic>,
}

impl ValidationResult {
    /// Creates an empty [`ValidationResult`] with no diagnostics.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a [`ValidationResult`] from a pre-built list of diagnostics.
    pub fn from_diagnostics(diagnostics: Vec<Diagnostic>) -> Self {
        Self { diagnostics }
    }

    /// Returns `true` if any diagnostic has [`Severity::Error`].
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error)
    }

    /// Returns `true` if there are zero [`Severity::Error`] diagnostics.
    ///
    /// A file is conformant even if it carries warnings or info findings.
    pub fn is_conformant(&self) -> bool {
        !self.has_errors()
    }

    /// Returns an iterator over all diagnostics with [`Severity::Error`].
    pub fn errors(&self) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
    }

    /// Returns an iterator over all diagnostics with [`Severity::Warning`].
    pub fn warnings(&self) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Warning)
    }

    /// Returns an iterator over all diagnostics with [`Severity::Info`].
    pub fn infos(&self) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Info)
    }

    /// Returns an iterator over all diagnostics produced by the given rule.
    pub fn by_rule(&self, rule: &RuleId) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics.iter().filter(move |d| &d.rule_id == rule)
    }

    /// Returns the total number of diagnostics.
    pub fn len(&self) -> usize {
        self.diagnostics.len()
    }

    /// Returns `true` if there are no diagnostics at all.
    pub fn is_empty(&self) -> bool {
        self.diagnostics.is_empty()
    }
}

// ---------------------------------------------------------------------------
// ParseError
// ---------------------------------------------------------------------------

/// A parse-level failure: the input is not a valid `.omts` file.
///
/// Parse errors prevent validation from running entirely. They are reported
/// as a [`ValidateOutput::ParseFailed`] variant rather than as [`Diagnostic`]
/// values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    /// Human-readable description of the parse failure, including location
    /// information (byte offset or line/column) where available.
    pub message: String,
}

impl ParseError {
    /// Constructs a [`ParseError`] from a message string.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "parse error: {}", self.message)
    }
}

impl std::error::Error for ParseError {}

// ---------------------------------------------------------------------------
// ValidateOutput
// ---------------------------------------------------------------------------

/// The top-level result type returned by the OMTSF validator.
///
/// Distinguishes between a file that could not be parsed at all
/// ([`ValidateOutput::ParseFailed`]) and one that was parsed successfully
/// ([`ValidateOutput::Validated`]), even if the parsed file contains L1 errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidateOutput {
    /// The input file could not be parsed as a valid OMTSF file.
    ///
    /// No validation was performed. The caller should report the [`ParseError`]
    /// and exit with code 2.
    ParseFailed(ParseError),
    /// The input parsed successfully; diagnostics may still be present.
    ///
    /// Check [`ValidationResult::is_conformant`] for overall pass/fail status.
    Validated(ValidationResult),
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    #![allow(clippy::panic)]

    use super::*;

    // --- Severity ---

    #[test]
    fn severity_display() {
        assert_eq!(Severity::Error.to_string(), "Error");
        assert_eq!(Severity::Warning.to_string(), "Warning");
        assert_eq!(Severity::Info.to_string(), "Info");
    }

    #[test]
    fn severity_clone_and_eq() {
        let s = Severity::Warning;
        assert_eq!(s, s.clone());
        assert_ne!(Severity::Error, Severity::Info);
    }

    // --- RuleId::code ---

    #[test]
    fn rule_id_code_l1_gdm() {
        assert_eq!(RuleId::L1Gdm01.code(), "L1-GDM-01");
        assert_eq!(RuleId::L1Gdm02.code(), "L1-GDM-02");
        assert_eq!(RuleId::L1Gdm03.code(), "L1-GDM-03");
        assert_eq!(RuleId::L1Gdm04.code(), "L1-GDM-04");
        assert_eq!(RuleId::L1Gdm05.code(), "L1-GDM-05");
        assert_eq!(RuleId::L1Gdm06.code(), "L1-GDM-06");
    }

    #[test]
    fn rule_id_code_l1_eid() {
        assert_eq!(RuleId::L1Eid01.code(), "L1-EID-01");
        assert_eq!(RuleId::L1Eid02.code(), "L1-EID-02");
        assert_eq!(RuleId::L1Eid03.code(), "L1-EID-03");
        assert_eq!(RuleId::L1Eid04.code(), "L1-EID-04");
        assert_eq!(RuleId::L1Eid05.code(), "L1-EID-05");
        assert_eq!(RuleId::L1Eid06.code(), "L1-EID-06");
        assert_eq!(RuleId::L1Eid07.code(), "L1-EID-07");
        assert_eq!(RuleId::L1Eid08.code(), "L1-EID-08");
        assert_eq!(RuleId::L1Eid09.code(), "L1-EID-09");
        assert_eq!(RuleId::L1Eid10.code(), "L1-EID-10");
        assert_eq!(RuleId::L1Eid11.code(), "L1-EID-11");
    }

    #[test]
    fn rule_id_code_l1_sdi() {
        assert_eq!(RuleId::L1Sdi01.code(), "L1-SDI-01");
        assert_eq!(RuleId::L1Sdi02.code(), "L1-SDI-02");
    }

    #[test]
    fn rule_id_code_l2_gdm() {
        assert_eq!(RuleId::L2Gdm01.code(), "L2-GDM-01");
        assert_eq!(RuleId::L2Gdm02.code(), "L2-GDM-02");
        assert_eq!(RuleId::L2Gdm03.code(), "L2-GDM-03");
        assert_eq!(RuleId::L2Gdm04.code(), "L2-GDM-04");
    }

    #[test]
    fn rule_id_code_l2_eid() {
        assert_eq!(RuleId::L2Eid01.code(), "L2-EID-01");
        assert_eq!(RuleId::L2Eid02.code(), "L2-EID-02");
        assert_eq!(RuleId::L2Eid03.code(), "L2-EID-03");
        assert_eq!(RuleId::L2Eid04.code(), "L2-EID-04");
        assert_eq!(RuleId::L2Eid05.code(), "L2-EID-05");
        assert_eq!(RuleId::L2Eid06.code(), "L2-EID-06");
        assert_eq!(RuleId::L2Eid07.code(), "L2-EID-07");
        assert_eq!(RuleId::L2Eid08.code(), "L2-EID-08");
    }

    #[test]
    fn rule_id_code_l3() {
        assert_eq!(RuleId::L3Eid01.code(), "L3-EID-01");
        assert_eq!(RuleId::L3Eid02.code(), "L3-EID-02");
        assert_eq!(RuleId::L3Eid03.code(), "L3-EID-03");
        assert_eq!(RuleId::L3Eid04.code(), "L3-EID-04");
        assert_eq!(RuleId::L3Eid05.code(), "L3-EID-05");
        assert_eq!(RuleId::L3Mrg01.code(), "L3-MRG-01");
        assert_eq!(RuleId::L3Mrg02.code(), "L3-MRG-02");
    }

    #[test]
    fn rule_id_code_extension() {
        let r = RuleId::Extension("com.acme.custom-check".to_owned());
        assert_eq!(r.code(), "com.acme.custom-check");
    }

    #[test]
    fn rule_id_code_internal() {
        assert_eq!(RuleId::Internal.code(), "internal");
    }

    #[test]
    fn rule_id_display_matches_code() {
        assert_eq!(RuleId::L1Gdm03.to_string(), RuleId::L1Gdm03.code());
        assert_eq!(RuleId::Extension("ext".to_owned()).to_string(), "ext");
        assert_eq!(RuleId::Internal.to_string(), "internal");
    }

    // --- Location ---

    #[test]
    fn location_display_header() {
        let loc = Location::Header {
            field: "spec_version",
        };
        assert_eq!(loc.to_string(), "header.spec_version");
    }

    #[test]
    fn location_display_node_no_field() {
        let loc = Location::Node {
            node_id: "n-1".to_owned(),
            field: None,
        };
        assert_eq!(loc.to_string(), "node \"n-1\"");
    }

    #[test]
    fn location_display_node_with_field() {
        let loc = Location::Node {
            node_id: "n-1".to_owned(),
            field: Some("type".to_owned()),
        };
        assert_eq!(loc.to_string(), "node \"n-1\" field \"type\"");
    }

    #[test]
    fn location_display_edge_no_field() {
        let loc = Location::Edge {
            edge_id: "e-42".to_owned(),
            field: None,
        };
        assert_eq!(loc.to_string(), "edge \"e-42\"");
    }

    #[test]
    fn location_display_edge_with_field() {
        let loc = Location::Edge {
            edge_id: "e-42".to_owned(),
            field: Some("source".to_owned()),
        };
        assert_eq!(loc.to_string(), "edge \"e-42\" field \"source\"");
    }

    #[test]
    fn location_display_identifier_no_field() {
        let loc = Location::Identifier {
            node_id: "n-1".to_owned(),
            index: 2,
            field: None,
        };
        assert_eq!(loc.to_string(), "node \"n-1\" identifiers[2]");
    }

    #[test]
    fn location_display_identifier_with_field() {
        let loc = Location::Identifier {
            node_id: "n-1".to_owned(),
            index: 0,
            field: Some("scheme".to_owned()),
        };
        assert_eq!(loc.to_string(), "node \"n-1\" identifiers[0].scheme");
    }

    #[test]
    fn location_display_global() {
        assert_eq!(Location::Global.to_string(), "(global)");
    }

    // --- Diagnostic construction and display ---

    fn make_error(rule: RuleId) -> Diagnostic {
        Diagnostic::new(rule, Severity::Error, Location::Global, "test error")
    }

    fn make_warning(rule: RuleId) -> Diagnostic {
        Diagnostic::new(rule, Severity::Warning, Location::Global, "test warning")
    }

    fn make_info(rule: RuleId) -> Diagnostic {
        Diagnostic::new(rule, Severity::Info, Location::Global, "test info")
    }

    #[test]
    fn diagnostic_construction() {
        let d = Diagnostic::new(
            RuleId::L1Gdm03,
            Severity::Error,
            Location::Edge {
                edge_id: "edge-042".to_owned(),
                field: Some("target".to_owned()),
            },
            "target \"node-999\" does not reference an existing node",
        );
        assert_eq!(d.rule_id, RuleId::L1Gdm03);
        assert_eq!(d.severity, Severity::Error);
        assert!(d.message.contains("node-999"));
    }

    #[test]
    fn diagnostic_display_error() {
        let d = make_error(RuleId::L1Gdm03);
        let s = d.to_string();
        assert!(s.starts_with("[E]"));
        assert!(s.contains("L1-GDM-03"));
    }

    #[test]
    fn diagnostic_display_warning() {
        let d = make_warning(RuleId::L2Eid01);
        let s = d.to_string();
        assert!(s.starts_with("[W]"));
        assert!(s.contains("L2-EID-01"));
    }

    #[test]
    fn diagnostic_display_info() {
        let d = make_info(RuleId::L3Mrg02);
        let s = d.to_string();
        assert!(s.starts_with("[I]"));
        assert!(s.contains("L3-MRG-02"));
    }

    // --- ValidationResult ---

    #[test]
    fn validation_result_empty_is_conformant() {
        let r = ValidationResult::new();
        assert!(r.is_conformant());
        assert!(!r.has_errors());
        assert!(r.is_empty());
        assert_eq!(r.len(), 0);
    }

    #[test]
    fn validation_result_with_only_warnings_is_conformant() {
        let r = ValidationResult::from_diagnostics(vec![
            make_warning(RuleId::L2Gdm01),
            make_info(RuleId::L3Eid01),
        ]);
        assert!(r.is_conformant());
        assert!(!r.has_errors());
    }

    #[test]
    fn validation_result_with_error_is_not_conformant() {
        let r = ValidationResult::from_diagnostics(vec![
            make_error(RuleId::L1Gdm01),
            make_warning(RuleId::L2Gdm01),
        ]);
        assert!(!r.is_conformant());
        assert!(r.has_errors());
    }

    #[test]
    fn validation_result_errors_iterator() {
        let r = ValidationResult::from_diagnostics(vec![
            make_error(RuleId::L1Gdm01),
            make_warning(RuleId::L2Gdm01),
            make_error(RuleId::L1Eid01),
            make_info(RuleId::L3Eid01),
        ]);
        let errors: Vec<_> = r.errors().collect();
        assert_eq!(errors.len(), 2);
        assert!(errors.iter().all(|d| d.severity == Severity::Error));
    }

    #[test]
    fn validation_result_warnings_iterator() {
        let r = ValidationResult::from_diagnostics(vec![
            make_error(RuleId::L1Gdm01),
            make_warning(RuleId::L2Gdm01),
            make_warning(RuleId::L2Eid04),
        ]);
        let warnings: Vec<_> = r.warnings().collect();
        assert_eq!(warnings.len(), 2);
        assert!(warnings.iter().all(|d| d.severity == Severity::Warning));
    }

    #[test]
    fn validation_result_infos_iterator() {
        let r = ValidationResult::from_diagnostics(vec![
            make_info(RuleId::L3Mrg01),
            make_info(RuleId::L3Mrg02),
            make_error(RuleId::L1Gdm03),
        ]);
        let infos: Vec<_> = r.infos().collect();
        assert_eq!(infos.len(), 2);
        assert!(infos.iter().all(|d| d.severity == Severity::Info));
    }

    #[test]
    fn validation_result_by_rule_filter() {
        let r = ValidationResult::from_diagnostics(vec![
            make_error(RuleId::L1Gdm01),
            make_error(RuleId::L1Gdm01),
            make_warning(RuleId::L2Gdm01),
        ]);
        let gdm01: Vec<_> = r.by_rule(&RuleId::L1Gdm01).collect();
        assert_eq!(gdm01.len(), 2);
        let gdm02: Vec<_> = r.by_rule(&RuleId::L1Gdm02).collect();
        assert_eq!(gdm02.len(), 0);
    }

    #[test]
    fn validation_result_len_and_is_empty() {
        let r = ValidationResult::from_diagnostics(vec![
            make_error(RuleId::L1Gdm01),
            make_warning(RuleId::L2Gdm02),
        ]);
        assert_eq!(r.len(), 2);
        assert!(!r.is_empty());
    }

    #[test]
    fn validation_result_default_is_empty() {
        let r = ValidationResult::default();
        assert!(r.is_empty());
        assert!(r.is_conformant());
    }

    // --- ParseError ---

    #[test]
    fn parse_error_display() {
        let e = ParseError::new("unexpected token at line 3");
        assert_eq!(e.to_string(), "parse error: unexpected token at line 3");
    }

    #[test]
    fn parse_error_is_std_error() {
        let e: Box<dyn std::error::Error> = Box::new(ParseError::new("malformed json"));
        assert!(!e.to_string().is_empty());
    }

    // --- ValidateOutput ---

    #[test]
    fn validate_output_parse_failed_variant() {
        let out = ValidateOutput::ParseFailed(ParseError::new("bad input"));
        match out {
            ValidateOutput::ParseFailed(e) => assert!(e.message.contains("bad input")),
            ValidateOutput::Validated(_) => panic!("expected ParseFailed"),
        }
    }

    #[test]
    fn validate_output_validated_variant() {
        let result = ValidationResult::from_diagnostics(vec![make_error(RuleId::L1Gdm03)]);
        let out = ValidateOutput::Validated(result);
        match out {
            ValidateOutput::Validated(r) => assert!(r.has_errors()),
            ValidateOutput::ParseFailed(_) => panic!("expected Validated"),
        }
    }

    #[test]
    fn validate_output_validated_clean() {
        let out = ValidateOutput::Validated(ValidationResult::new());
        match out {
            ValidateOutput::Validated(r) => {
                assert!(r.is_conformant());
                assert!(r.is_empty());
            }
            ValidateOutput::ParseFailed(_) => panic!("expected Validated"),
        }
    }
}
