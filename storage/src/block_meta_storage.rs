use std::sync::Arc;

use rocksdb::{ColumnFamilyDescriptor, MergeOperands, Options};

use tezos_encoding::hash::BlockHash;

use crate::persistent::{Codec, DatabaseWithSchema, Schema, SchemaError};
use crate::StorageError;

pub type BlockMetaStorageDatabase = dyn DatabaseWithSchema<BlockMetaStorage> + Sync + Send;

/// Structure for representing in-memory db for - just for demo purposes.
pub struct BlockMetaStorage {
    db: Arc<BlockMetaStorageDatabase>
}

impl BlockMetaStorage {

    pub fn new(db: Arc<BlockMetaStorageDatabase>) -> Self {
        BlockMetaStorage { db }
    }

    pub fn insert(&mut self, block_hash: &BlockHash, meta: &Meta) -> Result<(), StorageError> {
        self.db.put(block_hash, meta)
            .map_err(StorageError::from)
    }

    pub fn get(&self, block_hash: &BlockHash) -> Result<Option<Meta>, StorageError> {
        self.db.get(block_hash)
            .map_err(StorageError::from)
    }

}

const MASK_IS_PROCESSED: u8  = 0b0000_0001;
const MASK_HAS_SUCCESSOR: u8 = 0b0000_0010;

const IDX_MASK: usize = 0;
const IDX_PREDECESSOR: usize = IDX_MASK + 1;
const IDX_SUCCESSOR: usize = IDX_PREDECESSOR + 32;

const LEN_NO_SUCCESSOR: usize = 1 + 32 + 0;
const LEN_WITH_SUCCESSOR: usize = 1 + 32 + 32;

/// Meta information for the block
#[derive(Clone, PartialEq, Debug)]
pub struct Meta {
    predecessor: BlockHash,
    successor: Option<BlockHash>,
    is_processed: bool,
}

/// Codec for `Meta`
///
/// * bytes layout: `[mask(1)][predecessor(32)][opt_successor(32)]`
impl Codec for Meta {
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        if LEN_NO_SUCCESSOR <= bytes.len() {
            let mask = bytes[IDX_MASK];
            let is_processed = (mask & MASK_IS_PROCESSED) != 0;
            let predecessor = bytes[IDX_PREDECESSOR..IDX_SUCCESSOR].to_vec();

            let successor = if (mask & MASK_HAS_SUCCESSOR) != 0 {
                if LEN_WITH_SUCCESSOR == bytes.len() {
                    Some(bytes[IDX_SUCCESSOR..LEN_WITH_SUCCESSOR].to_vec())
                } else {
                    return Err(SchemaError::DecodeError);
                }
            } else {
                None
            };
            Ok(Meta { predecessor, successor, is_processed })
        } else {
            Err(SchemaError::DecodeError)
        }
    }

    fn encode(&self) -> Result<Vec<u8>, SchemaError> {
        let mut value = vec![];

        let byte_is_processed = if self.is_processed { MASK_IS_PROCESSED } else { 0 };
        let byte_has_successor = if self.successor.is_some() { MASK_HAS_SUCCESSOR } else { 0 };
        let mask = byte_is_processed | byte_has_successor;
        value.push(mask);
        value.extend(&self.predecessor);
        self.successor.as_ref().map(|successor| value.extend(successor));

        Ok(value)
    }
}

impl Schema for BlockMetaStorage {
    const COLUMN_FAMILY_NAME: &'static str = "block_meta_storage";
    type Key = BlockHash;
    type Value = Meta;

    fn cf_descriptor() -> ColumnFamilyDescriptor {
        let mut cf_opts = Options::default();
        cf_opts.set_merge_operator("block_meta_storage_merge_operator", merge_meta_value, None);
        ColumnFamilyDescriptor::new(Self::COLUMN_FAMILY_NAME, cf_opts)
    }
}

fn merge_meta_value(_new_key: &[u8], existing_val: Option<&[u8]>, operands: &mut MergeOperands) -> Option<Vec<u8>> {
    let mut result = existing_val.map(|v| v.to_vec());

    for op in operands {
        match result {
            Some(ref mut val) => {
                assert!(val.len() == LEN_NO_SUCCESSOR || val.len() == LEN_WITH_SUCCESSOR, "Value length is incorrect");
                assert!(val.len() <= op.len(), "Only extension of the value length is allowed");

                // merge `mask(1)`
                let mask = val[IDX_MASK] | op[IDX_MASK];

                if val.len() == op.len() {
                    val.clear();
                    val.extend(op);
                } else if op.len() == LEN_WITH_SUCCESSOR {
                    val.extend(&op[IDX_SUCCESSOR..LEN_WITH_SUCCESSOR]);
                }

                val[IDX_MASK] = mask;
            },
            None => result = Some(op.to_vec())
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use failure::Error;

    use super::*;

    #[test]
    fn block_meta_encoded_equals_decoded() -> Result<(), Error> {
        let expected = Meta {
            is_processed: false,
            predecessor: vec![98; 32],
            successor: Some(vec![21; 32])
        };
        let encoded_bytes = expected.encode()?;
        let decoded = Meta::decode(&encoded_bytes)?;
        Ok(assert_eq!(expected, decoded))
    }


    #[test]
    fn merge_meta_value_test() {
        use rocksdb::{Options, DB};

        let path = "__blockmeta_mergetest";
        if Path::new(path).exists() {
            std::fs::remove_dir_all(path).unwrap();
        }
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);
        opts.set_merge_operator("test operator", merge_meta_value, None);
        {
            let db = DB::open_cf_descriptors(&opts, path, vec![BlockMetaStorage::cf_descriptor()]).unwrap();
            let k = vec![44; 32];
            let mut v = Meta {
                is_processed: false,
                predecessor: vec![98; 32],
                successor: None
            };
            let p = BlockMetaStorageDatabase::merge(&db, &k, &v);
            assert!(p.is_ok(), "p: {:?}", p.unwrap_err());
            v.is_processed = true;
            v.successor = Some(vec![21; 32]);
            let _ = BlockMetaStorageDatabase::merge(&db, &k, &v);
            v.is_processed = false;
            let m = BlockMetaStorageDatabase::merge(&db, &k, &v);
            assert!(m.is_ok());
            match BlockMetaStorageDatabase::get(&db, &k) {
                Ok(Some(value)) => {
                    let expected = Meta {
                        is_processed: true,
                        predecessor: vec![98; 32],
                        successor: Some(vec![21; 32])
                    };
                    assert_eq!(expected, value);
                },
                Err(_) => println!("error reading value"),
                _ => panic!("value not present"),
            }
        }
        assert!(DB::destroy(&opts, path).is_ok());
    }
}