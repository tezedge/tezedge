use std::collections::HashSet;
use std::sync::Arc;

use rocksdb::{ColumnFamilyDescriptor, MergeOperands, Options, SliceTransform};

use networking::p2p::binary_message::BinaryMessage;
use networking::p2p::encoding::prelude::*;
use tezos_encoding::hash::{HashRef, HashType};

use crate::{BlockHeaderWithHash, StorageError};
use crate::persistent::{Codec, DatabaseWithSchema, Schema, SchemaError};

pub type OperationsStorageDatabase = dyn DatabaseWithSchema<OperationsStorage> + Sync + Send;


pub struct OperationsStorage {
    db: Arc<OperationsStorageDatabase>
}

impl OperationsStorage {
    pub fn new(db: Arc<OperationsStorageDatabase>) -> Self {
        OperationsStorage { db }
    }

    pub fn insert(&mut self, message: &OperationsForBlocksMessage) -> Result<(), StorageError> {
        let key = OperationKey {
            block_hash: HashRef::new(message.operations_for_block.hash.clone()),
            validation_pass: message.operations_for_block.validation_pass as u8
        };
        self.db.put(&key, &message)
            .map_err(|e| e.into())
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
    block_hash: HashRef,
    validation_pass: u8,
}

impl Codec for OperationKey {
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        Ok(OperationKey {
            block_hash: HashRef::new(bytes[0..HashType::BlockHash.size()].to_vec()),
            validation_pass: bytes[HashType::BlockHash.size()]
        })
    }

    fn encode(&self) -> Result<Vec<u8>, SchemaError> {
        let mut value = Vec::with_capacity(HashType::BlockHash.size() + 1);
        value.extend(&*self.block_hash.hash);
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




/// Convenience type for operation meta storage database
pub type OperationsMetaStorageDatabase = dyn DatabaseWithSchema<OperationsMetaStorage> + Sync + Send;

pub struct OperationsMetaStorage {
    db: Arc<OperationsMetaStorageDatabase>
}

impl OperationsMetaStorage {
    pub fn new(db: Arc<OperationsMetaStorageDatabase>) -> Self {
        OperationsMetaStorage { db }
    }

    pub fn initialize(&mut self, block_header: &BlockHeaderWithHash) -> Result<(), StorageError> {
        self.db.put(&block_header.hash.clone(),
            &Meta {
                validation_passes: block_header.header.validation_pass,
                is_validation_pass_present: vec![false as u8; block_header.header.validation_pass as usize],
                is_complete: false
            }
        ).map_err(|e| e.into())
    }

    pub fn insert(&mut self, message: &OperationsForBlocksMessage) -> Result<(), StorageError> {
        let block_hash =  HashRef::new(message.operations_for_block.hash.clone());

        match self.db.get(&block_hash)? {
            Some(mut meta) => {
                let validation_pass = message.operations_for_block.validation_pass as u8;

                // update validation passes and check if we have all operations
                meta.is_validation_pass_present[validation_pass as usize] = true as u8;
                meta.is_complete = meta.is_validation_pass_present.iter().all(|v| *v == (true as u8));
                self.db.merge(&block_hash, &meta)
                    .map_err(|e| e.into())
            }
            None => Err(StorageError::MissingKey),
        }
    }

    pub fn get_missing_validation_passes(&mut self, block_hash: &HashRef) -> Result<HashSet<i8>, StorageError> {
        match self.db.get(block_hash)? {
            Some(meta) => {
                let result  = if meta.is_complete {
                    HashSet::new()
                } else {
                    meta.is_validation_pass_present.iter().enumerate()
                        .filter(|(_, is_present)| **is_present == (false as u8))
                        .map(|(idx, _)| idx as i8)
                        .collect()
                };
                Ok(result)
            }
            None => Err(StorageError::MissingKey),
        }
    }

    pub fn is_complete(&self, block_hash: &HashRef) -> Result<bool, StorageError> {
        match self.db.get(block_hash)? {
            Some(Meta { is_complete, .. }) => {
                Ok(is_complete)
            }
            None => Ok(false),
        }
    }


    pub fn contains(&self, block_hash: &HashRef) -> Result<bool, StorageError> {
        self.db.get(block_hash)
            .map(|v| v.is_some())
            .map_err(StorageError::from)
    }
}

impl Schema for OperationsMetaStorage {
    const COLUMN_FAMILY_NAME: &'static str = "operations_meta_storage";
    type Key = HashRef;
    type Value = Meta;

    fn cf_descriptor() -> ColumnFamilyDescriptor {
        let mut cf_opts = Options::default();
        cf_opts.set_merge_operator("operations_meta_storage_merge_operator", merge_meta_value, None);
        ColumnFamilyDescriptor::new(Self::COLUMN_FAMILY_NAME, cf_opts)
    }
}

fn merge_meta_value(_new_key: &[u8], existing_val: Option<&[u8]>, operands: &mut MergeOperands) -> Option<Vec<u8>> {
    let mut result = existing_val.map(|v| v.to_vec());

    for op in operands {
        match result {
            Some(ref mut val) => {
                assert_eq!(val.len(), op.len(), "Value length is fixed. expected={}, found={}", val.len(), op.len());
                assert_ne!(0, val.len(), "Value cannot have zero size");
                assert_eq!(val[0], op[0], "Value of validation passes cannot change");

                let validation_passes = val[0] as usize;
                for i in 1..=validation_passes {
                    val[i] |= op[i]
                }
            },
            None => result = Some(op.to_vec())
        }
    }

    result
}

#[derive(PartialEq, Debug)]
pub struct Meta {
    validation_passes: u8,
    is_validation_pass_present: Vec<u8>,
    is_complete: bool
}

impl Codec for Meta {
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        if bytes.len() > 0 {
            let validation_passes = bytes[0];
            if bytes.len() == ((validation_passes as usize) + 2) {
                let is_complete_pos = validation_passes as usize + 1;
                let is_validation_pass_present = bytes[1..is_complete_pos].to_vec();
                let is_complete = bytes[is_complete_pos] != 0;
                Ok(Meta { validation_passes, is_validation_pass_present, is_complete })
            } else {
                Err(SchemaError::DecodeError)
            }
        } else {
            Err(SchemaError::DecodeError)
        }
    }

    fn encode(&self) -> Result<Vec<u8>, SchemaError> {
        if (self.validation_passes as usize) == self.is_validation_pass_present.len() {
            let mut result = vec![];
            result.push(self.validation_passes);
            result.extend(&self.is_validation_pass_present);
            result.push(self.is_complete as u8);
            Ok(result)
        } else {
            Err(SchemaError::EncodeError)
        }
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
            block_hash: HashRef::new(HashEncoding::new(HashType::BlockHash).string_to_bytes("BKyQ9EofHrgaZKENioHyP4FZNsTmiSEcVmcghgzCC9cGhE7oCET")?),
            validation_pass: 4,
        };
        let encoded_bytes = expected.encode()?;
        let decoded = OperationKey::decode(&encoded_bytes)?;
        Ok(assert_eq!(expected, decoded))
    }

    #[test]
    fn operations_meta_encoded_equals_decoded() -> Result<(), Error> {
        let expected = Meta {
            is_validation_pass_present: vec![false as u8; 5],
            is_complete: false,
            validation_passes: 5
        };
        let encoded_bytes = expected.encode()?;
        let decoded = Meta::decode(&encoded_bytes)?;
        Ok(assert_eq!(expected, decoded))
    }


    #[test]
    fn merge_meta_value_test() {
        use rocksdb::{Options, DB};

        let path = "_opstorage_mergetest";
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);
        opts.set_merge_operator("test operator", merge_meta_value, None);
        {
            let t = true as u8;
            let f = false as u8;

            let db = DB::open_cf_descriptors(&opts, path, vec![OperationsMetaStorage::cf_descriptor()]).unwrap();
            let k = HashRef::new(vec![3, 1, 3, 3, 7]);
            let mut v = Meta {
                is_complete: false,
                is_validation_pass_present: vec![f; 5],
                validation_passes: 5,
            };
            let p = OperationsMetaStorageDatabase::merge(&db, &k, &v);
            assert!(p.is_ok(), "p: {:?}", p.unwrap_err());
            v.is_validation_pass_present[2] = t;
            let _ = OperationsMetaStorageDatabase::merge(&db, &k, &v);
            v.is_validation_pass_present[2] = f;
            v.is_validation_pass_present[3] = t;
            let _ = OperationsMetaStorageDatabase::merge(&db, &k, &v);
            v.is_validation_pass_present[3] = f;
            let _ = OperationsMetaStorageDatabase::merge(&db, &k, &v);
            let m = OperationsMetaStorageDatabase::merge(&db, &k, &v);
            assert!(m.is_ok());
            match OperationsMetaStorageDatabase::get(&db, &k) {
                Ok(Some(value)) => {
                    assert_eq!(vec![f, f, t, t, f], value.is_validation_pass_present);
                },
                Err(_) => println!("error reading value"),
                _ => panic!("value not present"),
            }
        }
        assert!(DB::destroy(&opts, path).is_ok());
    }
}