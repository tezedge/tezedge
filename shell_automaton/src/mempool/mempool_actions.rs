// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::net::SocketAddr;

use serde::{Serialize, Deserialize};

use crypto::hash::OperationHash;
use tezos_messages::p2p::encoding::{mempool::Mempool, operation::Operation};

use crate::service::rpc_service::RpcId;

use super::mempool_state::HeadState;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MempoolRecvDoneAction {
    pub address: SocketAddr,
    pub head_state: HeadState,
    pub message: Mempool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MempoolGetOperationsAction {
    pub address: SocketAddr,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MempoolGetOperationsPendingAction {
    pub address: SocketAddr,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MempoolOperationRecvDoneAction {
    pub address: SocketAddr,
    pub operation: Operation,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MempoolOperationInjectAction {
    pub operation: Operation,
    pub operation_hash: OperationHash,
    pub rpc_id: RpcId,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MempoolBroadcastAction {
    pub address_exceptions: Vec<SocketAddr>,
    pub head_state: HeadState,
    pub known_valid: Vec<OperationHash>,
    pub pending: Vec<OperationHash>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MempoolBroadcastDoneAction {
    pub address: SocketAddr,
    pub known_valid: Vec<OperationHash>,
    pub pending: Vec<OperationHash>,
}
