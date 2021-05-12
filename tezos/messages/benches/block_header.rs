// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use criterion::{criterion_group, criterion_main, Criterion};
use tezos_messages::p2p::encoding::block_header::BlockHeaderMessage;

mod decoding_bench;
use decoding_bench::*;

const DATA: &str = "\
00094F1F048D51777EF01C0106A09F747615CC72271A46EA75E097B48C7200CA2F1EAE6617000000\
005D7F495004C8626895CC82299089F495F7AD8864D1D3B0F364D497F1D175296B5F4A901EC80000\
001100000001000000000800000000012631B27A9F0E1DA2D2CA10202938298CFB1133D5F9A642F8\
1E6697342263B6ECB621F10000000000032DB85C0E00961D14664ECBDF10CBE4DE7DD71096A4E1A1\
77DB0890B13F0AB85999EB0D715E807BCA0438D3CEAA5C58560D60767F28A9E16326657FBE7FC841\
4FDE3C54A504\
";

fn block_header_decode_benchmark(c: &mut Criterion) {
    let data = hex::decode(DATA).unwrap();
    bench_decode_serde_nom::<BlockHeaderMessage>(c, "block-header", data);
}

criterion_group! {
    name = benches;
    config = Criterion::default();
    targets = block_header_decode_benchmark
}

criterion_main!(benches);
