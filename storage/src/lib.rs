// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![feature(const_fn, const_if_match)]

use std::sync::Arc;

use failure::Fail;
use serde::{Deserialize, Serialize};
use slog::info;
use slog::Logger;

use crypto::hash::{BlockHash, ChainId, HashType};
use tezos_messages::p2p::binary_message::{BinaryMessage, MessageHash, MessageHashError};
use tezos_messages::p2p::encoding::prelude::BlockHeader;

pub use crate::block_meta_storage::{BlockMetaStorage, BlockMetaStorageKV};
pub use crate::block_storage::{BlockJsonDataBuilder, BlockStorage, BlockStorageReader};
pub use crate::context_storage::{ContextPrimaryIndexKey, ContextRecordValue, ContextStorage};
pub use crate::operations_meta_storage::{OperationsMetaStorage, OperationsMetaStorageKV};
pub use crate::operations_storage::{OperationKey, OperationsStorage, OperationsStorageKV, OperationsStorageReader};
use crate::persistent::{CommitLogError, DBError, Decoder, Encoder, PersistentStorage, SchemaError};
pub use crate::persistent::database::{Direction, IteratorMode};
use crate::persistent::sequence::SequenceError;
pub use crate::system_storage::SystemStorage;

pub mod persistent;
pub mod operations_storage;
pub mod operations_meta_storage;
pub mod block_storage;
pub mod block_meta_storage;
pub mod context_storage;
pub mod p2p_message_storage;
pub mod system_storage;
pub mod skip_list;

/// Extension of block header with block hash
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct BlockHeaderWithHash {
    pub hash: BlockHash,
    pub header: Arc<BlockHeader>,
}

impl BlockHeaderWithHash {
    /// Create block header extensions from plain block header
    pub fn new(block_header: BlockHeader) -> Result<Self, MessageHashError> {
        Ok(BlockHeaderWithHash {
            hash: block_header.message_hash()?,
            header: Arc::new(block_header),
        })
    }
}

impl Encoder for BlockHeaderWithHash {
    #[inline]
    fn encode(&self) -> Result<Vec<u8>, SchemaError> {
        let mut result = vec![];
        result.extend(&self.hash);
        result.extend(self.header.as_bytes().map_err(|_| SchemaError::EncodeError)?);
        Ok(result)
    }
}

impl Decoder for BlockHeaderWithHash {
    #[inline]
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        let hash = bytes[0..HashType::BlockHash.size()].to_vec();
        let header = BlockHeader::from_bytes(bytes[HashType::BlockHash.size()..].to_vec()).map_err(|_| SchemaError::DecodeError)?;
        Ok(BlockHeaderWithHash { hash, header: Arc::new(header) })
    }
}

/// Possible errors for storage
#[derive(Debug, Fail)]
pub enum StorageError {
    #[fail(display = "Database error: {}", error)]
    DBError {
        error: DBError
    },
    #[fail(display = "Commit log error: {}", error)]
    CommitLogError {
        error: CommitLogError
    },
    #[fail(display = "Key is missing in storage")]
    MissingKey,
    #[fail(display = "Column is not valid")]
    InvalidColumn,
    #[fail(display = "Block hash error")]
    BlockHashError,
    #[fail(display = "Sequence generator failed: {}", error)]
    SequenceError {
        error: SequenceError
    },
}

impl From<DBError> for StorageError {
    fn from(error: DBError) -> Self {
        StorageError::DBError { error }
    }
}

impl From<CommitLogError> for StorageError {
    fn from(error: CommitLogError) -> Self {
        StorageError::CommitLogError { error }
    }
}

impl From<SchemaError> for StorageError {
    fn from(error: SchemaError) -> Self {
        StorageError::DBError { error: error.into() }
    }
}

impl From<SequenceError> for StorageError {
    fn from(error: SequenceError) -> Self {
        StorageError::SequenceError { error }
    }
}

impl slog::Value for StorageError {
    fn serialize(&self, _record: &slog::Record, key: slog::Key, serializer: &mut dyn slog::Serializer) -> slog::Result {
        serializer.emit_arguments(key, &format_args!("{}", self))
    }
}

/// Genesis block needs extra handling because predecessor of the genesis block is genesis itself.
/// Which means that successor of the genesis block is also genesis block. By combining those
/// two statements we get cyclic relationship and everything breaks..
pub fn initialize_storage_with_genesis_block(genesis_hash: &BlockHash, genesis: &BlockHeader, genesis_chain_id: &ChainId, persistent_storage: &PersistentStorage, log: Logger) -> Result<(), StorageError> {
    let genesis_with_hash = BlockHeaderWithHash {
        hash: genesis_hash.clone(),
        header: Arc::new(genesis.clone()),
    };
    let mut block_storage = BlockStorage::new(persistent_storage);
    if block_storage.get(&genesis_with_hash.hash)?.is_none() {
        info!(log, "Initializing storage with genesis block");
        block_storage.put_block_header(&genesis_with_hash)?;
        // TODO: include the data for the other chains as well (mainet, zeronet, etc.)
        // just for babylonnet for now
        let genesis_meta_string = "{\"protocol\":\"PrihK96nBAFSxVL1GLJTVhu9YnzkMFiBeuJRPA8NwuZVZCE1L6i\",\"next_protocol\":\"PtBMwNZT94N7gXKw4i273CKcSaBrrBnqnt3RATExNKr9KNX2USV\",\"test_chain_status\":{\"status\":\"not_running\"},\"max_operations_ttl\":0,\"max_operation_data_length\":0,\"max_block_header_length\":115,\"max_operation_list_length\":[]}".to_string();
        let genesis_op_string = "{\"operations\":[]}".to_string();
        let genesis_prot_string = "".to_string();
        let block_json_data = BlockJsonDataBuilder::default()
            .block_header_proto_json(genesis_prot_string)
            .block_header_proto_metadata_json(genesis_meta_string)
            .operations_proto_metadata_json(genesis_op_string)
            .build().unwrap();
        block_storage.put_block_json_data(&genesis_with_hash.hash, block_json_data)?;
        let mut block_meta_storage = BlockMetaStorage::new(persistent_storage);
        block_meta_storage.put(&genesis_with_hash.hash, &block_meta_storage::Meta::genesis_meta(&genesis_with_hash.hash, genesis_chain_id))?;
        let mut operations_meta_storage = OperationsMetaStorage::new(persistent_storage);
        operations_meta_storage.put(&genesis_with_hash.hash, &operations_meta_storage::Meta::genesis_meta(genesis_chain_id))?;
    }

    Ok(())
}


pub mod tests_common {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    use failure::Error;

    use crate::block_storage;
    use crate::persistent::*;
    use crate::persistent::sequence::Sequences;
    use crate::skip_list::{DatabaseBackedSkipList, Lane, ListValue};

    use super::*;

    pub struct TmpStorage {
        persistent_storage: PersistentStorage,
        path: PathBuf,
    }

    impl TmpStorage {
        pub fn create<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
            let path = path.as_ref().to_path_buf();
            // remove previous data if exists
            if Path::new(&path).exists() {
                fs::remove_dir_all(&path).unwrap();
            }

            let kv = open_kv(&path, vec![
                block_storage::BlockPrimaryIndex::descriptor(),
                block_storage::BlockByLevelIndex::descriptor(),
                block_storage::BlockByContextHashIndex::descriptor(),
                BlockMetaStorage::descriptor(),
                OperationsStorage::descriptor(),
                OperationsMetaStorage::descriptor(),
                context_storage::ContextPrimaryIndex::descriptor(),
                context_storage::ContextByContractIndex::descriptor(),
                SystemStorage::descriptor(),
                Sequences::descriptor(),
                DatabaseBackedSkipList::descriptor(),
                Lane::descriptor(),
                ListValue::descriptor(),
            ])?;
            let clog = open_cl(&path, vec![
                BlockStorage::descriptor(),
                ContextStorage::descriptor(),
            ])?;

            Ok(Self {
                persistent_storage: PersistentStorage::new(Arc::new(kv), Arc::new(clog)),
                path,
            })
        }

        pub fn storage(&self) -> &PersistentStorage {
            &self.persistent_storage
        }
    }

    impl Drop for TmpStorage {
        fn drop(&mut self) {
            let _ = rocksdb::DB::destroy(&rocksdb::Options::default(), &self.path);
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}
