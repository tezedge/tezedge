// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use rocksdb::{ColumnFamilyDescriptor, Options, SliceTransform};

use crypto::hash::{BlockHash, HashType};
use tezos_messages::p2p::binary_message::BinaryMessage;
use tezos_messages::p2p::encoding::prelude::*;

use crate::persistent::{DatabaseWithSchema, Decoder, Encoder, KeyValueSchema, SchemaError};
use crate::StorageError;

pub type OperationsStorageDatabase = dyn DatabaseWithSchema<OperationsStorage> + Sync + Send;

pub trait OperationsStorageReader: Sync + Send {
    fn get(&self, key: &OperationKey) -> Result<Option<OperationsForBlocksMessage>, StorageError>;

    fn get_operations(&self, block_hash: &BlockHash) -> Result<Vec<OperationsForBlocksMessage>, StorageError>;
}

#[derive(Clone)]
pub struct OperationsStorage {
    db: Arc<OperationsStorageDatabase>
}

impl OperationsStorage {
    pub fn new(db: Arc<OperationsStorageDatabase>) -> Self {
        Self { db }
    }

    #[inline]
    pub fn put_operations(&mut self, message: &OperationsForBlocksMessage) -> Result<(), StorageError> {
        let key = OperationKey {
            block_hash: message.operations_for_block().hash().clone(),
            validation_pass: message.operations_for_block().validation_pass() as u8
        };
        self.put(&key, &message)
    }

    #[inline]
    pub fn put(&mut self, key: &OperationKey, value: &OperationsForBlocksMessage) -> Result<(), StorageError> {
        self.db.put(key, value)
            .map_err(StorageError::from)
    }
}

impl OperationsStorageReader for OperationsStorage {

    #[inline]
    fn get(&self, key: &OperationKey) -> Result<Option<OperationsForBlocksMessage>, StorageError> {
        self.db.get(key)
            .map_err(StorageError::from)
    }

    #[inline]
    fn get_operations(&self, block_hash: &BlockHash) -> Result<Vec<OperationsForBlocksMessage>, StorageError> {
        let key = OperationKey {
            block_hash: block_hash.clone(),
            validation_pass: 0
        };

        let mut operations = vec![];
        for (_key, value) in self.db.prefix_iterator(&key)? {
            operations.push(value?);
        }

        operations.sort_by_key(|v| v.operations_for_block().validation_pass());

        Ok(operations)
    }
}

impl KeyValueSchema for OperationsStorage {
    type Key = OperationKey;
    type Value = OperationsForBlocksMessage;

    fn descriptor() -> ColumnFamilyDescriptor {
        let mut cf_opts = Options::default();
        cf_opts.set_prefix_extractor(SliceTransform::create_fixed_prefix(HashType::BlockHash.size()));
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
            validation_pass
        }
    }
}

impl<'a> From<&'a OperationsForBlock> for OperationKey {
    fn from(ops: &'a OperationsForBlock) -> Self {
        OperationKey {
            block_hash: ops.hash().clone(),
            validation_pass: if ops.validation_pass() >= 0 { ops.validation_pass() as u8 } else { 0 }
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
        OperationsForBlocksMessage::from_bytes(bytes.to_vec())
            .map_err(|_| SchemaError::DecodeError)
    }
}

impl Encoder for OperationsForBlocksMessage {
    #[inline]
    fn encode(&self) -> Result<Vec<u8>, SchemaError> {
        self.as_bytes()
            .map_err(|_| SchemaError::EncodeError)
    }
}


#[cfg(test)]
mod tests {
    use failure::Error;

    use crypto::hash::HashType;

    use crate::persistent::open_db;

    use super::*;

    #[test]
    fn operations_key_encoded_equals_decoded() -> Result<(), Error> {
        let expected = OperationKey {
            block_hash: HashType::BlockHash.string_to_bytes("BKyQ9EofHrgaZKENioHyP4FZNsTmiSEcVmcghgzCC9cGhE7oCET")?,
            validation_pass: 4,
        };
        let encoded_bytes = expected.encode()?;
        let decoded = OperationKey::decode(&encoded_bytes)?;
        Ok(assert_eq!(expected, decoded))
    }

    #[test]
    fn test_get_operations() -> Result<(), Error> {
        use rocksdb::{Options, DB};

        let path = "__op_storage_get_operations";
        if std::path::Path::new(path).exists() {
            std::fs::remove_dir_all(path).unwrap();
        }

        {
            let db = open_db(path, vec![OperationsStorage::descriptor()])?;

            let block_hash_1 = HashType::BlockHash.string_to_bytes("BKyQ9EofHrgaZKENioHyP4FZNsTmiSEcVmcghgzCC9cGhE7oCET")?;
            let block_hash_2 = HashType::BlockHash.string_to_bytes("BLaf78njreWdt2WigJjM9e3ecEdVKm5ehahUfYBKvcWvZ8vfTcJ")?;
            let block_hash_3 = HashType::BlockHash.string_to_bytes("BKzyxvaMgoY5M3BUD7UaUCPivAku2NRiYRA1z1LQUzB7CX6e8yy")?;


            let mut storage = OperationsStorage::new(Arc::new(db));
            let message = OperationsForBlocksMessage::new(OperationsForBlock::new(block_hash_1.clone(), 3), Path::Op, vec![]);
            storage.put_operations(&message)?;
            let message = OperationsForBlocksMessage::new(OperationsForBlock::new(block_hash_1.clone(), 1), Path::Op, vec![]);
            storage.put_operations(&message)?;
            let message = OperationsForBlocksMessage::new(OperationsForBlock::new(block_hash_1.clone(), 0), Path::Op, vec![]);
            storage.put_operations(&message)?;
            let message = OperationsForBlocksMessage::new(OperationsForBlock::new(block_hash_2.clone(), 1), Path::Op, vec![]);
            storage.put_operations(&message)?;
            let message = OperationsForBlocksMessage::new(OperationsForBlock::new(block_hash_1.clone(), 2), Path::Op, vec![]);
            storage.put_operations(&message)?;
            let message = OperationsForBlocksMessage::new(OperationsForBlock::new(block_hash_3.clone(), 3), Path::Op, vec![]);
            storage.put_operations(&message)?;

            let operations = storage.get_operations(&block_hash_1)?;
            assert_eq!(4, operations.len(), "Was expecting vector of {} elements but instead found {}", 4, operations.len());
            for i in 0..4 {
                assert_eq!(i as i8, operations[i].operations_for_block().validation_pass(), "Was expecting operation pass {} but found {}", i, operations[i].operations_for_block().validation_pass());
                assert_eq!(&block_hash_1, operations[i].operations_for_block().hash(), "Block hash mismatch");
            }
            let operations = storage.get_operations(&block_hash_2)?;
            assert_eq!(1, operations.len(), "Was expecting vector of {} elements but instead found {}", 1, operations.len());
            let operations = storage.get_operations(&block_hash_3)?;
            assert_eq!(1, operations.len(), "Was expecting vector of {} elements but instead found {}", 1, operations.len());

        }
        Ok(assert!(DB::destroy(&Options::default(), path).is_ok()))
    }
}