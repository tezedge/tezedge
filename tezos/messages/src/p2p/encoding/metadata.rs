// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use getset::CopyGetters;
use serde::{Deserialize, Serialize};

use tezos_encoding::encoding::{Encoding, Field, HasEncoding};

use crate::p2p::binary_message::cache::{BinaryDataCache, CachedData, CacheReader, CacheWriter};

#[derive(Serialize, Deserialize, Debug, CopyGetters, Clone)]
pub struct MetadataMessage {
    #[get_copy = "pub"]
    disable_mempool: bool,
    #[get_copy = "pub"]
    private_node: bool,
    #[serde(skip_serializing)]
    body: BinaryDataCache
}

impl MetadataMessage {
    pub fn new(disable_mempool: bool, private_node: bool) -> Self {
        MetadataMessage {
            disable_mempool,
            private_node,
            body: Default::default()
        }
    }
}

impl HasEncoding for MetadataMessage {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("disable_mempool", Encoding::Bool),
            Field::new("private_node", Encoding::Bool)
        ])
    }
}

impl CachedData for MetadataMessage {
    #[inline]
    fn cache_reader(&self) -> & dyn CacheReader {
        &self.body
    }

    #[inline]
    fn cache_writer(&mut self) -> Option<&mut dyn CacheWriter> {
        Some(&mut self.body)
    }
}
