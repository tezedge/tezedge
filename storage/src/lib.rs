// Copyright (c) SimpleStaking and Tezos-RS Contributors
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use failure::Fail;
use log::info;

use networking::p2p::binary_message::{BinaryMessage, MessageHash, MessageHashError};
use networking::p2p::encoding::prelude::BlockHeader;
use tezos_encoding::hash::{BlockHash, HashType};

pub use crate::block_meta_storage::BlockMetaStorage;
pub use crate::block_state::BlockState;
pub use crate::block_storage::{BlockStorage, BlockStorageReader};
pub use crate::operations_meta_storage::OperationsMetaStorage;
pub use crate::operations_state::{MissingOperations, OperationsState};
pub use crate::operations_storage::OperationsStorage;
use crate::persistent::{Codec, DBError, SchemaError};
pub use crate::persistent::database::{Direction, IteratorMode};

pub mod persistent;
pub mod operations_storage;
pub mod operations_meta_storage;
pub mod operations_state;
pub mod block_storage;
pub mod block_meta_storage;
pub mod block_state;


#[derive(PartialEq, Clone, Debug)]
pub struct BlockHeaderWithHash {
    pub hash: BlockHash,
    pub header: Arc<BlockHeader>,
}

impl BlockHeaderWithHash {
    pub fn new(block_header: BlockHeader) -> Result<Self, MessageHashError> {
        Ok(BlockHeaderWithHash {
            hash: block_header.message_hash()?,
            header: Arc::new(block_header),
        })
    }
}

/// Codec for `BlockHeaderWithHash`
impl Codec for BlockHeaderWithHash {
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        let hash = bytes[0..HashType::BlockHash.size()].to_vec();
        let header = BlockHeader::from_bytes(bytes[HashType::BlockHash.size()..].to_vec()).map_err(|_| SchemaError::DecodeError)?;
        Ok(BlockHeaderWithHash { hash, header: Arc::new(header) })
    }

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

/// Genesys block needs extra handling because predecessor of the genesys block is genesys itself.
/// Which means that successor of the genesys block is also genesys block. By combining those
/// two statements we get cyclic relationship and everything breaks..
pub fn initialize_storage_with_genesys_block(genesys_hash: &BlockHash, genesys: &BlockHeader, db: Arc<rocksdb::DB>) -> Result<(), StorageError> {
    let genesys_with_hash = BlockHeaderWithHash {
        hash: genesys_hash.clone(),
        header: Arc::new(genesys.clone())
    };
    let mut block_storage = BlockStorage::new(db.clone());
    if let None = block_storage.get(&genesys_with_hash.hash)? {
        info!("Initializing storage with genesys block");
        block_storage.put_block_header(&genesys_with_hash)?;
        let mut block_meta_storage = BlockMetaStorage::new(db.clone());
        block_meta_storage.put(&genesys_with_hash.hash, &block_meta_storage::Meta::genesys_meta(&genesys_with_hash.hash))?;
        let mut operations_meta_storage = OperationsMetaStorage::new(db);
        operations_meta_storage.put(&genesys_with_hash.hash, &operations_meta_storage::Meta::genesys_meta())?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use failure::Error;

    use tezos_encoding::hash::{HashEncoding, HashType};

    use super::*;

    #[test]
    fn block_header_with_hash_encoded_equals_decoded() -> Result<(), Error> {
        let expected = BlockHeaderWithHash {
            hash: HashEncoding::new(HashType::BlockHash).string_to_bytes("BKyQ9EofHrgaZKENioHyP4FZNsTmiSEcVmcghgzCC9cGhE7oCET")?,
            header: Arc::new(BlockHeader {
                level: 34,
                proto: 1,
                predecessor: HashEncoding::new(HashType::BlockHash).string_to_bytes("BKyQ9EofHrgaZKENioHyP4FZNsTmiSEcVmcghgzCC9cGhE7oCET")?,
                timestamp: 5_635_634,
                validation_pass: 4,
                operations_hash: HashEncoding::new(HashType::OperationListListHash).string_to_bytes("LLoaGLRPRx3Zf8kB4ACtgku8F4feeBiskeb41J1ciwfcXB3KzHKXc")?,
                fitness: vec![vec![0, 0]],
                context: HashEncoding::new(HashType::ContextHash).string_to_bytes("CoVmAcMV64uAQo8XvfLr9VDuz7HVZLT4cgK1w1qYmTjQNbGwQwDd")?,
                protocol_data: vec![0, 1, 2, 3, 4, 5, 6, 7, 8]
            })
        };
        let encoded_bytes = expected.encode()?;
        let decoded = BlockHeaderWithHash::decode(&encoded_bytes)?;
        Ok(assert_eq!(expected, decoded))
    }
}
