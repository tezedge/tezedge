// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use criterion::{criterion_group, criterion_main, Criterion};
use tezos_messages::p2p::encoding::current_branch::CurrentBranchMessage;

mod decoding_bench;
use decoding_bench::*;

fn current_head_benchmark(c: &mut Criterion) {
    let data = read_data_unwrap("current-branch-big.msg");
    bench_decode_serde_nom::<CurrentBranchMessage>(c, "current-branch", data);
}

criterion_group! {
    name = benches;
    config = Criterion::default();
    targets = current_head_benchmark
}

criterion_main!(benches);
