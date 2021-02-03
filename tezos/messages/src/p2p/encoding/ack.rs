// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::fmt;
use std::mem::size_of;

use getset::Getters;
use serde::{Deserialize, Serialize};

use tezos_encoding::encoding::{Encoding, Field, HasEncoding, Tag, TagMap};
use tezos_encoding::has_encoding;

use crate::non_cached_data;

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub enum AckMessage {
    Ack,
    NackV0,
    Nack(NackInfo),
}

#[derive(Serialize, Deserialize, PartialEq)]
pub enum NackMotive {
    NoMotive,
    TooManyConnections,
    UnknownChainName,
    DeprecatedP2pVersion,
    DeprecatedDistributedDbVersion,
    AlreadyConnected,
}

impl fmt::Display for NackMotive {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let motive = match &self {
            NackMotive::NoMotive => "No_motive",
            NackMotive::TooManyConnections => "Too_many_connections ",
            NackMotive::UnknownChainName => "Unknown_chain_name",
            NackMotive::DeprecatedP2pVersion => "Deprecated_p2p_version",
            NackMotive::DeprecatedDistributedDbVersion => "Deprecated_distributed_db_version",
            NackMotive::AlreadyConnected => "Already_connected",
        };
        write!(f, "{}", motive)
    }
}

impl fmt::Debug for NackMotive {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", &self)
    }
}

#[derive(Serialize, Deserialize, Getters, PartialEq)]
pub struct NackInfo {
    #[get = "pub"]
    motive: NackMotive,
    #[get = "pub"]
    potential_peers_to_connect: Vec<String>,
}

impl NackInfo {
    pub fn new(motive: NackMotive, potential_peers_to_connect: &[String]) -> Self {
        Self {
            motive,
            potential_peers_to_connect: potential_peers_to_connect.to_vec(),
        }
    }
}

impl fmt::Debug for NackInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let potential_peers_to_connect = self.potential_peers_to_connect.join(", ");
        write!(
            f,
            "motive: {}, potential_peers_to_connect: {:?}",
            &self.motive, potential_peers_to_connect
        )
    }
}

impl NackInfo {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new(
                "motive",
                Encoding::Tags(
                    size_of::<u16>(),
                    TagMap::new(vec![
                        Tag::new(0, "NoMotive", Encoding::Unit),
                        Tag::new(1, "TooManyConnections", Encoding::Unit),
                        Tag::new(2, "UnknownChainName", Encoding::Unit),
                        Tag::new(3, "DeprecatedP2pVersion", Encoding::Unit),
                        Tag::new(4, "DeprecatedDistributedDbVersion", Encoding::Unit),
                        Tag::new(5, "AlreadyConnected", Encoding::Unit),
                    ]),
                ),
            ),
            Field::new(
                "potential_peers_to_connect",
                Encoding::dynamic(Encoding::list(Encoding::String)),
            ),
        ])
    }
}

non_cached_data!(AckMessage);
has_encoding!(AckMessage, ACK_MESSAGE_ENCODING, {
    Encoding::Tags(
        size_of::<u8>(),
        TagMap::new(vec![
            Tag::new(0x00, "Ack", Encoding::Unit),
            Tag::new(0x01, "Nack", NackInfo::encoding()),
            Tag::new(0xFF, "NackV0", Encoding::Unit),
        ]),
    )
});
