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
}