// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use tezos_encoding::encoding::{Encoding, Field, HasEncoding};

use crate::p2p::binary_message::cache::{BinaryDataCache, CachedData, CacheReader, CacheWriter};
use std::fmt;

#[derive(Serialize, Deserialize, Clone)]
pub struct Version {
    chain_name: String,
    distributed_db_version: u16,
    p2p_version: u16,
    #[serde(skip_serializing)]
    body: BinaryDataCache,
}

impl fmt::Debug for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Version")
            .field("chain_name", &self.chain_name)
            .field("distributed_db_version", &self.distributed_db_version)
            .field("p2p_version", &self.p2p_version)
            .finish()
    }
}

impl Version {
    pub fn new(chain_name: String, distributed_db_version: u16, p2p_version: u16) -> Self {
        Version { chain_name, distributed_db_version, p2p_version, body: Default::default() }
    }

    /// Returns true if version is compatibile.
    ///
    /// The version is compatible in case the `chain_name` and `distributed_db_version` version are the same.
    /// p2p_version is ignored.
    pub fn supports(&self, other: &Version) -> bool {
        self.chain_name == other.chain_name && self.distributed_db_version == other.distributed_db_version
    }
}

impl HasEncoding for Version {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("chain_name", Encoding::String),
            Field::new("distributed_db_version", Encoding::Uint16),
            Field::new("p2p_version", Encoding::Uint16)
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
        return self.chain_name == other.chain_name
            && self.distributed_db_version == other.distributed_db_version
            && self.p2p_version == other.p2p_version;
    }
}