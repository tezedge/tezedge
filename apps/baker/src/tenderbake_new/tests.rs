// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::tenderbake_new::{self as tb, hash, NoValue};

#[test]
fn machine() {
    const COMMITTEE_SIZE: i32 = 4;

    struct SlotsInfo(i32, hash::ContractTz1Hash);

    impl tb::ProposerMap for SlotsInfo {
        fn proposer(&self, level: i32, round: i32) -> Option<(i32, hash::ContractTz1Hash)> {
            let c = (self.0 + level + round) % COMMITTEE_SIZE;
            let round = if c == 0 {
                round
            } else {
                round + COMMITTEE_SIZE - c
            };
            Some((round, self.1.clone()))
        }
    }

    let participants = (0..COMMITTEE_SIZE)
        .map(|id| SlotsInfo(id, hash::ContractTz1Hash::no_value()))
        .collect::<Vec<_>>();

    let machine = tb::Machine::default();

    let _ = (participants, machine);
}
