// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::Arc;

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
    pub block: Arc<BlockHeaderWithHash>,
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
