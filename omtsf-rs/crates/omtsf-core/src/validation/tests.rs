#![allow(clippy::expect_used)]
#![allow(clippy::panic)]

use super::*;
use std::collections::BTreeMap;

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

#[test]
fn level_severity_mapping() {
    assert_eq!(Level::L1.severity(), Severity::Error);
    assert_eq!(Level::L2.severity(), Severity::Warning);
    assert_eq!(Level::L3.severity(), Severity::Info);
}

#[test]
fn level_display() {
    assert_eq!(Level::L1.to_string(), "L1");
    assert_eq!(Level::L2.to_string(), "L2");
    assert_eq!(Level::L3.to_string(), "L3");
}

#[test]
fn level_clone_and_eq() {
    assert_eq!(Level::L1, Level::L1.clone());
    assert_ne!(Level::L1, Level::L2);
    assert_ne!(Level::L2, Level::L3);
}

#[test]
fn validation_config_default() {
    let cfg = ValidationConfig::default();
    assert!(cfg.run_l1);
    assert!(cfg.run_l2);
    assert!(!cfg.run_l3);
}

#[test]
fn validation_config_clone_and_eq() {
    let cfg = ValidationConfig::default();
    assert_eq!(cfg, cfg.clone());
    let cfg2 = ValidationConfig {
        run_l1: true,
        run_l2: false,
        run_l3: true,
    };
    assert_ne!(cfg, cfg2);
}

#[test]
fn build_registry_default_config_has_l1_rules() {
    let cfg = ValidationConfig::default();
    let registry = build_registry(&cfg);
    assert!(
        !registry.is_empty(),
        "default config must include L1-GDM, L1-EID, and L1-SDI rules"
    );
    let ids: Vec<_> = registry.iter().map(|r| r.id()).collect();
    assert!(ids.contains(&RuleId::L1Gdm01));
    assert!(ids.contains(&RuleId::L1Gdm06));
    assert!(ids.contains(&RuleId::L1Eid01));
    assert!(ids.contains(&RuleId::L1Eid11));
    assert!(ids.contains(&RuleId::L1Sdi01));
    assert!(ids.contains(&RuleId::L1Sdi02));
}

#[test]
fn build_registry_all_levels_disabled_is_empty() {
    let cfg = ValidationConfig {
        run_l1: false,
        run_l2: false,
        run_l3: false,
    };
    let registry = build_registry(&cfg);
    assert!(registry.is_empty());
}

#[test]
fn build_registry_l1_only_has_nineteen_rules() {
    let cfg = ValidationConfig {
        run_l1: true,
        run_l2: false,
        run_l3: false,
    };
    let registry = build_registry(&cfg);
    assert_eq!(
        registry.len(),
        19,
        "6 L1-GDM + 11 L1-EID + 2 L1-SDI rules in the registry"
    );
}

#[test]
fn build_registry_default_config_includes_l2_rules() {
    let cfg = ValidationConfig::default();
    let registry = build_registry(&cfg);
    let ids: Vec<_> = registry.iter().map(|r| r.id()).collect();
    assert!(
        ids.contains(&RuleId::L2Gdm01),
        "L2-GDM-01 must be in registry"
    );
    assert!(
        ids.contains(&RuleId::L2Gdm02),
        "L2-GDM-02 must be in registry"
    );
    assert!(
        ids.contains(&RuleId::L2Gdm03),
        "L2-GDM-03 must be in registry"
    );
    assert!(
        ids.contains(&RuleId::L2Gdm04),
        "L2-GDM-04 must be in registry"
    );
    assert!(
        ids.contains(&RuleId::L2Eid01),
        "L2-EID-01 must be in registry"
    );
    assert!(
        ids.contains(&RuleId::L2Eid04),
        "L2-EID-04 must be in registry"
    );
}

#[test]
fn build_registry_l2_only_has_six_rules() {
    let cfg = ValidationConfig {
        run_l1: false,
        run_l2: true,
        run_l3: false,
    };
    let registry = build_registry(&cfg);
    assert_eq!(
        registry.len(),
        6,
        "L2-GDM-01..04 + L2-EID-01, L2-EID-04 = 6 L2 rules"
    );
    assert!(
        registry.iter().all(|r| r.level() == Level::L2),
        "all rules in L2-only registry must be L2 level"
    );
}

#[test]
fn build_registry_l3_only_has_three_rules() {
    let cfg = ValidationConfig {
        run_l1: false,
        run_l2: false,
        run_l3: true,
    };
    let registry = build_registry(&cfg);
    assert_eq!(
        registry.len(),
        3,
        "L3-EID-01 + L3-MRG-01 + L3-MRG-02 = 3 L3 rules registered when run_l3 is true"
    );
    let ids: Vec<_> = registry.iter().map(|r| r.id()).collect();
    assert!(
        ids.contains(&RuleId::L3Eid01),
        "L3-EID-01 must be in registry"
    );
    assert!(
        ids.contains(&RuleId::L3Mrg01),
        "L3-MRG-01 must be in registry"
    );
    assert!(
        ids.contains(&RuleId::L3Mrg02),
        "L3-MRG-02 must be in registry"
    );
    assert!(
        registry.iter().all(|r| r.level() == Level::L3),
        "all rules in L3-only registry must be L3 level"
    );
}

#[test]
fn build_registry_l3_rules_produce_info_severity() {
    let cfg = ValidationConfig {
        run_l1: false,
        run_l2: false,
        run_l3: true,
    };
    let registry = build_registry(&cfg);
    assert!(
        registry.iter().all(|r| r.severity() == Severity::Info),
        "all L3 rules must produce Info severity"
    );
}

/// Helper: build a minimal valid [`OmtsFile`] in-memory.
fn minimal_omts_file() -> crate::file::OmtsFile {
    use crate::newtypes::{CalendarDate, FileSalt, SemVer};

    const SALT: &str = "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef";
    crate::file::OmtsFile {
        omtsf_version: SemVer::try_from("1.0.0").expect("valid SemVer"),
        snapshot_date: CalendarDate::try_from("2026-02-19").expect("valid date"),
        file_salt: FileSalt::try_from(SALT).expect("valid salt"),
        disclosure_scope: None,
        previous_snapshot_ref: None,
        snapshot_sequence: None,
        reporting_entity: None,
        nodes: vec![],
        edges: vec![],
        extra: BTreeMap::new(),
    }
}

#[test]
fn validate_clean_minimal_file_produces_zero_diagnostics() {
    let file = minimal_omts_file();
    let cfg = ValidationConfig::default();
    let result = validate(&file, &cfg, None);
    assert!(
        result.is_empty(),
        "clean minimal file must produce no diagnostics; got: {:?}",
        result.diagnostics
    );
}

#[test]
fn validate_returns_validation_result() {
    let file = minimal_omts_file();
    let cfg = ValidationConfig {
        run_l1: false,
        run_l2: false,
        run_l3: false,
    };
    let result = validate(&file, &cfg, None);
    assert_eq!(result.len(), 0);
}

/// A mock rule that always emits one diagnostic.
struct MockRule {
    rule_id: RuleId,
    level: Level,
}

impl ValidationRule for MockRule {
    fn id(&self) -> RuleId {
        self.rule_id.clone()
    }

    fn level(&self) -> Level {
        self.level
    }

    fn check(
        &self,
        _file: &OmtsFile,
        diags: &mut Vec<Diagnostic>,
        _external_data: Option<&dyn crate::validation::external::ExternalDataSource>,
    ) {
        diags.push(Diagnostic::new(
            self.rule_id.clone(),
            self.level.severity(),
            Location::Global,
            "mock diagnostic",
        ));
    }
}

#[test]
fn mock_rule_diagnostic_appears_in_results() {
    let file = minimal_omts_file();
    let rule: Box<dyn ValidationRule> = Box::new(MockRule {
        rule_id: RuleId::L1Gdm01,
        level: Level::L1,
    });

    let mut diags: Vec<Diagnostic> = Vec::new();
    rule.check(&file, &mut diags, None);

    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].rule_id, RuleId::L1Gdm01);
    assert_eq!(diags[0].severity, Severity::Error);
    assert_eq!(diags[0].location, Location::Global);
    assert_eq!(diags[0].message, "mock diagnostic");
}

#[test]
fn mock_rule_severity_derived_from_level() {
    let l1_rule = MockRule {
        rule_id: RuleId::L1Gdm01,
        level: Level::L1,
    };
    assert_eq!(l1_rule.severity(), Severity::Error);

    let l2_rule = MockRule {
        rule_id: RuleId::L2Gdm01,
        level: Level::L2,
    };
    assert_eq!(l2_rule.severity(), Severity::Warning);

    let l3_rule = MockRule {
        rule_id: RuleId::L3Mrg01,
        level: Level::L3,
    };
    assert_eq!(l3_rule.severity(), Severity::Info);
}
