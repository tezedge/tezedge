// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::fmt;

use serde::{Deserialize, Serialize};

use tezos_encoding::encoding::{Encoding, Field, HasEncoding};
use tezos_encoding::has_encoding;

use crate::cached_data;
use crate::p2p::binary_message::cache::BinaryDataCache;

#[derive(Serialize, Deserialize, Clone)]
pub struct NetworkVersion {
    chain_name: String,
    distributed_db_version: u16,
    p2p_version: u16,
    #[serde(skip_serializing)]
    body: BinaryDataCache,
}

impl fmt::Debug for NetworkVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Version")
            .field("chain_name", &self.chain_name)
            .field("distributed_db_version", &self.distributed_db_version)
            .field("p2p_version", &self.p2p_version)
            .finish()
    }
}

impl NetworkVersion {
    pub fn new(chain_name: String, distributed_db_version: u16, p2p_version: u16) -> Self {
        NetworkVersion {
            chain_name,
            distributed_db_version,
            p2p_version,
            body: Default::default(),
        }
    }

    /// Returns true if version is compatibile.
    ///
    /// The version is compatible in case the `chain_name` and `distributed_db_version` version are the same.
    /// p2p_version is ignored.
    pub fn supports(&self, other: &NetworkVersion) -> bool {
        self.chain_name == other.chain_name
            && self.distributed_db_version == other.distributed_db_version
    }
}

cached_data!(NetworkVersion, body);
has_encoding!(NetworkVersion, NETWORK_VERSION_ENCODING, {
    Encoding::Obj(vec![
        Field::new("chain_name", Encoding::String),
        Field::new("distributed_db_version", Encoding::Uint16),
        Field::new("p2p_version", Encoding::Uint16),
    ])
});

impl Eq for NetworkVersion {}

impl PartialEq for NetworkVersion {
    fn eq(&self, other: &Self) -> bool {
        self.chain_name == other.chain_name
            && self.distributed_db_version == other.distributed_db_version
            && self.p2p_version == other.p2p_version
    }
}
