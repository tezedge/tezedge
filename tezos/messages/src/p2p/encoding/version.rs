// Copyright (c) SimpleStaking and Tezos-RS Contributors
// SPDX-License-Identifier: MIT

use derive_new::new;
use serde::{Deserialize, Serialize};

use tezos_encoding::encoding::{Encoding, Field, HasEncoding};

use crate::p2p::binary_message::cache::{BinaryDataCache, CachedData, CacheReader, CacheWriter};

#[derive(Serialize, Deserialize, Debug, new)]
pub struct Version {
    name: String,
    major: u16,
    minor: u16,
    #[new(default)]
    #[serde(skip_serializing)]
    body: BinaryDataCache,
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
