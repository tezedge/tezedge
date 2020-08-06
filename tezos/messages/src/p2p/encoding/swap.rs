// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use getset::Getters;
use serde::{Deserialize, Serialize};

use tezos_encoding::encoding::{Encoding, Field, HasEncoding};
use tezos_encoding::has_encoding;

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

has_encoding!(SwapMessage, SWAP_MESSAGE_ENCODING, {
        Encoding::Obj(vec![
            Field::new("point", Encoding::String),
            Field::new("peer_id", Encoding::String),
        ])
});

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