/// L1-EID validation rules for entity identification (SPEC-002 Section 6.1).
///
/// This module implements rules L1-EID-01 through L1-EID-11 as specified in
/// validation.md Section 4.1 and SPEC-002 Section 6.1.  Each rule is a
/// zero-sized struct that implements [`ValidationRule`].
///
/// Rules are registered in [`crate::validation::build_registry`] when
/// `config.run_l1` is `true`.
use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;

use crate::check_digits::{gs1_mod10, mod97_10};
use crate::file::OmtsFile;
use crate::validation::{Diagnostic, Level, Location, RuleId, Severity, ValidationRule};

// ---------------------------------------------------------------------------
// Compiled regex patterns
//
// These are static to avoid recompiling on every call to check().
// ---------------------------------------------------------------------------

/// LEI format: 18 uppercase alphanumeric characters followed by 2 digits.
static LEI_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[A-Z0-9]{18}[0-9]{2}$")
        .unwrap_or_else(|_| Regex::new(".").unwrap_or_else(|_| unreachable!("regex engine broken")))
});

/// DUNS format: exactly 9 digits.
static DUNS_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[0-9]{9}$")
        .unwrap_or_else(|_| Regex::new(".").unwrap_or_else(|_| unreachable!("regex engine broken")))
});

/// GLN format: exactly 13 digits.
static GLN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[0-9]{13}$")
        .unwrap_or_else(|_| Regex::new(".").unwrap_or_else(|_| unreachable!("regex engine broken")))
});

/// Core scheme codes defined by SPEC-002 Section 5.1, plus the reserved
/// `opaque` scheme used exclusively by `boundary_ref` nodes (SPEC-004 Section 5.1).
static CORE_SCHEMES: &[&str] = &["lei", "duns", "gln", "nat-reg", "vat", "internal", "opaque"];

/// Returns `true` if the scheme string is a valid core scheme or a
/// reverse-domain extension scheme (contains a dot, e.g. `"com.example.id"`).
fn is_valid_scheme(scheme: &str) -> bool {
    CORE_SCHEMES.contains(&scheme) || scheme.contains('.')
}

/// Returns `true` if the `authority` field is required for the given scheme.
fn requires_authority(scheme: &str) -> bool {
    matches!(scheme, "nat-reg" | "vat" | "internal")
}

// ---------------------------------------------------------------------------
// Helper: emit a diagnostic for a specific identifier field
// ---------------------------------------------------------------------------

fn eid_diag(
    rule_id: RuleId,
    node_id: &str,
    index: usize,
    field: Option<&'static str>,
    message: impl Into<String>,
) -> Diagnostic {
    Diagnostic::new(
        rule_id,
        Severity::Error,
        Location::Identifier {
            node_id: node_id.to_owned(),
            index,
            field: field.map(ToOwned::to_owned),
        },
        message,
    )
}

// ---------------------------------------------------------------------------
// L1-EID-01: Every identifier has a non-empty `scheme`
// ---------------------------------------------------------------------------

/// Every identifier record MUST have a non-empty `scheme` field (SPEC-002 L1-EID-01).
pub struct L1Eid01;

impl ValidationRule for L1Eid01 {
    fn id(&self) -> RuleId {
        RuleId::L1Eid01
    }

    fn level(&self) -> Level {
        Level::L1
    }

    fn check(
        &self,
        file: &OmtsFile,
        diags: &mut Vec<Diagnostic>,
        _external_data: Option<&dyn crate::validation::external::ExternalDataSource>,
    ) {
        for node in &file.nodes {
            let Some(identifiers) = &node.identifiers else {
                continue;
            };
            for (idx, ident) in identifiers.iter().enumerate() {
                if ident.scheme.is_empty() {
                    diags.push(eid_diag(
                        RuleId::L1Eid01,
                        node.id.as_ref(),
                        idx,
                        Some("scheme"),
                        "identifier `scheme` must not be empty",
                    ));
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// L1-EID-02: Every identifier has a non-empty `value`
// ---------------------------------------------------------------------------

/// Every identifier record MUST have a non-empty `value` field (SPEC-002 L1-EID-02).
pub struct L1Eid02;

impl ValidationRule for L1Eid02 {
    fn id(&self) -> RuleId {
        RuleId::L1Eid02
    }

    fn level(&self) -> Level {
        Level::L1
    }

    fn check(
        &self,
        file: &OmtsFile,
        diags: &mut Vec<Diagnostic>,
        _external_data: Option<&dyn crate::validation::external::ExternalDataSource>,
    ) {
        for node in &file.nodes {
            let Some(identifiers) = &node.identifiers else {
                continue;
            };
            for (idx, ident) in identifiers.iter().enumerate() {
                if ident.value.is_empty() {
                    diags.push(eid_diag(
                        RuleId::L1Eid02,
                        node.id.as_ref(),
                        idx,
                        Some("value"),
                        "identifier `value` must not be empty",
                    ));
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// L1-EID-03: `authority` is present when scheme is `nat-reg`, `vat`, or `internal`
// ---------------------------------------------------------------------------

/// For schemes requiring `authority` (`nat-reg`, `vat`, `internal`), the
/// `authority` field MUST be present and non-empty (SPEC-002 L1-EID-03).
pub struct L1Eid03;

impl ValidationRule for L1Eid03 {
    fn id(&self) -> RuleId {
        RuleId::L1Eid03
    }

    fn level(&self) -> Level {
        Level::L1
    }

    fn check(
        &self,
        file: &OmtsFile,
        diags: &mut Vec<Diagnostic>,
        _external_data: Option<&dyn crate::validation::external::ExternalDataSource>,
    ) {
        for node in &file.nodes {
            let Some(identifiers) = &node.identifiers else {
                continue;
            };
            for (idx, ident) in identifiers.iter().enumerate() {
                if requires_authority(&ident.scheme) {
                    let missing = match &ident.authority {
                        None => true,
                        Some(auth) => auth.is_empty(),
                    };
                    if missing {
                        diags.push(eid_diag(
                            RuleId::L1Eid03,
                            node.id.as_ref(),
                            idx,
                            Some("authority"),
                            format!(
                                "scheme `{}` requires a non-empty `authority` field",
                                ident.scheme
                            ),
                        ));
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// L1-EID-04: `scheme` is a core scheme or reverse-domain extension
// ---------------------------------------------------------------------------

/// `scheme` MUST be either a core scheme code or a valid extension scheme
/// code (reverse-domain notation) (SPEC-002 L1-EID-04).
///
/// Unknown extension schemes (containing a dot) are permitted.  Pure unknown
/// strings without a dot that are not core schemes are rejected.
pub struct L1Eid04;

impl ValidationRule for L1Eid04 {
    fn id(&self) -> RuleId {
        RuleId::L1Eid04
    }

    fn level(&self) -> Level {
        Level::L1
    }

    fn check(
        &self,
        file: &OmtsFile,
        diags: &mut Vec<Diagnostic>,
        _external_data: Option<&dyn crate::validation::external::ExternalDataSource>,
    ) {
        for node in &file.nodes {
            let Some(identifiers) = &node.identifiers else {
                continue;
            };
            for (idx, ident) in identifiers.iter().enumerate() {
                // Skip empty schemes — that is L1-EID-01's concern.
                if ident.scheme.is_empty() {
                    continue;
                }
                if !is_valid_scheme(&ident.scheme) {
                    diags.push(eid_diag(
                        RuleId::L1Eid04,
                        node.id.as_ref(),
                        idx,
                        Some("scheme"),
                        format!(
                            "scheme `{}` is not a recognised core scheme or reverse-domain extension",
                            ident.scheme
                        ),
                    ));
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// L1-EID-05: LEI format + MOD 97-10 check digit
// ---------------------------------------------------------------------------

/// For `lei` scheme: `value` MUST match `^[A-Z0-9]{18}[0-9]{2}$` and MUST
/// pass MOD 97-10 check digit verification (SPEC-002 L1-EID-05).
pub struct L1Eid05;

impl ValidationRule for L1Eid05 {
    fn id(&self) -> RuleId {
        RuleId::L1Eid05
    }

    fn level(&self) -> Level {
        Level::L1
    }

    fn check(
        &self,
        file: &OmtsFile,
        diags: &mut Vec<Diagnostic>,
        _external_data: Option<&dyn crate::validation::external::ExternalDataSource>,
    ) {
        for node in &file.nodes {
            let Some(identifiers) = &node.identifiers else {
                continue;
            };
            for (idx, ident) in identifiers.iter().enumerate() {
                if ident.scheme != "lei" {
                    continue;
                }
                if !LEI_RE.is_match(&ident.value) {
                    diags.push(eid_diag(
                        RuleId::L1Eid05,
                        node.id.as_ref(),
                        idx,
                        Some("value"),
                        format!(
                            "LEI `{}` does not match `^[A-Z0-9]{{18}}[0-9]{{2}}$`",
                            ident.value
                        ),
                    ));
                } else if !mod97_10(&ident.value) {
                    diags.push(eid_diag(
                        RuleId::L1Eid05,
                        node.id.as_ref(),
                        idx,
                        Some("value"),
                        format!(
                            "LEI `{}` fails MOD 97-10 check digit verification",
                            ident.value
                        ),
                    ));
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// L1-EID-06: DUNS format
// ---------------------------------------------------------------------------

/// For `duns` scheme: `value` MUST match `^[0-9]{9}$` (SPEC-002 L1-EID-06).
pub struct L1Eid06;

impl ValidationRule for L1Eid06 {
    fn id(&self) -> RuleId {
        RuleId::L1Eid06
    }

    fn level(&self) -> Level {
        Level::L1
    }

    fn check(
        &self,
        file: &OmtsFile,
        diags: &mut Vec<Diagnostic>,
        _external_data: Option<&dyn crate::validation::external::ExternalDataSource>,
    ) {
        for node in &file.nodes {
            let Some(identifiers) = &node.identifiers else {
                continue;
            };
            for (idx, ident) in identifiers.iter().enumerate() {
                if ident.scheme != "duns" {
                    continue;
                }
                if !DUNS_RE.is_match(&ident.value) {
                    diags.push(eid_diag(
                        RuleId::L1Eid06,
                        node.id.as_ref(),
                        idx,
                        Some("value"),
                        format!("DUNS `{}` does not match `^[0-9]{{9}}$`", ident.value),
                    ));
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// L1-EID-07: GLN format + GS1 mod-10 check digit
// ---------------------------------------------------------------------------

/// For `gln` scheme: `value` MUST match `^[0-9]{13}$` and MUST pass GS1
/// mod-10 check digit verification (SPEC-002 L1-EID-07).
pub struct L1Eid07;

impl ValidationRule for L1Eid07 {
    fn id(&self) -> RuleId {
        RuleId::L1Eid07
    }

    fn level(&self) -> Level {
        Level::L1
    }

    fn check(
        &self,
        file: &OmtsFile,
        diags: &mut Vec<Diagnostic>,
        _external_data: Option<&dyn crate::validation::external::ExternalDataSource>,
    ) {
        for node in &file.nodes {
            let Some(identifiers) = &node.identifiers else {
                continue;
            };
            for (idx, ident) in identifiers.iter().enumerate() {
                if ident.scheme != "gln" {
                    continue;
                }
                if !GLN_RE.is_match(&ident.value) {
                    diags.push(eid_diag(
                        RuleId::L1Eid07,
                        node.id.as_ref(),
                        idx,
                        Some("value"),
                        format!("GLN `{}` does not match `^[0-9]{{13}}$`", ident.value),
                    ));
                } else if !gs1_mod10(&ident.value) {
                    diags.push(eid_diag(
                        RuleId::L1Eid07,
                        node.id.as_ref(),
                        idx,
                        Some("value"),
                        format!(
                            "GLN `{}` fails GS1 mod-10 check digit verification",
                            ident.value
                        ),
                    ));
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// L1-EID-08: `valid_from` / `valid_to` are valid ISO 8601 dates
// ---------------------------------------------------------------------------

/// `valid_from` and `valid_to`, if present, MUST be valid ISO 8601 date
/// strings in `YYYY-MM-DD` format (SPEC-002 L1-EID-08).
///
/// Note: [`crate::newtypes::CalendarDate`] already enforces the `YYYY-MM-DD`
/// shape at deserialization time, so any [`crate::newtypes::CalendarDate`] value in a parsed
/// [`crate::types::Identifier`] is guaranteed to have the correct format.
/// This rule therefore checks the *semantic* calendar validity (e.g. month
/// must be 01–12, day must be within the month's range).
pub struct L1Eid08;

/// Returns `true` if the string `s` (already known to match `YYYY-MM-DD`)
/// represents a semantically valid calendar date.
fn is_calendar_date_valid(s: &str) -> bool {
    // CalendarDate guarantees the regex YYYY-MM-DD matched, so we can index directly.
    let bytes = s.as_bytes();
    // Parse year, month, day without allocating.
    let year = parse_u32_fixed(&bytes[0..4]);
    let month = parse_u32_fixed(&bytes[5..7]);
    let day = parse_u32_fixed(&bytes[8..10]);

    if !(1..=12).contains(&month) {
        return false;
    }
    let max_day = days_in_month(year, month);
    day >= 1 && day <= max_day
}

/// Parses a fixed-width ASCII decimal slice into a `u32`.
fn parse_u32_fixed(bytes: &[u8]) -> u32 {
    let mut n: u32 = 0;
    for &b in bytes {
        n = n * 10 + u32::from(b - b'0');
    }
    n
}

/// Returns the number of days in a given month of a given year.
fn days_in_month(year: u32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap_year(year) {
                29
            } else {
                28
            }
        }
        _ => 0,
    }
}

/// Returns `true` if `year` is a Gregorian leap year.
fn is_leap_year(year: u32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

impl ValidationRule for L1Eid08 {
    fn id(&self) -> RuleId {
        RuleId::L1Eid08
    }

    fn level(&self) -> Level {
        Level::L1
    }

    fn check(
        &self,
        file: &OmtsFile,
        diags: &mut Vec<Diagnostic>,
        _external_data: Option<&dyn crate::validation::external::ExternalDataSource>,
    ) {
        for node in &file.nodes {
            let Some(identifiers) = &node.identifiers else {
                continue;
            };
            for (idx, ident) in identifiers.iter().enumerate() {
                if let Some(vf) = &ident.valid_from {
                    if !is_calendar_date_valid(vf.as_ref()) {
                        diags.push(eid_diag(
                            RuleId::L1Eid08,
                            node.id.as_ref(),
                            idx,
                            Some("valid_from"),
                            format!("`valid_from` `{vf}` is not a valid ISO 8601 date"),
                        ));
                    }
                }
                // valid_to: None = absent, Some(None) = null (no expiry), Some(Some(d)) = date
                if let Some(Some(vt)) = &ident.valid_to {
                    if !is_calendar_date_valid(vt.as_ref()) {
                        diags.push(eid_diag(
                            RuleId::L1Eid08,
                            node.id.as_ref(),
                            idx,
                            Some("valid_to"),
                            format!("`valid_to` `{vt}` is not a valid ISO 8601 date"),
                        ));
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// L1-EID-09: `valid_from` <= `valid_to` when both present
// ---------------------------------------------------------------------------

/// If both `valid_from` and `valid_to` are present, `valid_from` MUST be
/// less than or equal to `valid_to` (SPEC-002 L1-EID-09).
///
/// Since [`crate::newtypes::CalendarDate`] derives `PartialOrd` / `Ord` on
/// its inner `String`, and ISO 8601 `YYYY-MM-DD` strings sort lexicographically
/// the same as chronologically, `a <= b` on `CalendarDate` gives the correct
/// temporal ordering.
pub struct L1Eid09;

impl ValidationRule for L1Eid09 {
    fn id(&self) -> RuleId {
        RuleId::L1Eid09
    }

    fn level(&self) -> Level {
        Level::L1
    }

    fn check(
        &self,
        file: &OmtsFile,
        diags: &mut Vec<Diagnostic>,
        _external_data: Option<&dyn crate::validation::external::ExternalDataSource>,
    ) {
        for node in &file.nodes {
            let Some(identifiers) = &node.identifiers else {
                continue;
            };
            for (idx, ident) in identifiers.iter().enumerate() {
                let Some(vf) = &ident.valid_from else {
                    continue;
                };
                // valid_to must be Some(Some(date)) — if it's None or Some(None) we skip.
                let Some(Some(vt)) = &ident.valid_to else {
                    continue;
                };
                if vf > vt {
                    diags.push(eid_diag(
                        RuleId::L1Eid09,
                        node.id.as_ref(),
                        idx,
                        None,
                        format!("`valid_from` `{vf}` is after `valid_to` `{vt}`"),
                    ));
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// L1-EID-10: `sensitivity` if present is a valid enum value
// ---------------------------------------------------------------------------

/// `sensitivity`, if present, MUST be one of `public`, `restricted`, or
/// `confidential` (SPEC-002 L1-EID-10).
///
/// Since [`crate::enums::Sensitivity`] is the concrete type for the
/// `sensitivity` field and serde rejects unknown variants at deserialization
/// time, any `Identifier` that was successfully parsed already has a valid
/// `Sensitivity` value.  This rule is therefore always satisfied for
/// deserialized data — it is included to satisfy the spec's requirement and
/// to cover identifiers constructed programmatically after the fact if the
/// type system is ever relaxed.
///
/// In practice, serde already enforces this invariant for JSON input.
pub struct L1Eid10;

impl ValidationRule for L1Eid10 {
    fn id(&self) -> RuleId {
        RuleId::L1Eid10
    }

    fn level(&self) -> Level {
        Level::L1
    }

    fn check(
        &self,
        file: &OmtsFile,
        diags: &mut Vec<Diagnostic>,
        _external_data: Option<&dyn crate::validation::external::ExternalDataSource>,
    ) {
        // The `Sensitivity` enum is exhaustively validated by serde at
        // deserialization; any parsed `Identifier` with a non-None sensitivity
        // field already holds a valid variant.  No additional runtime check is
        // needed for deserialized data.
        //
        // This rule is a no-op for correctly typed data.  It is registered in
        // the validator to document that the rule is covered and to provide a
        // hook if the type definition changes in the future.
        let _ = file;
        let _ = diags;
    }
}

// ---------------------------------------------------------------------------
// L1-EID-11: No duplicate {scheme, value, authority} tuples on the same node
// ---------------------------------------------------------------------------

/// No two identifier records on the same node may have identical `scheme`,
/// `value`, and `authority` (SPEC-002 L1-EID-11).
pub struct L1Eid11;

impl ValidationRule for L1Eid11 {
    fn id(&self) -> RuleId {
        RuleId::L1Eid11
    }

    fn level(&self) -> Level {
        Level::L1
    }

    fn check(
        &self,
        file: &OmtsFile,
        diags: &mut Vec<Diagnostic>,
        _external_data: Option<&dyn crate::validation::external::ExternalDataSource>,
    ) {
        for node in &file.nodes {
            let Some(identifiers) = &node.identifiers else {
                continue;
            };

            // Collect (scheme, value, authority) tuples to detect duplicates.
            // We report every duplicate index that produces a collision.
            let mut seen: HashSet<(&str, &str, Option<&str>)> = HashSet::new();

            for (idx, ident) in identifiers.iter().enumerate() {
                let key = (
                    ident.scheme.as_str(),
                    ident.value.as_str(),
                    ident.authority.as_deref(),
                );
                if !seen.insert(key) {
                    diags.push(eid_diag(
                        RuleId::L1Eid11,
                        node.id.as_ref(),
                        idx,
                        None,
                        format!(
                            "duplicate identifier tuple (scheme=`{}`, value=`{}`, authority={:?})",
                            ident.scheme, ident.value, ident.authority
                        ),
                    ));
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use crate::enums::{NodeType, NodeTypeTag, Sensitivity};
    use crate::newtypes::{CalendarDate, FileSalt, NodeId, SemVer};
    use crate::structures::Node;
    use crate::types::Identifier;
    use crate::validation::{ValidationConfig, build_registry, validate};

    const SALT: &str = "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef";

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn minimal_file() -> OmtsFile {
        OmtsFile {
            omtsf_version: SemVer::try_from("1.0.0").expect("semver"),
            snapshot_date: CalendarDate::try_from("2026-02-19").expect("date"),
            file_salt: FileSalt::try_from(SALT).expect("salt"),
            disclosure_scope: None,
            previous_snapshot_ref: None,
            snapshot_sequence: None,
            reporting_entity: None,
            nodes: vec![],
            edges: vec![],
            extra: serde_json::Map::new(),
        }
    }

    fn org_node(id: &str) -> Node {
        Node {
            id: NodeId::try_from(id).expect("node id"),
            node_type: NodeTypeTag::Known(NodeType::Organization),
            identifiers: None,
            data_quality: None,
            labels: None,
            name: None,
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
            extra: serde_json::Map::new(),
        }
    }

    fn basic_ident(scheme: &str, value: &str) -> Identifier {
        Identifier {
            scheme: scheme.to_owned(),
            value: value.to_owned(),
            authority: None,
            valid_from: None,
            valid_to: None,
            sensitivity: None,
            verification_status: None,
            verification_date: None,
            extra: serde_json::Map::new(),
        }
    }

    fn ident_with_authority(scheme: &str, value: &str, authority: &str) -> Identifier {
        Identifier {
            authority: Some(authority.to_owned()),
            ..basic_ident(scheme, value)
        }
    }

    fn ident_with_dates(
        scheme: &str,
        value: &str,
        valid_from: &str,
        valid_to: Option<&str>,
    ) -> Identifier {
        Identifier {
            valid_from: Some(CalendarDate::try_from(valid_from).expect("date")),
            valid_to: valid_to.map(|d| Some(CalendarDate::try_from(d).expect("date"))),
            ..basic_ident(scheme, value)
        }
    }

    fn check_rule(rule: &dyn ValidationRule, file: &OmtsFile) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        rule.check(file, &mut diags, None);
        diags
    }

    // -----------------------------------------------------------------------
    // Registry integration: L1-EID rules are included when run_l1 = true
    // -----------------------------------------------------------------------

    #[test]
    fn registry_includes_l1_eid_rules_when_run_l1() {
        let cfg = ValidationConfig::default();
        let registry = build_registry(&cfg);
        let ids: Vec<RuleId> = registry.iter().map(|r| r.id()).collect();
        assert!(
            ids.contains(&RuleId::L1Eid01),
            "L1-EID-01 must be in registry"
        );
        assert!(
            ids.contains(&RuleId::L1Eid11),
            "L1-EID-11 must be in registry"
        );
    }

    #[test]
    fn registry_excludes_l1_eid_rules_when_run_l1_false() {
        let cfg = ValidationConfig {
            run_l1: false,
            run_l2: false,
            run_l3: false,
        };
        let registry = build_registry(&cfg);
        let ids: Vec<RuleId> = registry.iter().map(|r| r.id()).collect();
        assert!(!ids.contains(&RuleId::L1Eid01));
        assert!(!ids.contains(&RuleId::L1Eid11));
    }

    // -----------------------------------------------------------------------
    // L1-EID-01
    // -----------------------------------------------------------------------

    #[test]
    fn l1_eid_01_pass_non_empty_scheme() {
        let rule = L1Eid01;
        let mut file = minimal_file();
        let mut node = org_node("n1");
        node.identifiers = Some(vec![basic_ident("lei", "5493006MHB84DD0ZWV18")]);
        file.nodes.push(node);
        assert!(check_rule(&rule, &file).is_empty());
    }

    #[test]
    fn l1_eid_01_fail_empty_scheme() {
        let rule = L1Eid01;
        let mut file = minimal_file();
        let mut node = org_node("n1");
        node.identifiers = Some(vec![basic_ident("", "some-value")]);
        file.nodes.push(node);
        let diags = check_rule(&rule, &file);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule_id, RuleId::L1Eid01);
    }

    #[test]
    fn l1_eid_01_pass_no_identifiers() {
        let rule = L1Eid01;
        let file = minimal_file();
        assert!(check_rule(&rule, &file).is_empty());
    }

    // -----------------------------------------------------------------------
    // L1-EID-02
    // -----------------------------------------------------------------------

    #[test]
    fn l1_eid_02_pass_non_empty_value() {
        let rule = L1Eid02;
        let mut file = minimal_file();
        let mut node = org_node("n1");
        node.identifiers = Some(vec![basic_ident("duns", "123456789")]);
        file.nodes.push(node);
        assert!(check_rule(&rule, &file).is_empty());
    }

    #[test]
    fn l1_eid_02_fail_empty_value() {
        let rule = L1Eid02;
        let mut file = minimal_file();
        let mut node = org_node("n1");
        node.identifiers = Some(vec![basic_ident("duns", "")]);
        file.nodes.push(node);
        let diags = check_rule(&rule, &file);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule_id, RuleId::L1Eid02);
    }

    // -----------------------------------------------------------------------
    // L1-EID-03
    // -----------------------------------------------------------------------

    #[test]
    fn l1_eid_03_pass_nat_reg_with_authority() {
        let rule = L1Eid03;
        let mut file = minimal_file();
        let mut node = org_node("n1");
        node.identifiers = Some(vec![ident_with_authority(
            "nat-reg", "HRB86891", "RA000548",
        )]);
        file.nodes.push(node);
        assert!(check_rule(&rule, &file).is_empty());
    }

    #[test]
    fn l1_eid_03_fail_nat_reg_missing_authority() {
        let rule = L1Eid03;
        let mut file = minimal_file();
        let mut node = org_node("n1");
        node.identifiers = Some(vec![basic_ident("nat-reg", "HRB86891")]);
        file.nodes.push(node);
        let diags = check_rule(&rule, &file);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule_id, RuleId::L1Eid03);
    }

    #[test]
    fn l1_eid_03_fail_vat_missing_authority() {
        let rule = L1Eid03;
        let mut file = minimal_file();
        let mut node = org_node("n1");
        node.identifiers = Some(vec![basic_ident("vat", "DE123456789")]);
        file.nodes.push(node);
        let diags = check_rule(&rule, &file);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule_id, RuleId::L1Eid03);
    }

    #[test]
    fn l1_eid_03_fail_internal_missing_authority() {
        let rule = L1Eid03;
        let mut file = minimal_file();
        let mut node = org_node("n1");
        node.identifiers = Some(vec![basic_ident("internal", "V-100234")]);
        file.nodes.push(node);
        let diags = check_rule(&rule, &file);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule_id, RuleId::L1Eid03);
    }

    #[test]
    fn l1_eid_03_fail_empty_authority() {
        let rule = L1Eid03;
        let mut file = minimal_file();
        let mut node = org_node("n1");
        node.identifiers = Some(vec![ident_with_authority("internal", "V-100234", "")]);
        file.nodes.push(node);
        let diags = check_rule(&rule, &file);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule_id, RuleId::L1Eid03);
    }

    #[test]
    fn l1_eid_03_pass_lei_no_authority_required() {
        // lei does not require authority
        let rule = L1Eid03;
        let mut file = minimal_file();
        let mut node = org_node("n1");
        node.identifiers = Some(vec![basic_ident("lei", "5493006MHB84DD0ZWV18")]);
        file.nodes.push(node);
        assert!(check_rule(&rule, &file).is_empty());
    }

    // -----------------------------------------------------------------------
    // L1-EID-04
    // -----------------------------------------------------------------------

    #[test]
    fn l1_eid_04_pass_core_scheme() {
        let rule = L1Eid04;
        let mut file = minimal_file();
        let mut node = org_node("n1");
        node.identifiers = Some(vec![basic_ident("gln", "0614141000418")]);
        file.nodes.push(node);
        assert!(check_rule(&rule, &file).is_empty());
    }

    #[test]
    fn l1_eid_04_pass_extension_scheme_with_dot() {
        let rule = L1Eid04;
        let mut file = minimal_file();
        let mut node = org_node("n1");
        node.identifiers = Some(vec![basic_ident("com.example.supplier-id", "S-12345")]);
        file.nodes.push(node);
        assert!(check_rule(&rule, &file).is_empty());
    }

    #[test]
    fn l1_eid_04_fail_unknown_scheme_no_dot() {
        let rule = L1Eid04;
        let mut file = minimal_file();
        let mut node = org_node("n1");
        node.identifiers = Some(vec![basic_ident("mystery", "some-value")]);
        file.nodes.push(node);
        let diags = check_rule(&rule, &file);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule_id, RuleId::L1Eid04);
    }

    // -----------------------------------------------------------------------
    // L1-EID-05
    // -----------------------------------------------------------------------

    #[test]
    fn l1_eid_05_pass_valid_lei() {
        let rule = L1Eid05;
        let mut file = minimal_file();
        let mut node = org_node("n1");
        // Known-valid LEI (BIS)
        node.identifiers = Some(vec![basic_ident("lei", "5493006MHB84DD0ZWV18")]);
        file.nodes.push(node);
        assert!(check_rule(&rule, &file).is_empty());
    }

    #[test]
    fn l1_eid_05_fail_wrong_format() {
        let rule = L1Eid05;
        let mut file = minimal_file();
        let mut node = org_node("n1");
        // Wrong format: lowercase letters
        node.identifiers = Some(vec![basic_ident("lei", "5493006mhb84dd0zwv18")]);
        file.nodes.push(node);
        let diags = check_rule(&rule, &file);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule_id, RuleId::L1Eid05);
    }

    #[test]
    fn l1_eid_05_fail_bad_check_digit() {
        let rule = L1Eid05;
        let mut file = minimal_file();
        let mut node = org_node("n1");
        // Correct format but corrupted check digit
        node.identifiers = Some(vec![basic_ident("lei", "5493006MHB84DD0ZWV19")]);
        file.nodes.push(node);
        let diags = check_rule(&rule, &file);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule_id, RuleId::L1Eid05);
    }

    #[test]
    fn l1_eid_05_pass_second_valid_lei() {
        let rule = L1Eid05;
        let mut file = minimal_file();
        let mut node = org_node("n1");
        // Apple Inc. LEI
        node.identifiers = Some(vec![basic_ident("lei", "HWUPKR0MPOU8FGXBT394")]);
        file.nodes.push(node);
        assert!(check_rule(&rule, &file).is_empty());
    }

    // -----------------------------------------------------------------------
    // L1-EID-06
    // -----------------------------------------------------------------------

    #[test]
    fn l1_eid_06_pass_valid_duns() {
        let rule = L1Eid06;
        let mut file = minimal_file();
        let mut node = org_node("n1");
        node.identifiers = Some(vec![basic_ident("duns", "081466849")]);
        file.nodes.push(node);
        assert!(check_rule(&rule, &file).is_empty());
    }

    #[test]
    fn l1_eid_06_fail_too_short() {
        let rule = L1Eid06;
        let mut file = minimal_file();
        let mut node = org_node("n1");
        node.identifiers = Some(vec![basic_ident("duns", "12345678")]);
        file.nodes.push(node);
        let diags = check_rule(&rule, &file);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule_id, RuleId::L1Eid06);
    }

    #[test]
    fn l1_eid_06_fail_non_numeric() {
        let rule = L1Eid06;
        let mut file = minimal_file();
        let mut node = org_node("n1");
        node.identifiers = Some(vec![basic_ident("duns", "08146684X")]);
        file.nodes.push(node);
        let diags = check_rule(&rule, &file);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule_id, RuleId::L1Eid06);
    }

    // -----------------------------------------------------------------------
    // L1-EID-07
    // -----------------------------------------------------------------------

    #[test]
    fn l1_eid_07_pass_valid_gln() {
        let rule = L1Eid07;
        let mut file = minimal_file();
        let mut node = org_node("n1");
        node.identifiers = Some(vec![basic_ident("gln", "0614141000418")]);
        file.nodes.push(node);
        assert!(check_rule(&rule, &file).is_empty());
    }

    #[test]
    fn l1_eid_07_fail_wrong_length() {
        let rule = L1Eid07;
        let mut file = minimal_file();
        let mut node = org_node("n1");
        // Only 12 digits
        node.identifiers = Some(vec![basic_ident("gln", "061414100041")]);
        file.nodes.push(node);
        let diags = check_rule(&rule, &file);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule_id, RuleId::L1Eid07);
    }

    #[test]
    fn l1_eid_07_fail_bad_check_digit() {
        let rule = L1Eid07;
        let mut file = minimal_file();
        let mut node = org_node("n1");
        // Correct format but wrong check digit
        node.identifiers = Some(vec![basic_ident("gln", "0614141000419")]);
        file.nodes.push(node);
        let diags = check_rule(&rule, &file);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule_id, RuleId::L1Eid07);
    }

    // -----------------------------------------------------------------------
    // L1-EID-08
    // -----------------------------------------------------------------------

    #[test]
    fn l1_eid_08_pass_valid_dates() {
        let rule = L1Eid08;
        let mut file = minimal_file();
        let mut node = org_node("n1");
        node.identifiers = Some(vec![ident_with_dates(
            "lei",
            "5493006MHB84DD0ZWV18",
            "2020-01-01",
            Some("2026-12-31"),
        )]);
        file.nodes.push(node);
        assert!(check_rule(&rule, &file).is_empty());
    }

    #[test]
    fn l1_eid_08_fail_invalid_month() {
        // We need to construct an Identifier with an out-of-range month.
        // CalendarDate allows "YYYY-MM-DD" shape but not semantic validation.
        // Month 13 passes regex but fails calendar check.
        let rule = L1Eid08;
        let mut file = minimal_file();
        let mut node = org_node("n1");
        // Directly create a CalendarDate with invalid month via try_from
        // (the regex only checks shape, not calendar validity).
        // "2026-13-01" passes the regex but fails L1-EID-08.
        let bad_date = CalendarDate::try_from("2026-13-01").expect("shape ok");
        node.identifiers = Some(vec![Identifier {
            valid_from: Some(bad_date),
            ..basic_ident("internal", "V-100")
        }]);
        // Add authority since internal requires it
        let ident = node.identifiers.as_mut().expect("set above");
        ident[0].authority = Some("sap-prod".to_owned());
        file.nodes.push(node);
        let diags = check_rule(&rule, &file);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule_id, RuleId::L1Eid08);
    }

    #[test]
    fn l1_eid_08_fail_invalid_day() {
        let rule = L1Eid08;
        let mut file = minimal_file();
        let mut node = org_node("n1");
        // February 30 is never valid
        let bad_date = CalendarDate::try_from("2026-02-30").expect("shape ok");
        node.identifiers = Some(vec![Identifier {
            valid_from: Some(bad_date),
            authority: Some("sap-prod".to_owned()),
            ..basic_ident("internal", "V-100")
        }]);
        file.nodes.push(node);
        let diags = check_rule(&rule, &file);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule_id, RuleId::L1Eid08);
    }

    #[test]
    fn l1_eid_08_pass_leap_day() {
        let rule = L1Eid08;
        let mut file = minimal_file();
        let mut node = org_node("n1");
        // 2024 is a leap year; Feb 29 is valid
        let leap_date = CalendarDate::try_from("2024-02-29").expect("shape ok");
        node.identifiers = Some(vec![Identifier {
            valid_from: Some(leap_date),
            authority: Some("sap-prod".to_owned()),
            ..basic_ident("internal", "V-100")
        }]);
        file.nodes.push(node);
        assert!(check_rule(&rule, &file).is_empty());
    }

    #[test]
    fn l1_eid_08_fail_non_leap_feb_29() {
        let rule = L1Eid08;
        let mut file = minimal_file();
        let mut node = org_node("n1");
        // 2026 is not a leap year; Feb 29 is invalid
        let bad_date = CalendarDate::try_from("2026-02-29").expect("shape ok");
        node.identifiers = Some(vec![Identifier {
            valid_from: Some(bad_date),
            authority: Some("sap-prod".to_owned()),
            ..basic_ident("internal", "V-100")
        }]);
        file.nodes.push(node);
        let diags = check_rule(&rule, &file);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule_id, RuleId::L1Eid08);
    }

    // -----------------------------------------------------------------------
    // L1-EID-09
    // -----------------------------------------------------------------------

    #[test]
    fn l1_eid_09_pass_from_before_to() {
        let rule = L1Eid09;
        let mut file = minimal_file();
        let mut node = org_node("n1");
        node.identifiers = Some(vec![ident_with_dates(
            "lei",
            "5493006MHB84DD0ZWV18",
            "2020-01-01",
            Some("2026-12-31"),
        )]);
        file.nodes.push(node);
        assert!(check_rule(&rule, &file).is_empty());
    }

    #[test]
    fn l1_eid_09_pass_from_equal_to() {
        let rule = L1Eid09;
        let mut file = minimal_file();
        let mut node = org_node("n1");
        node.identifiers = Some(vec![ident_with_dates(
            "lei",
            "5493006MHB84DD0ZWV18",
            "2020-01-01",
            Some("2020-01-01"),
        )]);
        file.nodes.push(node);
        assert!(check_rule(&rule, &file).is_empty());
    }

    #[test]
    fn l1_eid_09_fail_from_after_to() {
        let rule = L1Eid09;
        let mut file = minimal_file();
        let mut node = org_node("n1");
        node.identifiers = Some(vec![ident_with_dates(
            "lei",
            "5493006MHB84DD0ZWV18",
            "2026-12-31",
            Some("2020-01-01"),
        )]);
        file.nodes.push(node);
        let diags = check_rule(&rule, &file);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule_id, RuleId::L1Eid09);
    }

    #[test]
    fn l1_eid_09_pass_valid_to_null_no_expiry() {
        // valid_to: null means no expiry — ordering rule does not apply.
        let rule = L1Eid09;
        let mut file = minimal_file();
        let mut node = org_node("n1");
        node.identifiers = Some(vec![Identifier {
            valid_from: Some(CalendarDate::try_from("2020-01-01").expect("date")),
            valid_to: Some(None), // explicit null: no expiry
            ..basic_ident("lei", "5493006MHB84DD0ZWV18")
        }]);
        file.nodes.push(node);
        assert!(check_rule(&rule, &file).is_empty());
    }

    // -----------------------------------------------------------------------
    // L1-EID-10
    // -----------------------------------------------------------------------

    #[test]
    fn l1_eid_10_pass_valid_sensitivity() {
        let rule = L1Eid10;
        let mut file = minimal_file();
        let mut node = org_node("n1");
        node.identifiers = Some(vec![Identifier {
            sensitivity: Some(Sensitivity::Restricted),
            ..basic_ident("vat", "DE123456789")
        }]);
        if let Some(ids) = &mut node.identifiers {
            ids[0].authority = Some("DE".to_owned());
        }
        file.nodes.push(node);
        // L1-EID-10 is a no-op for type-safe data
        assert!(check_rule(&rule, &file).is_empty());
    }

    #[test]
    fn l1_eid_10_pass_no_sensitivity() {
        let rule = L1Eid10;
        let mut file = minimal_file();
        let mut node = org_node("n1");
        node.identifiers = Some(vec![basic_ident("duns", "081466849")]);
        file.nodes.push(node);
        assert!(check_rule(&rule, &file).is_empty());
    }

    // -----------------------------------------------------------------------
    // L1-EID-11
    // -----------------------------------------------------------------------

    #[test]
    fn l1_eid_11_pass_unique_tuples() {
        let rule = L1Eid11;
        let mut file = minimal_file();
        let mut node = org_node("n1");
        node.identifiers = Some(vec![
            basic_ident("lei", "5493006MHB84DD0ZWV18"),
            basic_ident("duns", "081466849"),
        ]);
        file.nodes.push(node);
        assert!(check_rule(&rule, &file).is_empty());
    }

    #[test]
    fn l1_eid_11_fail_duplicate_tuple() {
        let rule = L1Eid11;
        let mut file = minimal_file();
        let mut node = org_node("n1");
        node.identifiers = Some(vec![
            basic_ident("lei", "5493006MHB84DD0ZWV18"),
            basic_ident("lei", "5493006MHB84DD0ZWV18"), // exact duplicate
        ]);
        file.nodes.push(node);
        let diags = check_rule(&rule, &file);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule_id, RuleId::L1Eid11);
    }

    #[test]
    fn l1_eid_11_pass_same_scheme_value_different_authority() {
        // Distinct authority makes tuples different — no duplicate.
        let rule = L1Eid11;
        let mut file = minimal_file();
        let mut node = org_node("n1");
        node.identifiers = Some(vec![
            ident_with_authority("internal", "V-100", "sap-prod"),
            ident_with_authority("internal", "V-100", "oracle-scm"),
        ]);
        file.nodes.push(node);
        assert!(check_rule(&rule, &file).is_empty());
    }

    #[test]
    fn l1_eid_11_fail_duplicate_with_authority() {
        let rule = L1Eid11;
        let mut file = minimal_file();
        let mut node = org_node("n1");
        node.identifiers = Some(vec![
            ident_with_authority("internal", "V-100", "sap-prod"),
            ident_with_authority("internal", "V-100", "sap-prod"), // exact duplicate
        ]);
        file.nodes.push(node);
        let diags = check_rule(&rule, &file);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule_id, RuleId::L1Eid11);
    }

    // -----------------------------------------------------------------------
    // End-to-end via validate()
    // -----------------------------------------------------------------------

    #[test]
    fn validate_clean_file_no_eid_errors() {
        let mut file = minimal_file();
        let mut node = org_node("n1");
        node.identifiers = Some(vec![
            basic_ident("lei", "5493006MHB84DD0ZWV18"),
            basic_ident("duns", "081466849"),
            basic_ident("gln", "0614141000418"),
        ]);
        file.nodes.push(node);
        let cfg = ValidationConfig::default();
        let result = validate(&file, &cfg, None);
        let eid_errors: Vec<_> = result
            .errors()
            .filter(|d| {
                matches!(
                    d.rule_id,
                    RuleId::L1Eid01
                        | RuleId::L1Eid02
                        | RuleId::L1Eid03
                        | RuleId::L1Eid04
                        | RuleId::L1Eid05
                        | RuleId::L1Eid06
                        | RuleId::L1Eid07
                        | RuleId::L1Eid08
                        | RuleId::L1Eid09
                        | RuleId::L1Eid10
                        | RuleId::L1Eid11
                )
            })
            .collect();
        assert!(
            eid_errors.is_empty(),
            "clean file should have no L1-EID errors: {eid_errors:?}"
        );
    }

    #[test]
    fn validate_multiple_violations_all_collected() {
        let mut file = minimal_file();
        let mut node = org_node("n1");
        node.identifiers = Some(vec![
            basic_ident("", "some-value"),     // L1-EID-01
            basic_ident("duns", ""),           // L1-EID-02
            basic_ident("nat-reg", "HRB1234"), // L1-EID-03
        ]);
        file.nodes.push(node);
        let cfg = ValidationConfig::default();
        let result = validate(&file, &cfg, None);
        assert!(result.has_errors());
        let ids: Vec<&RuleId> = result.errors().map(|d| &d.rule_id).collect();
        assert!(ids.contains(&&RuleId::L1Eid01));
        assert!(ids.contains(&&RuleId::L1Eid02));
        assert!(ids.contains(&&RuleId::L1Eid03));
    }

    // -----------------------------------------------------------------------
    // Calendar date helper unit tests
    // -----------------------------------------------------------------------

    #[test]
    fn is_leap_year_examples() {
        assert!(is_leap_year(2000)); // divisible by 400
        assert!(is_leap_year(2024)); // divisible by 4, not 100
        assert!(!is_leap_year(1900)); // divisible by 100 but not 400
        assert!(!is_leap_year(2026)); // not divisible by 4
    }

    #[test]
    fn days_in_month_examples() {
        assert_eq!(days_in_month(2026, 1), 31);
        assert_eq!(days_in_month(2026, 2), 28);
        assert_eq!(days_in_month(2024, 2), 29);
        assert_eq!(days_in_month(2026, 4), 30);
        assert_eq!(days_in_month(2026, 12), 31);
    }

    #[test]
    fn is_calendar_date_valid_examples() {
        assert!(is_calendar_date_valid("2026-02-19"));
        assert!(is_calendar_date_valid("2024-02-29")); // leap year
        assert!(!is_calendar_date_valid("2026-02-29")); // not a leap year
        assert!(!is_calendar_date_valid("2026-13-01")); // month 13
        assert!(!is_calendar_date_valid("2026-04-31")); // April has 30 days
        assert!(is_calendar_date_valid("2026-12-31"));
    }
}
