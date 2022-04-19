// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::HashSet;
use std::sync::Arc;
use std::{fmt, thread};

use enum_kinds::EnumKind;
use serde::{Deserialize, Serialize};

use crypto::hash::{BlockHash, ChainId, ContextHash, ProtocolHash};
use storage::block_meta_storage::Meta;
use storage::cycle_eras_storage::CycleErasData;
use storage::cycle_storage::CycleData;
use storage::{
    BlockAdditionalData, BlockHeaderWithHash, BlockMetaStorage, BlockMetaStorageReader,
    BlockStorage, BlockStorageReader, ChainMetaStorage, ConstantsStorage, CycleErasStorage,
    CycleMetaStorage, OperationKey, OperationsMetaStorage, OperationsStorage,
    OperationsStorageReader, PersistentStorage, ShellAutomatonActionStorage,
    ShellAutomatonStateStorage, StorageInitInfo,
};
use tezos_api::ffi::{ApplyBlockRequest, ApplyBlockResponse, CommitGenesisResult};
use tezos_messages::p2p::encoding::block_header::{BlockHeader, Level};
use tezos_messages::p2p::encoding::fitness::Fitness;
use tezos_messages::p2p::encoding::operation::Operation;
use tezos_messages::p2p::encoding::operations_for_blocks::OperationsForBlocksMessage;

use crate::request::RequestId;
use crate::storage::kv_cycle_meta::CycleKey;
use crate::{Action, ActionId, ActionWithMeta, State};

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

    fn blocks_genesis_commit_result_put(
        &mut self,
        init_storage_info: &StorageInitInfo,
        commit_result: CommitGenesisResult,
    ) -> Result<(), StorageError>;
}

type StorageWorkerRequester = ServiceWorkerRequester<StorageRequest, StorageResponse>;
type StorageWorkerResponder = ServiceWorkerResponder<StorageRequest, StorageResponse>;

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone, thiserror::Error)]
#[error("Error accessing storage: {0}")]
pub struct StorageError(String);

impl StorageError {
    pub fn mocked() -> Self {
        Self("MockedStorageError".to_owned())
    }
}

impl From<storage::StorageError> for StorageError {
    fn from(err: storage::StorageError) -> Self {
        Self(err.to_string())
    }
}

#[derive(EnumKind, Serialize, Deserialize, Debug, Clone)]
#[enum_kind(
    StorageRequestPayloadKind,
    derive(strum_macros::Display, Serialize, Deserialize,)
)]
pub enum StorageRequestPayload {
    StateSnapshotPut(Box<State>),
    ActionPut(Box<ActionWithMeta>),

    BlockMetaGet(BlockHash),
    BlockHashByLevelGet(Level),
    BlockHeaderGet(BlockHash),
    BlockOperationsGet(OperationKey),
    BlockAdditionalDataGet(BlockHash),
    OperationsGet(BlockHash),
    ConstantsGet(ProtocolHash),
    CycleErasGet(ProtocolHash),
    CycleMetaGet(CycleKey),

    CurrentHeadGet(ChainId, Option<Level>, Vec<ContextHash>),

    BlockHeaderPut(ChainId, BlockHeaderWithHash),
    BlockOperationsPut(OperationsForBlocksMessage),
    BlockAdditionalDataPut((BlockHash, BlockAdditionalData)),

    PrepareApplyBlockData {
        chain_id: Arc<ChainId>,
        block_hash: Arc<BlockHash>,
    },
    StoreApplyBlockResult {
        block_hash: Arc<BlockHash>,
        block_fitness: Fitness,
        block_result: Arc<ApplyBlockResponse>,
        block_metadata: Arc<Meta>,
    },
}

impl StorageRequestPayload {
    #[inline(always)]
    pub fn kind(&self) -> StorageRequestPayloadKind {
        StorageRequestPayloadKind::from(self)
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum StorageResponseSuccess {
    StateSnapshotPutSuccess(ActionId),
    ActionPutSuccess(ActionId),

    BlockMetaGetSuccess(BlockHash, Option<Meta>),
    BlockHashByLevelGetSuccess(Option<BlockHash>),
    BlockHeaderGetSuccess(BlockHash, Option<BlockHeader>),
    BlockOperationsGetSuccess(Option<OperationsForBlocksMessage>),
    BlockAdditionalDataGetSuccess(BlockHash, Option<BlockAdditionalData>),
    OperationsGetSuccess(BlockHash, Option<Vec<Operation>>),
    ConstantsGetSuccess(ProtocolHash, Option<String>),
    CycleErasGetSuccess(ProtocolHash, Option<CycleErasData>),
    CycleMetaGetSuccess(CycleKey, Option<CycleData>),

    /// Returns: `(CurrentHead, CurrentHeadPredecessor)`.
    CurrentHeadGetSuccess(
        BlockHeaderWithHash,
        Option<BlockHeaderWithHash>,
        BlockAdditionalData,
    ),

    BlockHeaderPutSuccess(bool),
    BlockOperationsPutSuccess(bool),
    BlockAdditionalDataPutSuccess(()),

    PrepareApplyBlockDataSuccess {
        block: Arc<BlockHeaderWithHash>,
        block_meta: Arc<Meta>,
        apply_block_req: Arc<ApplyBlockRequest>,
    },
    StoreApplyBlockResultSuccess(Arc<BlockAdditionalData>),
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum StorageResponseError {
    StateSnapshotPutError(ActionId, StorageError),
    ActionPutError(StorageError),

    BlockMetaGetError(BlockHash, StorageError),
    BlockHashByLevelGetError(StorageError),
    BlockHeaderGetError(BlockHash, StorageError),
    BlockOperationsGetError(StorageError),
    BlockAdditionalDataGetError(BlockHash, StorageError),
    OperationsGetError(BlockHash, StorageError),
    ConstantsGetError(ProtocolHash, StorageError),
    CycleErasGetError(ProtocolHash, StorageError),
    CycleMetaGetError(CycleKey, StorageError),

    CurrentHeadGetError(StorageError),

    BlockHeaderPutError(StorageError),
    BlockOperationsPutError(StorageError),
    BlockAdditionalDataPutError(StorageError),

    PrepareApplyBlockDataError(StorageError),
    StoreApplyBlockResultError(StorageError),
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

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
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

pub struct StorageServiceDefault {
    worker_channel: StorageWorkerRequester,
    storage: PersistentStorage,
}

impl fmt::Debug for StorageServiceDefault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StorageServiceDefault").finish()
    }
}

fn find_block_matching_context_storage_impl(
    head_block_storage: BlockHeaderWithHash,
    latest_context_hashes: &[ContextHash],
    block_storage: &BlockStorage,
) -> Result<Option<BlockHeaderWithHash>, StorageError> {
    let mut head = head_block_storage;
    let latest_context_hashes = HashSet::<&ContextHash>::from_iter(latest_context_hashes.iter());

    while !latest_context_hashes.contains(head.header.context()) {
        if head.header.level() == 0 {
            return Ok(None);
        }

        head = match block_storage.get(head.header.predecessor())? {
            Some(head) => head,
            _ => return Ok(None),
        };
    }

    Ok(Some(head))
}

fn find_block_matching_context_storage(
    log: &slog::Logger,
    head_block_storage: BlockHeaderWithHash,
    latest_context_hashes: Vec<ContextHash>,
    block_storage: &BlockStorage,
) -> Result<BlockHeaderWithHash, StorageError> {
    if latest_context_hashes.is_empty() {
        return Ok(head_block_storage);
    }

    match find_block_matching_context_storage_impl(
        head_block_storage.clone(),
        &latest_context_hashes,
        block_storage,
    )? {
        Some(head) => Ok(head),
        None => {
            slog::warn!(
                &log,
                "Unable to find a block matching the context storage";
                "latest_context_hashes" => format!("{:?}", latest_context_hashes)
            );
            Ok(head_block_storage)
        }
    }
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

        let chain_meta_storage = ChainMetaStorage::new(&storage);
        let block_storage = BlockStorage::new(&storage);
        let block_meta_storage = BlockMetaStorage::new(&storage);
        let operations_storage = OperationsStorage::new(&storage);
        let operations_meta_storage = OperationsMetaStorage::new(&storage);
        let constants_storage = ConstantsStorage::new(&storage);
        let cycle_meta_storage = CycleMetaStorage::new(&storage);
        let cycle_eras_storage = CycleErasStorage::new(&storage);

        // let mut last_time_meta_saved = Instant::now();

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

                BlockMetaGet(block_hash) => block_meta_storage
                    .get(&block_hash)
                    .map(|block_meta| BlockMetaGetSuccess(block_hash.clone(), block_meta))
                    .map_err(|err| BlockMetaGetError(block_hash, err.into())),
                BlockHashByLevelGet(level) => block_storage
                    .get_block_hash_by_level(level)
                    .map(BlockHashByLevelGetSuccess)
                    .map_err(|err| BlockHashByLevelGetError(err.into())),
                BlockHeaderGet(block_hash) => block_storage
                    .get(&block_hash)
                    .map(|block_header_with_hash| {
                        BlockHeaderGetSuccess(
                            block_hash.clone(),
                            block_header_with_hash.map(|bhwh| bhwh.header.as_ref().clone()),
                        )
                    })
                    .map_err(|err| BlockHeaderGetError(block_hash, err.into())),
                BlockOperationsGet(key) => operations_storage
                    .get(&key)
                    .map(BlockOperationsGetSuccess)
                    .map_err(|err| BlockOperationsGetError(err.into())),
                BlockAdditionalDataGet(block_hash) => block_meta_storage
                    .get_additional_data(&block_hash)
                    .map(|data| BlockAdditionalDataGetSuccess(block_hash.clone(), data))
                    .map_err(|err| BlockAdditionalDataGetError(block_hash, err.into())),
                OperationsGet(block_hash) => operations_storage
                    .get_operations(&block_hash)
                    .map(|data| data.into_iter().map(Vec::from).flatten().collect())
                    .map(|ops| OperationsGetSuccess(block_hash.clone(), Some(ops)))
                    .map_err(|err| OperationsGetError(block_hash.clone(), err.into())),
                ConstantsGet(protocol_hash) => constants_storage
                    .get(&protocol_hash)
                    .map(|constants| ConstantsGetSuccess(protocol_hash.clone(), constants))
                    .map_err(|err| ConstantsGetError(protocol_hash, err.into())),
                CycleMetaGet(cycle) => cycle_meta_storage
                    .get(&cycle.0)
                    .map(|cycle_data| CycleMetaGetSuccess(cycle, cycle_data))
                    .map_err(|err| CycleMetaGetError(cycle, err.into())),
                CycleErasGet(proto_hash) => cycle_eras_storage
                    .get(&proto_hash)
                    .map(|cycle_eras| CycleErasGetSuccess(proto_hash.clone(), cycle_eras))
                    .map_err(|err| CycleErasGetError(proto_hash, err.into())),

                CurrentHeadGet(chain_id, level_override, latest_context_hashes) => {
                    let result = level_override
                        .and_then(|level| {
                            Some(match block_storage.get_block_by_level(level) {
                                Ok(head) => Ok(head?),
                                Err(err) => Err(err),
                            })
                        })
                        .unwrap_or_else(|| storage::hydrate_current_head(&chain_id, &storage))
                        .map_err(StorageError::from)
                        .and_then(|head| {
                            if level_override.is_none() {
                                find_block_matching_context_storage(
                                    &log,
                                    head,
                                    latest_context_hashes,
                                    &block_storage,
                                )
                            } else {
                                Ok(head)
                            }
                        })
                        .and_then(|head| {
                            let pred = if head.header.level() > 0 {
                                block_storage.get(head.header.predecessor())?
                            } else {
                                None
                            };
                            let additional_data = block_meta_storage
                                .get_additional_data(&head.hash)?
                                .ok_or(StorageError("missing additional_data".to_owned()))?;
                            Ok((head, pred, additional_data))
                        });
                    match result {
                        Ok((head, pred, additional_data)) => {
                            Ok(CurrentHeadGetSuccess(head, pred, additional_data))
                        }
                        Err(err) => Err(CurrentHeadGetError(err)),
                    }
                }

                BlockHeaderPut(chain_id, header) => {
                    match block_storage
                        .put_block_header(&header)
                        .and_then(|is_new| {
                            block_meta_storage
                                .put_block_header(&header, &chain_id, &log)
                                .map(move |_| is_new)
                        })
                        .and_then(|is_new| {
                            if !operations_meta_storage.contains(&header.hash)? {
                                operations_meta_storage.put_block_header(&header)?;
                            }
                            Ok(is_new)
                        }) {
                        Ok(is_new_block) => Ok(BlockHeaderPutSuccess(is_new_block)),
                        Err(err) => Err(BlockHeaderPutError(err.into())),
                    }
                }
                BlockOperationsPut(message) => {
                    match operations_storage
                        .put_operations(&message)
                        .and_then(|_| operations_meta_storage.put_operations(&message))
                        .map(|(is_complete, _)| is_complete)
                    {
                        Ok(is_complete) => Ok(BlockOperationsPutSuccess(is_complete)),
                        Err(err) => Err(BlockOperationsPutError(err.into())),
                    }
                }

                BlockAdditionalDataPut((block_hash, data)) => {
                    match block_meta_storage.put_block_additional_data(&block_hash, &data) {
                        Ok(()) => Ok(BlockAdditionalDataPutSuccess(())),
                        Err(err) => Err(BlockAdditionalDataPutError(err.into())),
                    }
                }

                PrepareApplyBlockData {
                    chain_id,
                    block_hash,
                } => {
                    let result = storage::prepare_block_apply_request(
                        &block_hash,
                        (*chain_id).clone(),
                        &block_storage,
                        &block_meta_storage,
                        &operations_storage,
                        // TODO: cache prev applied block data.
                        None,
                    );

                    match result {
                        Ok((req, meta, block)) => Ok(PrepareApplyBlockDataSuccess {
                            block,
                            block_meta: meta.into(),
                            apply_block_req: req.into(),
                        }),
                        Err(err) => Err(PrepareApplyBlockDataError(err.into())),
                    }
                }
                StoreApplyBlockResult {
                    block_hash,
                    block_fitness,
                    block_result,
                    block_metadata,
                } => {
                    let mut block_meta = (*block_metadata).clone();
                    let result = storage::store_applied_block_result(
                        &chain_meta_storage,
                        &block_storage,
                        &block_meta_storage,
                        &block_hash,
                        block_fitness,
                        (*block_result).clone(),
                        &mut block_meta,
                        &cycle_meta_storage,
                        &cycle_eras_storage,
                        &constants_storage,
                    );

                    match result {
                        Ok(data) => Ok(StoreApplyBlockResultSuccess(data.into())),
                        Err(err) => Err(StoreApplyBlockResultError(err.into())),
                    }
                }
            };

            if req.subscribe {
                let _ = channel.send(StorageResponse::new(req.id, result));
            } else if result.is_err() {
                slog::warn!(&log, "Storage request failed"; "result" => format!("{:?}", result));
            }

            // // Persist metas every 1 sec.
            // if Instant::now()
            //     .duration_since(last_time_meta_saved)
            //     .as_secs()
            //     >= 1
            // {
            //     last_time_meta_saved = Instant::now();
            // }
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

        let storage = persistent_storage.clone();

        thread::Builder::new()
            .name("storage-thread".to_owned())
            .spawn(move || Self::run_worker(log, storage, responder))
            .unwrap();

        Self {
            storage: persistent_storage,
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

    // TODO: this calls storage directly, which will block state machine
    // thread.
    #[inline(always)]
    fn blocks_genesis_commit_result_put(
        &mut self,
        init_storage_data: &StorageInitInfo,
        commit_result: CommitGenesisResult,
    ) -> Result<(), StorageError> {
        let block_storage = BlockStorage::new(&self.storage);
        let block_meta_storage = BlockMetaStorage::new(&self.storage);
        let chain_meta_storage = ChainMetaStorage::new(&self.storage);
        let operations_meta_storage = OperationsMetaStorage::new(&self.storage);

        Ok(storage::store_commit_genesis_result(
            &block_storage,
            &block_meta_storage,
            &chain_meta_storage,
            &operations_meta_storage,
            init_storage_data,
            commit_result,
        )?)
    }
}
