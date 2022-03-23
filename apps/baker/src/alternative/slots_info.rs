// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{collections::BTreeMap, convert::TryInto};

use crypto::hash::ContractTz1Hash;
use tenderbake as tb;
use tezos_messages::protocol::proto_012::operation::EndorsementOperation;

use super::event::OperationSimple;

pub struct SlotsInfo {
    committee_size: u32,
    ours: Vec<ContractTz1Hash>,
    level: i32,
    delegates: BTreeMap<i32, BTreeMap<ContractTz1Hash, Vec<u16>>>,
}

impl SlotsInfo {
    pub fn new(committee_size: u32, ours: Vec<ContractTz1Hash>) -> Self {
        SlotsInfo {
            committee_size,
            ours,
            level: 0,
            delegates: BTreeMap::new(),
        }
    }

    pub fn level(&self) -> i32 {
        self.level
    }

    pub fn insert(&mut self, level: i32, delegates: BTreeMap<ContractTz1Hash, Vec<u16>>) {
        self.level = level - 1;
        self.delegates.insert(level, delegates);
    }

    pub fn slots(&self, id: &ContractTz1Hash, level: i32) -> Option<&Vec<u16>> {
        self.delegates.get(&level)?.get(id)
    }

    pub fn validator(
        &self,
        level: i32,
        slot: u16,
        operation: OperationSimple,
    ) -> Option<tb::Validator<ContractTz1Hash, OperationSimple>> {
        let i = self.delegates.get(&level)?;
        let (id, s) = i.iter().find(|&(_, v)| v.first() == Some(&slot))?;
        Some(tb::Validator {
            id: id.clone(),
            power: s.len() as u32,
            operation,
        })
    }

    pub fn block_id(content: &EndorsementOperation) -> tb::BlockId {
        tb::BlockId {
            level: content.level,
            round: content.round,
            payload_hash: {
                let c = content
                    .block_payload_hash
                    .0
                    .as_slice()
                    .try_into()
                    .expect("payload hash is 32 bytes");
                tb::PayloadHash(c)
            },
        }
    }
}

impl tb::ProposerMap for SlotsInfo {
    type Id = ContractTz1Hash;

    fn proposer(&self, level: i32, round: i32) -> Option<(i32, Self::Id)> {
        self.ours
            .iter()
            .filter_map(|our| {
                self.slots(our, level)
                    .into_iter()
                    .flatten()
                    .skip_while(|c| **c < (round as u32 % self.committee_size) as u16)
                    .next()
                    .map(|r| (*r as i32, our.clone()))
            })
            .min_by(|(a, _), (b, _)| a.cmp(b))
    }
}
