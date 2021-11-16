// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::fmt;

use getset::Getters;
use serde::{Deserialize, Serialize};
use tezos_encoding::{enc::BinWriter, encoding::HasEncoding, generator::Generated, nom::NomReader};

use super::limits::CHAIN_NAME_MAX_LENGTH;
use std::hash::{Hash, Hasher};

/// Holds informations about chain compatibility, features compatibility...
#[derive(Serialize, Deserialize, Getters, Clone, HasEncoding, NomReader, BinWriter, Generated)]
pub struct NetworkVersion {
    #[get = "pub"]
    #[encoding(string = "CHAIN_NAME_MAX_LENGTH")]
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

impl Eq for NetworkVersion {}

impl PartialEq for NetworkVersion {
    fn eq(&self, other: &Self) -> bool {
        self.chain_name == other.chain_name
            && self.distributed_db_version == other.distributed_db_version
            && self.p2p_version == other.p2p_version
    }
}

impl Hash for NetworkVersion {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(self.chain_name.as_bytes());
        state.write_u16(self.distributed_db_version);
        state.write_u16(self.p2p_version);
    }
}
