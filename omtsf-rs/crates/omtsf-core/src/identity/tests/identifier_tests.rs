#![allow(clippy::expect_used)]

use crate::dynvalue::DynValue;
use serde_json::json;
use std::collections::BTreeMap;

use crate::newtypes::CalendarDate;

use crate::identity::{identifiers_match, is_lei_annulled, temporal_compatible};

pub(super) fn make_id(scheme: &str, value: &str) -> crate::types::Identifier {
    crate::types::Identifier {
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

pub(super) fn with_authority(
    mut id: crate::types::Identifier,
    authority: &str,
) -> crate::types::Identifier {
    id.authority = Some(authority.to_owned());
    id
}

pub(super) fn with_valid_from(
    mut id: crate::types::Identifier,
    date: &str,
) -> crate::types::Identifier {
    id.valid_from = Some(CalendarDate::try_from(date).expect("valid date"));
    id
}

pub(super) fn with_valid_to_date(
    mut id: crate::types::Identifier,
    date: &str,
) -> crate::types::Identifier {
    id.valid_to = Some(Some(CalendarDate::try_from(date).expect("valid date")));
    id
}

pub(super) fn with_valid_to_null(mut id: crate::types::Identifier) -> crate::types::Identifier {
    id.valid_to = Some(None);
    id
}

#[test]
fn same_scheme_and_value_matches() {
    let a = make_id("lei", "LEI_ACME");
    let b = make_id("lei", "LEI_ACME");
    assert!(identifiers_match(&a, &b));
}

#[test]
fn different_scheme_rejects() {
    let a = make_id("lei", "VALUE");
    let b = make_id("duns", "VALUE");
    assert!(!identifiers_match(&a, &b));
}

#[test]
fn internal_scheme_on_a_rejects() {
    let a = make_id("internal", "sap:1234");
    let b = make_id("lei", "sap:1234");
    assert!(!identifiers_match(&a, &b));
}

#[test]
fn internal_scheme_on_b_rejects() {
    let a = make_id("lei", "VAL");
    let b = make_id("internal", "VAL");
    assert!(!identifiers_match(&a, &b));
}

#[test]
fn both_internal_rejects() {
    let a = make_id("internal", "X");
    let b = make_id("internal", "X");
    assert!(!identifiers_match(&a, &b));
}

#[test]
fn whitespace_trimmed_values_match() {
    let a = make_id("lei", " LEI_ACME ");
    let b = make_id("lei", "LEI_ACME");
    assert!(identifiers_match(&a, &b));
}

#[test]
fn whitespace_trimmed_values_both_padded_match() {
    let a = make_id("duns", "  123456789  ");
    let b = make_id("duns", "  123456789  ");
    assert!(identifiers_match(&a, &b));
}

#[test]
fn different_values_rejects() {
    let a = make_id("lei", "LEI_A");
    let b = make_id("lei", "LEI_B");
    assert!(!identifiers_match(&a, &b));
}

#[test]
fn authority_case_insensitive_match() {
    let a = with_authority(make_id("nat-reg", "HRB12345"), "DE");
    let b = with_authority(make_id("nat-reg", "HRB12345"), "de");
    assert!(identifiers_match(&a, &b));
}

#[test]
fn authority_case_insensitive_mixed_case() {
    let a = with_authority(make_id("nat-reg", "HRB12345"), "GLEIF");
    let b = with_authority(make_id("nat-reg", "HRB12345"), "gleif");
    assert!(identifiers_match(&a, &b));
}

#[test]
fn authority_mismatch_rejects() {
    let a = with_authority(make_id("nat-reg", "HRB12345"), "DE");
    let b = with_authority(make_id("nat-reg", "HRB12345"), "FR");
    assert!(!identifiers_match(&a, &b));
}

#[test]
fn one_has_authority_other_does_not_rejects() {
    let a = with_authority(make_id("nat-reg", "HRB12345"), "DE");
    let b = make_id("nat-reg", "HRB12345");
    assert!(!identifiers_match(&a, &b));
    // Symmetric
    assert!(!identifiers_match(&b, &a));
}

#[test]
fn no_authority_on_either_matches_without_authority_check() {
    // Both lack authority → authority check is skipped.
    let a = make_id("lei", "SAME_VAL");
    let b = make_id("lei", "SAME_VAL");
    assert!(identifiers_match(&a, &b));
}

#[test]
fn both_missing_temporal_is_compatible() {
    let a = make_id("lei", "X");
    let b = make_id("lei", "X");
    // Both have no valid_from and no valid_to → compatible by default.
    assert!(temporal_compatible(&a, &b));
}

#[test]
fn one_missing_temporal_is_compatible() {
    // One record has temporal info, the other does not → compatible.
    let a = with_valid_from(make_id("lei", "X"), "2020-01-01");
    let b = make_id("lei", "X");
    assert!(temporal_compatible(&a, &b));
    assert!(temporal_compatible(&b, &a));
}

#[test]
fn overlapping_intervals_are_compatible() {
    // a: [2020-01-01, 2022-12-31], b: [2021-01-01, 2023-12-31] — overlap in 2021-2022
    let a = with_valid_to_date(
        with_valid_from(make_id("lei", "X"), "2020-01-01"),
        "2022-12-31",
    );
    let b = with_valid_to_date(
        with_valid_from(make_id("lei", "X"), "2021-01-01"),
        "2023-12-31",
    );
    assert!(temporal_compatible(&a, &b));
}

#[test]
fn non_overlapping_intervals_are_incompatible() {
    // a ends 2019-12-31, b starts 2020-01-01 → disjoint
    let a = with_valid_to_date(
        with_valid_from(make_id("lei", "X"), "2018-01-01"),
        "2019-12-31",
    );
    let b = with_valid_from(make_id("lei", "X"), "2020-01-01");
    assert!(!temporal_compatible(&a, &b));
    assert!(!temporal_compatible(&b, &a));
}

#[test]
fn adjacent_intervals_on_same_date_are_compatible() {
    // a: valid_to 2020-12-31, b: valid_from 2020-12-31 — same date → not strictly less than
    let a = with_valid_to_date(make_id("lei", "X"), "2020-12-31");
    let b = with_valid_from(make_id("lei", "X"), "2020-12-31");
    assert!(temporal_compatible(&a, &b));
}

#[test]
fn valid_to_null_no_expiry_is_compatible() {
    // Explicit no-expiry on one side, dated start on the other
    let a = with_valid_to_null(with_valid_from(make_id("lei", "X"), "2020-01-01"));
    let b = with_valid_from(make_id("lei", "X"), "2025-01-01");
    assert!(temporal_compatible(&a, &b));
}

#[test]
fn valid_to_null_both_sides_are_compatible() {
    let a = with_valid_to_null(with_valid_from(make_id("lei", "X"), "2020-01-01"));
    let b = with_valid_to_null(with_valid_from(make_id("lei", "X"), "2021-01-01"));
    assert!(temporal_compatible(&a, &b));
}

#[test]
fn identifiers_match_temporal_incompatibility_rejects() {
    // Same scheme/value/authority but non-overlapping temporal windows → reject
    let a = with_valid_to_date(make_id("lei", "LEI_ACME"), "2019-12-31");
    let b = with_valid_from(make_id("lei", "LEI_ACME"), "2020-06-01");
    assert!(!identifiers_match(&a, &b));
}

#[test]
fn non_lei_scheme_not_annulled() {
    let id = make_id("duns", "123");
    assert!(!is_lei_annulled(&id));
}

#[test]
fn lei_without_entity_status_not_annulled() {
    let id = make_id("lei", "SOME_LEI");
    assert!(!is_lei_annulled(&id));
}

#[test]
fn lei_with_annulled_status_is_annulled() {
    let mut id = make_id("lei", "SOME_LEI");
    id.extra.insert(
        "entity_status".to_owned(),
        DynValue::from(json!("ANNULLED")),
    );
    assert!(is_lei_annulled(&id));
}

#[test]
fn lei_with_active_status_not_annulled() {
    let mut id = make_id("lei", "SOME_LEI");
    id.extra
        .insert("entity_status".to_owned(), DynValue::from(json!("ACTIVE")));
    assert!(!is_lei_annulled(&id));
}

#[test]
fn lei_with_lowercase_annulled_not_annulled() {
    // GLEIF uses uppercase; lowercase is not a match (case-sensitive).
    let mut id = make_id("lei", "SOME_LEI");
    id.extra.insert(
        "entity_status".to_owned(),
        DynValue::from(json!("annulled")),
    );
    assert!(!is_lei_annulled(&id));
}

#[test]
fn internal_scheme_is_not_annulled_check() {
    let mut id = make_id("internal", "LEI_VAL");
    id.extra.insert(
        "entity_status".to_owned(),
        DynValue::from(json!("ANNULLED")),
    );
    assert!(!is_lei_annulled(&id), "non-lei scheme must return false");
}
