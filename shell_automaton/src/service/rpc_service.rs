// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
    thread,
    time::Instant,
};

use crypto::hash::{BlockHash, ChainId, OperationHash};
use storage::shell_automaton_action_meta_storage::ShellAutomatonActionsStats;
use tezos_messages::p2p::encoding::{
    block_header::{BlockHeader, Level},
    operation::Operation,
};

use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, oneshot};

use crate::{request::RequestId, rpc::ValidBlocksQuery, storage::request::StorageRequestor, State};

use super::{
    statistics_service::ActionGraph, storage_service::StorageRequestPayloadKind, BlockApplyStats,
};

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct RpcId(u64);

pub type RpcRecvError = mpsc::error::TryRecvError;

pub trait RpcService {
    /// Try to receive a request from rpc
    fn try_recv(&mut self) -> Result<(RpcRequest, RpcId), RpcRecvError>;

    /// Respond on the request, `json` is `None` means close the stream
    fn respond<J>(&mut self, call_id: RpcId, json: J)
    where
        J: 'static + Send + Serialize;

    /// Try to receive a request from rpc, but response is expected to be stream
    fn try_recv_stream(&mut self) -> Result<(RpcRequestStream, RpcId), RpcRecvError>;

    /// Respond on the request with json value, `None` means the stream is terminated
    fn respond_stream(&mut self, call_id: RpcId, json: Option<serde_json::Value>);
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageRequest {
    pub req_id: RequestId,
    pub pending_since: u64,
    /// How long request has been pending for. `now - pending_since`.
    pub pending_for: u64,
    pub kind: StorageRequestPayloadKind,
    pub requestor: StorageRequestor,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageRequests {
    pub pending: Vec<StorageRequest>,
    pub finished: Vec<crate::service::statistics_service::StorageRequestFinished>,
}

#[derive(Debug)]
pub enum RpcRequest {
    GetCurrentGlobalState {
        channel: oneshot::Sender<State>,
    },
    GetStorageRequests {
        channel: oneshot::Sender<StorageRequests>,
    },
    GetActionKindStats {
        channel: oneshot::Sender<ShellAutomatonActionsStats>,
    },
    GetActionGraph {
        channel: oneshot::Sender<ActionGraph>,
    },

    GetMempoolOperationStats {
        channel: oneshot::Sender<crate::mempool::OperationsStats>,
    },
    GetMempooEndrosementsStats {
        channel: oneshot::Sender<BTreeMap<OperationHash, crate::mempool::OperationStats>>,
    },
    GetBlockStats {
        channel: oneshot::Sender<Option<crate::service::statistics_service::BlocksApplyStats>>,
    },

    InjectBlockStart {
        chain_id: ChainId,
        block_header: Arc<BlockHeader>,
        block_hash: BlockHash,
        injected: Instant,
    },
    InjectBlock {
        block_hash: BlockHash,
    },
    InjectOperation {
        operation_hash: OperationHash,
        operation: Operation,
        injected: Instant,
    },
    RequestCurrentHeadFromConnectedPeers,
    MempoolStatus,
    GetPendingOperations,
    GetBakingRights {
        block_hash: BlockHash,
        level: Option<Level>,
    },
    GetEndorsingRights {
        block_hash: BlockHash,
        level: Option<Level>,
    },
    GetEndorsementsStatus {
        block_hash: Option<BlockHash>,
    },
    GetStatsCurrentHeadStats {
        channel: oneshot::Sender<Vec<(BlockHash, BlockApplyStats)>>,
        level: Level,
    },
}

#[derive(Debug)]
pub enum RpcRequestStream {
    Bootstrapped,
    ValidBlocks(ValidBlocksQuery),
    GetOperations {
        applied: bool,
        refused: bool,
        branch_delayed: bool,
        branch_refused: bool,
        outdated: bool,
    },
}

#[derive(Clone)]
pub struct RpcShellAutomatonSender {
    channel: mpsc::Sender<(RpcRequest, oneshot::Sender<serde_json::Value>)>,
    channel_stream: mpsc::Sender<(RpcRequestStream, mpsc::UnboundedSender<serde_json::Value>)>,
    mio_waker: Arc<mio::Waker>,
}

#[derive(Clone, Debug, thiserror::Error)]
#[error("the channel between rpc and shell is overflown")]
pub struct RpcShellAutomatonChannelSendError;

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

    fn respond<J>(&mut self, call_id: RpcId, json: J)
    where
        J: 'static + Send + Serialize,
    {
        if let Some(sender) = self.outgoing.remove(&call_id) {
            thread::spawn(move || {
                let _ = sender.send(
                    serde_json::to_value(json)
                        .unwrap_or_else(|e| serde_json::json!({"error": e.to_string()})),
                );
            });
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
