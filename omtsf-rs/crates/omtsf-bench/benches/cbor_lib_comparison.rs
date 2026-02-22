//! CBOR library comparison: `serde_json` vs ciborium vs cbor4ii.
//!
//! Evaluates `cbor4ii` 1.x as a potential faster replacement for `ciborium` 0.2,
//! using `serde_json` as a baseline. All three encode/decode the same `OmtsFile`
//! at S, M, and L size tiers.
//!
//! # Self-describing tag
//!
//! `cbor4ii` does not prepend the self-describing CBOR tag 55799 automatically.
//! The helpers below prepend `[0xD9, 0xD9, 0xF7]` on encode and strip it on
//! decode to produce byte-for-byte equivalent output to `omtsf_core::cbor`.
#![allow(clippy::expect_used)]

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use omtsf_bench::{SizeTier, generate_supply_chain};
use omtsf_core::OmtsFile;
use omtsf_core::cbor::{decode_cbor, encode_cbor};

const SELF_DESCRIBING_TAG: [u8; 3] = [0xD9, 0xD9, 0xF7];

fn cbor4ii_encode(file: &OmtsFile) -> Vec<u8> {
    let buf = Vec::from(SELF_DESCRIBING_TAG);
    cbor4ii::serde::to_vec(buf, file).expect("cbor4ii encode")
}

fn cbor4ii_decode(bytes: &[u8]) -> OmtsFile {
    let payload = if bytes.starts_with(&SELF_DESCRIBING_TAG) {
        &bytes[3..]
    } else {
        bytes
    };
    cbor4ii::serde::from_slice(payload).expect("cbor4ii decode")
}

fn bench_decode(c: &mut Criterion) {
    let mut group = c.benchmark_group("cbor_lib_comparison/decode");

    for (name, tier) in [
        ("S", SizeTier::Small),
        ("M", SizeTier::Medium),
        ("L", SizeTier::Large),
    ] {
        let file = generate_supply_chain(&tier.config(42));

        let json = serde_json::to_string(&file).expect("serialize");
        let json_len = json.len() as u64;
        group.throughput(Throughput::Bytes(json_len));
        group.bench_with_input(BenchmarkId::new("serde_json", name), &json, |b, json| {
            b.iter(|| {
                let _: OmtsFile = serde_json::from_str(json).expect("json decode");
            });
        });

        let ciborium_bytes = encode_cbor(&file).expect("ciborium encode");
        let ciborium_len = ciborium_bytes.len() as u64;
        group.throughput(Throughput::Bytes(ciborium_len));
        group.bench_with_input(
            BenchmarkId::new("ciborium", name),
            &ciborium_bytes,
            |b, bytes| {
                b.iter(|| {
                    let _ = decode_cbor(bytes).expect("ciborium decode");
                });
            },
        );

        let cbor4ii_bytes = cbor4ii_encode(&file);
        let cbor4ii_len = cbor4ii_bytes.len() as u64;
        group.throughput(Throughput::Bytes(cbor4ii_len));
        group.bench_with_input(
            BenchmarkId::new("cbor4ii", name),
            &cbor4ii_bytes,
            |b, bytes| {
                b.iter(|| {
                    let _ = cbor4ii_decode(bytes);
                });
            },
        );
    }

    group.finish();
}

fn bench_encode(c: &mut Criterion) {
    let mut group = c.benchmark_group("cbor_lib_comparison/encode");

    for (name, tier) in [
        ("S", SizeTier::Small),
        ("M", SizeTier::Medium),
        ("L", SizeTier::Large),
    ] {
        let file = generate_supply_chain(&tier.config(42));

        let json = serde_json::to_string(&file).expect("serialize");
        let json_len = json.len() as u64;
        group.throughput(Throughput::Bytes(json_len));
        group.bench_with_input(BenchmarkId::new("serde_json", name), &file, |b, file| {
            b.iter(|| {
                let _ = serde_json::to_string(file).expect("json encode");
            });
        });

        let ciborium_bytes = encode_cbor(&file).expect("ciborium encode");
        let ciborium_len = ciborium_bytes.len() as u64;
        group.throughput(Throughput::Bytes(ciborium_len));
        group.bench_with_input(BenchmarkId::new("ciborium", name), &file, |b, file| {
            b.iter(|| {
                let _ = encode_cbor(file).expect("ciborium encode");
            });
        });

        let cbor4ii_bytes = cbor4ii_encode(&file);
        let cbor4ii_len = cbor4ii_bytes.len() as u64;
        group.throughput(Throughput::Bytes(cbor4ii_len));
        group.bench_with_input(BenchmarkId::new("cbor4ii", name), &file, |b, file| {
            b.iter(|| {
                let _ = cbor4ii_encode(file);
            });
        });
    }

    group.finish();
}

criterion_group!(benches, bench_decode, bench_encode);
criterion_main!(benches);
