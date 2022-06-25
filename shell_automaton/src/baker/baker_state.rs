// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crypto::hash::{
    BlockHash, BlockMetadataHash, BlockPayloadHash, OperationHash, OperationMetadataListListHash,
};
use storage::BlockHeaderWithHash;
use tezos_messages::p2p::encoding::block_header::{BlockHeader, Level};
use tezos_messages::p2p::encoding::operation::Operation;

use super::block_baker::{BakerBlockBakerState, LiquidityBakingToggleVote};
use super::block_endorser::BakerBlockEndorserState;
use super::persisted::BakerPersistedState;
use super::seed_nonce::BakerSeedNonceState;

/// Locked payload by endorser.
///
/// Once we observe prequorum, we lock the payload and round. After that
/// we will only preendorse/endorse block which has higher level or round
/// and payload hash is same. If payload hash is different, then we will
/// only preendorse/endorse it, if we observe prequorum for that payload.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LockedPayload {
    pub(super) block: BlockHeaderWithHash,
    pub(super) round: i32,
    pub(super) payload_hash: BlockPayloadHash,
    pub(super) payload_round: i32,
    pub(super) pred_header: BlockHeader,
    pub(super) pred_block_metadata_hash: BlockMetadataHash,
    pub(super) pred_ops_metadata_hash: OperationMetadataListListHash,
    pub(super) operations: Vec<Vec<Operation>>,
}

impl LockedPayload {
    pub fn hash(&self) -> &BlockHash {
        &self.block.hash
    }

    pub fn header(&self) -> &BlockHeader {
        &*self.block.header
    }

    pub fn payload_hash(&self) -> &BlockPayloadHash {
        &self.payload_hash
    }

    pub fn round(&self) -> i32 {
        self.round
    }
}

impl LockedPayload {
    pub fn level(&self) -> Level {
        self.block.header.level()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ElectedBlock {
    pub(super) block: BlockHeaderWithHash,
    pub(super) round: i32,
    pub(super) payload_hash: BlockPayloadHash,
    pub(super) block_metadata_hash: BlockMetadataHash,
    pub(super) ops_metadata_hash: OperationMetadataListListHash,
    pub(super) operations: Vec<Vec<Operation>>,
    pub(super) non_consensus_op_hashes: Vec<OperationHash>,
}

impl ElectedBlock {
    pub fn hash(&self) -> &BlockHash {
        &self.block.hash
    }

    pub fn header(&self) -> &BlockHeader {
        &*self.block.header
    }

    pub fn payload_hash(&self) -> &BlockPayloadHash {
        &self.payload_hash
    }

    pub fn round(&self) -> i32 {
        self.round
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BakerState {
    pub liquidity_baking_escape_vote: LiquidityBakingToggleVote,

    pub block_endorser: BakerBlockEndorserState,
    pub block_baker: BakerBlockBakerState,
    pub seed_nonces: BTreeMap<Level, BakerSeedNonceState>,
    pub locked_payload: Option<LockedPayload>,
    pub elected_block: Option<ElectedBlock>,

    pub persisted: BakerPersistedState,
}

impl BakerState {
    pub fn new(liquidity_baking_escape_vote: LiquidityBakingToggleVote) -> Self {
        Self {
            liquidity_baking_escape_vote,
            block_endorser: BakerBlockEndorserState::Idle { time: 0 },
            block_baker: BakerBlockBakerState::Idle { time: 0 },
            seed_nonces: Default::default(),
            locked_payload: None,
            elected_block: None,

            persisted: BakerPersistedState::new(),
        }
    }

    pub fn elected_block_header_with_hash(&self) -> Option<&BlockHeaderWithHash> {
        self.elected_block.as_ref().map(|v| &v.block)
    }
}
