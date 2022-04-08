// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::net::SocketAddr;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crypto::hash::{BlockHash, ChainId, OperationHash};
use tezos_messages::p2p::encoding::block_header::BlockHeader;
use tezos_messages::p2p::encoding::{mempool::Mempool, operation::Operation};

use crate::prechecker::OperationDecodedContents;
use crate::service::rpc_service::RpcId;

use crate::{action::EnablingCondition, state::State};

#[cfg(feature = "fuzzing")]
use crate::fuzzing::net::SocketAddrMutator;

use super::MempoolOperation;

/// Process the mempool received from the peer
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MempoolRecvDoneAction {
    #[cfg_attr(feature = "fuzzing", field_mutator(SocketAddrMutator))]
    pub address: SocketAddr,
    pub block_hash: BlockHash,
    pub block_header: BlockHeader,
    pub message: Mempool,
}

impl EnablingCondition<State> for MempoolRecvDoneAction {
    fn is_enabled(&self, state: &State) -> bool {
        state.mempool.running_since.is_some()
    }
}

/// Query operations from the peer
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MempoolGetOperationsAction {
    #[cfg_attr(feature = "fuzzing", field_mutator(SocketAddrMutator))]
    pub address: SocketAddr,
}

impl EnablingCondition<State> for MempoolGetOperationsAction {
    fn is_enabled(&self, state: &State) -> bool {
        state.mempool.peer_state.contains_key(&self.address)
    }
}

/// Mark operations requested from the peer as pending
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MempoolMarkOperationsAsPendingAction {
    #[cfg_attr(feature = "fuzzing", field_mutator(SocketAddrMutator))]
    pub address: SocketAddr,
}

impl EnablingCondition<State> for MempoolMarkOperationsAsPendingAction {
    fn is_enabled(&self, state: &State) -> bool {
        state.mempool.peer_state.contains_key(&self.address)
    }
}

/// Take the operation received from the peer
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MempoolOperationRecvDoneAction {
    pub hash: OperationHash,
    pub operation: Operation,
}

impl EnablingCondition<State> for MempoolOperationRecvDoneAction {
    fn is_enabled(&self, state: &State) -> bool {
        let _ = state;
        true
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MempoolOperationInjectAction {
    pub operation: Operation,
    pub hash: OperationHash,
    pub rpc_id: RpcId,
    pub injected_timestamp: u64,
}

impl EnablingCondition<State> for MempoolOperationInjectAction {
    fn is_enabled(&self, state: &State) -> bool {
        // TODO(vlad):
        let _ = state;
        true
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BlockInjectAction {
    pub chain_id: ChainId,
    pub block_hash: BlockHash,
    pub block_header: Arc<BlockHeader>,
    pub injected_timestamp: u64,
}

impl EnablingCondition<State> for BlockInjectAction {
    fn is_enabled(&self, _state: &State) -> bool {
        true
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MempoolRegisterOperationsStreamAction {
    pub rpc_id: RpcId,
    pub applied: bool,
    pub refused: bool,
    pub branch_delayed: bool,
    pub branch_refused: bool,
    pub outdated: bool,
}

impl EnablingCondition<State> for MempoolRegisterOperationsStreamAction {
    fn is_enabled(&self, state: &State) -> bool {
        // TODO(vlad):
        let _ = state;
        true
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MempoolUnregisterOperationsStreamsAction {}

impl EnablingCondition<State> for MempoolUnregisterOperationsStreamsAction {
    fn is_enabled(&self, state: &State) -> bool {
        // TODO(vlad):
        let _ = state;
        true
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MempoolSendAction {
    #[cfg_attr(feature = "fuzzing", field_mutator(SocketAddrMutator))]
    pub address: SocketAddr,
    pub send_operations: bool,
    pub requested_explicitly: bool,
    pub prechecked_head: Option<BlockHash>,
}

impl EnablingCondition<State> for MempoolSendAction {
    fn is_enabled(&self, state: &State) -> bool {
        // TODO(vlad):
        let _ = state;
        true
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MempoolSendValidatedAction {
    #[cfg_attr(feature = "fuzzing", field_mutator(SocketAddrMutator))]
    pub address: SocketAddr,
    pub known_valid: Vec<OperationHash>,
}

impl EnablingCondition<State> for MempoolSendValidatedAction {
    fn is_enabled(&self, state: &State) -> bool {
        // TODO(vlad):
        let _ = state;
        true
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MempoolAskCurrentHeadAction {}

impl EnablingCondition<State> for MempoolAskCurrentHeadAction {
    fn is_enabled(&self, state: &State) -> bool {
        // TODO(vlad):
        let _ = state;
        true
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MempoolBroadcastAction {
    pub send_operations: bool,
    pub prechecked_head: Option<BlockHash>,
}

impl EnablingCondition<State> for MempoolBroadcastAction {
    fn is_enabled(&self, state: &State) -> bool {
        // TODO(vlad):
        let _ = state;
        true
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MempoolRpcRespondAction {}

impl EnablingCondition<State> for MempoolRpcRespondAction {
    fn is_enabled(&self, state: &State) -> bool {
        // TODO(vlad):
        let _ = state;
        true
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MempoolBroadcastDoneAction {
    #[cfg_attr(feature = "fuzzing", field_mutator(SocketAddrMutator))]
    pub address: SocketAddr,
    pub known_valid: Vec<OperationHash>,
    pub pending: Vec<OperationHash>,
}

impl EnablingCondition<State> for MempoolBroadcastDoneAction {
    fn is_enabled(&self, state: &State) -> bool {
        // TODO(vlad):
        let _ = state;
        true
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MempoolGetPendingOperationsAction {
    pub rpc_id: RpcId,
}

impl EnablingCondition<State> for MempoolGetPendingOperationsAction {
    fn is_enabled(&self, state: &State) -> bool {
        // TODO(vlad):
        let _ = state;
        true
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MempoolOperationDecodedAction {
    pub operation: OperationHash,
    pub operation_decoded_contents: OperationDecodedContents,
}

impl EnablingCondition<State> for MempoolOperationDecodedAction {
    fn is_enabled(&self, _state: &State) -> bool {
        true
    }
}

// RPC

pub(super) trait MempoolOperationMatcher {
    fn matches(&self, op: &MempoolOperation) -> bool {
        op.operation_decoded_contents.as_ref().map_or_else(
            || self.matches_non_decoded(),
            |op| self.matches_decoded_content(op),
        )
    }

    fn matches_non_decoded(&self) -> bool {
        false
    }

    fn matches_decoded_content(&self, _op: &OperationDecodedContents) -> bool {
        false
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum ConsensusOperationMatcher {
    Branch(OperationBranchMatcher),
    Endorsement(OperationLevelRoundMatcher),
    Preendorsement(OperationLevelRoundMatcher),
}

impl ConsensusOperationMatcher {
    pub fn endorsement_branch(branch: BlockHash) -> Self {
        Self::Branch(OperationBranchMatcher { branch })
    }

    pub fn endorsement_level_round(level: i32, round: Option<i32>) -> Self {
        Self::Endorsement(OperationLevelRoundMatcher { level, round })
    }

    pub fn preendorsement_level_round(level: i32, round: Option<i32>) -> Self {
        Self::Preendorsement(OperationLevelRoundMatcher { level, round })
    }
}

impl MempoolOperationMatcher for ConsensusOperationMatcher {
    fn matches_decoded_content(&self, op: &OperationDecodedContents) -> bool {
        match self {
            ConsensusOperationMatcher::Branch(m) => {
                op.is_endorsement() && m.matches_decoded_content(op)
            }
            ConsensusOperationMatcher::Endorsement(m) => {
                op.is_endorsement() && m.matches_decoded_content(op)
            }
            ConsensusOperationMatcher::Preendorsement(m) => {
                op.is_preendorsement() && m.matches_decoded_content(op)
            }
        }
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OperationBranchMatcher {
    branch: BlockHash,
}

impl MempoolOperationMatcher for OperationBranchMatcher {
    fn matches_decoded_content(&self, op: &OperationDecodedContents) -> bool {
        op.branch() == &self.branch
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OperationLevelRoundMatcher {
    level: i32,
    round: Option<i32>,
}

impl MempoolOperationMatcher for OperationLevelRoundMatcher {
    fn matches_decoded_content(&self, op: &OperationDecodedContents) -> bool {
        op.level_round().map_or(false, |(level, round)| {
            self.level == level && self.round.map_or(true, |r| r == round)
        })
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MempoolRpcEndorsementsStatusGetAction {
    pub rpc_id: RpcId,
    pub matcher: ConsensusOperationMatcher,
}

impl EnablingCondition<State> for MempoolRpcEndorsementsStatusGetAction {
    fn is_enabled(&self, _state: &State) -> bool {
        true
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MempoolOperationValidateNextAction {}

impl EnablingCondition<State> for MempoolOperationValidateNextAction {
    fn is_enabled(&self, state: &State) -> bool {
        !state.mempool.pending_operations.is_empty() && state.mempool.validator.is_ready()
    }
}
