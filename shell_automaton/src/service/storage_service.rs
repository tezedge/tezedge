// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::BTreeSet;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use std::thread;
use std::time::Instant;

use crypto::hash::{BlockHash, HashBase58, ProtocolHash};
use serde::{Deserialize, Serialize};
use storage::block_meta_storage::Meta;
use storage::cycle_eras_storage::CycleErasData;
use storage::cycle_storage::CycleData;
use strum::IntoEnumIterator;

use storage::persistent::BincodeEncoded;
use storage::shell_automaton_action_meta_storage::{
    ShellAutomatonActionStats, ShellAutomatonActionsStats,
};
use storage::{
    BlockAdditionalData, BlockMetaStorage, BlockMetaStorageReader, BlockStorage,
    BlockStorageReader, ConstantsStorage, CycleErasStorage, CycleMetaStorage, OperationsStorage,
    OperationsStorageReader, PersistentStorage, ShellAutomatonActionMetaStorage,
    ShellAutomatonActionStorage, ShellAutomatonStateStorage,
};
use tezos_messages::p2p::encoding::{block_header::BlockHeader, operation::Operation};

use crate::request::RequestId;
use crate::storage::kv_cycle_meta::CycleKey;
use crate::{Action, ActionId, ActionKind, ActionWithMeta, State};

use super::service_channel::{
    worker_channel, RequestSendError, ResponseTryRecvError, ServiceWorkerRequester,
    ServiceWorkerResponder,
};

pub trait StorageService {
    /// Send the request to storage for execution.
    fn request_send(&mut self, req: StorageRequest)
        -> Result<(), RequestSendError<StorageRequest>>;

    /// Try to receive/read queued response, if there is any.
    fn response_try_recv(&mut self) -> Result<StorageResponse, ResponseTryRecvError>;
}

type StorageWorkerRequester = ServiceWorkerRequester<StorageRequest, StorageResponse>;
type StorageWorkerResponder = ServiceWorkerResponder<StorageRequest, StorageResponse>;

#[derive(Serialize, Deserialize, Debug, Clone, thiserror::Error)]
#[error("Error accessing storage: {0}")]
pub struct StorageError(String);

impl From<storage::StorageError> for StorageError {
    fn from(err: storage::StorageError) -> Self {
        Self(err.to_string())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum StorageRequestPayload {
    StateSnapshotPut(Box<State>),
    ActionPut(Box<ActionWithMeta>),
    ActionMetaUpdate {
        action_id: ActionId,
        action_kind: ActionKind,

        /// Duration until next action.
        duration_nanos: u64,
    },

    BlockMetaGet(HashBase58<BlockHash>),
    BlockHeaderGet(HashBase58<BlockHash>),
    BlockAdditionalDataGet(HashBase58<BlockHash>),
    OperationsGet(HashBase58<BlockHash>),
    ConstantsGet(HashBase58<ProtocolHash>),
    CycleErasGet(HashBase58<ProtocolHash>),
    CycleMetaGet(CycleKey),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum StorageResponseSuccess {
    StateSnapshotPutSuccess(ActionId),
    ActionPutSuccess(ActionId),
    ActionMetaUpdateSuccess(ActionId),

    BlockMetaGetSuccess(HashBase58<BlockHash>, Option<Meta>),
    BlockHeaderGetSuccess(HashBase58<BlockHash>, Option<BlockHeader>),
    BlockAdditionalDataGetSuccess(HashBase58<BlockHash>, Option<BlockAdditionalData>),
    OperationsGetSuccess(HashBase58<BlockHash>, Option<Vec<Operation>>),
    ConstantsGetSuccess(HashBase58<ProtocolHash>, Option<String>),
    CycleErasGetSuccess(HashBase58<ProtocolHash>, Option<CycleErasData>),
    CycleMetaGetSuccess(CycleKey, Option<CycleData>),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum StorageResponseError {
    StateSnapshotPutError(ActionId, StorageError),
    ActionPutError(StorageError),
    ActionMetaUpdateError(StorageError),

    BlockMetaGetError(HashBase58<BlockHash>, StorageError),
    BlockHeaderGetError(HashBase58<BlockHash>, StorageError),
    BlockAdditionalDataGetError(HashBase58<BlockHash>, StorageError),
    OperationsGetError(HashBase58<BlockHash>, StorageError),
    ConstantsGetError(HashBase58<ProtocolHash>, StorageError),
    CycleErasGetError(HashBase58<ProtocolHash>, StorageError),
    CycleMetaGetError(CycleKey, StorageError),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageRequest {
    /// Identifier for the Request.
    pub id: Option<RequestId>,
    pub payload: StorageRequestPayload,
    /// Subscribe for the result (StorageResponse).
    ///
    /// True by default if request **id** is `Some`.
    pub subscribe: bool,
}

impl StorageRequest {
    pub fn new(id: Option<RequestId>, payload: StorageRequestPayload) -> Self {
        let subscribe = id.is_some();
        Self {
            id,
            payload,
            subscribe,
        }
    }

    pub fn subscribe(mut self) -> Self {
        self.subscribe = true;
        self
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageResponse {
    pub req_id: Option<RequestId>,
    pub result: Result<StorageResponseSuccess, StorageResponseError>,
}

impl StorageResponse {
    pub fn new(
        req_id: Option<RequestId>,
        result: Result<StorageResponseSuccess, StorageResponseError>,
    ) -> Self {
        Self { req_id, result }
    }
}

#[derive(Debug)]
pub struct StorageServiceDefault {
    worker_channel: StorageWorkerRequester,
}

#[derive(Serialize, Deserialize, Default)]
pub struct ActionGraph(Vec<ActionGraphNode>);

impl IntoIterator for ActionGraph {
    type IntoIter = std::vec::IntoIter<ActionGraphNode>;
    type Item = ActionGraphNode;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl Deref for ActionGraph {
    type Target = Vec<ActionGraphNode>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for ActionGraph {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl BincodeEncoded for ActionGraph {}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionGraphNode {
    pub action_kind: ActionKind,
    pub next_actions: BTreeSet<usize>,
}

impl StorageServiceDefault {
    fn run_worker(
        log: slog::Logger,
        storage: PersistentStorage,
        mut channel: StorageWorkerResponder,
    ) {
        use StorageRequestPayload::*;
        use StorageResponseError::*;
        use StorageResponseSuccess::*;

        let snapshot_storage = ShellAutomatonStateStorage::new(&storage);
        let action_storage = ShellAutomatonActionStorage::new(&storage);
        let action_meta_storage = ShellAutomatonActionMetaStorage::new(&storage);
        let block_meta_storage = BlockMetaStorage::new(&storage);
        let block_storage = BlockStorage::new(&storage);
        let operations_storage = OperationsStorage::new(&storage);
        let constants_storage = ConstantsStorage::new(&storage);
        let cycle_meta_storage = CycleMetaStorage::new(&storage);
        let cycle_eras_storage = CycleErasStorage::new(&storage);

        let mut last_time_meta_saved = Instant::now();
        let mut action_meta_update_prev_action_kind = None;
        let mut action_graph = ActionGraph(
            ActionKind::iter()
                .map(|action_kind| ActionGraphNode {
                    action_kind,
                    next_actions: BTreeSet::new(),
                })
                .collect(),
        );

        let mut action_metas = action_meta_storage
            .get_stats()
            .ok()
            .flatten()
            .unwrap_or_else(|| ShellAutomatonActionsStats::new());

        while let Ok(req) = channel.recv() {
            let result = match req.payload {
                StateSnapshotPut(state) => {
                    let last_action_id = state.last_action.id();
                    snapshot_storage
                        .put(&last_action_id.into(), &*state)
                        .map(|_| StateSnapshotPutSuccess(last_action_id))
                        .map_err(|err| StateSnapshotPutError(last_action_id, err.into()))
                }
                ActionPut(action) => action_storage
                    .put::<Action>(&action.id.into(), &action.action)
                    .map(|_| ActionPutSuccess(action.id))
                    .map_err(|err| ActionPutError(err.into())),
                ActionMetaUpdate {
                    action_id,
                    action_kind,
                    duration_nanos,
                } => {
                    if let Some(prev_action_kind) = action_meta_update_prev_action_kind.clone() {
                        action_graph[prev_action_kind as usize]
                            .next_actions
                            .insert(action_kind as usize);
                    }

                    let meta = action_metas
                        .stats
                        .entry(action_kind.to_string())
                        .or_insert_with(|| ShellAutomatonActionStats {
                            total_calls: 0,
                            total_duration: 0,
                        });

                    meta.total_calls += 1;
                    meta.total_duration += duration_nanos;
                    action_meta_update_prev_action_kind = Some(action_kind);

                    Ok(ActionMetaUpdateSuccess(action_id))
                }
                BlockMetaGet(block_hash) => block_meta_storage
                    .get(&block_hash.0)
                    .map(|block_meta| BlockMetaGetSuccess(block_hash.clone(), block_meta))
                    .map_err(|err| BlockMetaGetError(block_hash, err.into())),
                BlockHeaderGet(block_hash) => block_storage
                    .get(&block_hash.0)
                    .map(|block_header_with_hash| {
                        BlockHeaderGetSuccess(
                            block_hash.clone(),
                            block_header_with_hash.map(|bhwh| bhwh.header.as_ref().clone()),
                        )
                    })
                    .map_err(|err| BlockHeaderGetError(block_hash, err.into())),
                BlockAdditionalDataGet(block_hash) => block_meta_storage
                    .get_additional_data(&block_hash.0)
                    .map(|data| BlockAdditionalDataGetSuccess(block_hash.clone(), data))
                    .map_err(|err| BlockAdditionalDataGetError(block_hash, err.into())),
                OperationsGet(block_hash) => operations_storage
                    .get_operations(&block_hash.0)
                    .map(|data| data.into_iter().map(Vec::from).flatten().collect())
                    .map(|ops| OperationsGetSuccess(block_hash.clone(), Some(ops)))
                    .map_err(|err| OperationsGetError(block_hash.clone(), err.into())),
                ConstantsGet(protocol_hash) => constants_storage
                    .get(&protocol_hash.0)
                    .map(|constants| ConstantsGetSuccess(protocol_hash.clone(), constants))
                    .map_err(|err| ConstantsGetError(protocol_hash, err.into())),
                CycleMetaGet(cycle) => cycle_meta_storage
                    .get(&cycle.0)
                    .map(|cycle_data| CycleMetaGetSuccess(cycle, cycle_data))
                    .map_err(|err| CycleMetaGetError(cycle, err.into())),
                CycleErasGet(proto_hash) => cycle_eras_storage
                    .get(&proto_hash.0)
                    .map(|cycle_eras| CycleErasGetSuccess(proto_hash.clone(), cycle_eras))
                    .map_err(|err| CycleErasGetError(proto_hash, err.into())),
            };

            if req.subscribe {
                let _ = channel.send(StorageResponse::new(req.id, result));
            } else if result.is_err() {
                slog::warn!(&log, "Storage request failed"; "result" => format!("{:?}", result));
            }

            // Persist metas every 1 sec.
            if Instant::now()
                .duration_since(last_time_meta_saved)
                .as_secs()
                >= 1
            {
                let _ = action_meta_storage.set_graph(&action_graph);
                let _ = action_meta_storage.set_stats(&action_metas);
                last_time_meta_saved = Instant::now();
            }
        }
    }

    // TODO: remove unwraps
    pub fn init(
        log: slog::Logger,
        waker: Arc<mio::Waker>,
        persistent_storage: PersistentStorage,
        channel_bound: usize,
    ) -> Self {
        let (requester, responder) = worker_channel(waker, channel_bound);

        thread::Builder::new()
            .name("storage-thread".to_owned())
            .spawn(move || Self::run_worker(log, persistent_storage, responder))
            .unwrap();

        Self {
            worker_channel: requester,
        }
    }
}

impl StorageService for StorageServiceDefault {
    #[inline(always)]
    fn request_send(
        &mut self,
        req: StorageRequest,
    ) -> Result<(), RequestSendError<StorageRequest>> {
        self.worker_channel.send(req)
    }

    #[inline(always)]
    fn response_try_recv(&mut self) -> Result<StorageResponse, ResponseTryRecvError> {
        self.worker_channel.try_recv()
    }
}
