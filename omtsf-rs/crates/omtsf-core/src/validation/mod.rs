/// Diagnostic types and rule dispatch for the OMTSF validation engine.
///
/// This module defines [`Diagnostic`], [`Severity`], [`RuleId`], [`Location`],
/// [`ValidationResult`], and [`ValidateOutput`] — the types that represent every
/// finding produced by the three-level validation engine described in
/// `omtsf-rs/docs/validation.md` Section 2.
///
/// It also defines the [`ValidationRule`] trait, [`ValidationConfig`],
/// [`build_registry`], and the top-level [`validate`] dispatch function
/// described in Sections 3.1 and 3.2.
pub mod external;
pub mod rules_l1_gdm;
pub mod rules_l1_sdi;
pub mod rules_l2;
pub mod rules_l3;

use std::fmt;

use crate::file::OmtsFile;
use external::ExternalDataSource;

#[cfg(test)]
mod tests;

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

    /// L1-SDI-01: `boundary_ref` nodes have exactly one identifier with scheme `opaque`.
    L1Sdi01,
    /// L1-SDI-02: If `disclosure_scope` is declared, sensitivity constraints are satisfied.
    L1Sdi02,

    /// L2-GDM-01: A facility with no edge connecting it to an organisation.
    L2Gdm01,
    /// L2-GDM-02: An ownership edge missing `valid_from`.
    L2Gdm02,
    /// L2-GDM-03: L2 graph data model rule 03.
    L2Gdm03,
    /// L2-GDM-04: L2 graph data model rule 04.
    L2Gdm04,

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

    /// L3-MRG-01: Ownership percentage sum verification.
    L3Mrg01,
    /// L3-MRG-02: Legal parentage cycle detection via topological sort.
    L3Mrg02,

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

/// The validation level that a rule belongs to.
///
/// Maps to the three tiers defined in the OMTSF specification:
/// - L1 rules enforce MUST constraints and produce [`Severity::Error`] findings.
/// - L2 rules enforce SHOULD constraints and produce [`Severity::Warning`] findings.
/// - L3 rules cross-reference external data and produce [`Severity::Info`] findings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Level {
    /// Structural conformance rules — violations make a file non-conformant.
    L1,
    /// Semantic quality rules — violations are warnings, not errors.
    L2,
    /// Enrichment rules — require external data, off by default.
    L3,
}

impl Level {
    /// Returns the [`Severity`] that rules at this level produce.
    pub fn severity(self) -> Severity {
        match self {
            Self::L1 => Severity::Error,
            Self::L2 => Severity::Warning,
            Self::L3 => Severity::Info,
        }
    }
}

impl fmt::Display for Level {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::L1 => f.write_str("L1"),
            Self::L2 => f.write_str("L2"),
            Self::L3 => f.write_str("L3"),
        }
    }
}

/// A single, stateless validation rule that inspects an [`OmtsFile`].
///
/// Each rule in the OMTSF validation engine implements this trait.  Rules push
/// zero or more [`Diagnostic`] values into the provided `diags` vector.  A rule
/// that finds nothing wrong pushes nothing.
///
/// Rules are stateless: they hold no mutable state between invocations and
/// receive the file only by shared reference.  The dispatch loop in [`validate`]
/// calls each rule's [`check`][ValidationRule::check] method exactly once per
/// validation pass.
///
/// # Object safety
///
/// The trait is object-safe; the registry stores rules as
/// `Vec<Box<dyn ValidationRule>>`.
///
/// # External data
///
/// The `external_data` parameter carries an optional reference to an
/// [`ExternalDataSource`] implementation.  L1 and L2 rules ignore this
/// parameter entirely.  L3 rules query the data source when `Some` and skip
/// their checks silently when `None`.
///
/// # Extension rules
///
/// Third-party validators implement this trait and use [`RuleId::Extension`]
/// to carry their own identifiers.  Extension rules MUST NOT use `L1-*`,
/// `L2-*`, or `L3-*` prefixes in their codes — those are reserved for
/// spec-defined rules.
pub trait ValidationRule {
    /// The unique identifier for this rule.
    fn id(&self) -> RuleId;

    /// The validation level this rule belongs to (L1, L2, or L3).
    fn level(&self) -> Level;

    /// The severity of diagnostics produced by this rule.
    ///
    /// Derived from [`level`][ValidationRule::level]: L1 → Error, L2 → Warning,
    /// L3 → Info.  Rules SHOULD NOT override this to return a severity
    /// inconsistent with their level.
    fn severity(&self) -> Severity {
        self.level().severity()
    }

    /// Inspect `file` and push any findings into `diags`.
    ///
    /// Called exactly once per validation pass with the fully parsed file.
    /// The rule must not mutate any state outside `diags`.
    ///
    /// `external_data` is `Some` only when L3 rules are active and a concrete
    /// data source has been provided.  L1 and L2 rules MUST ignore this
    /// parameter.  L3 rules MUST skip their checks silently when it is `None`.
    fn check(
        &self,
        file: &OmtsFile,
        diags: &mut Vec<Diagnostic>,
        external_data: Option<&dyn ExternalDataSource>,
    );
}

/// Controls which validation levels are active during a validation pass.
///
/// A conformant validator always runs L1 rules.  L2 rules are on by default.
/// L3 rules are off by default because they require external data sources.
///
/// # Default
///
/// ```
/// # use omtsf_core::ValidationConfig;
/// let cfg = ValidationConfig::default();
/// assert!(cfg.run_l1);
/// assert!(cfg.run_l2);
/// assert!(!cfg.run_l3);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationConfig {
    /// Run L1 (structural) rules.  Always `true` in a conformant validator.
    pub run_l1: bool,
    /// Run L2 (semantic) rules.  Default `true`.
    pub run_l2: bool,
    /// Run L3 (enrichment) rules.  Default `false`; requires external data.
    pub run_l3: bool,
}

impl Default for ValidationConfig {
    fn default() -> Self {
        Self {
            run_l1: true,
            run_l2: true,
            run_l3: false,
        }
    }
}

/// Builds the ordered rule registry for the given configuration.
///
/// Returns a `Vec<Box<dyn ValidationRule>>` containing every built-in rule
/// whose level is enabled in `config`.  Rules are compiled into `omtsf-core`;
/// this is not a plugin system.
///
/// L1-GDM, L1-EID, and L1-SDI rules are gated by [`ValidationConfig::run_l1`].
/// L3 rules are gated by [`ValidationConfig::run_l3`].
pub fn build_registry(config: &ValidationConfig) -> Vec<Box<dyn ValidationRule>> {
    use crate::rules_l1_eid::{
        L1Eid01, L1Eid02, L1Eid03, L1Eid04, L1Eid05, L1Eid06, L1Eid07, L1Eid08, L1Eid09, L1Eid10,
        L1Eid11,
    };
    use rules_l1_gdm::{GdmRule01, GdmRule02, GdmRule03, GdmRule04, GdmRule05, GdmRule06};
    use rules_l1_sdi::{L1Sdi01, L1Sdi02};
    use rules_l2::{L2Eid01, L2Eid04, L2Gdm01, L2Gdm02, L2Gdm03, L2Gdm04};
    use rules_l3::{L3Eid01, L3Mrg01, L3Mrg02};

    let mut registry: Vec<Box<dyn ValidationRule>> = Vec::new();

    if config.run_l1 {
        registry.push(Box::new(GdmRule01));
        registry.push(Box::new(GdmRule02));
        registry.push(Box::new(GdmRule03));
        registry.push(Box::new(GdmRule04));
        registry.push(Box::new(GdmRule05));
        registry.push(Box::new(GdmRule06));
        registry.push(Box::new(L1Eid01));
        registry.push(Box::new(L1Eid02));
        registry.push(Box::new(L1Eid03));
        registry.push(Box::new(L1Eid04));
        registry.push(Box::new(L1Eid05));
        registry.push(Box::new(L1Eid06));
        registry.push(Box::new(L1Eid07));
        registry.push(Box::new(L1Eid08));
        registry.push(Box::new(L1Eid09));
        registry.push(Box::new(L1Eid10));
        registry.push(Box::new(L1Eid11));
        registry.push(Box::new(L1Sdi01));
        registry.push(Box::new(L1Sdi02));
    }

    if config.run_l2 {
        registry.push(Box::new(L2Gdm01));
        registry.push(Box::new(L2Gdm02));
        registry.push(Box::new(L2Gdm03));
        registry.push(Box::new(L2Gdm04));
        registry.push(Box::new(L2Eid01));
        registry.push(Box::new(L2Eid04));
    }

    if config.run_l3 {
        registry.push(Box::new(L3Eid01));
        registry.push(Box::new(L3Mrg01));
        registry.push(Box::new(L3Mrg02));
    }

    registry
}

/// Run the full validation pipeline on a parsed [`OmtsFile`].
///
/// Builds the rule registry from `config`, walks it linearly, and collects all
/// diagnostics.  The engine never fails fast — all diagnostics are collected
/// before returning.
///
/// `external_data` is passed to every rule's `check` method.  L1 and L2 rules
/// ignore it.  L3 rules use it when `Some` and skip their checks when `None`.
/// Callers that do not have an external data source should pass `None` even
/// when `config.run_l3` is `true`; L3 rules will produce no diagnostics.
///
/// Returns a [`ValidationResult`] containing every diagnostic produced.  An
/// empty result indicates a clean file (with respect to the active rule set).
pub fn validate(
    file: &OmtsFile,
    config: &ValidationConfig,
    external_data: Option<&dyn ExternalDataSource>,
) -> ValidationResult {
    let registry = build_registry(config);
    let mut diags: Vec<Diagnostic> = Vec::new();
    for rule in &registry {
        rule.check(file, &mut diags, external_data);
    }
    ValidationResult::from_diagnostics(diags)
}
