use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};

use bytes::{Buf, BufMut, IntoBuf};
use rocksdb::{ColumnFamilyDescriptor, Options, SliceTransform, MergeOperands};
use serde::{Deserialize, Serialize};

use networking::p2p::encoding::prelude::*;
use tezos_encoding::hash::{BlockHash, HashRef, HashType};

use crate::{BlockHeaderWithHash, StorageError};
use crate::persistent::{Codec, Schema, SchemaError, DatabaseWithSchema};
use std::sync::Arc;

pub type OperationsStorageDatabase = dyn DatabaseWithSchema<OperationsStorage> + Sync + Send;

pub struct OperationsStorage {
    db: Arc<OperationsStorageDatabase>
}

impl OperationsStorage {
    pub fn new(db: Arc<OperationsStorageDatabase>) -> Self {
        OperationsStorage { db }
    }

    pub fn initialize(&mut self, block_header: &BlockHeaderWithHash) -> Result<(), StorageError> {
        self.db.put(
            &OperationKey::Meta {
                block_hash: block_header.hash.clone()
            },
            &OperationValue::Meta {
                validation_passes: block_header.header.validation_pass,
                is_validation_pass_present: vec![false as u8; block_header.header.validation_pass as usize],
                is_complete: false
            }
        ).map_err(|e| e.into())
    }

    pub fn insert(&mut self, message: OperationsForBlocksMessage) -> Result<(), StorageError> {
        let block_hash =  HashRef::new(message.operations_for_block.hash.clone());
        let meta_key = OperationKey::Meta { block_hash: block_hash.clone() };

        match self.db.get(&meta_key)? {
            Some(OperationValue::Meta { mut is_validation_pass_present, is_complete, validation_passes }) => {
                let validation_pass = u8::from(message.operations_for_block.validation_pass);

                // update validation passes and check if we have all operations
                is_validation_pass_present[validation_pass as usize] = true as u8;
                let is_complete = is_validation_pass_present.iter().all(|v| *v == (true as u8));
                self.db.merge(&meta_key, &OperationValue::Meta { is_validation_pass_present, validation_passes, is_complete })?;

                // store value of the message
                let message_key = OperationKey::Message {
                    block_hash: block_hash.clone(),
                    validation_pass
                };
                let message_value = OperationValue::Message { message };
                self.db.put(&message_key, &message_value)
                    .map_err(|e| e.into())
            }
            None => Err(StorageError::MissingKey),
            _ => Err(StorageError::UnexpectedValue)
        }
    }

    pub fn get_missing_validation_passes(&mut self, block_hash: &HashRef) -> Result<HashSet<i8>, StorageError> {
        let meta_key = OperationKey::Meta { block_hash: block_hash.clone() };
        match self.db.get(&meta_key)? {
            Some(OperationValue::Meta { is_complete, is_validation_pass_present, .. }) => {
                let result  = if is_complete {
                    HashSet::new()
                } else {
                    missing_validation_passes.iter().enumerate()
                        .filter(|(_, is_present)| **is_present == (true as u8))
                        .map(|(idx, _)| idx as i8)
                        .collect()
                };
                Ok(result)
            }
            None => Err(StorageError::MissingKey),
            _ => Err(StorageError::UnexpectedValue)
        }
    }

    pub fn is_complete(&self, block_hash: &HashRef) -> Result<bool, StorageError> {
        let meta_key = OperationKey::Meta { block_hash: block_hash.clone() };
        match self.db.get(&meta_key)? {
            Some(OperationValue::Meta { is_complete, .. }) => {
                Ok(is_complete)
            }
            None => Err(StorageError::MissingKey),
            _ => Err(StorageError::UnexpectedValue)
        }
    }


    pub fn contains_operations(&self, block_hash: &HashRef) -> bool {
        self.db.get(&OperationKey::Meta { block_hash: block_hash.clone() }).unwrap().is_some()
    }
}

const KEY_META_VARIANT_ID: u8 = 0;
const KEY_MESSAGE_VARIANT_ID: u8 = 1;
const VALUE_META_VARIANT_ID: u8 = 0xf0;
const VALUE_MESSAGE_VARIANT_ID: u8 = 0xf1;


/// Layout of the `OperationKey` is:
/// `[block_hash][variant_id]<validation_pass>`
#[derive(Debug, PartialEq)]
pub enum OperationKey {
    Meta {
        block_hash: HashRef,
    },
    Message {
        block_hash: HashRef,
        validation_pass: u8,
    },
}

impl Codec for OperationKey {
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        let mut buf = bytes.to_vec().into_buf();
        let mut block_hash = vec![0; HashType::BlockHash.size()];
        buf.copy_to_slice(&mut block_hash);
        match buf.get_u8() {
            KEY_META_VARIANT_ID => Ok(OperationKey::Meta {
                block_hash: HashRef::new(block_hash)
            }),
            KEY_MESSAGE_VARIANT_ID => Ok(OperationKey::Message {
                block_hash: HashRef::new(block_hash),
                validation_pass: buf.get_u8(),
            }),
            _ => Err(SchemaError::DecodeError)
        }
    }

    fn encode(&self) -> Result<Vec<u8>, SchemaError> {
        let mut value = vec![];
        match self {
            OperationKey::Meta { block_hash } => {
                value.extend(&block_hash.hash);
                value.put_u8(KEY_META_VARIANT_ID);
            }
            OperationKey::Message { block_hash, validation_pass } => {
                value.extend(&block_hash.hash);
                value.put_u8(KEY_MESSAGE_VARIANT_ID);
                value.put_u8(*validation_pass);
            }
        }
        Ok(value)
    }
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub enum OperationValue {
    Meta {
        is_validation_pass_present: Vec<u8>,
        validation_passes: u8,
        is_complete: bool
    },
    Message {
        message: OperationsForBlocksMessage
    },
}

impl Codec for OperationValue {
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        bincode::deserialize(&bytes[1..])
            .map_err(|_| SchemaError::DecodeError)
    }

    fn encode(&self) -> Result<Vec<u8>, SchemaError> {
        bincode::serialize(self)
            .map_err(|_| SchemaError::DecodeError)
    }
}


impl Schema for OperationsStorage {
    const COLUMN_FAMILY_NAME: &'static str = "operations_storage";
    type Key = OperationKey;
    type Value = OperationValue;

    fn cf_descriptor() -> ColumnFamilyDescriptor {
        let mut cf_opts = Options::default();
        cf_opts.set_prefix_extractor(SliceTransform::create_fixed_prefix(HashType::BlockHash.size()));
        cf_opts.set_memtable_prefix_bloom_ratio(0.2);
        cf_opts.set_merge_operator("operations_storage_merge_operator", merge_meta_value, None);
        ColumnFamilyDescriptor::new(Self::COLUMN_FAMILY_NAME, cf_opts)
    }
}

fn merge_meta_value(new_key: &[u8], existing_val: Option<&[u8]>, operands: &mut MergeOperands) -> Option<Vec<u8>> {

   let mut result: Vec<u8> = Vec::with_capacity(operands.size_hint().0);
   existing_val.map(|v| {
       for e in v {
           result.push(*e)
       }
   });
   for op in operands {
       for e in op {
           result.push(*e)
       }
   }
   Some(result)
}

#[derive(Clone)]
pub struct MissingOperations {
    pub block_hash: HashRef,
    pub validation_passes: HashSet<i8>
}

impl PartialEq for MissingOperations {
    fn eq(&self, other: &Self) -> bool {
        self.block_hash == other.block_hash
    }
}

impl Eq for MissingOperations {}

impl Hash for MissingOperations {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.block_hash.hash(state);
    }
}


impl From<&MissingOperations> for Vec<OperationsForBlock> {
    fn from(ops: &MissingOperations) -> Self {
        ops.validation_passes
            .iter()
            .map(|vp| {
                OperationsForBlock {
                    hash: ops.block_hash.get_hash(),
                    validation_pass: *vp
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use failure::Error;
    use tezos_encoding::hash::HashEncoding;

    use super::*;
    use std::iter::FromIterator;

    #[test]
    fn operations_key_encoded_equals_decoded() -> Result<(), Error> {
        let expected = OperationKey::Meta {
            block_hash: HashRef::new(HashEncoding::new(HashType::BlockHash).string_to_bytes("BKyQ9EofHrgaZKENioHyP4FZNsTmiSEcVmcghgzCC9cGhE7oCET")?)
        };
        let encoded_bytes = expected.encode()?;
        let decoded = OperationKey::decode(&encoded_bytes)?;
        assert_eq!(expected, decoded);

        let expected = OperationKey::Message {
            block_hash: HashRef::new(HashEncoding::new(HashType::BlockHash).string_to_bytes("BKyQ9EofHrgaZKENioHyP4FZNsTmiSEcVmcghgzCC9cGhE7oCET")?),
            validation_pass: 4,
        };
        let encoded_bytes = expected.encode()?;
        let decoded = OperationKey::decode(&encoded_bytes)?;
        Ok(assert_eq!(expected, decoded))
    }

    #[test]
    fn operations_value_encoded_equals_decoded() -> Result<(), Error> {
        let expected = OperationValue::Meta {
            is_validation_pass_present: vec![false as u8; 5],
            is_complete: false,
            validation_passes: 5
        };
        let encoded_bytes = expected.encode()?;
        let decoded = OperationValue::decode(&encoded_bytes)?;
        Ok(assert_eq!(expected, decoded))
    }


    #[test]
    fn mergetest() {
        use rocksdb::{Options, DB};

        let path = "_opstorage_mergetest";
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.set_merge_operator("test operator", test_provided_merge, None);
        {
            let db = DB::open(&opts, path).unwrap();
            let p = db.put(b"k1", b"a");
            assert!(p.is_ok());
            let _ = db.merge(b"k1", b"b");
            let _ = db.merge(b"k1", b"c");
            let _ = db.merge(b"k1", b"d");
            let _ = db.merge(b"k1", b"efg");
            let m = db.merge(b"k1", b"h");
            assert!(m.is_ok());
            match db.get(b"k1") {
                Ok(Some(value)) => match value.to_utf8() {
                    Some(v) => println!("retrieved utf8 value: {}", v),
                    None => println!("did not read valid utf-8 out of the db"),
                },
                Err(_) => println!("error reading value"),
                _ => panic!("value not present"),
            }

            assert!(m.is_ok());
            let r = db.get(b"k1");
            assert!(r.unwrap().unwrap().to_utf8().unwrap() == "abcdefgh");
            assert!(db.delete(b"k1").is_ok());
            assert!(db.get(b"k1").unwrap().is_none());
        }
        assert!(DB::destroy(&opts, path).is_ok());
    }
}