// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use tezos_messages::p2p::{binary_message::{BinaryMessageNom, BinaryMessageSerde}, encoding::block_header::BlockHeaderMessage};

fn connection_benchmark(c: &mut Criterion) {
    let data = hex::decode("00094F1F048D51777EF01C0106A09F747615CC72271A46EA75E097B48C7200CA2F1EAE6617000000005D7F495004C8626895CC82299089F495F7AD8864D1D3B0F364D497F1D175296B5F4A901EC80000001100000001000000000800000000012631B27A9F0E1DA2D2CA10202938298CFB1133D5F9A642F81E6697342263B6ECB621F10000000000032DB85C0E00961D14664ECBDF10CBE4DE7DD71096A4E1A177DB0890B13F0AB85999EB0D715E807BCA0438D3CEAA5C58560D60767F28A9E16326657FBE7FC8414FDE3C54A504").unwrap();
    c.bench_function("block_header_decode_serde", |b| {
        b.iter(|| {
            let _ = <BlockHeaderMessage as BinaryMessageSerde>::from_bytes(&data).unwrap();
        });
    });
    c.bench_function("block_header_decode_nom", |b| {
        b.iter(|| {
            let _ = <BlockHeaderMessage as BinaryMessageNom>::from_bytes(&data).unwrap();
        });
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default();
    targets = connection_benchmark
}

criterion_main!(benches);
