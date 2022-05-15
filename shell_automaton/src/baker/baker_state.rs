// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use crypto::hash::BlockPayloadHash;
use tezos_messages::p2p::encoding::block_header::Level;

use super::block_endorser::BakerBlockEndorserState;

/// Locked payload by endorser.
///
/// Once we observe prequorum, we lock the payload and round. After that
/// we will only preendorse/endorse block which has higher level or round
/// and payload hash is same. If payload hash is different, then we will
/// only preendorse/endorse it, if we observe prequorum for that payload.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LockedPayload {
    pub level: Level,
    pub round: i32,
    pub payload_hash: BlockPayloadHash,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BakerState {
    pub block_endorser: BakerBlockEndorserState,

    pub locked_payload: Option<LockedPayload>,
}

impl BakerState {
    pub fn new() -> Self {
        Self {
            block_endorser: BakerBlockEndorserState::Idle { time: 0 },

            locked_payload: None,
        }
    }
}
