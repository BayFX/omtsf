//! Benchmark measuring the overhead of `#[serde(flatten)]` vs
//! `#[serde(deny_unknown_fields)]` for CBOR and JSON deserialization.
//!
//! The delta between the `flat` and `strict` variants isolates the
//! Content-buffering overhead introduced by serde's `flatten` implementation.
#![allow(clippy::expect_used)]

use std::collections::BTreeMap;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use omtsf_core::DynMap;
use serde::{Deserialize, Serialize};

const SMALL: usize = 50;
const MEDIUM: usize = 500;
const LARGE: usize = 2000;

/// Simplified node with `#[serde(flatten)]` for unknown-field capture.
///
/// Mimics the structure of `omtsf_core::Node` with a representative subset
/// of fields. The `extra` field activates serde's Content-buffering path
/// during deserialization, which is the overhead under measurement.
#[derive(Serialize, Deserialize)]
struct NodeFlat {
    id: String,
    #[serde(rename = "type")]
    node_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    jurisdiction: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    quantity: Option<f64>,
    /// Captures unknown fields; empty in benchmark data so the wire payload
    /// is bit-for-bit identical to the `NodeStrict` representation.
    #[serde(flatten)]
    extra: DynMap,
}

/// Simplified node without flatten, using `#[serde(deny_unknown_fields)]`.
///
/// Structurally identical to [`NodeFlat`] except that unknown fields are
/// rejected rather than captured, allowing serde to skip the
/// Content-buffering machinery entirely. Serves as the baseline.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct NodeStrict {
    id: String,
    #[serde(rename = "type")]
    node_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    jurisdiction: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    quantity: Option<f64>,
}

fn make_flat_nodes(count: usize) -> Vec<NodeFlat> {
    (0..count)
        .map(|i| NodeFlat {
            id: format!("node-{i}"),
            node_type: "organization".to_owned(),
            name: Some(format!("Supplier Org {i}")),
            jurisdiction: Some("US".to_owned()),
            status: Some("active".to_owned()),
            address: Some(format!("{i} Commerce Blvd, City {}", i % 50)),
            role: None,
            quantity: Some(i as f64 * 1.5),
            extra: BTreeMap::new(),
        })
        .collect()
}

fn make_strict_nodes(count: usize) -> Vec<NodeStrict> {
    (0..count)
        .map(|i| NodeStrict {
            id: format!("node-{i}"),
            node_type: "organization".to_owned(),
            name: Some(format!("Supplier Org {i}")),
            jurisdiction: Some("US".to_owned()),
            status: Some("active".to_owned()),
            address: Some(format!("{i} Commerce Blvd, City {}", i % 50)),
            role: None,
            quantity: Some(i as f64 * 1.5),
        })
        .collect()
}

fn cbor_encode<T: serde::Serialize>(value: &T) -> Vec<u8> {
    let mut buf = Vec::new();
    ciborium::into_writer(value, &mut buf).expect("ciborium encode");
    buf
}

fn bench_flatten_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("flatten_overhead");

    for (label, count) in [("S", SMALL), ("M", MEDIUM), ("L", LARGE)] {
        let flat = make_flat_nodes(count);
        let strict = make_strict_nodes(count);

        let json_flat = serde_json::to_vec(&flat).expect("json encode flat");
        let json_strict = serde_json::to_vec(&strict).expect("json encode strict");
        let cbor_flat = cbor_encode(&flat);
        let cbor_strict = cbor_encode(&strict);

        group.throughput(Throughput::Bytes(json_flat.len() as u64));
        group.bench_with_input(
            BenchmarkId::new("json_flat", label),
            &json_flat,
            |b, bytes| {
                b.iter(|| {
                    let _: Vec<NodeFlat> = serde_json::from_slice(bytes).expect("json decode flat");
                });
            },
        );

        group.throughput(Throughput::Bytes(json_strict.len() as u64));
        group.bench_with_input(
            BenchmarkId::new("json_strict", label),
            &json_strict,
            |b, bytes| {
                b.iter(|| {
                    let _: Vec<NodeStrict> =
                        serde_json::from_slice(bytes).expect("json decode strict");
                });
            },
        );

        group.throughput(Throughput::Bytes(cbor_flat.len() as u64));
        group.bench_with_input(
            BenchmarkId::new("cbor_flat", label),
            &cbor_flat,
            |b, bytes| {
                b.iter(|| {
                    let _: Vec<NodeFlat> =
                        ciborium::from_reader(bytes.as_slice()).expect("cbor decode flat");
                });
            },
        );

        group.throughput(Throughput::Bytes(cbor_strict.len() as u64));
        group.bench_with_input(
            BenchmarkId::new("cbor_strict", label),
            &cbor_strict,
            |b, bytes| {
                b.iter(|| {
                    let _: Vec<NodeStrict> =
                        ciborium::from_reader(bytes.as_slice()).expect("cbor decode strict");
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_flatten_overhead);
criterion_main!(benches);
