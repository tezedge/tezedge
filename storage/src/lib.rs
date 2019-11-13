// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use failure::Fail;
use slog::info;
use slog::Logger;

use tezos_encoding::hash::{BlockHash, HashType, ChainId};
use tezos_messages::p2p::binary_message::{BinaryMessage, MessageHash, MessageHashError};
use tezos_messages::p2p::encoding::prelude::BlockHeader;

pub use crate::block_meta_storage::{BlockMetaStorage, BlockMetaStorageDatabase};
pub use crate::block_storage::{BlockStorage, BlockStorageDatabase, BlockStorageReader};
pub use crate::operations_meta_storage::{OperationsMetaStorage, OperationsMetaStorageDatabase};
pub use crate::operations_storage::{OperationKey, OperationsStorage, OperationsStorageDatabase, OperationsStorageReader};
use crate::persistent::{Codec, DBError, SchemaError};
pub use crate::persistent::database::{Direction, IteratorMode};

pub mod persistent;
pub mod operations_storage;
pub mod operations_meta_storage;
pub mod block_storage;
pub mod block_meta_storage;
pub mod context_storage;

/// Extension of block header with block hash
#[derive(PartialEq, Clone, Debug)]
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

impl Codec for BlockHeaderWithHash {
    #[inline]
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        let hash = bytes[0..HashType::BlockHash.size()].to_vec();
        let header = BlockHeader::from_bytes(bytes[HashType::BlockHash.size()..].to_vec()).map_err(|_| SchemaError::DecodeError)?;
        Ok(BlockHeaderWithHash { hash, header: Arc::new(header) })
    }

    #[inline]
    fn encode(&self) -> Result<Vec<u8>, SchemaError> {
        let mut result = vec![];
        result.extend(&self.hash);
        result.extend(self.header.as_bytes().map_err(|_| SchemaError::EncodeError)?);
        Ok(result)
    }
}

/// Possible errors for storage
#[derive(Debug, Fail)]
pub enum StorageError {
    #[fail(display = "Database error: {}", error)]
    DBError {
        error: DBError
    },
    #[fail(display = "Key is missing in storage")]
    MissingKey,
    #[fail(display = "Block hash error")]
    BlockHashError,
}

impl From<DBError> for StorageError {
    fn from(error: DBError) -> Self {
        StorageError::DBError { error }
    }
}

impl From<SchemaError> for StorageError {
    fn from(error: SchemaError) -> Self {
        StorageError::DBError { error: error.into() }
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
pub fn initialize_storage_with_genesis_block(genesis_hash: &BlockHash, genesis: &BlockHeader, genesis_chain_id: &ChainId, db: Arc<rocksdb::DB>, log: Logger) -> Result<(), StorageError> {
    let genesis_with_hash = BlockHeaderWithHash {
        hash: genesis_hash.clone(),
        header: Arc::new(genesis.clone()),
    };
    let mut block_storage = BlockStorage::new(db.clone());
    if block_storage.get(&genesis_with_hash.hash)?.is_none() {
        info!(log, "Initializing storage with genesis block");
        block_storage.put_block_header(&genesis_with_hash)?;
        let mut block_meta_storage = BlockMetaStorage::new(db.clone());
        block_meta_storage.put(&genesis_with_hash.hash, &block_meta_storage::Meta::genesis_meta(&genesis_with_hash.hash, genesis_chain_id))?;
        let mut operations_meta_storage = OperationsMetaStorage::new(db);
        operations_meta_storage.put(&genesis_with_hash.hash, &operations_meta_storage::Meta::genesis_meta(genesis_chain_id))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use failure::Error;

    use tezos_encoding::hash::{HashEncoding, HashType};
    use tezos_messages::p2p::encoding::prelude::BlockHeaderBuilder;

    use super::*;

    #[test]
    fn block_header_with_hash_encoded_equals_decoded() -> Result<(), Error> {
        let expected = BlockHeaderWithHash {
            hash: HashEncoding::new(HashType::BlockHash).string_to_bytes("BKyQ9EofHrgaZKENioHyP4FZNsTmiSEcVmcghgzCC9cGhE7oCET")?,
            header: Arc::new(
                BlockHeaderBuilder::default()
                    .level(34)
                    .proto(1)
                    .predecessor(HashEncoding::new(HashType::BlockHash).string_to_bytes("BKyQ9EofHrgaZKENioHyP4FZNsTmiSEcVmcghgzCC9cGhE7oCET")?)
                    .timestamp(5_635_634)
                    .validation_pass(4)
                    .operations_hash(HashEncoding::new(HashType::OperationListListHash).string_to_bytes("LLoaGLRPRx3Zf8kB4ACtgku8F4feeBiskeb41J1ciwfcXB3KzHKXc")?)
                    .fitness(vec![vec![0, 0]])
                    .context(HashEncoding::new(HashType::ContextHash).string_to_bytes("CoVmAcMV64uAQo8XvfLr9VDuz7HVZLT4cgK1w1qYmTjQNbGwQwDd")?)
                    .protocol_data(vec![0, 1, 2, 3, 4, 5, 6, 7, 8])
                    .build().unwrap()
            ),
        };
        let encoded_bytes = expected.encode()?;
        let decoded = BlockHeaderWithHash::decode(&encoded_bytes)?;
        Ok(assert_eq!(expected, decoded))
    }
}
