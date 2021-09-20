// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use criterion::{criterion_group, criterion_main, Criterion};

mod codecs_bench_common;
use codecs_bench_common::*;
use tezos_messages::p2p::encoding::{
    ack::AckMessage,
    block_header::{BlockHeaderMessage, GetBlockHeadersMessage},
    connection::ConnectionMessage,
    current_head::{CurrentHeadMessage, GetCurrentHeadMessage},
    operations_for_blocks::{GetOperationsForBlocksMessage, OperationsForBlocksMessage},
    prelude::{CurrentBranchMessage, GetCurrentBranchMessage},
};

fn connection_benchmark(c: &mut Criterion) {
    let data = read_data_unwrap("connection.msg");
    bench_decode::<ConnectionMessage>(c, "connection", &data);
}

fn ack_benchmark(c: &mut Criterion) {
    let data = read_data_unwrap("ack.msg");
    bench_decode::<AckMessage>(c, "ack", &data)
}

fn get_current_branch_benchmark(c: &mut Criterion) {
    let data = read_data_unwrap("get-current-branch.msg");
    bench_decode::<GetCurrentBranchMessage>(c, "get-current-branch", &data);
}

fn current_branch_benchmark(c: &mut Criterion) {
    let data = read_data_unwrap("current-branch.big.msg");
    bench_decode::<CurrentBranchMessage>(c, "current-branch", &data);
}

fn get_current_head_benchmark(c: &mut Criterion) {
    let data = read_data_unwrap("get-current-head.msg");
    bench_decode::<GetCurrentHeadMessage>(c, "get-current-head", &data);
}

fn current_head_benchmark(c: &mut Criterion) {
    let data = read_data_unwrap("current-head.big.msg");
    bench_decode::<CurrentHeadMessage>(c, "current-head", &data);
}

fn get_block_headers_benchmark(c: &mut Criterion) {
    let data = read_data_unwrap("get-block-headers.msg");
    bench_decode::<GetBlockHeadersMessage>(c, "get-block-header", &data);
}

fn block_header_benchmark(c: &mut Criterion) {
    let data = read_data_unwrap("block-header.msg");
    bench_decode::<BlockHeaderMessage>(c, "block-header", &data);
}

fn get_operations_for_blocks_benchmark(c: &mut Criterion) {
    let data = read_data_unwrap("get-operations-for-blocks.msg");
    bench_decode::<GetOperationsForBlocksMessage>(c, "get-operations-for-blocks", &data);
}

fn operations_for_blocks_benchmark(c: &mut Criterion) {
    let data = read_data_unwrap("operations-for-blocks.huge.msg");
    bench_decode::<OperationsForBlocksMessage>(c, "operations-for-blocks", &data);
}

criterion_group! {
    name = benches;
    config = Criterion::default();
    targets = connection_benchmark, ack_benchmark,
    get_current_head_benchmark, current_head_benchmark,
    get_current_branch_benchmark, current_branch_benchmark,
    get_block_headers_benchmark, block_header_benchmark,
    get_operations_for_blocks_benchmark, operations_for_blocks_benchmark,
}

criterion_main!(benches);
