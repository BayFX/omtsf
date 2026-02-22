#![allow(clippy::expect_used, clippy::panic, clippy::needless_pass_by_value)]

//! Cross-cutting fixture tests covering identifiers, optional fields, delta mode,
//! file integrity, merge metadata, geo variants, conflicts, extensions, and
//! the kitchen-sink regression test.

use serde_json::json;

use super::super::{base_file, base_file_with, edge, good_node, org_node, validate_and_parse};
use super::validator;

#[test]
fn fixture_all_identifier_schemes() {
    let v = validator();
    let node = json!({
        "id": "org-ids", "type": "organization", "name": "Identifier Test Corp",
        "identifiers": [
            {"scheme": "lei", "value": "5493006MHB84DD0ZWV18",
             "valid_from": "2020-01-01", "valid_to": null,
             "sensitivity": "public", "verification_status": "verified",
             "verification_date": "2026-01-01"},
            {"scheme": "duns", "value": "081466849"},
            {"scheme": "gln", "value": "5060012340001"},
            {"scheme": "nat-reg", "value": "HRB86891", "authority": "RA000548"},
            {"scheme": "vat", "value": "DE123456789", "authority": "DE"},
            {"scheme": "internal", "value": "V-100234", "authority": "sap-mm-prod"},
            {"scheme": "opaque", "value": "a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4"},
            {"scheme": "org.gs1.gtin", "value": "05060012340018",
             "valid_from": "2023-01-01", "valid_to": "2030-12-31",
             "sensitivity": "restricted", "verification_status": "reported"}
        ]
    });
    validate_and_parse(&base_file_with(vec![node], vec![]), &v);
}

#[test]
fn fixture_all_optional_top_level_fields() {
    let v = validator();
    let mut doc = base_file();
    let obj = doc.as_object_mut().expect("object");
    obj.insert("disclosure_scope".to_owned(), json!("partner"));
    obj.insert(
        "previous_snapshot_ref".to_owned(),
        json!("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"),
    );
    obj.insert("snapshot_sequence".to_owned(), json!(42));
    obj.insert("reporting_entity".to_owned(), json!("org-1"));

    let nodes = vec![org_node("org-1", "Reporter Corp")];
    obj["nodes"] = json!(nodes);

    validate_and_parse(&doc, &v);
}

#[test]
fn fixture_delta_mode() {
    let v = validator();
    let mut doc = base_file();
    let obj = doc.as_object_mut().expect("object");
    obj.insert("update_type".to_owned(), json!("delta"));
    obj.insert(
        "base_snapshot_ref".to_owned(),
        json!("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"),
    );

    let nodes = vec![
        {
            let mut n = org_node("org-add", "Added Corp");
            n.as_object_mut()
                .expect("obj")
                .insert("_operation".to_owned(), json!("add"));
            n
        },
        {
            let mut n = org_node("org-mod", "Modified Corp");
            n.as_object_mut()
                .expect("obj")
                .insert("_operation".to_owned(), json!("modify"));
            n
        },
        {
            let mut n = org_node("org-rm", "Removed Corp");
            n.as_object_mut()
                .expect("obj")
                .insert("_operation".to_owned(), json!("remove"));
            n
        },
    ];
    let edges = vec![{
        let mut e = edge(
            "e-1",
            "supplies",
            "org-add",
            "org-mod",
            json!({"valid_from": "2025-01-01"}),
        );
        e.as_object_mut()
            .expect("obj")
            .insert("_operation".to_owned(), json!("add"));
        e
    }];
    obj["nodes"] = json!(nodes);
    obj["edges"] = json!(edges);

    validate_and_parse(&doc, &v);
}

#[test]
fn fixture_file_integrity() {
    let v = validator();
    let mut doc = base_file();
    let obj = doc.as_object_mut().expect("object");
    obj.insert(
        "file_integrity".to_owned(),
        json!({
            "content_hash": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            "algorithm": "sha-256",
            "signature": "dGVzdC1zaWduYXR1cmU=",
            "signer": "5493006MHB84DD0ZWV18"
        }),
    );
    validate_and_parse(&doc, &v);
}

#[test]
fn fixture_merge_metadata() {
    let v = validator();
    let mut doc = base_file();
    let obj = doc.as_object_mut().expect("object");
    obj.insert(
        "merge_metadata".to_owned(),
        json!({
            "source_file_hashes": [
                "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
                "d7a8fbb307d7809469ca9abcb0082e4f8d5651e46d3cdb762d02d0bf37c9e592"
            ],
            "merge_timestamp": "2026-02-01T12:00:00Z",
            "nodes_merged": 5,
            "edges_merged": 3,
            "conflicts_detected": 1,
            "contributor_id": "merge-system-001",
            "signature": "dGVzdC1tZXJnZS1zaWc="
        }),
    );
    validate_and_parse(&doc, &v);
}

#[test]
fn fixture_geo_point() {
    let v = validator();
    let node = json!({
        "id": "fac-geo", "type": "facility", "name": "Geo Point Plant",
        "geo": {"lat": 53.3811, "lon": -1.4701}
    });
    validate_and_parse(&base_file_with(vec![node], vec![]), &v);
}

#[test]
fn fixture_geo_geojson() {
    let v = validator();
    let node = json!({
        "id": "fac-geo", "type": "facility", "name": "Geo Polygon Plant",
        "geo": {
            "type": "Polygon",
            "coordinates": [[[100.0, 0.0], [101.0, 0.0], [101.0, 1.0], [100.0, 1.0], [100.0, 0.0]]]
        }
    });
    validate_and_parse(&base_file_with(vec![node], vec![]), &v);
}

#[test]
fn fixture_conflicts() {
    let v = validator();
    let node = json!({
        "id": "org-c", "type": "organization", "name": "Conflicted Corp",
        "_conflicts": [
            {"field": "name", "values": [
                {"value": "Name A", "source_file": "file-a.omts"},
                {"value": "Name B", "source_file": "file-b.omts"}
            ]}
        ]
    });
    let e = json!({
        "id": "e-c", "type": "supplies", "source": "org-c", "target": "org-d",
        "properties": {"valid_from": "2025-01-01"},
        "_conflicts": [
            {"field": "commodity", "values": [
                {"value": "Steel", "source_file": "file-a.omts"},
                {"value": "Iron", "source_file": "file-b.omts"}
            ]}
        ]
    });
    let doc = base_file_with(vec![node, org_node("org-d", "Other")], vec![e]);
    validate_and_parse(&doc, &v);
}

#[test]
fn fixture_extension_properties() {
    let v = validator();
    let mut node = org_node("org-ext", "Extension Corp");
    node.as_object_mut()
        .expect("obj")
        .insert("com.example.custom_field".to_owned(), json!("custom-value"));

    let mut e = edge(
        "e-ext",
        "supplies",
        "org-ext",
        "org-tgt",
        json!({
            "valid_from": "2025-01-01",
            "com.example.methodology": "ISO-14064"
        }),
    );
    e.as_object_mut()
        .expect("obj")
        .insert("com.example.edge_tag".to_owned(), json!("tagged"));

    let doc = base_file_with(vec![node, org_node("org-tgt", "Target")], vec![e]);
    validate_and_parse(&doc, &v);
}

#[test]
fn fixture_data_quality_all_confidence() {
    let v = validator();
    for conf in &["verified", "reported", "inferred", "estimated"] {
        let node = json!({
            "id": "org-dq", "type": "organization", "name": "DQ Test",
            "data_quality": {"confidence": conf, "source": "test", "last_verified": "2026-01-01"}
        });
        validate_and_parse(&base_file_with(vec![node], vec![]), &v);
    }
}

#[test]
fn fixture_kitchen_sink() {
    let v = validator();

    let nodes = vec![
        json!({
            "id": "org-main", "type": "organization", "name": "Main Corp",
            "jurisdiction": "US", "status": "active", "governance_structure": "joint_venture",
            "identifiers": [
                {"scheme": "lei", "value": "5493006MHB84DD0ZWV18", "sensitivity": "public"},
                {"scheme": "duns", "value": "081466849"},
                {"scheme": "nat-reg", "value": "12345", "authority": "SEC"},
                {"scheme": "vat", "value": "US123", "authority": "US"},
                {"scheme": "internal", "value": "INT-1", "authority": "erp"}
            ],
            "data_quality": {"confidence": "verified", "source": "manual"},
            "com.example.custom": "value"
        }),
        json!({
            "id": "org-sub", "type": "organization", "name": "Subsidiary Ltd",
            "jurisdiction": "GB", "status": "active"
        }),
        json!({
            "id": "fac-1", "type": "facility", "name": "Factory Alpha",
            "operator": "org-main",
            "address": "1 Industrial Ave",
            "geo": {"lat": 40.7128, "lon": -74.0060},
            "identifiers": [{"scheme": "gln", "value": "5060012340018"}]
        }),
        json!({
            "id": "fac-2", "type": "facility", "name": "Farm Beta",
            "geo": {
                "type": "Polygon",
                "coordinates": [[[10.0, 50.0], [11.0, 50.0], [11.0, 51.0], [10.0, 51.0], [10.0, 50.0]]]
            }
        }),
        json!({
            "id": "good-1", "type": "good", "name": "Widget A",
            "commodity_code": "8471.30", "unit": "pcs",
            "identifiers": [{"scheme": "org.gs1.gtin", "value": "12345678901234"}]
        }),
        good_node("good-2", "Component B"),
        json!({
            "id": "per-1", "type": "person", "name": "John Smith",
            "jurisdiction": "US", "role": "Director",
            "identifiers": [{"scheme": "internal", "value": "DIR-001", "authority": "hr", "sensitivity": "confidential"}]
        }),
        json!({
            "id": "att-1", "type": "attestation", "name": "ISO 14001 Cert",
            "attestation_type": "certification", "standard": "ISO 14001:2015",
            "issuer": "Bureau Veritas",
            "valid_from": "2025-01-01", "valid_to": "2028-01-01",
            "outcome": "pass", "status": "active",
            "reference": "CERT-ENV-2025", "risk_severity": "medium", "risk_likelihood": "possible"
        }),
        json!({
            "id": "att-2", "type": "attestation", "name": "Self Declaration",
            "attestation_type": "self_declaration",
            "valid_from": "2026-01-01", "valid_to": null,
            "outcome": "not_applicable", "status": "active"
        }),
        json!({
            "id": "con-1", "type": "consignment", "name": "Batch 2026-Q1",
            "lot_id": "LOT-001", "quantity": 10000, "unit": "kg",
            "production_date": "2026-01-15", "origin_country": "DE",
            "direct_emissions_co2e": 50.0, "indirect_emissions_co2e": 15.0,
            "emission_factor_source": "default_eu", "installation_id": "fac-1"
        }),
        json!({
            "id": "bref-1", "type": "boundary_ref", "name": "Redacted Supplier",
            "identifiers": [
                {"scheme": "opaque", "value": "a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4"}
            ]
        }),
    ];

    let edges = vec![
        edge(
            "e-own",
            "ownership",
            "org-main",
            "org-sub",
            json!({
                "percentage": 100.0, "direct": true,
                "valid_from": "2020-01-01", "valid_to": null,
                "data_quality": {"confidence": "verified"},
                "_property_sensitivity": {"percentage": "restricted"}
            }),
        ),
        edge(
            "e-oc",
            "operational_control",
            "org-main",
            "org-sub",
            json!({
                "control_type": "management", "valid_from": "2020-01-01"
            }),
        ),
        edge(
            "e-lp",
            "legal_parentage",
            "org-main",
            "org-sub",
            json!({
                "valid_from": "2020-01-01", "consolidation_basis": "ifrs10"
            }),
        ),
        edge(
            "e-fi",
            "former_identity",
            "org-main",
            "org-sub",
            json!({
                "event_type": "acquisition", "effective_date": "2020-01-01",
                "description": "Acquired subsidiary"
            }),
        ),
        edge(
            "e-bo",
            "beneficial_ownership",
            "per-1",
            "org-main",
            json!({
                "percentage": 75.0, "control_type": "voting_rights",
                "direct": true, "valid_from": "2015-01-01", "valid_to": null
            }),
        ),
        edge(
            "e-sup",
            "supplies",
            "org-sub",
            "org-main",
            json!({
                "valid_from": "2023-01-01", "commodity": "8471.30",
                "contract_ref": "PO-2023", "volume": 50000, "volume_unit": "pcs",
                "annual_value": 1000000, "value_currency": "USD",
                "tier": 1, "share_of_buyer_demand": 60.0
            }),
        ),
        edge(
            "e-sc",
            "subcontracts",
            "org-sub",
            "org-main",
            json!({
                "valid_from": "2024-01-01", "commodity": "Assembly",
                "contract_ref": "SC-001", "volume": 5000, "volume_unit": "units",
                "share_of_buyer_demand": 10.0
            }),
        ),
        edge(
            "e-tol",
            "tolls",
            "org-sub",
            "org-main",
            json!({
                "valid_from": "2023-06-01", "commodity": "Raw material"
            }),
        ),
        edge(
            "e-dist",
            "distributes",
            "org-sub",
            "org-main",
            json!({
                "valid_from": "2024-01-01", "service_type": "transport"
            }),
        ),
        edge(
            "e-brk",
            "brokers",
            "org-sub",
            "org-main",
            json!({
                "valid_from": "2024-06-01", "commodity": "Rare earths"
            }),
        ),
        edge(
            "e-op",
            "operates",
            "org-main",
            "fac-1",
            json!({
                "valid_from": "2018-01-01"
            }),
        ),
        edge(
            "e-pr",
            "produces",
            "fac-1",
            "good-1",
            json!({
                "valid_from": "2020-01-01"
            }),
        ),
        edge(
            "e-comp",
            "composed_of",
            "good-1",
            "good-2",
            json!({
                "quantity": 4, "unit": "pcs", "valid_from": "2024-01-01"
            }),
        ),
        edge(
            "e-sell",
            "sells_to",
            "org-main",
            "org-sub",
            json!({
                "valid_from": "2025-01-01", "commodity": "Widgets", "contract_ref": "SA-001"
            }),
        ),
        edge(
            "e-att",
            "attested_by",
            "fac-1",
            "att-1",
            json!({
                "scope": "environmental"
            }),
        ),
        edge(
            "e-sa",
            "same_as",
            "org-sub",
            "bref-1",
            json!({
                "confidence": "probable", "basis": "lei_match"
            }),
        ),
    ];

    let mut doc = base_file_with(nodes, edges);
    let obj = doc.as_object_mut().expect("object");
    obj.insert("disclosure_scope".to_owned(), json!("partner"));
    obj.insert(
        "previous_snapshot_ref".to_owned(),
        json!("abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789"),
    );
    obj.insert("snapshot_sequence".to_owned(), json!(5));
    obj.insert("reporting_entity".to_owned(), json!("org-main"));
    obj.insert(
        "file_integrity".to_owned(),
        json!({
            "content_hash": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            "algorithm": "sha-256"
        }),
    );
    obj.insert(
        "merge_metadata".to_owned(),
        json!({
            "source_file_hashes": [
                "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
            ],
            "merge_timestamp": "2026-02-01T00:00:00Z",
            "nodes_merged": 2, "edges_merged": 1,
            "conflicts_detected": 0, "contributor_id": "system"
        }),
    );

    validate_and_parse(&doc, &v);
}
