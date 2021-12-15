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
    /// Try to receive a request from rpc
    fn try_recv(&mut self) -> Result<(RpcRequest, RpcId), RpcRecvError>;

    /// Respond on the request, `json` is `None` means close the stream
    fn respond(&mut self, call_id: RpcId, json: serde_json::Value);

    /// Try to receive a request from rpc, but response is expected to be stream
    fn try_recv_stream(&mut self) -> Result<(RpcRequestStream, RpcId), RpcRecvError>;

    /// Respond on the request with json value, `None` means the stream is terminated
    fn respond_stream(&mut self, call_id: RpcId, json: Option<serde_json::Value>);
}

#[derive(Debug)]
pub enum RpcRequest {
    GetCurrentGlobalState {
        channel: oneshot::Sender<State>,
    },
    GetMempoolOperationStats {
        channel: oneshot::Sender<crate::mempool::OperationsStats>,
    },

    InjectOperation {
        operation_hash: OperationHash,
        operation: Operation,
    },
    RequestCurrentHeadFromConnectedPeers,
    RemoveOperations {
        operation_hashes: Vec<OperationHash>,
    },
    MempoolStatus,
    GetPendingOperations,
}

#[derive(Debug)]
pub enum RpcRequestStream {
    GetOperations {
        applied: bool,
        refused: bool,
        branch_delayed: bool,
        branch_refused: bool,
    },
}

#[derive(Clone)]
pub struct RpcShellAutomatonSender {
    channel: mpsc::Sender<(RpcRequest, oneshot::Sender<serde_json::Value>)>,
    channel_stream: mpsc::Sender<(RpcRequestStream, mpsc::UnboundedSender<serde_json::Value>)>,
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
        msg: RpcRequest,
    ) -> Result<oneshot::Receiver<serde_json::Value>, RpcShellAutomatonChannelSendError> {
        let (rx, tx) = oneshot::channel();
        self.channel
            .send((msg, rx))
            .await
            .map_err(|_| RpcShellAutomatonChannelSendError)?;
        let _ = self.mio_waker.wake();
        Ok(tx)
    }

    pub async fn request_stream(
        &self,
        msg: RpcRequestStream,
    ) -> Result<mpsc::UnboundedReceiver<serde_json::Value>, RpcShellAutomatonChannelSendError> {
        let (rx, tx) = mpsc::unbounded_channel();
        self.channel_stream
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
    incoming: mpsc::Receiver<(RpcRequest, oneshot::Sender<serde_json::Value>)>,
    incoming_streams: mpsc::Receiver<(RpcRequestStream, mpsc::UnboundedSender<serde_json::Value>)>,
    outgoing: HashMap<RpcId, oneshot::Sender<serde_json::Value>>,
    outgoing_streams: HashMap<RpcId, mpsc::UnboundedSender<serde_json::Value>>,
}

impl RpcServiceDefault {
    pub fn new(mio_waker: Arc<mio::Waker>, bound: usize) -> (Self, RpcShellAutomatonSender) {
        let (tx, rx) = mpsc::channel(bound);
        let (stx, srx) = mpsc::channel(bound);
        (
            RpcServiceDefault {
                id_allocator: 0,
                incoming: rx,
                outgoing: HashMap::new(),
                incoming_streams: srx,
                outgoing_streams: HashMap::new(),
            },
            RpcShellAutomatonSender {
                channel: tx,
                channel_stream: stx,
                mio_waker,
            },
        )
    }
}

impl RpcService for RpcServiceDefault {
    fn try_recv(&mut self) -> Result<(RpcRequest, RpcId), RpcRecvError> {
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

    fn try_recv_stream(&mut self) -> Result<(RpcRequestStream, RpcId), RpcRecvError> {
        let (msg, sender) = self.incoming_streams.try_recv()?;
        let id = RpcId(self.id_allocator);
        self.id_allocator += 1;
        self.outgoing_streams.insert(id, sender);
        Ok((msg, id))
    }

    fn respond_stream(&mut self, call_id: RpcId, json: Option<serde_json::Value>) {
        match json {
            Some(json) => {
                if let Some(sender) = self.outgoing_streams.get(&call_id) {
                    let _ = sender.send(json);
                }
            }
            None => drop(self.outgoing_streams.remove(&call_id)),
        }
    }
}
