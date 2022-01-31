// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::net::SocketAddr;

use serde::{Deserialize, Serialize};

use crypto::hash::{BlockHash, OperationHash};
use tezos_messages::p2p::encoding::block_header::Level;
use tezos_messages::p2p::encoding::{mempool::Mempool, operation::Operation};

use crate::prechecker::OperationDecodedContents;
use crate::service::rpc_service::RpcId;

use crate::{action::EnablingCondition, state::State};

/// Process the mempool received from the peer
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MempoolRecvDoneAction {
    pub address: SocketAddr,
    pub block_hash: BlockHash,
    pub message: Mempool,
    pub level: Level,
    pub timestamp: i64,
    pub proto: u8,
}

impl EnablingCondition<State> for MempoolRecvDoneAction {
    fn is_enabled(&self, state: &State) -> bool {
        let _ = state;
        true
    }
}

/// Query operations from the peer
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MempoolGetOperationsAction {
    pub address: SocketAddr,
}

impl EnablingCondition<State> for MempoolGetOperationsAction {
    fn is_enabled(&self, state: &State) -> bool {
        state.mempool.peer_state.contains_key(&self.address)
    }
}

/// Mark operations requested from the peer as pending
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MempoolMarkOperationsAsPendingAction {
    pub address: SocketAddr,
}

impl EnablingCondition<State> for MempoolMarkOperationsAsPendingAction {
    fn is_enabled(&self, state: &State) -> bool {
        state.mempool.peer_state.contains_key(&self.address)
    }
}

/// Take the operation received from the peer
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MempoolOperationRecvDoneAction {
    pub operation: Operation,
}

impl EnablingCondition<State> for MempoolOperationRecvDoneAction {
    fn is_enabled(&self, state: &State) -> bool {
        let _ = state;
        true
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MempoolOperationInjectAction {
    pub operation: Operation,
    pub operation_hash: OperationHash,
    pub rpc_id: RpcId,
}

impl EnablingCondition<State> for MempoolOperationInjectAction {
    fn is_enabled(&self, state: &State) -> bool {
        // TODO(vlad):
        let _ = state;
        true
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MempoolValidateStartAction {
    pub operation: Operation,
}

impl EnablingCondition<State> for MempoolValidateStartAction {
    fn is_enabled(&self, state: &State) -> bool {
        // TODO(vlad):
        let _ = state;
        true
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MempoolValidateWaitPrevalidatorAction {
    pub operation: Operation,
}

impl EnablingCondition<State> for MempoolValidateWaitPrevalidatorAction {
    fn is_enabled(&self, state: &State) -> bool {
        // TODO(vlad):
        let _ = state;
        true
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MempoolCleanupWaitPrevalidatorAction {}

impl EnablingCondition<State> for MempoolCleanupWaitPrevalidatorAction {
    fn is_enabled(&self, state: &State) -> bool {
        // TODO(vlad):
        let _ = state;
        true
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MempoolRegisterOperationsStreamAction {
    pub rpc_id: RpcId,
    pub applied: bool,
    pub refused: bool,
    pub branch_delayed: bool,
    pub branch_refused: bool,
}

impl EnablingCondition<State> for MempoolRegisterOperationsStreamAction {
    fn is_enabled(&self, state: &State) -> bool {
        // TODO(vlad):
        let _ = state;
        true
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MempoolUnregisterOperationsStreamsAction {}

impl EnablingCondition<State> for MempoolUnregisterOperationsStreamsAction {
    fn is_enabled(&self, state: &State) -> bool {
        // TODO(vlad):
        let _ = state;
        true
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MempoolSendAction {
    pub address: SocketAddr,
    pub send_operations: bool,
    pub requested_explicitly: bool,
}

impl EnablingCondition<State> for MempoolSendAction {
    fn is_enabled(&self, state: &State) -> bool {
        // TODO(vlad):
        let _ = state;
        true
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MempoolSendValidatedAction {
    pub address: SocketAddr,
}

impl EnablingCondition<State> for MempoolSendValidatedAction {
    fn is_enabled(&self, state: &State) -> bool {
        // TODO(vlad):
        let _ = state;
        true
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MempoolAskCurrentHeadAction {}

impl EnablingCondition<State> for MempoolAskCurrentHeadAction {
    fn is_enabled(&self, state: &State) -> bool {
        // TODO(vlad):
        let _ = state;
        true
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MempoolBroadcastAction {
    pub send_operations: bool,
}

impl EnablingCondition<State> for MempoolBroadcastAction {
    fn is_enabled(&self, state: &State) -> bool {
        // TODO(vlad):
        let _ = state;
        true
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MempoolRpcRespondAction {}

impl EnablingCondition<State> for MempoolRpcRespondAction {
    fn is_enabled(&self, state: &State) -> bool {
        // TODO(vlad):
        let _ = state;
        true
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MempoolBroadcastDoneAction {
    pub address: SocketAddr,
    pub known_valid: Vec<OperationHash>,
    pub pending: Vec<OperationHash>,
    pub cleanup_known_valid: bool,
}

impl EnablingCondition<State> for MempoolBroadcastDoneAction {
    fn is_enabled(&self, state: &State) -> bool {
        // TODO(vlad):
        let _ = state;
        true
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MempoolRemoveAppliedOperationsAction {
    pub operation_hashes: Vec<OperationHash>,
}

impl EnablingCondition<State> for MempoolRemoveAppliedOperationsAction {
    fn is_enabled(&self, state: &State) -> bool {
        // TODO(vlad):
        let _ = state;
        true
    }
}

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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MempoolFlushAction {}

impl EnablingCondition<State> for MempoolFlushAction {
    fn is_enabled(&self, state: &State) -> bool {
        if let Some(state) = &state.mempool.local_head_state {
            state.ops_removed && state.prevalidator_ready
        } else {
            false
        }
    }
}

// RPC

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MempoolRpcEndorsementsStatusGetAction {
    pub rpc_id: RpcId,
    pub block_hash: Option<BlockHash>,
}

impl EnablingCondition<State> for MempoolRpcEndorsementsStatusGetAction {
    fn is_enabled(&self, _state: &State) -> bool {
        true
    }
}
