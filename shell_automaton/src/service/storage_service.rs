use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::thread;
use storage::shell_automaton_action_meta_storage::{
    ShellAutomatonActionMeta, ShellAutomatonActionMetas,
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

impl StorageServiceDefault {
    fn run_worker(storage: PersistentStorage, mut channel: StorageWorkerResponder) {
        use StorageRequestPayload::*;
        use StorageResponseError::*;
        use StorageResponseSuccess::*;

        let block_storage = BlockStorage::new(&storage);
        let snapshot_storage = ShellAutomatonStateStorage::new(&storage);
        let action_storage = ShellAutomatonActionStorage::new(&storage);
        let action_meta_storage = ShellAutomatonActionMetaStorage::new(&storage);

        let mut action_metas = action_meta_storage
            .get()
            .ok()
            .flatten()
            .unwrap_or_else(|| ShellAutomatonActionMetas::new());

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
                    let meta = action_metas
                        .metas
                        .entry(action_kind.to_string())
                        .or_insert_with(|| ShellAutomatonActionMeta {
                            total_calls: 0,
                            total_duration: 0,
                        });

                    meta.total_calls += 1;
                    meta.total_duration += duration_nanos;

                    action_meta_storage
                        .set(&action_metas)
                        .map(|_| ActionMetaUpdateSuccess(action_id))
                        .map_err(|err| ActionMetaUpdateError(err.into()))
                }
            };

            if let Some(req_id) = req.id {
                let _ = channel.send(StorageResponse::new(req_id, result));
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
