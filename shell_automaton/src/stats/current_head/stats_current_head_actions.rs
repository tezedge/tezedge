// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::net::SocketAddr;

use crate::{service::rpc_service::RpcId, EnablingCondition};
use crypto::hash::BlockHash;
use tezos_messages::{
    base::signature_public_key::SignaturePublicKey, p2p::encoding::block_header::Level,
};

use crate::{ActionId, State};

#[cfg(fuzzing)]
use crate::fuzzing::net::SocketAddrMutator;

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StatsCurrentHeadRpcGetPeersAction {
    pub rpc_id: RpcId,
    pub level: Level,
}

impl EnablingCondition<State> for StatsCurrentHeadRpcGetPeersAction {
    fn is_enabled(&self, _state: &State) -> bool {
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StatsCurrentHeadRpcGetApplicationAction {
    pub rpc_id: RpcId,
    pub level: Level,
}

impl EnablingCondition<State> for StatsCurrentHeadRpcGetApplicationAction {
    fn is_enabled(&self, _state: &State) -> bool {
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StatsCurrentHeadReceivedAction {
    #[cfg_attr(fuzzing, field_mutator(SocketAddrMutator))]
    pub address: SocketAddr,
    pub level: Level,
    pub hash: BlockHash,
    pub block_timestamp: u64,
    pub receive_timestamp: ActionId,
    pub empty_mempool: bool,
}

impl EnablingCondition<State> for StatsCurrentHeadReceivedAction {
    fn is_enabled(&self, _state: &State) -> bool {
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StatsCurrentHeadPrecheckSuccessAction {
    pub hash: BlockHash,
    pub baker: SignaturePublicKey,
    pub priority: u16,
}

impl EnablingCondition<State> for StatsCurrentHeadPrecheckSuccessAction {
    fn is_enabled(&self, _state: &State) -> bool {
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StatsCurrentHeadPrepareSendAction {
    #[cfg_attr(fuzzing, field_mutator(SocketAddrMutator))]
    pub address: SocketAddr,
    pub level: Level,
    pub hash: BlockHash,
    pub empty_mempool: bool,
}

impl EnablingCondition<State> for StatsCurrentHeadPrepareSendAction {
    fn is_enabled(&self, state: &State) -> bool {
        !state
            .stats
            .current_head
            .pending_messages
            .contains_key(&self.address)
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StatsCurrentHeadSentAction {
    #[cfg_attr(fuzzing, field_mutator(SocketAddrMutator))]
    pub address: SocketAddr,
    pub timestamp: ActionId,
}

impl EnablingCondition<State> for StatsCurrentHeadSentAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .stats
            .current_head
            .pending_messages
            .contains_key(&self.address)
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StatsCurrentHeadSentErrorAction {
    #[cfg_attr(fuzzing, field_mutator(SocketAddrMutator))]
    pub address: SocketAddr,
}

impl EnablingCondition<State> for StatsCurrentHeadSentErrorAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .stats
            .current_head
            .pending_messages
            .contains_key(&self.address)
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StatsCurrentHeadPruneAction {
    pub timestamp: ActionId,
}

impl EnablingCondition<State> for StatsCurrentHeadPruneAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .stats
            .current_head
            .last_pruned
            .map_or(true, |last_pruned| {
                self.timestamp.duration_since(last_pruned) >= super::PRUNE_PERIOD
            })
    }
}
