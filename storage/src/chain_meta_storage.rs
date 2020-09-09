// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use rocksdb::ColumnFamilyDescriptor;
use serde::{Deserialize, Serialize};

use crypto::hash::{ChainId, HashType};
use tezos_messages::Head;

use crate::persistent::{BincodeEncoded, Decoder, default_table_options, Encoder, KeyValueSchema, KeyValueStoreWithSchema, PersistentStorage, SchemaError};
use crate::StorageError;

pub type ChainMetaStorageKv = dyn KeyValueStoreWithSchema<ChainMetaStorage> + Sync + Send;
pub type DbVersion = i64;

pub trait ChainMetaStorageReader: Sync + Send {
    /// Load current head from dedicated storage
    fn get_current_head(&self, chain_id: &ChainId) -> Result<Option<Head>, StorageError>;
}

/// Represents storage of the chain metadata (current_head, test_chain, ...).
/// Metadata are related to concrete chain_id, which is used as a part of key.
///
/// Reason for this storage is that, for example current head cannot be easily selected from block_meta_storage (lets say by level),
/// because there are some computations (with fitness) that need to be done...
///
/// This storage differs from the other in regard that it is not exposing key-value pair
/// but instead it provides get_ and set_ methods for each property prefixed with chain_id.
///
/// Maybe this properties split is not very nice but, we need to access properties separatly,
/// if we stored metadata grouped to struct K-V: <chain_id> - MetadataStruct,
/// we need to all the time deserialize whole sturct.
///
/// e.g. storage key-value will looks like:
/// (<main_chain_id>, 'current_head') - block_hash_xyz
/// (<main_chain_id>, 'test_chain_id') - chain_id_xyz
///
#[derive(Clone)]
pub struct ChainMetaStorage {
    kv: Arc<ChainMetaStorageKv>
}

impl ChainMetaStorage {
    pub fn new(persistent_storage: &PersistentStorage) -> Self {
        Self { kv: persistent_storage.kv() }
    }

    #[inline]
    pub fn set_current_head(&self, chain_id: &ChainId, head: &Head) -> Result<(), StorageError> {
        self.kv
            .put(
                &MetaKey::key_current_head(chain_id.clone()),
                &MetadataValue::CurrentHead(head.clone()),
            )
            .map_err(StorageError::from)
    }


    #[inline]
    pub fn get_test_chain_id(&self, chain_id: &ChainId) -> Result<Option<ChainId>, StorageError> {
        self.kv
            .get(&MetaKey::key_test_chain_id(chain_id.clone()))
            .map(|result| match result {
                Some(MetadataValue::TestChainId(value)) => Some(value),
                _ => None
            })
            .map_err(StorageError::from)
    }

    #[inline]
    pub fn set_test_chain_id(&self, chain_id: &ChainId, test_chain_id: &ChainId) -> Result<(), StorageError> {
        self.kv
            .put(
                &MetaKey::key_test_chain_id(chain_id.clone()),
                &MetadataValue::TestChainId(test_chain_id.clone()),
            )
            .map_err(StorageError::from)
    }

    #[inline]
    pub fn remove_test_chain_id(&self, chain_id: &ChainId) -> Result<(), StorageError> {
        self.kv
            .delete(&MetaKey::key_test_chain_id(chain_id.clone()))
            .map_err(StorageError::from)
    }
}

impl ChainMetaStorageReader for ChainMetaStorage {
    #[inline]
    fn get_current_head(&self, chain_id: &ChainId) -> Result<Option<Head>, StorageError> {
        self.kv
            .get(&MetaKey::key_current_head(chain_id.clone()))
            .map(|result| match result {
                Some(MetadataValue::CurrentHead(value)) => Some(value),
                _ => None
            })
            .map_err(StorageError::from)
    }
}

impl KeyValueSchema for ChainMetaStorage {
    type Key = MetaKey;
    type Value = MetadataValue;

    fn descriptor() -> ColumnFamilyDescriptor {
        let mut cf_opts = default_table_options();
        // 1 MB
        cf_opts.set_write_buffer_size(1024 * 1024);
        ColumnFamilyDescriptor::new(Self::name(), cf_opts)
    }

    #[inline]
    fn name() -> &'static str {
        "chain_meta_storage"
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct MetaKey {
    chain_id: ChainId,
    key: String,
}

impl MetaKey {
    const LEN_CHAIN_ID: usize = HashType::ChainId.size();
    const IDX_CHAIN_ID: usize = 0;
    const IDX_KEY: usize = Self::IDX_CHAIN_ID + Self::LEN_CHAIN_ID;

    const KEY_CURRENT_HEAD: &'static str = "ch";
    const KEY_TEST_CHAIN_ID: &'static str = "tcid";

    fn key_current_head(chain_id: ChainId) -> MetaKey {
        MetaKey {
            chain_id,
            key: Self::KEY_CURRENT_HEAD.to_string(),
        }
    }

    fn key_test_chain_id(chain_id: ChainId) -> MetaKey {
        MetaKey {
            chain_id,
            key: Self::KEY_TEST_CHAIN_ID.to_string(),
        }
    }
}

impl Encoder for MetaKey {
    fn encode(&self) -> Result<Vec<u8>, SchemaError> {
        if self.chain_id.len() == Self::LEN_CHAIN_ID {
            let mut bytes = Vec::with_capacity(Self::LEN_CHAIN_ID);
            bytes.extend(&self.chain_id);
            bytes.extend(self.key.encode()?);
            Ok(bytes)
        } else {
            Err(SchemaError::EncodeError)
        }
    }
}

impl Decoder for MetaKey {
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        if bytes.len() > Self::LEN_CHAIN_ID {
            let chain_id = bytes[Self::IDX_CHAIN_ID..Self::IDX_KEY].to_vec();
            let key = String::decode(&bytes[Self::IDX_KEY..])?;
            Ok(MetaKey { chain_id, key })
        } else {
            Err(SchemaError::DecodeError)
        }
    }
}

#[derive(Serialize, Deserialize)]
pub enum MetadataValue {
    CurrentHead(Head),
    TestChainId(ChainId),
}

impl BincodeEncoded for MetadataValue {}

impl BincodeEncoded for Head {}


#[cfg(test)]
mod tests {
    use failure::Error;

    use crypto::hash::HashType;

    use crate::tests_common::TmpStorage;

    use super::*;

    #[test]
    fn test_current_head() -> Result<(), Error> {
        let tmp_storage = TmpStorage::create("__test_current_head")?;
        let index = ChainMetaStorage::new(tmp_storage.storage());

        let chain_id1 = HashType::ChainId.string_to_bytes("NetXgtSLGNJvNye")?;
        let block_1 = Head {
            hash: HashType::BlockHash.string_to_bytes("BLockGenesisGenesisGenesisGenesisGenesisb83baZgbyZe")?,
            level: 1,
            fitness: vec![],
        };

        let chain_id2 = HashType::ChainId.string_to_bytes("NetXjD3HPJJjmcd")?;
        let block_2 = Head {
            hash: HashType::BlockHash.string_to_bytes("BLockGenesisGenesisGenesisGenesisGenesisd6f5afWyME7")?,
            level: 2,
            fitness: vec![],
        };

        // no current heads
        assert!(index.get_current_head(&chain_id1)?.is_none());
        assert!(index.get_current_head(&chain_id2)?.is_none());

        // set for chain_id1
        index.set_current_head(&chain_id1, &block_1)?;
        assert!(index.get_current_head(&chain_id1)?.is_some());
        assert_eq!(index.get_current_head(&chain_id1)?.unwrap().hash, block_1.hash.clone());
        assert!(index.get_current_head(&chain_id2)?.is_none());

        // set for chain_id2
        index.set_current_head(&chain_id2, &block_2)?;
        assert!(index.get_current_head(&chain_id1)?.is_some());
        assert_eq!(index.get_current_head(&chain_id1)?.unwrap().hash, block_1.hash.clone());
        assert!(index.get_current_head(&chain_id2)?.is_some());
        assert_eq!(index.get_current_head(&chain_id2)?.unwrap().hash, block_2.hash.clone());

        // update for chain_id1
        index.set_current_head(&chain_id1, &block_2)?;
        assert!(index.get_current_head(&chain_id1)?.is_some());
        assert_eq!(index.get_current_head(&chain_id1)?.unwrap().hash, block_2.hash.clone());
        assert!(index.get_current_head(&chain_id2)?.is_some());
        assert_eq!(index.get_current_head(&chain_id2)?.unwrap().hash, block_2.hash.clone());

        Ok(())
    }

    #[test]
    fn test_test_chain_id() -> Result<(), Error> {
        let tmp_storage = TmpStorage::create("__test_test_chain_id")?;
        let index = ChainMetaStorage::new(tmp_storage.storage());

        let chain_id1 = HashType::ChainId.string_to_bytes("NetXgtSLGNJvNye")?;
        let chain_id2 = HashType::ChainId.string_to_bytes("NetXjD3HPJJjmcd")?;
        let chain_id3 = HashType::ChainId.string_to_bytes("NetXjD3HPJJjmcd")?;

        assert!(index.get_test_chain_id(&chain_id1)?.is_none());
        assert!(index.get_test_chain_id(&chain_id2)?.is_none());

        // update for chain_id1
        index.set_test_chain_id(&chain_id1, &chain_id3)?;
        assert!(index.get_test_chain_id(&chain_id1)?.is_some());
        assert_eq!(index.get_test_chain_id(&chain_id1)?.unwrap(), chain_id3.clone());
        assert!(index.get_test_chain_id(&chain_id2)?.is_none());

        // update for chain_id2
        index.set_test_chain_id(&chain_id2, &chain_id3)?;
        assert!(index.get_test_chain_id(&chain_id1)?.is_some());
        assert_eq!(index.get_test_chain_id(&chain_id1)?.unwrap(), chain_id3.clone());
        assert!(index.get_test_chain_id(&chain_id2)?.is_some());
        assert_eq!(index.get_test_chain_id(&chain_id2)?.unwrap(), chain_id3.clone());

        // update for chain_id1
        index.set_test_chain_id(&chain_id1, &chain_id2)?;
        assert!(index.get_test_chain_id(&chain_id1)?.is_some());
        assert_eq!(index.get_test_chain_id(&chain_id1)?.unwrap(), chain_id2.clone());
        assert!(index.get_test_chain_id(&chain_id2)?.is_some());
        assert_eq!(index.get_test_chain_id(&chain_id2)?.unwrap(), chain_id3.clone());

        // remove for chain_id1
        index.remove_test_chain_id(&chain_id1)?;
        assert!(index.get_test_chain_id(&chain_id1)?.is_none());
        assert!(index.get_test_chain_id(&chain_id2)?.is_some());
        assert_eq!(index.get_test_chain_id(&chain_id2)?.unwrap(), chain_id3.clone());

        Ok(())
    }
}