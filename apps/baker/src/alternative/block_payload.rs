// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crypto::{blake2b, hash::OperationListHash};
use tenderbake as tb;

use super::event::{OperationKind, OperationSimple};

#[derive(Clone, Default)]
pub struct BlockPayload {
    pub votes_payload: Vec<OperationSimple>,
    pub anonymous_payload: Vec<OperationSimple>,
    pub managers_payload: Vec<OperationSimple>,
}

impl BlockPayload {
    pub fn operation_list_hash(&self) -> Result<OperationListHash, blake2b::Blake2bError> {
        let votes_ops = self.votes_payload.iter();
        let anonymous_ops = self.anonymous_payload.iter();
        let managers_ops = self.managers_payload.iter();
        let ops = votes_ops.chain(anonymous_ops).chain(managers_ops);
        let hashes = ops
            .filter_map(|op| op.hash.as_ref().cloned())
            .collect::<Vec<_>>();
        OperationListHash::calculate(&hashes)
    }
}

impl tb::Payload for BlockPayload {
    type Item = OperationSimple;

    const EMPTY: Self = BlockPayload {
        votes_payload: vec![],
        anonymous_payload: vec![],
        managers_payload: vec![],
    };

    fn update(&mut self, item: Self::Item) {
        match item.kind() {
            None => (),
            Some(OperationKind::Preendorsement(_)) => (),
            Some(OperationKind::Endorsement(_)) => (),
            Some(OperationKind::Votes) => self.votes_payload.push(item),
            Some(OperationKind::Anonymous) => self.anonymous_payload.push(item),
            Some(OperationKind::Managers) => self.managers_payload.push(item),
        }
    }
}
