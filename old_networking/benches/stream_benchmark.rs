// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::time::Duration;

use criterion::{criterion_group, criterion_main, Criterion};

use tezos_messages::p2p::binary_message::BinaryWrite;
use tezos_messages::p2p::encoding::{
    limits::BLOCK_HEADER_MAX_SIZE,
    peer::{PeerMessage, PeerMessageResponse},
    swap::SwapMessage,
};

pub mod common;

use self::common::{block_header_message_encoded, read_message_bench, CryptoMock};

fn read_message_small(c: &mut Criterion) {
    let crypto_mock = CryptoMock::new();
    let message = SwapMessage::new("0.0.0.0:1234".to_string(), crypto_mock.remote.pkh);
    let message = PeerMessageResponse::from(PeerMessage::SwapRequest(message));
    let message = message.as_bytes().unwrap();

    read_message_bench(c, message, None);
}

fn read_message_medium(c: &mut Criterion) {
    read_message_bench(c, block_header_message_encoded(256), None);
}

fn read_message_big(c: &mut Criterion) {
    read_message_bench(c, block_header_message_encoded(1024), None);
}

fn read_message_8mib(c: &mut Criterion) {
    read_message_bench(c, block_header_message_encoded(BLOCK_HEADER_MAX_SIZE), None);
}

fn read_message_8mib_short_chunks(c: &mut Criterion) {
    read_message_bench(
        c,
        block_header_message_encoded(BLOCK_HEADER_MAX_SIZE),
        Some(1024),
    );
}

fn read_message_8mib_micro_chunks(c: &mut Criterion) {
    read_message_bench(
        c,
        block_header_message_encoded(BLOCK_HEADER_MAX_SIZE),
        Some(256),
    );
}

criterion_group!(
    fast_benches,
    read_message_small,
    read_message_medium,
    read_message_big
);

criterion_group! {
    name = long_benches;
    config = Criterion::default().measurement_time(Duration::from_secs(120));
    targets = read_message_8mib, read_message_8mib_short_chunks, read_message_8mib_micro_chunks
}

criterion_main!(fast_benches, long_benches);
