use std::collections::BTreeSet;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use std::thread;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use strum::IntoEnumIterator;

use storage::persistent::BincodeEncoded;
use storage::shell_automaton_action_meta_storage::{
    ShellAutomatonActionStats, ShellAutomatonActionsStats,
};
use storage::{
    BlockHeaderWithHash, BlockStorage, PersistentStorage, ShellAutomatonActionMetaStorage,
    ShellAutomatonActionStorage, ShellAutomatonStateStorage, StorageError,
};

use crate::request::RequestId;
use crate::{Action, ActionId, ActionKind, ActionWithId, State};

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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageErrorTmp;

impl From<StorageError> for StorageErrorTmp {
    fn from(_: StorageError) -> Self {
        Self {}
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum StorageRequestPayload {
    BlockHeaderWithHashPut(BlockHeaderWithHash),

    StateSnapshotPut(Arc<State>),
    ActionPut(Box<ActionWithId<Action>>),
    ActionMetaUpdate {
        action_id: ActionId,
        action_kind: ActionKind,

        /// Duration until next action.
        duration_nanos: u64,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum StorageResponseSuccess {
    BlockHeaderWithHashPutSuccess(bool),

    StateSnapshotPutSuccess(ActionId),
    ActionPutSuccess(ActionId),
    ActionMetaUpdateSuccess(ActionId),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum StorageResponseError {
    BlockHeaderWithHashPutError(StorageErrorTmp),

    StateSnapshotPutError(StorageErrorTmp),
    ActionPutError(StorageErrorTmp),
    ActionMetaUpdateError(StorageErrorTmp),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageRequest {
    /// Identifier for the Request.
    ///
    /// If `None`, response won't be received about that request.
    pub id: Option<RequestId>,
    pub payload: StorageRequestPayload,
}

impl StorageRequest {
    pub fn new(id: Option<RequestId>, payload: StorageRequestPayload) -> Self {
        Self { id, payload }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageResponse {
    pub req_id: RequestId,
    pub result: Result<StorageResponseSuccess, StorageResponseError>,
}

impl StorageResponse {
    pub fn new(
        req_id: RequestId,
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
    fn run_worker(storage: PersistentStorage, mut channel: StorageWorkerResponder) {
        use StorageRequestPayload::*;
        use StorageResponseError::*;
        use StorageResponseSuccess::*;

        let block_storage = BlockStorage::new(&storage);
        let snapshot_storage = ShellAutomatonStateStorage::new(&storage);
        let action_storage = ShellAutomatonActionStorage::new(&storage);
        let action_meta_storage = ShellAutomatonActionMetaStorage::new(&storage);

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
                BlockHeaderWithHashPut(block_header_with_hash) => block_storage
                    .put_block_header(&block_header_with_hash)
                    .map(|res| BlockHeaderWithHashPutSuccess(res))
                    .map_err(|err| BlockHeaderWithHashPutError(err.into())),

                StateSnapshotPut(state) => {
                    let last_action_id = state.last_action.id();
                    snapshot_storage
                        .put(&last_action_id.into(), &*state)
                        .map(|_| StateSnapshotPutSuccess(last_action_id))
                        .map_err(|err| StateSnapshotPutError(err.into()))
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
            };

            if let Some(req_id) = req.id {
                let _ = channel.send(StorageResponse::new(req_id, result));
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
        waker: Arc<mio::Waker>,
        persistent_storage: PersistentStorage,
        channel_bound: usize,
    ) -> Self {
        let (requester, responder) = worker_channel(waker, channel_bound);

        thread::Builder::new()
            .name("storage-thread".to_owned())
            .spawn(move || Self::run_worker(persistent_storage, responder))
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
