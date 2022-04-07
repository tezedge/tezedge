// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crypto::hash::{BlockHash, ProtocolHash};
use storage::BlockHeaderWithHash;

use crate::{service::rpc_service::RpcId, EnablingCondition, State};

use super::ValidBlocksQuery;

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RpcBootstrappedAction {
    pub rpc_id: RpcId,
}

impl EnablingCondition<State> for RpcBootstrappedAction {
    fn is_enabled(&self, state: &State) -> bool {
        !state.rpc.bootstrapped.requests.contains(&self.rpc_id)
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RpcBootstrappedNewBlockAction {
    pub block: BlockHash,
    pub timestamp: i64,
    pub is_bootstrapped: bool,
}

impl EnablingCondition<State> for RpcBootstrappedNewBlockAction {
    fn is_enabled(&self, state: &State) -> bool {
        !state.rpc.bootstrapped.requests.is_empty()
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RpcBootstrappedDoneAction {
    pub rpc_ids: Vec<RpcId>,
}

impl EnablingCondition<State> for RpcBootstrappedDoneAction {
    fn is_enabled(&self, _state: &State) -> bool {
        true
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RpcMonitorValidBlocksAction {
    pub rpc_id: RpcId,
    pub query: ValidBlocksQuery,
}

impl EnablingCondition<State> for RpcMonitorValidBlocksAction {
    fn is_enabled(&self, state: &State) -> bool {
        !state.rpc.valid_blocks.requests.contains_key(&self.rpc_id)
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RpcReplyValidBlockAction {
    pub rpc_id: RpcId,
    pub block: BlockHeaderWithHash,
    pub protocol: ProtocolHash,
    pub next_protocol: ProtocolHash,
}

impl EnablingCondition<State> for RpcReplyValidBlockAction {
    fn is_enabled(&self, state: &State) -> bool {
        let query = state.rpc.valid_blocks.requests.get(&self.rpc_id);
        match query {
            Some(ValidBlocksQuery {
                protocol,
                next_protocol,
                ..
            }) => {
                protocol.as_ref().map_or(true, |q| q == &self.protocol)
                    && next_protocol
                        .as_ref()
                        .map_or(true, |q| q == &self.next_protocol)
            }
            _ => false,
        }
    }
}

pub type RpcInjectedBlock = RpcInjectBlockAction;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RpcInjectBlockAction {
    pub rpc_id: RpcId,
    pub block: BlockHeaderWithHash,
}

impl EnablingCondition<State> for RpcInjectBlockAction {
    fn is_enabled(&self, state: &State) -> bool {
        can_accept_injected_block(state, &self.block)
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RpcRejectOutdatedInjectedBlockAction {
    pub rpc_id: RpcId,
    pub block_hash: BlockHash,
}

impl EnablingCondition<State> for RpcRejectOutdatedInjectedBlockAction {
    fn is_enabled(&self, state: &State) -> bool {
        let data = match state.rpc.injected_blocks.get(&self.block_hash) {
            Some(v) => v,
            None => return false,
        };
        data.rpc_id == self.rpc_id && !can_accept_injected_block(state, &data.block)
    }
}

fn can_accept_injected_block(state: &State, block: &BlockHeaderWithHash) -> bool {
    state.current_head.get().map_or(false, |h| {
        h.header.level() == block.header.level() || h.header.level() + 1 == block.header.level()
    }) && state.can_accept_new_head(block)
        && !state.is_same_head(block.header.level(), &block.hash)
        && state
            .block_applier
            .current
            .block_hash()
            .map_or(true, |hash| hash != &block.hash)
}
