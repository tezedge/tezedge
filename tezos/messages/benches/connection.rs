// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use tezos_encoding::nom::NomReader;
use tezos_messages::p2p::{binary_message::BinaryMessage, encoding::connection::ConnectionMessage};

fn connection_benchmark(c: &mut Criterion) {
    let data = hex::decode("260470387c4cbd115833626a88a9f2b9bbadd70705f5ad059f494655ac823620c6410ec0d9b1716ad6d59d0cc551c96992c8031df8c77735d21158dc510aabbca27bd5fff7f085912327012a4d91e587e3bc0000000d54455a4f535f4d41494e4e455400000001").unwrap();
    c.bench_function("connection_decode_serde", |b| {
        b.iter(|| {
            use tezos_encoding::encoding::HasEncoding;
            let value = tezos_encoding::binary_reader::BinaryReader::new().read(&data, &ConnectionMessage::encoding()).unwrap();
            let _: ConnectionMessage = tezos_encoding::de::from_value(&value).unwrap();
        });
    });
    c.bench_function("connection_decode_nom", |b| {
        b.iter(|| {
            let _ = ConnectionMessage::from_bytes_nom(&data).unwrap();
        });
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default();
    targets = connection_benchmark
}

criterion_main!(benches);
