// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use tezos_encoding::hash::{BlockHash, ContextHash};

use crate::{BlockHeaderWithHash, StorageError};
use crate::persistent::{BincodeEncoded, DatabaseWithSchema, Schema};

pub type ContextMetaStorageDatabase = dyn DatabaseWithSchema<ContextMetaStorage> + Sync + Send;

#[derive(Clone)]
pub struct ContextMetaStorage {
    db: Arc<ContextMetaStorageDatabase>
}

impl ContextMetaStorage {

    pub fn new(db: Arc<ContextMetaStorageDatabase>) -> Self {
        ContextMetaStorage { db }
    }

    #[inline]
    pub fn put_block_header(&mut self, block: &BlockHeaderWithHash) -> Result<(), StorageError> {
        let value = Meta {
            block_hash: block.hash.clone(),
            level: block.header.level()
        };
        self.db.put(block.header.context(), &value)
            .map_err(StorageError::from)
    }


    #[inline]
    pub fn get(&self, context_hash: &ContextHash) -> Result<Option<Meta>, StorageError> {
        self.db.get(context_hash)
            .map_err(StorageError::from)
    }
}

/// Meta information for the context
#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct Meta {
    pub block_hash: BlockHash,
    pub level: i32,
}

impl BincodeEncoded for Meta {}

impl Schema for ContextMetaStorage {
    const COLUMN_FAMILY_NAME: &'static str = "context_meta_storage";
    type Key = ContextHash;
    type Value = Meta;
}

