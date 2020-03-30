// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use getset::Getters;
use serde::{Serialize, Deserialize};

use tezos_encoding::encoding::{Encoding, Field, HasEncoding};

use crate::p2p::binary_message::cache::{BinaryDataCache, CachedData, CacheReader, CacheWriter};

#[derive(Serialize, Deserialize, Debug, Getters, Clone)]
pub struct SwapMessage {
    #[get = "pub"]
    point: String,
    #[get = "pub"]
    peer_id: String,

    #[serde(skip_serializing)]
    body: BinaryDataCache,
}

impl HasEncoding for SwapMessage {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("point", Encoding::String),
            Field::new("peer_id", Encoding::String),
        ])
    }
}

impl CachedData for SwapMessage {
    #[inline]
    fn cache_reader(&self) -> &dyn CacheReader {
        &self.body
    }

    #[inline]
    fn cache_writer(&mut self) -> Option<&mut dyn CacheWriter> {
        Some(&mut self.body)
    }
}