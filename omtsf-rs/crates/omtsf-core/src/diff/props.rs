use std::collections::{HashMap, HashSet};

use crate::canonical::CanonicalId;
use crate::structures::{EdgeProperties, Node};
use crate::types::{DataQuality, Identifier, Label};

use super::compare::{maybe_change, normalise_date, normalise_date_value, to_value};
use super::types::{IdentifierFieldDiff, IdentifierSetDiff, LabelSetDiff, PropertyChange};

/// Compares the scalar fields of a [`DataQuality`] object.
///
/// Produces changes for `confidence`, `source`, and `last_verified`.
pub(super) fn compare_data_quality(
    field_prefix: &str,
    a: Option<&DataQuality>,
    b: Option<&DataQuality>,
    ignore: &HashSet<String>,
    out: &mut Vec<PropertyChange>,
) {
    match (a, b) {
        (None, None) => {}
        (Some(aq), Some(bq)) => {
            // Compare each sub-field individually.
            let sub =
                |sub_name: &str, av: Option<serde_json::Value>, bv: Option<serde_json::Value>| {
                    let name = format!("{field_prefix}.{sub_name}");
                    (name, av, bv)
                };
            let checks = [
                sub(
                    "confidence",
                    to_value(&aq.confidence),
                    to_value(&bq.confidence),
                ),
                sub(
                    "source",
                    aq.source
                        .as_deref()
                        .map(|s| serde_json::Value::String(s.to_owned())),
                    bq.source
                        .as_deref()
                        .map(|s| serde_json::Value::String(s.to_owned())),
                ),
                sub(
                    "last_verified",
                    to_value(&aq.last_verified),
                    to_value(&bq.last_verified),
                ),
            ];
            for (name, av, bv) in checks {
                if !ignore.contains(&name) {
                    maybe_change(&name, av, bv, out);
                }
            }
            // Extra fields in data_quality are compared as raw JSON values.
            // Collect all keys from both sides into a set so that keys present
            // in both maps are visited exactly once (avoiding duplicate entries).
            let mut extra_keys: HashSet<&str> = HashSet::new();
            for k in aq.extra.keys() {
                extra_keys.insert(k.as_str());
            }
            for k in bq.extra.keys() {
                extra_keys.insert(k.as_str());
            }
            for key in &extra_keys {
                let name = format!("{field_prefix}.{key}");
                if ignore.contains(&name) {
                    continue;
                }
                let av = aq.extra.get(*key).cloned().map(serde_json::Value::from);
                let bv = bq.extra.get(*key).cloned().map(serde_json::Value::from);
                maybe_change(&name, av, bv, out);
            }
        }
        // One side has data_quality, the other doesn't.
        (Some(aq), None) => {
            let name = field_prefix.to_owned();
            if !ignore.contains(&name) {
                out.push(PropertyChange {
                    field: name,
                    old_value: serde_json::to_value(aq).ok(),
                    new_value: None,
                });
            }
        }
        (None, Some(bq)) => {
            let name = field_prefix.to_owned();
            if !ignore.contains(&name) {
                out.push(PropertyChange {
                    field: name,
                    old_value: None,
                    new_value: serde_json::to_value(bq).ok(),
                });
            }
        }
    }
}

/// Compares the scalar properties of two matched [`Node`]s.
///
/// Fields listed in `ignore` are skipped. Date fields are normalised before
/// comparison. Numeric fields use epsilon comparison.
pub(super) fn compare_node_properties(
    a: &Node,
    b: &Node,
    ignore: &HashSet<String>,
) -> Vec<PropertyChange> {
    let mut changes: Vec<PropertyChange> = Vec::new();

    macro_rules! check {
        ($field:expr, $a:expr, $b:expr) => {
            if !ignore.contains($field) {
                maybe_change($field, $a, $b, &mut changes);
            }
        };
    }

    check!(
        "name",
        a.name
            .as_deref()
            .map(|s| serde_json::Value::String(s.to_owned())),
        b.name
            .as_deref()
            .map(|s| serde_json::Value::String(s.to_owned()))
    );
    check!(
        "jurisdiction",
        to_value(&a.jurisdiction),
        to_value(&b.jurisdiction)
    );
    check!("status", to_value(&a.status), to_value(&b.status));
    check!(
        "governance_structure",
        a.governance_structure.clone().map(serde_json::Value::from),
        b.governance_structure.clone().map(serde_json::Value::from)
    );
    check!(
        "operator",
        a.operator
            .as_ref()
            .map(|id| serde_json::Value::String(id.to_string())),
        b.operator
            .as_ref()
            .map(|id| serde_json::Value::String(id.to_string()))
    );
    check!(
        "address",
        a.address
            .as_deref()
            .map(|s| serde_json::Value::String(s.to_owned())),
        b.address
            .as_deref()
            .map(|s| serde_json::Value::String(s.to_owned()))
    );
    check!(
        "geo",
        a.geo.clone().map(serde_json::Value::from),
        b.geo.clone().map(serde_json::Value::from)
    );
    check!(
        "commodity_code",
        a.commodity_code
            .as_deref()
            .map(|s| serde_json::Value::String(s.to_owned())),
        b.commodity_code
            .as_deref()
            .map(|s| serde_json::Value::String(s.to_owned()))
    );
    check!(
        "unit",
        a.unit
            .as_deref()
            .map(|s| serde_json::Value::String(s.to_owned())),
        b.unit
            .as_deref()
            .map(|s| serde_json::Value::String(s.to_owned()))
    );
    check!(
        "role",
        a.role
            .as_deref()
            .map(|s| serde_json::Value::String(s.to_owned())),
        b.role
            .as_deref()
            .map(|s| serde_json::Value::String(s.to_owned()))
    );
    check!(
        "attestation_type",
        to_value(&a.attestation_type),
        to_value(&b.attestation_type)
    );
    check!(
        "standard",
        a.standard
            .as_deref()
            .map(|s| serde_json::Value::String(s.to_owned())),
        b.standard
            .as_deref()
            .map(|s| serde_json::Value::String(s.to_owned()))
    );
    check!(
        "issuer",
        a.issuer
            .as_deref()
            .map(|s| serde_json::Value::String(s.to_owned())),
        b.issuer
            .as_deref()
            .map(|s| serde_json::Value::String(s.to_owned()))
    );
    check!(
        "valid_from",
        to_value(&a.valid_from).map(|v| normalise_date_value(&v)),
        to_value(&b.valid_from).map(|v| normalise_date_value(&v))
    );
    // valid_to: Option<Option<CalendarDate>> — serialise as nullable JSON.
    {
        let av = match &a.valid_to {
            None => None,
            Some(None) => Some(serde_json::Value::Null),
            Some(Some(d)) => Some(serde_json::Value::String(normalise_date(d))),
        };
        let bv = match &b.valid_to {
            None => None,
            Some(None) => Some(serde_json::Value::Null),
            Some(Some(d)) => Some(serde_json::Value::String(normalise_date(d))),
        };
        check!("valid_to", av, bv);
    }
    check!("outcome", to_value(&a.outcome), to_value(&b.outcome));
    check!(
        "attestation_status",
        to_value(&a.attestation_status),
        to_value(&b.attestation_status)
    );
    check!(
        "reference",
        a.reference
            .as_deref()
            .map(|s| serde_json::Value::String(s.to_owned())),
        b.reference
            .as_deref()
            .map(|s| serde_json::Value::String(s.to_owned()))
    );
    check!(
        "risk_severity",
        to_value(&a.risk_severity),
        to_value(&b.risk_severity)
    );
    check!(
        "risk_likelihood",
        to_value(&a.risk_likelihood),
        to_value(&b.risk_likelihood)
    );
    check!(
        "lot_id",
        a.lot_id
            .as_deref()
            .map(|s| serde_json::Value::String(s.to_owned())),
        b.lot_id
            .as_deref()
            .map(|s| serde_json::Value::String(s.to_owned()))
    );
    check!(
        "quantity",
        a.quantity
            .and_then(serde_json::Number::from_f64)
            .map(serde_json::Value::Number),
        b.quantity
            .and_then(serde_json::Number::from_f64)
            .map(serde_json::Value::Number)
    );
    check!(
        "production_date",
        to_value(&a.production_date).map(|v| normalise_date_value(&v)),
        to_value(&b.production_date).map(|v| normalise_date_value(&v))
    );
    check!(
        "origin_country",
        to_value(&a.origin_country),
        to_value(&b.origin_country)
    );
    check!(
        "direct_emissions_co2e",
        a.direct_emissions_co2e
            .and_then(serde_json::Number::from_f64)
            .map(serde_json::Value::Number),
        b.direct_emissions_co2e
            .and_then(serde_json::Number::from_f64)
            .map(serde_json::Value::Number)
    );
    check!(
        "indirect_emissions_co2e",
        a.indirect_emissions_co2e
            .and_then(serde_json::Number::from_f64)
            .map(serde_json::Value::Number),
        b.indirect_emissions_co2e
            .and_then(serde_json::Number::from_f64)
            .map(serde_json::Value::Number)
    );
    check!(
        "emission_factor_source",
        to_value(&a.emission_factor_source),
        to_value(&b.emission_factor_source)
    );
    check!(
        "installation_id",
        a.installation_id
            .as_ref()
            .map(|id| serde_json::Value::String(id.to_string())),
        b.installation_id
            .as_ref()
            .map(|id| serde_json::Value::String(id.to_string()))
    );

    if !ignore.contains("data_quality") {
        compare_data_quality(
            "data_quality",
            a.data_quality.as_ref(),
            b.data_quality.as_ref(),
            ignore,
            &mut changes,
        );
    }

    {
        let mut extra_keys: HashSet<&str> = HashSet::new();
        for k in a.extra.keys() {
            extra_keys.insert(k.as_str());
        }
        for k in b.extra.keys() {
            extra_keys.insert(k.as_str());
        }
        for key in &extra_keys {
            if ignore.contains(*key) {
                continue;
            }
            let av = a.extra.get(*key).cloned().map(serde_json::Value::from);
            let bv = b.extra.get(*key).cloned().map(serde_json::Value::from);
            maybe_change(key, av, bv, &mut changes);
        }
    }

    changes
}

/// Compares the properties of two matched [`EdgeProperties`] values.
///
/// Returns scalar `PropertyChange`s, ignoring fields in `ignore`.
pub(super) fn compare_edge_props(
    a: &EdgeProperties,
    b: &EdgeProperties,
    ignore: &HashSet<String>,
) -> Vec<PropertyChange> {
    let mut changes: Vec<PropertyChange> = Vec::new();

    macro_rules! check {
        ($field:expr, $a:expr, $b:expr) => {
            if !ignore.contains($field) {
                maybe_change($field, $a, $b, &mut changes);
            }
        };
    }

    check!(
        "valid_from",
        to_value(&a.valid_from).map(|v| normalise_date_value(&v)),
        to_value(&b.valid_from).map(|v| normalise_date_value(&v))
    );
    {
        let av = match &a.valid_to {
            None => None,
            Some(None) => Some(serde_json::Value::Null),
            Some(Some(d)) => Some(serde_json::Value::String(normalise_date(d))),
        };
        let bv = match &b.valid_to {
            None => None,
            Some(None) => Some(serde_json::Value::Null),
            Some(Some(d)) => Some(serde_json::Value::String(normalise_date(d))),
        };
        check!("valid_to", av, bv);
    }

    check!(
        "percentage",
        a.percentage
            .and_then(serde_json::Number::from_f64)
            .map(serde_json::Value::Number),
        b.percentage
            .and_then(serde_json::Number::from_f64)
            .map(serde_json::Value::Number)
    );
    check!(
        "volume",
        a.volume
            .and_then(serde_json::Number::from_f64)
            .map(serde_json::Value::Number),
        b.volume
            .and_then(serde_json::Number::from_f64)
            .map(serde_json::Value::Number)
    );
    check!(
        "annual_value",
        a.annual_value
            .and_then(serde_json::Number::from_f64)
            .map(serde_json::Value::Number),
        b.annual_value
            .and_then(serde_json::Number::from_f64)
            .map(serde_json::Value::Number)
    );
    check!(
        "share_of_buyer_demand",
        a.share_of_buyer_demand
            .and_then(serde_json::Number::from_f64)
            .map(serde_json::Value::Number),
        b.share_of_buyer_demand
            .and_then(serde_json::Number::from_f64)
            .map(serde_json::Value::Number)
    );
    check!(
        "quantity",
        a.quantity
            .and_then(serde_json::Number::from_f64)
            .map(serde_json::Value::Number),
        b.quantity
            .and_then(serde_json::Number::from_f64)
            .map(serde_json::Value::Number)
    );

    check!(
        "direct",
        a.direct.map(serde_json::Value::Bool),
        b.direct.map(serde_json::Value::Bool)
    );

    check!(
        "control_type",
        a.control_type.clone().map(serde_json::Value::from),
        b.control_type.clone().map(serde_json::Value::from)
    );
    check!(
        "consolidation_basis",
        to_value(&a.consolidation_basis),
        to_value(&b.consolidation_basis)
    );
    check!(
        "event_type",
        to_value(&a.event_type),
        to_value(&b.event_type)
    );
    check!(
        "effective_date",
        to_value(&a.effective_date).map(|v| normalise_date_value(&v)),
        to_value(&b.effective_date).map(|v| normalise_date_value(&v))
    );
    check!(
        "description",
        a.description
            .as_deref()
            .map(|s| serde_json::Value::String(s.to_owned())),
        b.description
            .as_deref()
            .map(|s| serde_json::Value::String(s.to_owned()))
    );
    check!(
        "commodity",
        a.commodity
            .as_deref()
            .map(|s| serde_json::Value::String(s.to_owned())),
        b.commodity
            .as_deref()
            .map(|s| serde_json::Value::String(s.to_owned()))
    );
    check!(
        "contract_ref",
        a.contract_ref
            .as_deref()
            .map(|s| serde_json::Value::String(s.to_owned())),
        b.contract_ref
            .as_deref()
            .map(|s| serde_json::Value::String(s.to_owned()))
    );
    check!(
        "volume_unit",
        a.volume_unit
            .as_deref()
            .map(|s| serde_json::Value::String(s.to_owned())),
        b.volume_unit
            .as_deref()
            .map(|s| serde_json::Value::String(s.to_owned()))
    );
    check!(
        "value_currency",
        a.value_currency
            .as_deref()
            .map(|s| serde_json::Value::String(s.to_owned())),
        b.value_currency
            .as_deref()
            .map(|s| serde_json::Value::String(s.to_owned()))
    );
    check!(
        "tier",
        a.tier
            .map(|n| serde_json::Value::Number(serde_json::Number::from(n))),
        b.tier
            .map(|n| serde_json::Value::Number(serde_json::Number::from(n)))
    );
    check!(
        "service_type",
        to_value(&a.service_type),
        to_value(&b.service_type)
    );
    check!(
        "unit",
        a.unit
            .as_deref()
            .map(|s| serde_json::Value::String(s.to_owned())),
        b.unit
            .as_deref()
            .map(|s| serde_json::Value::String(s.to_owned()))
    );
    check!(
        "scope",
        a.scope
            .as_deref()
            .map(|s| serde_json::Value::String(s.to_owned())),
        b.scope
            .as_deref()
            .map(|s| serde_json::Value::String(s.to_owned()))
    );

    if !ignore.contains("data_quality") {
        compare_data_quality(
            "data_quality",
            a.data_quality.as_ref(),
            b.data_quality.as_ref(),
            ignore,
            &mut changes,
        );
    }

    {
        let mut extra_keys: HashSet<&str> = HashSet::new();
        for k in a.extra.keys() {
            extra_keys.insert(k.as_str());
        }
        for k in b.extra.keys() {
            extra_keys.insert(k.as_str());
        }
        for key in &extra_keys {
            if ignore.contains(*key) {
                continue;
            }
            let av = a.extra.get(*key).cloned().map(serde_json::Value::from);
            let bv = b.extra.get(*key).cloned().map(serde_json::Value::from);
            maybe_change(key, av, bv, &mut changes);
        }
    }

    changes
}

/// Compares two `identifiers` slices and returns the set diff.
///
/// Identifiers are keyed by their canonical string (scheme:value or
/// scheme:authority:value). Identifiers with the same key in both slices are
/// checked for field-level changes to `valid_from`, `valid_to`, `sensitivity`,
/// `verification_status`, and `verification_date`.
pub(super) fn compare_identifiers(a_ids: &[Identifier], b_ids: &[Identifier]) -> IdentifierSetDiff {
    let mut a_map: HashMap<CanonicalId, &Identifier> = HashMap::new();
    for id in a_ids {
        if id.scheme != "internal" {
            a_map.insert(CanonicalId::from_identifier(id), id);
        }
    }
    let mut b_map: HashMap<CanonicalId, &Identifier> = HashMap::new();
    for id in b_ids {
        if id.scheme != "internal" {
            b_map.insert(CanonicalId::from_identifier(id), id);
        }
    }

    let mut added: Vec<Identifier> = Vec::new();
    let mut removed: Vec<Identifier> = Vec::new();
    let mut modified: Vec<IdentifierFieldDiff> = Vec::new();

    for (cid, id) in &a_map {
        if !b_map.contains_key(cid) {
            removed.push((*id).clone());
        }
    }

    for (cid, id) in &b_map {
        if !a_map.contains_key(cid) {
            added.push((*id).clone());
        }
    }

    for (cid, id_a) in &a_map {
        let Some(id_b) = b_map.get(cid) else {
            continue;
        };
        let mut field_changes: Vec<PropertyChange> = Vec::new();
        maybe_change(
            "valid_from",
            to_value(&id_a.valid_from).map(|v| normalise_date_value(&v)),
            to_value(&id_b.valid_from).map(|v| normalise_date_value(&v)),
            &mut field_changes,
        );
        {
            let av = match &id_a.valid_to {
                None => None,
                Some(None) => Some(serde_json::Value::Null),
                Some(Some(d)) => Some(serde_json::Value::String(normalise_date(d))),
            };
            let bv = match &id_b.valid_to {
                None => None,
                Some(None) => Some(serde_json::Value::Null),
                Some(Some(d)) => Some(serde_json::Value::String(normalise_date(d))),
            };
            maybe_change("valid_to", av, bv, &mut field_changes);
        }
        maybe_change(
            "sensitivity",
            to_value(&id_a.sensitivity),
            to_value(&id_b.sensitivity),
            &mut field_changes,
        );
        maybe_change(
            "verification_status",
            to_value(&id_a.verification_status),
            to_value(&id_b.verification_status),
            &mut field_changes,
        );
        maybe_change(
            "verification_date",
            to_value(&id_a.verification_date).map(|v| normalise_date_value(&v)),
            to_value(&id_b.verification_date).map(|v| normalise_date_value(&v)),
            &mut field_changes,
        );
        // authority (even though it's part of the canonical key for some schemes,
        // a change to a non-authority-required scheme's authority is a field change).
        let av_auth = id_a
            .authority
            .as_deref()
            .map(|s| serde_json::Value::String(s.to_owned()));
        let bv_auth = id_b
            .authority
            .as_deref()
            .map(|s| serde_json::Value::String(s.to_owned()));
        maybe_change("authority", av_auth, bv_auth, &mut field_changes);

        let mut extra_keys: HashSet<&str> = HashSet::new();
        for k in id_a.extra.keys() {
            extra_keys.insert(k.as_str());
        }
        for k in id_b.extra.keys() {
            extra_keys.insert(k.as_str());
        }
        for key in &extra_keys {
            let av = id_a.extra.get(*key).cloned().map(serde_json::Value::from);
            let bv = id_b.extra.get(*key).cloned().map(serde_json::Value::from);
            maybe_change(key, av, bv, &mut field_changes);
        }

        if !field_changes.is_empty() {
            modified.push(IdentifierFieldDiff {
                canonical_key: cid.clone(),
                field_changes,
            });
        }
    }

    IdentifierSetDiff {
        added,
        removed,
        modified,
    }
}

/// Compares two `labels` slices and returns the set diff.
///
/// Labels are matched by `(key, value)` pair. A change in value for a given
/// key appears as a deletion of the old pair and an addition of the new one
/// (diff.md Section 3.3).
pub(super) fn compare_labels(a_labels: &[Label], b_labels: &[Label]) -> LabelSetDiff {
    let a_set: HashSet<(&str, Option<&str>)> = a_labels
        .iter()
        .map(|l| (l.key.as_str(), l.value.as_deref()))
        .collect();
    let b_set: HashSet<(&str, Option<&str>)> = b_labels
        .iter()
        .map(|l| (l.key.as_str(), l.value.as_deref()))
        .collect();

    let mut removed: Vec<Label> = Vec::new();
    for label in a_labels {
        let pair = (label.key.as_str(), label.value.as_deref());
        if !b_set.contains(&pair) {
            removed.push(label.clone());
        }
    }

    let mut added: Vec<Label> = Vec::new();
    for label in b_labels {
        let pair = (label.key.as_str(), label.value.as_deref());
        if !a_set.contains(&pair) {
            added.push(label.clone());
        }
    }

    LabelSetDiff { added, removed }
}
