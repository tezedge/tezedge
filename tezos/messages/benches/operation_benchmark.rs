// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::path::Path;

use criterion::{criterion_group, criterion_main, Criterion};

mod codecs_bench_common;
use codecs_bench_common::*;

use tezos_messages::protocol::proto_009::operation::Operation;

const DATA_DIR: &str = "operations/009_florence";

macro_rules! operation_benches {
	($($name:ident),*) => {
		$(operation_benches!{ test $name })*
        criterion_group! {
            name = benches;
            config = Criterion::default();
            targets = $($name),*,
        }
        criterion_main!(benches);
	};

	(test $name:ident) => {
        fn $name(c: &mut Criterion) {
            let data = read_data_unwrap(Path::new(DATA_DIR).join(stringify!($name.bin)));
            bench_decode::<Operation>(c, stringify!($name), &data);
        }
    }
}

operation_benches!(
    endorsement,
    activate_account,
    ballot,
    delegation_withdrawal,
    delegation,
    endorsement_with_slot,
    seed_nonce_revelation,
    proposals,
    double_endorsement_evidence,
    reveal,
    origination,
    transaction_to_implicit,
    transaction_to_originated,
    double_baking_evidence
);
