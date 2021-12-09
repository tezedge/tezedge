// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{collections::HashMap, fmt, sync::Arc};

use crypto::hash::OperationHash;
use tezos_messages::p2p::encoding::operation::Operation;

use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, oneshot};

use crate::State;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct RpcId(u64);

pub type RpcRecvError = mpsc::error::TryRecvError;

pub trait RpcService {
    /// Try to receive/read queued message, if there is any.
    fn try_recv(&mut self) -> Result<(RpcResponse, RpcId), RpcRecvError>;

    fn respond(&mut self, call_id: RpcId, json: serde_json::Value);
}

#[derive(Debug)]
pub enum RpcResponse {
    GetCurrentGlobalState {
        channel: oneshot::Sender<State>,
    },
    InjectOperation {
        operation_hash: OperationHash,
        operation: Operation,
    },
    RequestCurrentHeadFromConnectedPeers,
    RemoveOperations {
        operation_hashes: Vec<OperationHash>,
    }
}

#[derive(Clone)]
pub struct RpcShellAutomatonSender {
    channel: mpsc::Sender<(RpcResponse, oneshot::Sender<serde_json::Value>)>,
    mio_waker: Arc<mio::Waker>,
}

pub struct RpcShellAutomatonChannelSendError;

impl fmt::Display for RpcShellAutomatonChannelSendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "the channel between rpc and shell is overflown")
    }
}

impl RpcShellAutomatonSender {
    pub async fn send(
        &self,
        msg: RpcResponse,
    ) -> Result<oneshot::Receiver<serde_json::Value>, RpcShellAutomatonChannelSendError> {
        let (rx, tx) = oneshot::channel();
        self.channel
            .send((msg, rx))
            .await
            .map_err(|_| RpcShellAutomatonChannelSendError)?;
        let _ = self.mio_waker.wake();
        Ok(tx)
    }
}

#[derive(Debug)]
pub struct RpcServiceDefault {
    id_allocator: u64,
    incoming: mpsc::Receiver<(RpcResponse, oneshot::Sender<serde_json::Value>)>,
    outgoing: HashMap<RpcId, oneshot::Sender<serde_json::Value>>,
}

impl RpcServiceDefault {
    pub fn new(mio_waker: Arc<mio::Waker>, bound: usize) -> (Self, RpcShellAutomatonSender) {
        let (tx, rx) = mpsc::channel(bound);
        (
            RpcServiceDefault {
                id_allocator: 0,
                incoming: rx,
                outgoing: HashMap::new(),
            },
            RpcShellAutomatonSender {
                channel: tx,
                mio_waker,
            },
        )
    }
}

impl RpcService for RpcServiceDefault {
    fn try_recv(&mut self) -> Result<(RpcResponse, RpcId), RpcRecvError> {
        let (msg, sender) = self.incoming.try_recv()?;
        let id = RpcId(self.id_allocator);
        self.id_allocator += 1;
        self.outgoing.insert(id, sender);
        Ok((msg, id))
    }

    fn respond(&mut self, call_id: RpcId, json: serde_json::Value) {
        if let Some(sender) = self.outgoing.remove(&call_id) {
            let _ = sender.send(json);
        }
    }
}
