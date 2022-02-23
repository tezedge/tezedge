// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crypto::hash::BlockHash;

use crate::{service::rpc_service::RpcId, EnablingCondition, State};

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
