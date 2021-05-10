// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::fmt;
use std::mem::size_of;

use getset::Getters;
use serde::{Deserialize, Serialize};

use tezos_encoding::{binary_reader::{ActualSize, BinaryReaderErrorKind}, encoding::{Encoding, Field, HasEncoding, Tag, TagMap}, has_encoding_test, nom::NomReader, raw::RawReader};

use crate::non_cached_data;

use super::limits::{NACK_PEERS_MAX_LENGTH, P2P_POINT_MAX_SIZE};

#[derive(Serialize, Deserialize, PartialEq, Debug, HasEncoding, NomReader)]
pub enum AckMessage {
    #[encoding(tag = 0x00)]
    Ack,
    #[encoding(tag = 0xff)]
    NackV0,
    #[encoding(tag = 0x01)]
    Nack(NackInfo),
}

#[derive(Serialize, Deserialize, Getters, PartialEq, HasEncoding, NomReader)]
pub struct NackInfo {
    #[get = "pub"]
    motive: NackMotive,
    #[get = "pub"]
    #[encoding(dynamic, list = "NACK_PEERS_MAX_LENGTH", bounded = "P2P_POINT_MAX_SIZE")]
    potential_peers_to_connect: Vec<String>,
}

#[derive(Serialize, Deserialize, PartialEq, HasEncoding, NomReader)]
#[encoding(tags="u16")]
pub enum NackMotive {
    NoMotive,
    TooManyConnections,
    UnknownChainName,
    DeprecatedP2pVersion,
    DeprecatedDistributedDbVersion,
    AlreadyConnected,
}

impl RawReader for AckMessage {
    fn from_bytes(bytes: &[u8]) -> Result<(&[u8], Self), tezos_encoding::binary_reader::BinaryReaderError> {
        if bytes.len() < 1 {
            return Err(BinaryReaderErrorKind::Underflow { bytes: 1 }.into());
        }
        match bytes[0] {
            0x00 => Ok((&bytes[1..], AckMessage::Ack)),
            0xff => Ok((&bytes[1..], AckMessage::NackV0)),
            0x01 => {
                let (bytes, info) = <NackInfo as RawReader>::from_bytes(&bytes[1..])?;
                Ok((bytes, AckMessage::Nack(info)))
            }
            tag => Err(BinaryReaderErrorKind::UnsupportedTag { tag: tag as u16 }.into()),
        }
    }
}

impl NackInfo {
    pub fn new(motive: NackMotive, potential_peers_to_connect: &[String]) -> Self {
        Self {
            motive,
            potential_peers_to_connect: potential_peers_to_connect.to_vec(),
        }
    }
}

impl RawReader for NackInfo {
    fn from_bytes(bytes: &[u8]) -> Result<(&[u8], Self), tezos_encoding::binary_reader::BinaryReaderError> {
        let (bytes, motive) = <NackMotive as RawReader>::from_bytes(bytes)?;
        if bytes.len() < 4 {
            return Err(BinaryReaderErrorKind::Underflow { bytes: 4 - bytes.len() }.into());
        }
        let dynamic_len: u32 = ((bytes[0] as u32) << 24) + ((bytes[1] as u32) << 16) + ((bytes[2] as u32) << 8) + bytes[3] as u32;
        let dynamic_len = dynamic_len as usize;
        let dynamic = &bytes[4..4+dynamic_len];
        let mut off = 0;
        let mut potential_peers_to_connect = Vec::new();
        while off != dynamic.len() {
            if potential_peers_to_connect.len() >= NACK_PEERS_MAX_LENGTH {
                return Err(BinaryReaderErrorKind::EncodingBoundaryExceeded { name: "BoundedString".to_string(), boundary: NACK_PEERS_MAX_LENGTH, actual: ActualSize::GreaterThan(NACK_PEERS_MAX_LENGTH) }.into());
            }
            let max = std::cmp::min(P2P_POINT_MAX_SIZE, dynamic.len() - off);
            if max < 4 {
                return Err(BinaryReaderErrorKind::Underflow { bytes: 4 - max }.into());
            }
            let peer_len: u32 = ((dynamic[off] as u32) << 24) + ((dynamic[off+1] as u32) << 16) + ((dynamic[off+2] as u32) << 8) + dynamic[off+3] as u32;
            let peer_len = peer_len as usize;
            if peer_len >= P2P_POINT_MAX_SIZE {
                return Err(BinaryReaderErrorKind::EncodingBoundaryExceeded { name: "BoundedString".to_string(), boundary: P2P_POINT_MAX_SIZE, actual: ActualSize::GreaterThan(NACK_PEERS_MAX_LENGTH) }.into());
            }
            if max - 4 < peer_len {
                return Err(BinaryReaderErrorKind::Underflow { bytes: peer_len - max + 4 }.into());
            }
            let peer = std::str::from_utf8(&dynamic[off+4..off+4+peer_len])?.to_string();
            potential_peers_to_connect.push(peer);
            off += 4 + peer_len;
        }
        Ok((&bytes[4+dynamic_len..], NackInfo { motive, potential_peers_to_connect }))
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

impl RawReader for NackMotive {
    fn from_bytes(bytes: &[u8]) -> Result<(&[u8], Self), tezos_encoding::binary_reader::BinaryReaderError> {
        if bytes.len() < 2 {
            return Err(BinaryReaderErrorKind::Underflow { bytes: 2 - bytes.len() }.into());
        }
        let tag = ((bytes[0] as u16) << 8) + (bytes[1] as u16);
        match tag {
            0x0000 => Ok((&bytes[2..], NackMotive::NoMotive)),
            0x0001 => Ok((&bytes[2..], NackMotive::TooManyConnections)),
            0x0002 => Ok((&bytes[2..], NackMotive::UnknownChainName)),
            0x0003 => Ok((&bytes[2..], NackMotive::DeprecatedP2pVersion)),
            0x0004 => Ok((&bytes[2..], NackMotive::DeprecatedDistributedDbVersion)),
            0x0005 => Ok((&bytes[2..], NackMotive::AlreadyConnected)),
            tag => Err(BinaryReaderErrorKind::UnsupportedTag { tag }.into()),
        }
    }
}

has_encoding_test!(NackInfo, NACK_INFO_ENCODING, {
    Encoding::Obj(
        "NackInfo",
        vec![
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
                Encoding::dynamic(Encoding::bounded_list(
                    NACK_PEERS_MAX_LENGTH,
                    Encoding::bounded(P2P_POINT_MAX_SIZE, Encoding::String),
                )),
            ),
        ],
    )
});

non_cached_data!(AckMessage);
has_encoding_test!(AckMessage, ACK_MESSAGE_ENCODING, {
    Encoding::Tags(
        size_of::<u8>(),
        TagMap::new(vec![
            Tag::new(0x00, "Ack", Encoding::Unit),
            Tag::new(0x01, "Nack", NackInfo::encoding().clone()),
            Tag::new(0xFF, "NackV0", Encoding::Unit),
        ]),
    )
});
