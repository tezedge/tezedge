// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::fmt;

use getset::Getters;
use serde::{Deserialize, Serialize};
use tezos_encoding::{binary_reader::{BinaryReaderError, BinaryReaderErrorKind}, encoding::{Encoding, Field}, has_encoding_test};

use crate::non_cached_data;

use super::limits::CHAIN_NAME_MAX_LENGTH;

/// Holds informations about chain compatibility, features compatibility...
#[derive(Serialize, Deserialize, Getters, Clone, tezos_encoding::HasEncoding, tezos_encoding::NomReader)]
pub struct NetworkVersion {
    #[get = "pub"]
    #[encoding(BoundedString("CHAIN_NAME_MAX_LENGTH"))]
    chain_name: String,
    #[get = "pub"]
    distributed_db_version: u16,
    #[get = "pub"]
    p2p_version: u16,
}

impl tezos_encoding::raw::RawReader for NetworkVersion {
    fn from_bytes(bytes: &[u8]) -> Result<(&[u8], Self), BinaryReaderError> {
        if bytes.len() < 4 {
            return Err(BinaryReaderErrorKind::Underflow { bytes: 4 - bytes.len() }.into());
        }
        let chain_name_len: u32 = ((bytes[0] as u32) << 24) + ((bytes[1] as u32) << 16) + ((bytes[2] as u32) << 8) + bytes[3] as u32;
        let chain_name_len = chain_name_len as usize;
        let off = 4usize;
        if bytes.len() - off < chain_name_len {
            return Err(BinaryReaderErrorKind::Underflow { bytes: chain_name_len - bytes.len() - off }.into());
        }
        let chain_name = std::str::from_utf8(&bytes[off..off + chain_name_len])?.to_string();
        let off = off + chain_name_len;
        if bytes.len() - off < 2 {
            return Err(BinaryReaderErrorKind::Underflow { bytes: 2 - bytes.len() - off }.into());
        }
        let distributed_db_version = (bytes[off] as u16) << 8 + bytes[off+1] as u16;
        let off = off + 2;
        if bytes.len() - off < 2 {
            return Err(BinaryReaderErrorKind::Underflow { bytes: 2 - bytes.len() - off }.into());
        }
        let p2p_version = (bytes[off] as u16) << 8 + bytes[off+1] as u16;
        let off = off + 2;
        Ok((&bytes[off..], NetworkVersion { chain_name, distributed_db_version, p2p_version}))
    }
}

/*
impl tezos_encoding::nom::NomReader for NetworkVersion {
    fn from_bytes_nom(bytes: &[u8]) -> nom::IResult<&[u8], Self> {
        let (bytes, chain_name) = nom::combinator::map_res(
            nom::combinator::flat_map(
                nom::combinator::verify(
                    nom::number::complete::u32(nom::number::Endianness::Big),
                    |v| *v <= CHAIN_NAME_MAX_LENGTH as u32,
                ),
                nom::bytes::complete::take,
            ),
            |bytes| std::str::from_utf8(bytes).map(str::to_string),
        )(bytes)?;
        let (bytes, distributed_db_version) =
            nom::number::complete::u16(nom::number::Endianness::Big)(bytes)?;
        let (bytes, p2p_version) = nom::number::complete::u16(nom::number::Endianness::Big)(bytes)?;
        Ok((
            bytes,
            NetworkVersion {
                chain_name,
                distributed_db_version,
                p2p_version,
            },
        ))
    }
}
*/

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
has_encoding_test!(NetworkVersion, NETWORK_VERSION_ENCODING, {
    Encoding::Obj(
        "NetworkVersion",
        vec![
            Field::new("chain_name", Encoding::BoundedString(CHAIN_NAME_MAX_LENGTH)),
            Field::new("distributed_db_version", Encoding::Uint16),
            Field::new("p2p_version", Encoding::Uint16),
        ],
    )
});

impl Eq for NetworkVersion {}

impl PartialEq for NetworkVersion {
    fn eq(&self, other: &Self) -> bool {
        self.chain_name == other.chain_name
            && self.distributed_db_version == other.distributed_db_version
            && self.p2p_version == other.p2p_version
    }
}
