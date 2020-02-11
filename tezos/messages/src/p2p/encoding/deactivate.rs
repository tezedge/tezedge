// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use getset::Getters;
use serde::{Serialize, Deserialize};

use tezos_encoding::encoding::{Encoding, Field, HasEncoding};

use crate::p2p::binary_message::cache::{BinaryDataCache, CachedData, CacheReader, CacheWriter};
use crypto::hash::{ChainId, HashType};

#[derive(Serialize, Deserialize, Debug, Getters)]
pub struct DeactivateMessage {
    #[get = "pub"]
    deactivate: ChainId,

    #[serde(skip_serializing)]
    body: BinaryDataCache,
}

impl DeactivateMessage {
    pub fn new(deactivate: ChainId) -> Self {
        Self {
            deactivate,
            body: Default::default(),
        }
    }
}

impl HasEncoding for DeactivateMessage {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("deactivate", Encoding::Hash(HashType::ChainId)),
        ])
    }
}

impl CachedData for DeactivateMessage {
    #[inline]
    fn cache_reader(&self) -> &dyn CacheReader {
        &self.body
    }

    #[inline]
    fn cache_writer(&mut self) -> Option<&mut dyn CacheWriter> {
        Some(&mut self.body)
    }
}