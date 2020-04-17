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

    /// Returns true if protocol version is supported.
    ///
    /// The protocol is supported in case the `name` and `major` version are the same.
    /// Minor version is ignored.
    pub fn supports(&self, other: &Version) -> bool {
        self.name == other.name && self.major == other.major
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

impl Eq for Version { }

impl PartialEq for Version {
    fn eq(&self, other: &Self) -> bool {
        return self.name == other.name
            && self.major == other.major
            && self.minor == other.minor;
    }
}