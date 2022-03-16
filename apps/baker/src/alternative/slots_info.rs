// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{collections::BTreeMap, convert::TryInto, mem};

use crypto::hash::ContractTz1Hash;
use tenderbake as tb;
use tezos_messages::protocol::proto_012::operation::EndorsementOperation;

pub struct SlotsInfo {
    committee_size: u32,
    ours: Vec<ContractTz1Hash>,
    level: i32,
    this_level: BTreeMap<ContractTz1Hash, Vec<u16>>,
    next_level: BTreeMap<ContractTz1Hash, Vec<u16>>,
}

impl SlotsInfo {
    pub fn new(committee_size: u32, ours: Vec<ContractTz1Hash>) -> Self {
        SlotsInfo {
            committee_size,
            ours,
            level: 0,
            this_level: BTreeMap::new(),
            next_level: BTreeMap::new(),
        }
    }

    pub fn level(&self) -> i32 {
        self.level
    }

    pub fn insert(&mut self, level: i32, delegates: BTreeMap<ContractTz1Hash, Vec<u16>>) {
        self.level = level - 1;
        mem::swap(&mut self.this_level, &mut self.next_level);
        self.next_level = delegates;
    }

    pub fn slots(&self, id: &ContractTz1Hash, level: i32) -> Option<&Vec<u16>> {
        if level == self.level {
            self.this_level.get(id)
        } else if level == self.level + 1 {
            self.next_level.get(id)
        } else {
            None
        }
    }

    pub fn validator(&self, level: i32, slot: u16) -> Option<tb::Validator<ContractTz1Hash>> {
        let i = if level == self.level {
            &self.this_level
        } else if level == self.level + 1 {
            &self.next_level
        } else {
            return None;
        };
        let (id, s) = i.iter().find(|&(_, v)| v.first() == Some(&slot))?;
        Some(tb::Validator {
            id: id.clone(),
            power: s.len() as u32,
        })
    }

    fn block_id(content: &EndorsementOperation) -> tb::BlockId {
        tb::BlockId {
            level: content.level,
            round: content.round,
            payload_hash: content
                .block_payload_hash
                .0
                .as_slice()
                .try_into()
                .expect("payload hash is 32 bytes"),
            payload_round: content.round,
        }
    }

    pub fn preendorsement(
        &self,
        content: &EndorsementOperation,
    ) -> Option<tb::Preendorsement<ContractTz1Hash>> {
        Some(tb::Preendorsement {
            validator: self.validator(content.level, content.slot)?,
            block_id: Self::block_id(content),
        })
    }

    pub fn endorsement(
        &self,
        content: &EndorsementOperation,
    ) -> Option<tb::Endorsement<ContractTz1Hash>> {
        Some(tb::Endorsement {
            validator: self.validator(content.level, content.slot)?,
            block_id: Self::block_id(content),
        })
    }
}

impl tb::ValidatorMap for SlotsInfo {
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
