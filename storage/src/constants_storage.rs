// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

// use std::io::SeekFrom;
use std::sync::Arc;

use rocksdb::{Cache, ColumnFamilyDescriptor};

use crypto::hash::ProtocolHash;

use crate::database::tezedge_database::{KVStoreKeyValueSchema, TezedgeDatabaseWithIterator};
use crate::persistent::database::{default_table_options, RocksDbKeyValueSchema};
use crate::persistent::KeyValueSchema;
use crate::{IteratorMode, PersistentStorage, StorageError};

pub type ConstantsStorageKV = dyn TezedgeDatabaseWithIterator<ConstantsStorage> + Sync + Send;

type ConstantsKey = ProtocolHash;

#[derive(Clone)]
pub struct ConstantsStorage {
    kv: Arc<ConstantsStorageKV>,
}

pub type ConstantsData = String;

impl ConstantsStorage {
    pub fn new(persistent_storage: &PersistentStorage) -> Self {
        Self {
            kv: persistent_storage.main_db(),
        }
    }

    pub fn store_constants_data(
        &self,
        protocol_hash: ProtocolHash,
        new_constants: String,
    ) -> Result<(), StorageError> {
        self.put(&protocol_hash, &new_constants)?;

        Ok(())
    }

    #[inline]
    fn put(&self, key: &ConstantsKey, data: &str) -> Result<(), StorageError> {
        self.kv
            .put(key, &data.to_string())
            .map_err(StorageError::from)
    }

    pub fn get(&self, key: &ConstantsKey) -> Result<Option<ConstantsData>, StorageError> {
        self.kv.get(key).map_err(StorageError::from)
    }

    pub fn iterator(&self) -> Result<Vec<(ConstantsKey, ConstantsData)>, StorageError> {
        use crate::persistent::codec::Decoder;
        self.kv
            .find(IteratorMode::Start)?
            .map(|result| {
                let result = result?;
                let k = <Self as KeyValueSchema>::Key::decode(&result.0)?;
                let v = <Self as KeyValueSchema>::Value::decode(&result.1)?;
                Ok((k, v))
            })
            .collect()
    }
}

impl KeyValueSchema for ConstantsStorage {
    type Key = ConstantsKey;
    type Value = ConstantsData;
}

impl RocksDbKeyValueSchema for ConstantsStorage {
    fn descriptor(cache: &Cache) -> ColumnFamilyDescriptor {
        let cf_opts = default_table_options(cache);
        ColumnFamilyDescriptor::new(Self::name(), cf_opts)
    }

    #[inline]
    fn name() -> &'static str {
        "constants_storage"
    }
}

impl KVStoreKeyValueSchema for ConstantsStorage {
    fn column_name() -> &'static str {
        Self::name()
    }
}
