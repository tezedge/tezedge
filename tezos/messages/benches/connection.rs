// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use criterion::{criterion_group, criterion_main, Criterion};
use tezos_messages::p2p::encoding::connection::ConnectionMessage;

mod decoding_bench;
use decoding_bench::*;

const DATA: &str = "\
260470387c4cbd115833626a88a9f2b9bbadd70705f5ad059f494655ac823620c6410ec0d9b1716a\
d6d59d0cc551c96992c8031df8c77735d21158dc510aabbca27bd5fff7f085912327012a4d91e587\
e3bc0000000d54455a4f535f4d41494e4e455400000001\
";

fn connection_benchmark(c: &mut Criterion) {
    let data = hex::decode(DATA).unwrap();
    bench_decode_serde_nom_raw::<ConnectionMessage>(c, "connection", data);
}

criterion_group! {
    name = benches;
    config = Criterion::default();
    targets = connection_benchmark
}

criterion_main!(benches);
