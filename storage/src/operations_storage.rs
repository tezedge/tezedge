// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use rocksdb::{Cache, ColumnFamilyDescriptor, SliceTransform};

use crypto::hash::{BlockHash, HashType};
use tezos_messages::p2p::binary_message::BinaryMessage;
use tezos_messages::p2p::encoding::prelude::*;

use crate::persistent::{
    default_table_options, Decoder, Encoder, KeyValueSchema, KeyValueStoreWithSchema,
    PersistentStorage, SchemaError, StorageType,
};
use crate::StorageError;

pub type OperationsStorageKV = dyn KeyValueStoreWithSchema<OperationsStorage> + Sync + Send;

pub trait OperationsStorageReader: Sync + Send {
    fn get(&self, key: &OperationKey) -> Result<Option<OperationsForBlocksMessage>, StorageError>;

    fn get_operations(
        &self,
        block_hash: &BlockHash,
    ) -> Result<Vec<OperationsForBlocksMessage>, StorageError>;
}

#[derive(Clone)]
pub struct OperationsStorage {
    kv: Arc<OperationsStorageKV>,
}

impl OperationsStorage {
    pub fn new(persistent_storage: &PersistentStorage) -> Self {
        Self {
            kv: persistent_storage.kv(StorageType::Database),
        }
    }

    #[inline]
    pub fn put_operations(&self, message: &OperationsForBlocksMessage) -> Result<(), StorageError> {
        let key = OperationKey {
            block_hash: message.operations_for_block().hash().clone(),
            validation_pass: message.operations_for_block().validation_pass() as u8,
        };
        self.put(&key, &message)
    }

    #[inline]
    pub fn put(
        &self,
        key: &OperationKey,
        value: &OperationsForBlocksMessage,
    ) -> Result<(), StorageError> {
        self.kv.put(key, value).map_err(StorageError::from)
    }
}

impl OperationsStorageReader for OperationsStorage {
    #[inline]
    fn get(&self, key: &OperationKey) -> Result<Option<OperationsForBlocksMessage>, StorageError> {
        self.kv.get(key).map_err(StorageError::from)
    }

    #[inline]
    fn get_operations(
        &self,
        block_hash: &BlockHash,
    ) -> Result<Vec<OperationsForBlocksMessage>, StorageError> {
        let key = OperationKey {
            block_hash: block_hash.clone(),
            validation_pass: 0,
        };

        let mut operations = vec![];
        for (_key, value) in self.kv.prefix_iterator(&key)? {
            operations.push(value?);
        }

        operations.sort_by_key(|v| v.operations_for_block().validation_pass());

        Ok(operations)
    }
}

impl KeyValueSchema for OperationsStorage {
    type Key = OperationKey;
    type Value = OperationsForBlocksMessage;

    fn descriptor(cache: &Cache) -> ColumnFamilyDescriptor {
        let mut cf_opts = default_table_options(cache);
        cf_opts.set_prefix_extractor(SliceTransform::create_fixed_prefix(
            HashType::BlockHash.size(),
        ));
        cf_opts.set_memtable_prefix_bloom_ratio(0.2);
        ColumnFamilyDescriptor::new(Self::name(), cf_opts)
    }

    #[inline]
    fn name() -> &'static str {
        "operations_storage"
    }
}

#[derive(Debug, PartialEq)]
pub struct OperationKey {
    block_hash: BlockHash,
    validation_pass: u8,
}

impl OperationKey {
    pub fn new(block_hash: &BlockHash, validation_pass: u8) -> Self {
        OperationKey {
            block_hash: block_hash.clone(),
            validation_pass,
        }
    }
}

impl<'a> From<&'a OperationsForBlock> for OperationKey {
    fn from(ops: &'a OperationsForBlock) -> Self {
        OperationKey {
            block_hash: ops.hash().clone(),
            validation_pass: if ops.validation_pass() >= 0 {
                ops.validation_pass() as u8
            } else {
                0
            },
        }
    }
}

/// Layout of the `OperationKey` is:
///
/// * bytes layout: `[block_hash(32)][validation_pass(1)]`
impl Decoder for OperationKey {
    #[inline]
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        Ok(OperationKey {
            block_hash: bytes[0..HashType::BlockHash.size()].to_vec(),
            validation_pass: bytes[HashType::BlockHash.size()],
        })
    }
}

impl Encoder for OperationKey {
    #[inline]
    fn encode(&self) -> Result<Vec<u8>, SchemaError> {
        let mut value = Vec::with_capacity(HashType::BlockHash.size() + 1);
        value.extend(&self.block_hash);
        value.push(self.validation_pass);
        Ok(value)
    }
}

impl Decoder for OperationsForBlocksMessage {
    #[inline]
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        OperationsForBlocksMessage::from_bytes(bytes).map_err(|_| SchemaError::DecodeError)
    }
}

impl Encoder for OperationsForBlocksMessage {
    #[inline]
    fn encode(&self) -> Result<Vec<u8>, SchemaError> {
        self.as_bytes().map_err(|_| SchemaError::EncodeError)
    }
}

#[cfg(test)]
mod tests {
    use failure::Error;

    use crypto::hash::HashType;

    use super::*;

    #[test]
    fn operations_key_encoded_equals_decoded() -> Result<(), Error> {
        let expected = OperationKey {
            block_hash: HashType::BlockHash
                .b58check_to_hash("BKyQ9EofHrgaZKENioHyP4FZNsTmiSEcVmcghgzCC9cGhE7oCET")?,
            validation_pass: 4,
        };
        let encoded_bytes = expected.encode()?;
        let decoded = OperationKey::decode(&encoded_bytes)?;
        assert_eq!(expected, decoded);
        Ok(())
    }
}
