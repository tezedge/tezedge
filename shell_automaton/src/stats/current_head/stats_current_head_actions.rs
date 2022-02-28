// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::net::SocketAddr;

use crate::EnablingCondition;
use crypto::hash::BlockHash;
use tezos_messages::base::signature_public_key::SignaturePublicKey;

use crate::State;

#[cfg(feature = "fuzzing")]
use crate::fuzzing::net::SocketAddrMutator;

use super::PendingMessage;

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StatsCurrentHeadPrecheckInitAction {
    pub hash: BlockHash,
}

impl EnablingCondition<State> for StatsCurrentHeadPrecheckInitAction {
    fn is_enabled(&self, _state: &State) -> bool {
        true
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
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

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StatsCurrentHeadPrepareSendAction {
    #[cfg_attr(feature = "fuzzing", field_mutator(SocketAddrMutator))]
    pub address: SocketAddr,
    pub message: PendingMessage,
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

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StatsCurrentHeadSentAction {
    #[cfg_attr(feature = "fuzzing", field_mutator(SocketAddrMutator))]
    pub address: SocketAddr,
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

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StatsCurrentHeadSentErrorAction {
    #[cfg_attr(feature = "fuzzing", field_mutator(SocketAddrMutator))]
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
