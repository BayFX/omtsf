#![allow(clippy::expect_used)]

//! Group C — Auto-generated fixture tests.
//!
//! Each test programmatically builds JSON, validates it against the schema,
//! parses it as `OmtsFile`, re-serialises, and validates again.

use jsonschema::Validator;
use serde_json::{Value, json};

mod cross_cutting;

use super::{
    attestation_node, base_file_with, compile_schema, edge, facility_node, good_node, load_schema,
    org_node, validate_and_parse,
};

pub(super) fn validator() -> Validator {
    compile_schema(&load_schema())
}

// =========================================================================
// Per-node-type tests
// =========================================================================

#[test]
fn fixture_organization_node() {
    let v = validator();
    let node = json!({
        "id": "org-1", "type": "organization", "name": "Acme Corp",
        "jurisdiction": "DE",
        "status": "active",
        "governance_structure": "sole_subsidiary",
        "identifiers": [
            {"scheme": "lei", "value": "5493006MHB84DD0ZWV18", "sensitivity": "public",
             "verification_status": "verified", "verification_date": "2026-01-01"},
            {"scheme": "duns", "value": "081466849"},
            {"scheme": "nat-reg", "value": "HRB86891", "authority": "RA000548"},
            {"scheme": "vat", "value": "DE123456789", "authority": "DE"},
            {"scheme": "internal", "value": "V-100234", "authority": "sap"}
        ],
        "data_quality": {"confidence": "verified", "source": "gleif-api", "last_verified": "2026-01-15"}
    });
    validate_and_parse(&base_file_with(vec![node], vec![]), &v);
}

#[test]
fn fixture_facility_node() {
    let v = validator();
    let node = json!({
        "id": "fac-1", "type": "facility", "name": "Sheffield Plant",
        "operator": "org-1",
        "address": "1 Steel Lane, Sheffield, UK",
        "geo": {"lat": 53.3811, "lon": -1.4701},
        "identifiers": [
            {"scheme": "gln", "value": "5060012340018"}
        ]
    });
    validate_and_parse(
        &base_file_with(vec![org_node("org-1", "Operator"), node], vec![]),
        &v,
    );
}

#[test]
fn fixture_good_node() {
    let v = validator();
    let node = json!({
        "id": "good-1", "type": "good", "name": "M10 Steel Bolts",
        "commodity_code": "7318.15",
        "unit": "pcs",
        "identifiers": [
            {"scheme": "org.gs1.gtin", "value": "05060012340018"}
        ]
    });
    validate_and_parse(&base_file_with(vec![node], vec![]), &v);
}

#[test]
fn fixture_person_node() {
    let v = validator();
    let node = json!({
        "id": "per-1", "type": "person", "name": "Jane Doe",
        "jurisdiction": "DE",
        "role": "Ultimate Beneficial Owner",
        "identifiers": [
            {"scheme": "internal", "value": "UBO-001", "authority": "compliance-db",
             "sensitivity": "confidential"}
        ]
    });
    validate_and_parse(&base_file_with(vec![node], vec![]), &v);
}

#[test]
fn fixture_attestation_node() {
    let v = validator();
    // Use status=active (overlaps both OrganizationStatus and AttestationStatus in Rust).
    // See Known Gap in plan — revoked/expired/withdrawn fail Rust deser.
    let node = json!({
        "id": "att-1", "type": "attestation", "name": "SA8000 Cert",
        "attestation_type": "certification",
        "standard": "SA8000:2014",
        "issuer": "SAI",
        "valid_from": "2025-06-01",
        "valid_to": "2028-05-31",
        "outcome": "pass",
        "status": "active",
        "reference": "CERT-2025-001",
        "risk_severity": "low",
        "risk_likelihood": "unlikely"
    });
    validate_and_parse(&base_file_with(vec![node], vec![]), &v);
}

/// Attestation `status` values that exist only in the schema's attestation enum
/// but NOT in the Rust `OrganizationStatus` enum (which `Node::status` maps to)
/// are valid JSON Schema but fail Rust deserialization. This test documents the gap.
#[test]
fn attestation_status_schema_only_values() {
    let v = validator();
    for status in &["revoked", "expired", "withdrawn"] {
        let node = json!({
            "id": "att-gap", "type": "attestation", "name": "Gap test",
            "attestation_type": "audit", "valid_from": "2025-01-01",
            "status": status
        });
        let doc = base_file_with(vec![node], vec![]);
        // Schema accepts these values
        assert!(
            v.is_valid(&doc),
            "Schema should accept attestation status={status}"
        );
        // But Rust deser fails because Node::status is OrganizationStatus
        let text = serde_json::to_string(&doc).expect("serialize");
        let result = serde_json::from_str::<omtsf_core::OmtsFile>(&text);
        assert!(
            result.is_err(),
            "Rust should reject attestation status={status} (known gap)"
        );
    }
}

#[test]
fn fixture_consignment_node() {
    let v = validator();
    let node = json!({
        "id": "con-1", "type": "consignment", "name": "Batch 2026-Q1",
        "lot_id": "LOT-001",
        "quantity": 50000,
        "unit": "pcs",
        "production_date": "2026-01-20",
        "origin_country": "GB",
        "direct_emissions_co2e": 12.5,
        "indirect_emissions_co2e": 3.2,
        "emission_factor_source": "actual",
        "installation_id": "fac-1"
    });
    validate_and_parse(
        &base_file_with(vec![facility_node("fac-1", "Plant"), node], vec![]),
        &v,
    );
}

#[test]
fn fixture_boundary_ref_node() {
    let v = validator();
    let node = json!({
        "id": "bref-1", "type": "boundary_ref", "name": "Redacted Entity",
        "identifiers": [
            {"scheme": "opaque", "value": "e8798687b081da98b7cd1c4e5e2423bd3214fbab0f1f476a2dcdbf67c2e21141"}
        ]
    });
    validate_and_parse(&base_file_with(vec![node], vec![]), &v);
}

// =========================================================================
// Per-edge-type tests
// =========================================================================

fn two_orgs_with_edge(edge_type: &str, props: Value) -> Value {
    base_file_with(
        vec![org_node("org-a", "A"), org_node("org-b", "B")],
        vec![edge("e-1", edge_type, "org-a", "org-b", props)],
    )
}

#[test]
fn fixture_edge_ownership() {
    let v = validator();
    let doc = two_orgs_with_edge(
        "ownership",
        json!({
            "percentage": 51.0, "direct": true,
            "valid_from": "2020-01-01", "valid_to": null,
            "data_quality": {"confidence": "verified"},
            "_property_sensitivity": {"percentage": "confidential"}
        }),
    );
    validate_and_parse(&doc, &v);
}

#[test]
fn fixture_edge_operational_control() {
    let v = validator();
    for ct in &[
        "franchise",
        "management",
        "tolling",
        "licensed_manufacturing",
        "other",
    ] {
        let doc = two_orgs_with_edge(
            "operational_control",
            json!({
                "control_type": ct, "valid_from": "2020-01-01", "valid_to": "2025-12-31"
            }),
        );
        validate_and_parse(&doc, &v);
    }
}

#[test]
fn fixture_edge_legal_parentage() {
    let v = validator();
    for cb in &["ifrs10", "us_gaap_asc810", "other", "unknown"] {
        let doc = two_orgs_with_edge(
            "legal_parentage",
            json!({
                "valid_from": "2019-01-01", "valid_to": null,
                "consolidation_basis": cb
            }),
        );
        validate_and_parse(&doc, &v);
    }
}

#[test]
fn fixture_edge_former_identity() {
    let v = validator();
    for et in &["merger", "acquisition", "rename", "demerger", "spin_off"] {
        let doc = two_orgs_with_edge(
            "former_identity",
            json!({
                "event_type": et, "effective_date": "2023-07-01",
                "description": "Identity event"
            }),
        );
        validate_and_parse(&doc, &v);
    }
}

#[test]
fn fixture_edge_beneficial_ownership() {
    let v = validator();
    let nodes = vec![
        json!({"id": "per-1", "type": "person", "name": "Jane Doe"}),
        org_node("org-1", "Corp"),
    ];
    for ct in &[
        "voting_rights",
        "capital",
        "other_means",
        "senior_management",
    ] {
        let doc = base_file_with(
            nodes.clone(),
            vec![edge(
                "e-bo",
                "beneficial_ownership",
                "per-1",
                "org-1",
                json!({
                    "percentage": 60.0, "control_type": ct, "direct": true,
                    "valid_from": "2015-03-01", "valid_to": null
                }),
            )],
        );
        validate_and_parse(&doc, &v);
    }
}

#[test]
fn fixture_edge_supplies() {
    let v = validator();
    let doc = two_orgs_with_edge(
        "supplies",
        json!({
            "valid_from": "2023-01-15", "valid_to": null,
            "commodity": "7318.15", "contract_ref": "PO-2023-001",
            "volume": 100000, "volume_unit": "pcs",
            "annual_value": 250000, "value_currency": "EUR",
            "tier": 1, "share_of_buyer_demand": 35.0,
            "data_quality": {"confidence": "reported", "source": "erp"}
        }),
    );
    validate_and_parse(&doc, &v);
}

#[test]
fn fixture_edge_subcontracts() {
    let v = validator();
    let doc = two_orgs_with_edge(
        "subcontracts",
        json!({
            "valid_from": "2024-01-01", "valid_to": "2025-12-31",
            "commodity": "Assembly", "contract_ref": "SC-2024-001",
            "volume": 5000, "volume_unit": "units",
            "share_of_buyer_demand": 20.0
        }),
    );
    validate_and_parse(&doc, &v);
}

#[test]
fn fixture_edge_tolls() {
    let v = validator();
    let doc = two_orgs_with_edge(
        "tolls",
        json!({
            "valid_from": "2022-06-01", "valid_to": null,
            "commodity": "Raw steel"
        }),
    );
    validate_and_parse(&doc, &v);
}

#[test]
fn fixture_edge_distributes() {
    let v = validator();
    for st in &["warehousing", "transport", "fulfillment", "other"] {
        let doc = two_orgs_with_edge(
            "distributes",
            json!({
                "valid_from": "2023-03-01", "valid_to": null,
                "service_type": st
            }),
        );
        validate_and_parse(&doc, &v);
    }
}

#[test]
fn fixture_edge_brokers() {
    let v = validator();
    let doc = two_orgs_with_edge(
        "brokers",
        json!({
            "valid_from": "2024-01-01", "valid_to": null,
            "commodity": "Copper"
        }),
    );
    validate_and_parse(&doc, &v);
}

#[test]
fn fixture_edge_operates() {
    let v = validator();
    let nodes = vec![
        org_node("org-1", "Operator"),
        facility_node("fac-1", "Plant"),
    ];
    let doc = base_file_with(
        nodes,
        vec![edge(
            "e-op",
            "operates",
            "org-1",
            "fac-1",
            json!({
                "valid_from": "2018-06-01", "valid_to": null
            }),
        )],
    );
    validate_and_parse(&doc, &v);
}

#[test]
fn fixture_edge_produces() {
    let v = validator();
    let nodes = vec![
        facility_node("fac-1", "Plant"),
        good_node("good-1", "Bolts"),
    ];
    let doc = base_file_with(
        nodes,
        vec![edge(
            "e-pr",
            "produces",
            "fac-1",
            "good-1",
            json!({
                "valid_from": "2020-03-01", "valid_to": null
            }),
        )],
    );
    validate_and_parse(&doc, &v);
}

#[test]
fn fixture_edge_composed_of() {
    let v = validator();
    let nodes = vec![good_node("good-1", "Assembly"), good_node("good-2", "Part")];
    let doc = base_file_with(
        nodes,
        vec![edge(
            "e-co",
            "composed_of",
            "good-1",
            "good-2",
            json!({
                "quantity": 4, "unit": "pcs",
                "valid_from": "2024-01-01", "valid_to": null
            }),
        )],
    );
    validate_and_parse(&doc, &v);
}

#[test]
fn fixture_edge_sells_to() {
    let v = validator();
    let doc = two_orgs_with_edge(
        "sells_to",
        json!({
            "valid_from": "2024-06-01", "valid_to": null,
            "commodity": "7318.15", "contract_ref": "SA-2024-001"
        }),
    );
    validate_and_parse(&doc, &v);
}

#[test]
fn fixture_edge_attested_by() {
    let v = validator();
    let nodes = vec![
        facility_node("fac-1", "Plant"),
        attestation_node("att-1", "SA8000 Cert"),
    ];
    let doc = base_file_with(
        nodes,
        vec![edge(
            "e-att",
            "attested_by",
            "fac-1",
            "att-1",
            json!({
                "scope": "labor_rights"
            }),
        )],
    );
    validate_and_parse(&doc, &v);
}

#[test]
fn fixture_edge_same_as() {
    let v = validator();
    for conf in &["definite", "probable", "possible"] {
        let doc = two_orgs_with_edge(
            "same_as",
            json!({
                "confidence": conf, "basis": "name_match"
            }),
        );
        validate_and_parse(&doc, &v);
    }
}
