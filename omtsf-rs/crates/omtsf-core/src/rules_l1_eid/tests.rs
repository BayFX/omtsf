#![allow(clippy::expect_used)]

use super::{
    L1Eid01, L1Eid02, L1Eid03, L1Eid04, L1Eid05, L1Eid06, L1Eid07, L1Eid08, L1Eid09, L1Eid10,
    L1Eid11, days_in_month, is_calendar_date_valid, is_leap_year,
};
use crate::enums::{NodeType, NodeTypeTag, Sensitivity};
use crate::file::OmtsFile;
use crate::newtypes::{CalendarDate, FileSalt, NodeId, SemVer};
use crate::structures::Node;
use crate::types::Identifier;
use crate::validation::{
    Diagnostic, RuleId, ValidationConfig, ValidationRule, build_registry, validate,
};
use std::collections::BTreeMap;

const SALT: &str = "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef";

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
        extra: BTreeMap::new(),
    }
}

fn org_node(id: &str) -> Node {
    Node {
        id: NodeId::try_from(id).expect("node id"),
        node_type: NodeTypeTag::Known(NodeType::Organization),
        ..Node::default()
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
        extra: BTreeMap::new(),
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
