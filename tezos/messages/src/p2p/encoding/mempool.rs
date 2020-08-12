// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use getset::Getters;
use serde::{Deserialize, Serialize};

use crypto::hash::{HashType, OperationHash};
use tezos_encoding::encoding::{Encoding, Field, FieldName, HasEncoding};

use crate::p2p::binary_message::cache::{BinaryDataCache, CachedData, CacheReader, CacheWriter};

#[derive(Clone, Serialize, Deserialize, Debug, Default, Getters)]
pub struct Mempool {
    #[get = "pub"]
    known_valid: Vec<OperationHash>,
    #[get = "pub"]
    pending: Vec<OperationHash>,
    #[serde(skip_serializing)]
    body: BinaryDataCache,
}

impl Mempool {
    pub fn new(known_valid: Vec<OperationHash>, pending: Vec<OperationHash>) -> Self {
        Mempool {
            known_valid,
            pending,
            body: Default::default(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.known_valid.is_empty() && self.pending.is_empty()
    }
}

impl HasEncoding for Mempool {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new(FieldName::KnownValid, Encoding::dynamic(Encoding::list(Encoding::Hash(HashType::OperationHash)))),
            Field::new(FieldName::Pending, Encoding::dynamic(Encoding::dynamic(Encoding::list(Encoding::Hash(HashType::OperationHash))))),
        ])
    }
}

impl CachedData for Mempool {
    #[inline]
    fn cache_reader(&self) -> &dyn CacheReader {
        &self.body
    }

    #[inline]
    fn cache_writer(&mut self) -> Option<&mut dyn CacheWriter> {
        Some(&mut self.body)
    }
}
