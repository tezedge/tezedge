// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

#![forbid(unsafe_code)]
#![cfg_attr(feature = "fuzzing", feature(no_coverage))]

mod proof_of_work;

pub mod machine;

mod services;
pub use self::services::{
    client::{LiquidityBakingToggleVote, Protocol, ProtocolBlockHeaderI, ProtocolBlockHeaderJ},
    EventWithTime, Services,
};

pub use tenderbake_new::Timestamp;

pub mod tenderbake_new;

#[cfg(all(test, not(feature = "testing-mock")))]
#[ignore]
#[test]
fn scanner() {
    use std::collections::BTreeMap;
    use tenderbake_new::OperationSimple;

    let (tx, _) = std::sync::mpsc::channel();
    let client =
        services::client::RpcClient::new("https://mainnet-tezos.giganode.io".parse().unwrap(), tx);

    let mut counters = BTreeMap::<String, usize>::new();
    let cycle = 490;
    for i in 0..64 {
        println!("{i}: {counters:?}");

        for n in 1..4 {
            let ops_list = client
                .get_operations_for_level(cycle * 4096 + i, n)
                .unwrap();
            for op in ops_list {
                match op {
                    OperationSimple::Preendorsement(_) => panic!(),
                    OperationSimple::Endorsement(_) => panic!(),
                    OperationSimple::Votes(op) => {
                        assert_eq!(n, 1);
                        for content in &op.contents {
                            let kind = content
                                .as_object()
                                .unwrap()
                                .get("kind")
                                .unwrap()
                                .as_str()
                                .unwrap();
                            *counters.entry(kind.to_string()).or_default() += 1;
                        }
                    }
                    OperationSimple::Anonymous(op) => {
                        assert_eq!(n, 2);
                        for content in &op.contents {
                            let kind = content
                                .as_object()
                                .unwrap()
                                .get("kind")
                                .unwrap()
                                .as_str()
                                .unwrap();
                            *counters.entry(kind.to_string()).or_default() += 1;
                        }
                    }
                    OperationSimple::Managers(op) => {
                        assert_eq!(n, 3);
                        for content in &op.contents {
                            let kind = content
                                .as_object()
                                .unwrap()
                                .get("kind")
                                .unwrap()
                                .as_str()
                                .unwrap();
                            *counters.entry(kind.to_string()).or_default() += 1;
                        }
                    }
                }
            }
        }
    }
    println!("{counters:?}");
}
