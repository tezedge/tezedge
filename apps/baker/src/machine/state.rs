// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::time::Duration;

use crypto::hash::ChainId;

use super::super::types::{
    PreendorsementUnsignedOperation, EndorsementUnsignedOperation, LevelState, ProtocolBlockHeader, RoundState,
};

#[derive(Debug)]
pub enum State {
    Initial,
    RpcError(String),
    ContextConstantsParseError,
    GotChainId(ChainId),
    GotConstants(Config),
    Ready {
        config: Config,
        preendorsement: Option<PreendorsementUnsignedOperation>,
        endorsement: Option<EndorsementUnsignedOperation>,
        block: Option<ProtocolBlockHeader>,

        level_state: LevelState,
        round_state: RoundState,
    },
}

#[derive(Debug, Clone)]
pub struct Config {
    pub chain_id: ChainId,
    pub quorum_size: usize,
    pub minimal_block_delay: Duration,
    pub delay_increment_per_round: Duration,
}
