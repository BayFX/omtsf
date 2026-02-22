//! Group 1: Parse and serialize benchmarks (JSON and CBOR).
#![allow(clippy::expect_used)]

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use omtsf_bench::{SizeTier, generate_supply_chain};
use omtsf_core::OmtsFile;
use omtsf_core::cbor::{decode_cbor, encode_cbor};

fn bench_deserialize(c: &mut Criterion) {
    let mut group = c.benchmark_group("deserialize");

    for (name, tier) in [
        ("S", SizeTier::Small),
        ("M", SizeTier::Medium),
        ("L", SizeTier::Large),
        ("XL", SizeTier::XLarge),
    ] {
        let file = generate_supply_chain(&tier.config(42));
        let json = serde_json::to_string(&file).expect("serialize");
        let bytes = json.len() as u64;

        group.throughput(Throughput::Bytes(bytes));
        group.bench_with_input(BenchmarkId::new("json", name), &json, |b, json| {
            b.iter(|| {
                let _: OmtsFile = serde_json::from_str(json).expect("deserialize");
            });
        });

        let cbor_bytes = encode_cbor(&file).expect("encode cbor");
        let cbor_len = cbor_bytes.len() as u64;
        group.throughput(Throughput::Bytes(cbor_len));
        group.bench_with_input(
            BenchmarkId::new("cbor", name),
            &cbor_bytes,
            |b, cbor_bytes| {
                b.iter(|| {
                    let _ = decode_cbor(cbor_bytes).expect("decode cbor");
                });
            },
        );
    }
    group.finish();
}

fn bench_serialize_compact(c: &mut Criterion) {
    let mut group = c.benchmark_group("serialize_compact");

    for (name, tier) in [
        ("S", SizeTier::Small),
        ("M", SizeTier::Medium),
        ("L", SizeTier::Large),
        ("XL", SizeTier::XLarge),
    ] {
        let file = generate_supply_chain(&tier.config(42));
        let json = serde_json::to_string(&file).expect("serialize");
        let bytes = json.len() as u64;

        group.throughput(Throughput::Bytes(bytes));
        group.bench_with_input(BenchmarkId::new("json", name), &file, |b, file| {
            b.iter(|| {
                let _ = serde_json::to_string(file).expect("serialize");
            });
        });

        let cbor_bytes = encode_cbor(&file).expect("encode cbor");
        let cbor_len = cbor_bytes.len() as u64;
        group.throughput(Throughput::Bytes(cbor_len));
        group.bench_with_input(BenchmarkId::new("cbor", name), &file, |b, file| {
            b.iter(|| {
                let _ = encode_cbor(file).expect("encode cbor");
            });
        });
    }
    group.finish();
}

fn bench_serialize_pretty(c: &mut Criterion) {
    let mut group = c.benchmark_group("serialize_pretty");

    for (name, tier) in [
        ("S", SizeTier::Small),
        ("M", SizeTier::Medium),
        ("L", SizeTier::Large),
        ("XL", SizeTier::XLarge),
    ] {
        let file = generate_supply_chain(&tier.config(42));
        let json = serde_json::to_string_pretty(&file).expect("serialize");
        let bytes = json.len() as u64;

        group.throughput(Throughput::Bytes(bytes));
        group.bench_with_input(BenchmarkId::new("json", name), &file, |b, file| {
            b.iter(|| {
                let _ = serde_json::to_string_pretty(file).expect("serialize");
            });
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_deserialize,
    bench_serialize_compact,
    bench_serialize_pretty,
);
criterion_main!(benches);
