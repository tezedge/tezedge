// Copyright (c) SimpleStaking and Tezos-RS Contributors
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use rocksdb::{ColumnFamilyDescriptor, Options, SliceTransform};

use networking::p2p::binary_message::BinaryMessage;
use networking::p2p::encoding::prelude::*;
use tezos_encoding::hash::{BlockHash, HashType};

use crate::StorageError;
use crate::persistent::{Codec, DatabaseWithSchema, Schema, SchemaError};

pub type OperationsStorageDatabase = dyn DatabaseWithSchema<OperationsStorage> + Sync + Send;

#[derive(Clone)]
pub struct OperationsStorage {
    db: Arc<OperationsStorageDatabase>
}

impl OperationsStorage {
    pub fn new(db: Arc<OperationsStorageDatabase>) -> Self {
        OperationsStorage { db }
    }

    pub fn get_operations(&self, block_hash: &BlockHash) -> Result<Vec<OperationsForBlocksMessage>, StorageError> {
        let key = OperationKey {
            block_hash: block_hash.clone(),
            validation_pass: 0
        };

        let mut operations = vec![];
        for (_key, value) in self.db.prefix_iterator(&key)? {
            operations.push(value?);
        }

        operations.sort_by_key(|v| v.operations_for_block.validation_pass);

        return Ok(operations)
    }

    pub fn insert(&mut self, message: &OperationsForBlocksMessage) -> Result<(), StorageError> {
        let key = OperationKey {
            block_hash: message.operations_for_block.hash.clone(),
            validation_pass: message.operations_for_block.validation_pass as u8
        };
        self.db.put(&key, &message)
            .map_err(StorageError::from)
    }
}

impl Schema for OperationsStorage {
    const COLUMN_FAMILY_NAME: &'static str = "operations_storage";
    type Key = OperationKey;
    type Value = OperationsForBlocksMessage;

    fn cf_descriptor() -> ColumnFamilyDescriptor {
        let mut cf_opts = Options::default();
        cf_opts.set_prefix_extractor(SliceTransform::create_fixed_prefix(HashType::BlockHash.size()));
        cf_opts.set_memtable_prefix_bloom_ratio(0.2);
        ColumnFamilyDescriptor::new(Self::COLUMN_FAMILY_NAME, cf_opts)
    }
}

/// Layout of the `OperationKey` is:
///
/// * bytes layout: `[block_hash(32)][validation_pass(1)]`
#[derive(Debug, PartialEq)]
pub struct OperationKey {
    block_hash: BlockHash,
    validation_pass: u8,
}

impl Codec for OperationKey {
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        Ok(OperationKey {
            block_hash: bytes[0..HashType::BlockHash.size()].to_vec(),
            validation_pass: bytes[HashType::BlockHash.size()]
        })
    }

    fn encode(&self) -> Result<Vec<u8>, SchemaError> {
        let mut value = Vec::with_capacity(HashType::BlockHash.size() + 1);
        value.extend(&self.block_hash);
        value.push(self.validation_pass);
        Ok(value)
    }
}

impl Codec for OperationsForBlocksMessage {
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        OperationsForBlocksMessage::from_bytes(bytes.to_vec())
            .map_err(|_| SchemaError::DecodeError)
    }

    fn encode(&self) -> Result<Vec<u8>, SchemaError> {
        self.as_bytes()
            .map_err(|_| SchemaError::EncodeError)
    }
}



#[cfg(test)]
mod tests {
    use failure::Error;

    use tezos_encoding::hash::HashEncoding;

    use super::*;

    #[test]
    fn operations_key_encoded_equals_decoded() -> Result<(), Error> {
        let expected = OperationKey {
            block_hash: HashEncoding::new(HashType::BlockHash).string_to_bytes("BKyQ9EofHrgaZKENioHyP4FZNsTmiSEcVmcghgzCC9cGhE7oCET")?,
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
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);
        {
            let db = DB::open_cf_descriptors(&opts, path, vec![OperationsStorage::cf_descriptor()]).unwrap();
            let block_hash = HashEncoding::new(HashType::BlockHash).string_to_bytes("BKyQ9EofHrgaZKENioHyP4FZNsTmiSEcVmcghgzCC9cGhE7oCET")?;
            let mut message = OperationsForBlocksMessage {
                operations: vec![],
                operation_hashes_path: Path::Op,
                operations_for_block: OperationsForBlock {
                    hash: block_hash.clone(),
                    validation_pass: 0
                }
            };


            let mut storage = OperationsStorage::new(Arc::new(db));
            message.operations_for_block.validation_pass = 3;
            storage.insert(&message)?;
            message.operations_for_block.validation_pass = 1;
            storage.insert(&message)?;
            message.operations_for_block.validation_pass = 0;
            storage.insert(&message)?;
            message.operations_for_block.hash = HashEncoding::new(HashType::BlockHash).string_to_bytes("BLaf78njreWdt2WigJjM9e3ecEdVKm5ehahUfYBKvcWvZ8vfTcJ")?;
            message.operations_for_block.validation_pass = 1;
            storage.insert(&message)?;
            message.operations_for_block.hash = block_hash.clone();
            message.operations_for_block.validation_pass = 2;
            storage.insert(&message)?;
            message.operations_for_block.hash = HashEncoding::new(HashType::BlockHash).string_to_bytes("BKzyxvaMgoY5M3BUD7UaUCPivAku2NRiYRA1z1LQUzB7CX6e8yy")?;
            message.operations_for_block.validation_pass = 3;
            storage.insert(&message)?;

            let operations = storage.get_operations(&block_hash)?;
            assert_eq!(4, operations.len(), "Was expecting vector of {} elements but instead found {}", 4, operations.len());
            for i in 0..4 {
                assert_eq!(i as i8, operations[i].operations_for_block.validation_pass, "Was expecting operation pass {} but found {}", i, operations[i].operations_for_block.validation_pass);
                assert_eq!(block_hash, operations[i].operations_for_block.hash, "Block hash mismatch");
            }
        }
        assert!(DB::destroy(&opts, path).is_ok());
        Ok(())
    }
}