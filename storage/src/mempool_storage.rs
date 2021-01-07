// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::fmt;
use std::fmt::Formatter;
use std::sync::Arc;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use crypto::hash::{HashType, OperationHash};
use tezos_messages::p2p::binary_message::MessageHash;
use tezos_messages::p2p::encoding::operation::OperationMessage;

use crate::persistent::{
    BincodeEncoded, Decoder, Encoder, KeyValueSchema, KeyValueStoreWithSchema, PersistentStorage,
    SchemaError, StorageType,
};
use crate::{num_from_slice, IteratorMode, StorageError};

/// Convenience type for operation meta storage database
pub type MempoolStorageKV = dyn KeyValueStoreWithSchema<MempoolStorage> + Sync + Send;

/// TODO: do we need this?
/// Distinct
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum MempoolOperationType {
    Pending,
    KnownValid,
}

impl MempoolOperationType {
    pub fn to_u8(&self) -> u8 {
        match self {
            MempoolOperationType::Pending => 0,
            MempoolOperationType::KnownValid => 1,
        }
    }

    pub fn from_u8(num: u8) -> Result<Self, MempoolOperationTypeParseError> {
        match num {
            0 => Ok(MempoolOperationType::Pending),
            1 => Ok(MempoolOperationType::KnownValid),
            invalid_num => Err(MempoolOperationTypeParseError(invalid_num)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MempoolOperationTypeParseError(u8);

impl fmt::Display for MempoolOperationTypeParseError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_fmt(format_args!("Invalid value {}", self.0))
    }
}

/// Operation metadata storage
#[derive(Clone)]
pub struct MempoolStorage {
    kv: Arc<MempoolStorageKV>,
}

impl MempoolStorage {
    pub fn new(persistent_storage: &PersistentStorage) -> Self {
        Self {
            kv: persistent_storage.kv(StorageType::Database),
        }
    }

    #[inline]
    pub fn put_pending(
        &mut self,
        message: OperationMessage,
        time_to_live: SystemTime,
    ) -> Result<(), StorageError> {
        self.put(MempoolOperationType::Pending, message, time_to_live)
    }

    #[inline]
    pub fn put_known_valid(
        &mut self,
        message: OperationMessage,
        time_to_live: SystemTime,
    ) -> Result<(), StorageError> {
        self.put(MempoolOperationType::KnownValid, message, time_to_live)
    }

    #[inline]
    pub fn put(
        &mut self,
        operation_type: MempoolOperationType,
        operation: OperationMessage,
        time_to_live: SystemTime,
    ) -> Result<(), StorageError> {
        let key = MempoolKey {
            operation_type,
            operation_hash: operation.message_hash()?,
        };
        let value = MempoolValue {
            operation,
            time_to_live,
        };

        self.kv.put(&key, &value).map_err(StorageError::from)
    }

    #[inline]
    pub fn get(
        &self,
        operation_type: MempoolOperationType,
        operation_hash: OperationHash,
    ) -> Result<Option<OperationMessage>, StorageError> {
        let key = MempoolKey {
            operation_type,
            operation_hash,
        };
        self.kv
            .get(&key)
            .map(|value| value.map(|value| value.operation))
            .map_err(StorageError::from)
    }

    #[inline]
    pub fn delete(&self, operation_hash: &OperationHash) -> Result<(), StorageError> {
        // TODO: implement correctly and effectively

        let key = MempoolKey {
            operation_type: MempoolOperationType::Pending,
            operation_hash: operation_hash.clone(),
        };
        self.kv.delete(&key).map_err(StorageError::from)?;

        let key = MempoolKey {
            operation_type: MempoolOperationType::KnownValid,
            operation_hash: operation_hash.clone(),
        };
        self.kv.delete(&key).map_err(StorageError::from)?;

        Ok(())
    }

    #[inline]
    pub fn find(
        &self,
        operation_hash: &OperationHash,
    ) -> Result<Option<OperationMessage>, StorageError> {
        // TODO: implement correctly and effectively

        // check pendings
        if let Some(found) = self.get(MempoolOperationType::Pending, operation_hash.clone())? {
            return Ok(Some(found));
        }

        // check known_valids
        if let Some(found) = self.get(MempoolOperationType::KnownValid, operation_hash.clone())? {
            return Ok(Some(found));
        }

        Ok(None)
    }

    #[inline]
    pub fn iter(&self) -> Result<Vec<(OperationHash, OperationMessage)>, StorageError> {
        let mut operations = Vec::new();
        for (key, value) in self.kv.iterator(IteratorMode::Start)? {
            let (key, value) = (key?, value?);
            operations.push((key.operation_hash, value.operation));
        }
        Ok(operations)
    }
}

impl KeyValueSchema for MempoolStorage {
    type Key = MempoolKey;
    type Value = MempoolValue;

    #[inline]
    fn name() -> &'static str {
        "mempool_storage"
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct MempoolKey {
    operation_type: MempoolOperationType,
    operation_hash: OperationHash,
}

impl MempoolKey {
    const LEN_TYPE: usize = 1;
    const LEN_HASH: usize = HashType::OperationHash.size();
    const LEN_KEY: usize = Self::LEN_TYPE + Self::LEN_HASH;

    const IDX_TYPE: usize = 0;
    const IDX_HASH: usize = Self::IDX_TYPE + Self::LEN_TYPE;
}

impl Encoder for MempoolKey {
    fn encode(&self) -> Result<Vec<u8>, SchemaError> {
        if self.operation_hash.len() == Self::LEN_HASH {
            let mut bytes = Vec::with_capacity(Self::LEN_KEY);
            bytes.push(self.operation_type.to_u8());
            bytes.extend(&self.operation_hash);
            Ok(bytes)
        } else {
            Err(SchemaError::EncodeError)
        }
    }
}

impl Decoder for MempoolKey {
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        if bytes.len() == Self::LEN_KEY {
            let operation_type =
                MempoolOperationType::from_u8(num_from_slice!(bytes, Self::IDX_TYPE, u8))
                    .map_err(|_| SchemaError::DecodeError)?;
            let operation_hash = bytes[Self::IDX_HASH..Self::IDX_HASH + Self::LEN_HASH].to_vec();
            Ok(MempoolKey {
                operation_type,
                operation_hash,
            })
        } else {
            Err(SchemaError::DecodeError)
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct MempoolValue {
    operation: OperationMessage,
    time_to_live: SystemTime,
}

impl BincodeEncoded for MempoolValue {}
