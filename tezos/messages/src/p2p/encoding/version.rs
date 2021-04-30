// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::fmt;

use getset::Getters;
use serde::{Deserialize, Serialize};

use crate::non_cached_data;

use super::limits::CHAIN_NAME_MAX_LENGTH;

/// Holds informations about chain compatibility, features compatibility...
#[derive(Serialize, Deserialize, Getters, Clone, tezos_encoding_derive::HasEncoding)]
pub struct NetworkVersion {
    #[get = "pub"]
    #[encoding(BoundedString("CHAIN_NAME_MAX_LENGTH"))]
    chain_name: String,
    #[get = "pub"]
    distributed_db_version: u16,
    #[get = "pub"]
    p2p_version: u16,
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
        }
    }

    pub fn supports_nack_with_list_and_motive(&self) -> bool {
        self.p2p_version > 0
    }
}

non_cached_data!(NetworkVersion);
/*
has_encoding!(NetworkVersion, NETWORK_VERSION_ENCODING, {
    Encoding::Obj(
        "NetworkVersion",
        vec![
            Field::new("chain_name", Encoding::BoundedString(CHAIN_NAME_MAX_LENGTH)),
            Field::new("distributed_db_version", Encoding::Uint16),
            Field::new("p2p_version", Encoding::Uint16),
        ],
    )
});
*/

impl Eq for NetworkVersion {}

impl PartialEq for NetworkVersion {
    fn eq(&self, other: &Self) -> bool {
        self.chain_name == other.chain_name
            && self.distributed_db_version == other.distributed_db_version
            && self.p2p_version == other.p2p_version
    }
}
