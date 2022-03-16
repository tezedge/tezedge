// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{collections::BTreeMap, convert::TryInto};

use crypto::hash::ContractTz1Hash;
use tenderbake as tb;
use tezos_messages::protocol::proto_012::operation::EndorsementOperation;

use super::event::{Block, Validator};

pub struct SlotsInfo {
    consensus_committee_size: u32,
    ours: Vec<ContractTz1Hash>,
    delegates: BTreeMap<i32, BTreeMap<ContractTz1Hash, Vec<u16>>>,
}

impl SlotsInfo {
    pub fn new(consensus_committee_size: u32, ours: Vec<ContractTz1Hash>) -> Self {
        SlotsInfo {
            consensus_committee_size,
            ours,
            delegates: BTreeMap::new(),
        }
    }

    pub fn insert(&mut self, block: &Block) {
        let delegates = block
            .validators
            .clone()
            .into_iter()
            .map(|Validator { delegate, slots }| (delegate, slots))
            .collect();
        self.delegates.insert(block.level + 1, delegates);
    }

    fn slots(&self, id: &ContractTz1Hash, level: i32) -> Option<&Vec<u16>> {
        self.delegates.get(&level)?.get(id)
    }

    pub fn slot(&self, id: &ContractTz1Hash, level: i32) -> Option<u16> {
        self.delegates
            .get(&level)?
            .get(id)?
            .first()
            .cloned()
    }

    pub fn validator(&self, level: i32, slot: u16) -> Option<tb::Validator<ContractTz1Hash>> {
        let i = self.delegates.get(&level)?;
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

    fn preendorser(&self, level: i32, round: i32) -> Option<tb::Validator<Self::Id>> {
        let _ = round;
        let this = self.ours[0].clone();
        Some(tb::Validator {
            id: this.clone(),
            power: self.slots(&this, level)?.len() as u32,
        })
    }

    fn endorser(&self, level: i32, round: i32) -> Option<tb::Validator<Self::Id>> {
        self.preendorser(level, round)
    }

    fn proposer(&self, level: i32, round: i32) -> Option<i32> {
        self.ours
            .iter()
            .filter_map(|our| {
                self.slots(our, level)
                    .into_iter()
                    .flatten()
                    .skip_while(|c| **c < (round as u32 % self.consensus_committee_size) as u16)
                    .next()
                    .map(|r| *r as i32)
            })
            .min()
    }
}
