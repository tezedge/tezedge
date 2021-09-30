use redux_rs::{ActionId, ActionWithId};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::thread;

use storage::{
    BlockHeaderWithHash, BlockStorage, PersistentStorage, ShellAutomatonActionStorage, ShellAutomatonStateStorage,
    StorageError,
};

use crate::action::Action;
use crate::request::RequestId;
use crate::State;

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
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum StorageResponseSuccess {
    BlockHeaderWithHashPutSuccess(bool),

    StateSnapshotPutSuccess(ActionId),
    ActionPutSuccess(ActionId),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum StorageResponseError {
    BlockHeaderWithHashPutError(StorageErrorTmp),

    StateSnapshotPutError(StorageErrorTmp),
    ActionPutError(StorageErrorTmp),
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

        while let Ok(req) = channel.recv() {
            let result = match req.payload {
                BlockHeaderWithHashPut(block_header_with_hash) => block_storage
                    .put_block_header(&block_header_with_hash)
                    .map(|res| BlockHeaderWithHashPutSuccess(res))
                    .map_err(|err| BlockHeaderWithHashPutError(err.into())),

                ActionPut(action) => action_storage
                    .put::<Action>(&action.id.into(), &action.action)
                    .map(|_| ActionPutSuccess(action.id))
                    .map_err(|err| ActionPutError(err.into())),
                StateSnapshotPut(state) => {
                    let last_action_id = state.last_action_id;
                    snapshot_storage
                        .put(&last_action_id.into(), &*state)
                        .map(|_| StateSnapshotPutSuccess(last_action_id))
                        .map_err(|err| StateSnapshotPutError(err.into()))
                }
            };

            if let Some(req_id) = req.id {
                let _ = channel.send(StorageResponse::new(req_id, result));
            }
        }
    }

    // TODO: remove unwraps
    pub fn init(waker: Arc<mio::Waker>, persistent_storage: PersistentStorage) -> Self {
        let (requester, responder) = worker_channel(waker);

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
