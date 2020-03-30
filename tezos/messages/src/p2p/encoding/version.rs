// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use tezos_encoding::encoding::{Encoding, Field, HasEncoding};

use crate::p2p::binary_message::cache::{BinaryDataCache, CachedData, CacheReader, CacheWriter};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Version {
    name: String,
    major: u16,
    minor: u16,
    #[serde(skip_serializing)]
    body: BinaryDataCache,
}

impl Version {
    pub fn new(name: String, major: u16, minor: u16) -> Self {
        Version { name, major, minor, body: Default::default() }
    }
}

impl HasEncoding for Version {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("name", Encoding::String),
            Field::new("major", Encoding::Uint16),
            Field::new("minor", Encoding::Uint16)
        ])
    }
}

impl CachedData for Version {
    #[inline]
    fn cache_reader(&self) -> & dyn CacheReader {
        &self.body
    }

    #[inline]
    fn cache_writer(&mut self) -> Option<&mut dyn CacheWriter> {
        Some(&mut self.body)
    }
}
