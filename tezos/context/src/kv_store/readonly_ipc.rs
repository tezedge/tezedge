// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! Implementation of a repository that is accessed through IPC calls.
//! It is used by read-only protocol runners to be able to access the in-memory context
//! owned by the writable protocol runner.

use std::sync::Arc;
use std::{borrow::Cow, path::Path};

use super::in_memory::BATCH_CHUNK_CAPACITY;
use super::inline_boxed_slice::InlinedBoxedSlice;
use crate::chunks::ChunkedVec;
#[cfg(test)]
use crate::serialize::persistent::AbsoluteOffset;
use crate::ContextKeyValueStore;

use crypto::hash::ContextHash;
use parking_lot::RwLock;
use slog::{error, info};
use tezos_timing::{RepositoryMemoryUsage, SerializeStats};
use thiserror::Error;

use crate::persistent::{
    get_commit_hash, DBError, Flushable, Persistable, ReadStatistics, ReloadError,
};

use crate::serialize::{in_memory, persistent, ObjectHeader};
use crate::working_tree::shape::{DirectoryShapeId, ShapeStrings};
use crate::working_tree::storage::{DirEntryId, DirectoryOrInodeId, Storage};
use crate::working_tree::string_interner::{StringId, StringInterner};
use crate::working_tree::working_tree::{PostCommitData, SerializeOutput, WorkingTree};
use crate::working_tree::{Object, ObjectReference};
use crate::{
    ffi::TezedgeIndexError, gc::NotGarbageCollected, persistent::KeyValueStoreBackend, ObjectHash,
};

pub struct ReadonlyIpcBackend {
    client: IpcContextClient,
    hashes: HashObjectStore,
}

// TODO - TE-261: quick hack to make the initializer happy, but must be fixed.
// Probably needs a separate thread for the controller, and communication
// should happen through a channel.
unsafe impl Send for ReadonlyIpcBackend {}
unsafe impl Sync for ReadonlyIpcBackend {}

impl ReadonlyIpcBackend {
    /// Connects the IPC backend to a socket in `socket_path`. This operation is blocking.
    /// Will wait for a few seconds if the socket file is not found yet.
    pub fn try_connect<P: AsRef<Path>>(socket_path: P) -> Result<Self, IpcError> {
        let client = IpcContextClient::try_connect(socket_path)?;
        Ok(Self {
            client,
            hashes: HashObjectStore::new(None),
        })
    }

    fn clear_objects(&mut self) -> Result<(), DBError> {
        self.hashes.clear();
        Ok(())
    }
}

impl NotGarbageCollected for ReadonlyIpcBackend {}

impl KeyValueStoreBackend for ReadonlyIpcBackend {
    fn reload_database(&mut self) -> Result<(), ReloadError> {
        Ok(())
    }

    fn store_own_repository(&mut self, _repository: Arc<RwLock<ContextKeyValueStore>>) {
        // no-op
    }

    fn latest_context_hashes(&self, _count: i64) -> Result<Vec<ContextHash>, DBError> {
        Ok(vec![])
    }

    fn contains(&self, hash_id: HashId) -> Result<bool, DBError> {
        if let Some(hash_id) = hash_id.get_in_working_tree()? {
            self.hashes.contains(hash_id).map_err(Into::into)
        } else {
            self.client
                .contains_object(hash_id)
                .map_err(|reason| DBError::IpcAccessError { reason })
        }
    }

    fn put_context_hash(&mut self, _object_ref: ObjectReference) -> Result<(), DBError> {
        // This context is readonly
        Ok(())
    }

    fn get_context_hash(
        &self,
        context_hash: &ContextHash,
    ) -> Result<Option<ObjectReference>, DBError> {
        self.client
            .get_context_hash_id(context_hash)
            .map_err(|reason| DBError::IpcAccessError { reason })
    }

    fn get_hash(&self, object_ref: ObjectReference) -> Result<Cow<ObjectHash>, DBError> {
        let hash_id = object_ref.hash_id_opt();

        if let Some(hash_id) = hash_id.and_then(|h| h.get_in_working_tree().ok()?) {
            self.hashes
                .get_hash(hash_id)
                .map(Cow::Owned)
                .ok_or(DBError::HashNotFound { object_ref })
        } else {
            self.client
                .get_hash(object_ref)
                .map_err(|reason| DBError::IpcAccessError { reason })
        }
    }

    fn get_vacant_object_hash(&mut self) -> Result<VacantObjectHash, DBError> {
        Ok(self.hashes.get_vacant_object_hash()?.set_readonly_runner())
    }

    fn memory_usage(&self) -> RepositoryMemoryUsage {
        self.hashes.get_memory_usage(0, 0, 0)
    }

    fn get_shape(&self, shape_id: DirectoryShapeId) -> Result<ShapeStrings, DBError> {
        self.client
            .get_shape(shape_id)
            .map(ShapeStrings::Owned)
            .map_err(|reason| DBError::IpcAccessError { reason })
    }

    fn make_shape(
        &mut self,
        _dir: &[(StringId, DirEntryId)],
    ) -> Result<Option<DirectoryShapeId>, DBError> {
        // Readonly protocol runner doesn't make shapes.
        Ok(None)
    }

    fn get_str(&self, _: StringId) -> Option<Cow<str>> {
        // Readonly protocol runner doesn't have the `StringInterner`.
        None
    }

    fn synchronize_strings_from(&mut self, _string_interner: &StringInterner) {
        // Readonly protocol runner doesn't update strings.
    }

    fn get_object(
        &self,
        object_ref: ObjectReference,
        storage: &mut Storage,
        strings: &mut StringInterner,
    ) -> Result<Object, DBError> {
        self.get_object_bytes(object_ref, &mut storage.data)?;
        let bytes = std::mem::take(&mut storage.data);

        let object_header: ObjectHeader = ObjectHeader::from_bytes([bytes[0]; 1]);

        let result = if object_header.get_is_persistent() {
            persistent::deserialize_object(&bytes, object_ref.offset(), storage, strings, self)
        } else {
            in_memory::deserialize_object(&bytes, storage, strings, self)
        };

        storage.data = bytes;

        result.map_err(Into::into)
    }

    fn get_inode(
        &self,
        object_ref: ObjectReference,
        storage: &mut Storage,
        strings: &mut StringInterner,
    ) -> Result<DirectoryOrInodeId, DBError> {
        self.get_object_bytes(object_ref, &mut storage.data)?;
        let object_bytes = std::mem::take(&mut storage.data);

        let object_header: ObjectHeader = ObjectHeader::from_bytes([object_bytes[0]; 1]);

        let result = if object_header.get_is_persistent() {
            persistent::deserialize_inode(
                &object_bytes,
                object_ref.offset(),
                storage,
                self,
                strings,
            )
        } else {
            in_memory::deserialize_inode(&object_bytes, storage, strings, self)
        };

        storage.data = object_bytes;

        result.map_err(Into::into)
    }

    fn get_object_bytes<'a>(
        &self,
        object_ref: ObjectReference,
        buffer: &'a mut Vec<u8>,
    ) -> Result<&'a [u8], DBError> {
        buffer.clear();

        if let Some(hash_id) = object_ref
            .hash_id_opt()
            .and_then(|h| h.get_in_working_tree().ok()?)
        {
            buffer.clear();

            self.hashes.with_value(hash_id, |value| {
                if let Some(Some(value)) = value {
                    buffer.extend_from_slice(value)
                };
            })?;

            return Ok(buffer);
        };

        self.client
            .get_object_bytes(object_ref, buffer)
            .map_err(|reason| DBError::IpcAccessError { reason })
    }

    fn commit(
        &mut self,
        working_tree: &WorkingTree,
        parent_commit_ref: Option<ObjectReference>,
        author: String,
        message: String,
        date: u64,
    ) -> Result<(ContextHash, Box<SerializeStats>), DBError> {
        let PostCommitData {
            commit_ref,
            serialize_stats,
            ..
        } = working_tree
            .prepare_commit(
                date,
                author,
                message,
                parent_commit_ref,
                self,
                None,
                None,
                false,
            )
            .map_err(Box::new)?;

        let commit_hash = get_commit_hash(commit_ref, self).map_err(Box::new)?;

        self.clear_objects()?;

        Ok((commit_hash, serialize_stats))
    }

    fn add_serialized_objects(
        &mut self,
        _batch: ChunkedVec<(HashId, InlinedBoxedSlice), { BATCH_CHUNK_CAPACITY }>,
        _output: &mut SerializeOutput,
    ) -> Result<(), DBError> {
        Ok(())
    }

    fn get_hash_id(&self, object_ref: ObjectReference) -> Result<HashId, DBError> {
        if let Some(hash_id) = object_ref.hash_id_opt() {
            return Ok(hash_id);
        };

        self.client
            .get_hash_id(object_ref)
            .map_err(|reason| DBError::IpcAccessError { reason })
    }

    fn take_strings_on_reload(&mut self) -> Option<StringInterner> {
        None
    }

    fn make_hash_id_ready_for_commit(&mut self, hash_id: HashId) -> Result<HashId, DBError> {
        // HashId in the read-only backend are never commited
        Ok(hash_id)
    }

    fn get_read_statistics(&self) -> Result<Option<ReadStatistics>, DBError> {
        Ok(None)
    }

    #[cfg(test)]
    fn synchronize_data(
        &mut self,
        _batch: &[(HashId, InlinedBoxedSlice)],
        _output: &[u8],
    ) -> Result<Option<AbsoluteOffset>, DBError> {
        Ok(None) // no-op
    }
}

impl Flushable for ReadonlyIpcBackend {
    fn flush(&self) -> Result<(), anyhow::Error> {
        Ok(())
    }
}

impl Persistable for ReadonlyIpcBackend {
    fn is_persistent(&self) -> bool {
        false
    }
}

// IPC communication

use std::{cell::RefCell, time::Duration};

use ipc::{IpcClient, IpcError, IpcReceiver, IpcSender, IpcServer};
use serde::{Deserialize, Serialize};
use slog::{warn, Logger};
use strum_macros::IntoStaticStr;

use super::{in_memory::HashObjectStore, HashId, VacantObjectHash};

/// This request is generated by a readonly protool runner and is received by the writable protocol runner.
#[derive(Serialize, Deserialize, Debug, IntoStaticStr)]
enum ContextRequest {
    GetContextHashId(ContextHash),
    GetHash(ObjectReference),
    GetHashId(ObjectReference),
    GetObjectBytes(ObjectReference),
    GetShape(DirectoryShapeId),
    ContainsObject(HashId),
    ShutdownCall, // TODO: is this required?
}

/// This is generated as a response to the `ContextRequest` command.
#[derive(Serialize, Deserialize, Debug, IntoStaticStr)]
enum ContextResponse {
    GetHashResponse(Result<ObjectHash, String>),
    GetHashIdResponse(Result<HashId, String>),
    GetContextHashIdResponse(Result<Option<ObjectReference>, String>),
    GetObjectBytesResponse(Result<Vec<u8>, String>),
    GetShapeResponse(Result<Vec<String>, String>),
    ContainsObjectResponse(Result<bool, String>),
    ShutdownResult,
}

#[derive(Error, Debug)]
pub enum ContextError {
    #[error("Context get object error: {reason}")]
    GetValueError { reason: String },
    #[error("Context get object from offset error: {reason}")]
    GetValueFromOffsetError { reason: String },
    #[error("Context get object bytes error: {reason}")]
    GetObjectBytesError { reason: String },
    #[error("Context get shape error: {reason}")]
    GetShapeError { reason: String },
    #[error("Context contains object error: {reason}")]
    ContainsObjectError { reason: String },
    #[error("Context get hash id error: {reason}")]
    GetContextHashIdError { reason: String },
    #[error("Context get hash error: {reason}")]
    GetHashError { reason: String },
    #[error("Context get hash id error: {reason}")]
    GetHashIdError { reason: String },
}

#[derive(Error, Debug)]
pub enum IpcContextError {
    #[error("Could not obtain a read lock to the TezEdge index")]
    TezedgeIndexReadLockError,
    #[error("IPC error: {reason}")]
    IpcError { reason: IpcError },
}

impl From<TezedgeIndexError> for IpcContextError {
    fn from(_: TezedgeIndexError) -> Self {
        Self::TezedgeIndexReadLockError
    }
}

impl From<IpcError> for IpcContextError {
    fn from(error: IpcError) -> Self {
        IpcContextError::IpcError { reason: error }
    }
}

/// Errors generated by `protocol_runner`.
#[derive(Error, Debug)]
pub enum ContextServiceError {
    /// Generic IPC communication error. See `reason` for more details.
    #[error("IPC error: {reason}")]
    IpcError { reason: IpcError },
    /// Tezos protocol error.
    #[error("Protocol error: {reason}")]
    ContextError { reason: ContextError },
    /// Unexpected message was received from IPC channel
    #[error("Received unexpected message: {message}")]
    UnexpectedMessage { message: &'static str },
    /// Lock error
    #[error("Lock error: {message:?}")]
    LockPoisonError { message: String },
}

impl<T> From<std::sync::PoisonError<T>> for ContextServiceError {
    fn from(source: std::sync::PoisonError<T>) -> Self {
        Self::LockPoisonError {
            message: source.to_string(),
        }
    }
}

impl slog::Value for ContextServiceError {
    fn serialize(
        &self,
        _record: &slog::Record,
        key: slog::Key,
        serializer: &mut dyn slog::Serializer,
    ) -> slog::Result {
        serializer.emit_arguments(key, &format_args!("{}", self))
    }
}

impl From<IpcError> for ContextServiceError {
    fn from(error: IpcError) -> Self {
        ContextServiceError::IpcError { reason: error }
    }
}

impl From<ContextError> for ContextServiceError {
    fn from(error: ContextError) -> Self {
        ContextServiceError::ContextError { reason: error }
    }
}

/// IPC context server that listens for new connections.
pub struct IpcContextListener(IpcServer<ContextRequest, ContextResponse>);

pub struct ContextIncoming<'a> {
    listener: &'a mut IpcContextListener,
}

struct IpcClientIO {
    rx: IpcReceiver<ContextResponse>,
    tx: IpcSender<ContextRequest>,
}

struct IpcServerIO {
    rx: IpcReceiver<ContextRequest>,
    tx: IpcSender<ContextResponse>,
}

/// Encapsulate IPC communication.
pub struct IpcContextClient {
    io: RefCell<IpcClientIO>,
}

pub struct IpcContextServer {
    io: RefCell<IpcServerIO>,
}

/// IPC context client for readers.
impl IpcContextClient {
    const TIMEOUT: Duration = Duration::from_secs(180);

    pub fn try_connect<P: AsRef<Path>>(socket_path: P) -> Result<Self, IpcError> {
        // TODO - TE-261: do this in a better way
        for _ in 0..5 {
            if socket_path.as_ref().exists() {
                break;
            }
            std::thread::sleep(Duration::from_secs(1));
        }
        let ipc_client: IpcClient<ContextResponse, ContextRequest> = IpcClient::new(socket_path);
        let (rx, tx) = ipc_client.connect()?;
        let io = RefCell::new(IpcClientIO { rx, tx });
        Ok(Self { io })
    }

    /// Check if object with hash id exists
    pub fn contains_object(&self, hash_id: HashId) -> Result<bool, ContextServiceError> {
        let mut io = self.io.borrow_mut();
        io.tx.send(&ContextRequest::ContainsObject(hash_id))?;

        // this might take a while, so we will use unusually long timeout
        match io
            .rx
            .try_receive(Some(Self::TIMEOUT), Some(IpcContextListener::IO_TIMEOUT))?
        {
            ContextResponse::ContainsObjectResponse(result) => {
                result.map_err(|err| ContextError::ContainsObjectError { reason: err }.into())
            }
            message => Err(ContextServiceError::UnexpectedMessage {
                message: message.into(),
            }),
        }
    }

    /// Check if object with hash id exists
    pub fn get_context_hash_id(
        &self,
        context_hash: &ContextHash,
    ) -> Result<Option<ObjectReference>, ContextServiceError> {
        let mut io = self.io.borrow_mut();
        io.tx
            .send(&ContextRequest::GetContextHashId(context_hash.clone()))?;

        // this might take a while, so we will use unusually long timeout
        match io
            .rx
            .try_receive(Some(Self::TIMEOUT), Some(IpcContextListener::IO_TIMEOUT))?
        {
            ContextResponse::GetContextHashIdResponse(result) => {
                result.map_err(|err| ContextError::GetContextHashIdError { reason: err }.into())
            }
            message => Err(ContextServiceError::UnexpectedMessage {
                message: message.into(),
            }),
        }
    }

    pub fn get_hash(
        &self,
        object_ref: ObjectReference,
    ) -> Result<Cow<ObjectHash>, ContextServiceError> {
        let mut io = self.io.borrow_mut();
        io.tx.send(&ContextRequest::GetHash(object_ref))?;

        // this might take a while, so we will use unusually long timeout
        match io
            .rx
            .try_receive(Some(Self::TIMEOUT), Some(IpcContextListener::IO_TIMEOUT))?
        {
            ContextResponse::GetHashResponse(result) => result
                .map(Cow::Owned)
                .map_err(|err| ContextError::GetHashError { reason: err }.into()),
            message => Err(ContextServiceError::UnexpectedMessage {
                message: message.into(),
            }),
        }
    }

    pub fn get_hash_id(&self, object_ref: ObjectReference) -> Result<HashId, ContextServiceError> {
        let mut io = self.io.borrow_mut();
        io.tx.send(&ContextRequest::GetHashId(object_ref))?;

        // this might take a while, so we will use unusually long timeout
        match io
            .rx
            .try_receive(Some(Self::TIMEOUT), Some(IpcContextListener::IO_TIMEOUT))?
        {
            ContextResponse::GetHashIdResponse(result) => {
                result.map_err(|err| ContextError::GetHashIdError { reason: err }.into())
            }
            message => Err(ContextServiceError::UnexpectedMessage {
                message: message.into(),
            }),
        }
    }

    /// Get object by hash id
    pub fn get_shape(
        &self,
        shape_id: DirectoryShapeId,
    ) -> Result<Vec<String>, ContextServiceError> {
        let mut io = self.io.borrow_mut();
        io.tx.send(&ContextRequest::GetShape(shape_id))?;

        // this might take a while, so we will use unusually long timeout
        match io
            .rx
            .try_receive(Some(Self::TIMEOUT), Some(IpcContextListener::IO_TIMEOUT))?
        {
            ContextResponse::GetShapeResponse(result) => {
                result.map_err(|err| ContextError::GetShapeError { reason: err }.into())
            }
            message => Err(ContextServiceError::UnexpectedMessage {
                message: message.into(),
            }),
        }
    }

    fn get_object_bytes<'a>(
        &self,
        object_ref: ObjectReference,
        buffer: &'a mut Vec<u8>,
    ) -> Result<&'a [u8], ContextServiceError> {
        let mut io = self.io.borrow_mut();
        io.tx.send(&ContextRequest::GetObjectBytes(object_ref))?;

        match io
            .rx
            .try_receive(Some(Self::TIMEOUT), Some(IpcContextListener::IO_TIMEOUT))?
        {
            ContextResponse::GetObjectBytesResponse(result) => {
                let mut result = match result {
                    Ok(result) => result,
                    Err(err) => {
                        return Err(ContextError::GetObjectBytesError { reason: err }.into())
                    }
                };
                buffer.clear();
                buffer.append(&mut result);
                Ok(buffer)
            }
            message => Err(ContextServiceError::UnexpectedMessage {
                message: message.into(),
            }),
        }
    }
}

impl<'a> Iterator for ContextIncoming<'a> {
    type Item = Result<IpcContextServer, IpcError>;
    fn next(&mut self) -> Option<Result<IpcContextServer, IpcError>> {
        Some(self.listener.accept())
    }
}

impl IpcContextListener {
    const IO_TIMEOUT: Duration = Duration::from_secs(180);

    /// Create new IPC endpoint
    pub fn try_new<P: AsRef<Path>>(socket_path: P) -> Result<Self, IpcError> {
        // Remove file first, otherwise bind will fail.
        std::fs::remove_file(&socket_path).ok();

        Ok(IpcContextListener(IpcServer::bind_path(socket_path)?))
    }

    /// Start accepting incoming IPC connections.
    ///
    /// Returns an [`ipc context server`](IpcContextServer) if new IPC channel is successfully created.
    /// This is a blocking operation.
    pub fn accept(&mut self) -> Result<IpcContextServer, IpcError> {
        let (rx, tx) = self.0.accept()?;

        Ok(IpcContextServer {
            io: RefCell::new(IpcServerIO { rx, tx }),
        })
    }

    /// Returns an iterator over the connections being received on this context IPC listener.
    pub fn incoming(&mut self) -> ContextIncoming<'_> {
        ContextIncoming { listener: self }
    }

    /// Starts accepting connections.
    ///
    /// A new thread is launched to serve each connection.
    pub fn handle_incoming_connections(&mut self, log: &Logger) {
        for connection in self.incoming() {
            match connection {
                Err(err) => {
                    error!(&log, "Error accepting IPC connection"; "reason" => format!("{:?}", err))
                }
                Ok(server) => {
                    info!(
                        &log,
                        "IpcContextServer accepted new IPC connection for context"
                    );
                    let log_inner = log.clone();
                    if let Err(spawn_error) = std::thread::Builder::new()
                        .name("ctx-ipc-server-thread".to_string())
                        .spawn(move || {
                            if let Err(err) = server.process_context_requests(&log_inner) {
                                error!(
                                    &log_inner,
                                    "Error when processing context IPC requests";
                                    "reason" => format!("{:?}", err),
                                );
                            }
                        })
                    {
                        error!(
                            &log,
                            "Failed to spawn thread to IpcContextServer";
                            "reason" => spawn_error,
                        );
                    }
                }
            }
        }
    }
}

impl IpcContextServer {
    fn get_shape(
        shape_id: DirectoryShapeId,
        repo: &ContextKeyValueStore,
    ) -> Result<Vec<String>, ContextError> {
        let shape = repo
            .get_shape(shape_id)
            .map_err(|_| ContextError::GetShapeError {
                reason: "Fail to get shape".to_string(),
            })?;

        // We send the owned `String` to the read only protocol runner.
        // We do not send the `StringId`s because the read only protocol
        // runner doesn't have access to the same `StringInterner`.
        match shape {
            ShapeStrings::SliceIds(slice_ids) => slice_ids
                .iter()
                .map(|s| {
                    repo.get_str(*s)
                        .ok_or_else(|| ContextError::GetShapeError {
                            reason: "String not found".to_string(),
                        })
                        .map(|s| s.to_string())
                })
                .collect(),
            ShapeStrings::Owned(_) => Err(ContextError::GetShapeError {
                reason: "Should receive a slice of StringId".to_string(),
            }),
        }
    }

    fn get_object_bytes(
        object_ref: ObjectReference,
        repository: &ContextKeyValueStore,
    ) -> Result<Vec<u8>, ContextError> {
        let mut buffer = Vec::with_capacity(1000);
        repository
            .get_object_bytes(object_ref, &mut buffer)
            .map(Into::into)
            .map_err(|e| ContextError::GetObjectBytesError {
                reason: format!("{:?}", e),
            })
    }

    fn get_hash_id(
        object_ref: ObjectReference,
        repository: &ContextKeyValueStore,
    ) -> Result<HashId, ContextError> {
        repository
            .get_hash_id(object_ref)
            .map_err(|e| ContextError::GetHashIdError {
                reason: format!("{:?}", e),
            })
    }

    /// Listen to new connections from context readers.
    /// Begin receiving commands from context readers until `ShutdownCall` command is received.
    pub fn process_context_requests(&self, log: &Logger) -> Result<(), IpcContextError> {
        let mut io = self.io.borrow_mut();
        loop {
            let cmd = io.rx.receive()?;

            match cmd {
                ContextRequest::GetShape(shape_id) => match crate::ffi::get_context_index()? {
                    None => io.tx.send(&ContextResponse::GetShapeResponse(Err(
                        "Context index unavailable".to_owned(),
                    )))?,
                    Some(index) => {
                        let repository = index.repository.read();

                        let res = Self::get_shape(shape_id, &*repository)
                            .map_err(|err| format!("Context error: {:?}", err));

                        io.tx.send(&ContextResponse::GetShapeResponse(res))?;
                    }
                },
                ContextRequest::ContainsObject(hash) => match crate::ffi::get_context_index()? {
                    None => io.tx.send(&ContextResponse::ContainsObjectResponse(Err(
                        "Context index unavailable".to_owned(),
                    )))?,
                    Some(index) => {
                        let res = index
                            .contains(hash)
                            .map_err(|err| format!("Context error: {:?}", err));
                        io.tx.send(&ContextResponse::ContainsObjectResponse(res))?;
                    }
                },

                ContextRequest::ShutdownCall => {
                    if let Err(e) = io.tx.send(&ContextResponse::ShutdownResult) {
                        warn!(log, "Failed to send shutdown response"; "reason" => format!("{}", e));
                    }

                    break;
                }
                ContextRequest::GetContextHashId(context_hash) => {
                    match crate::ffi::get_context_index()? {
                        None => io.tx.send(&ContextResponse::GetContextHashIdResponse(Err(
                            "Context index unavailable".to_owned(),
                        )))?,
                        Some(index) => {
                            let res = index
                                .fetch_context_hash_id(&context_hash)
                                .map_err(|err| format!("Context error: {:?}", err));

                            io.tx
                                .send(&ContextResponse::GetContextHashIdResponse(res))?;
                        }
                    }
                }
                ContextRequest::GetHash(object_ref) => match crate::ffi::get_context_index()? {
                    None => io.tx.send(&ContextResponse::GetHashResponse(Err(
                        "Context index unavailable".to_owned(),
                    )))?,
                    Some(index) => {
                        let res = index
                            .fetch_hash(object_ref)
                            .map_err(|err| format!("Context error: {:?}", err));

                        io.tx.send(&ContextResponse::GetHashResponse(res))?;
                    }
                },
                ContextRequest::GetObjectBytes(object_ref) => {
                    match crate::ffi::get_context_index()? {
                        None => io.tx.send(&ContextResponse::GetObjectBytesResponse(Err(
                            "Context index unavailable".to_owned(),
                        )))?,
                        Some(index) => {
                            let repository = index.repository.read();

                            let res = Self::get_object_bytes(object_ref, &*repository)
                                .map_err(|err| format!("Context error: {:?}", err));

                            io.tx.send(&ContextResponse::GetObjectBytesResponse(res))?;
                        }
                    }
                }
                ContextRequest::GetHashId(object_ref) => match crate::ffi::get_context_index()? {
                    None => io.tx.send(&ContextResponse::GetHashIdResponse(Err(
                        "Context index unavailable".to_owned(),
                    )))?,
                    Some(index) => {
                        let repository = index.repository.read();

                        let res = Self::get_hash_id(object_ref, &*repository)
                            .map_err(|err| format!("Context error: {:?}", err));

                        io.tx.send(&ContextResponse::GetHashIdResponse(res))?;
                    }
                },
            }
        }

        Ok(())
    }
}
