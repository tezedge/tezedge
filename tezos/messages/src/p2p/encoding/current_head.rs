// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use getset::Getters;
use serde::{Deserialize, Serialize};

use crypto::hash::ChainId;
use tezos_encoding::enc::BinWriter;
use tezos_encoding::encoding::HasEncoding;
use tezos_encoding::nom::NomReader;

use super::block_header::BlockHeader;
use super::limits::BLOCK_HEADER_MAX_SIZE;
use super::mempool::Mempool;

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(
    Serialize, Deserialize, Debug, Eq, PartialEq, Getters, Clone, HasEncoding, NomReader, BinWriter,
)]
pub struct CurrentHeadMessage {
    #[get = "pub"]
    chain_id: ChainId,
    #[get = "pub"]
    #[encoding(dynamic = "BLOCK_HEADER_MAX_SIZE")]
    current_block_header: BlockHeader,
    #[get = "pub"]
    current_mempool: Mempool,
}

impl CurrentHeadMessage {
    pub fn new(
        chain_id: ChainId,
        current_block_header: BlockHeader,
        current_mempool: Mempool,
    ) -> Self {
        CurrentHeadMessage {
            chain_id,
            current_block_header,
            current_mempool,
        }
    }
}

// -----------------------------------------------------------------------------------------------
#[derive(
    Serialize, Deserialize, Debug, Eq, PartialEq, Getters, Clone, HasEncoding, NomReader, BinWriter,
)]
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
pub struct GetCurrentHeadMessage {
    #[get = "pub"]
    chain_id: ChainId,
}

impl GetCurrentHeadMessage {
    pub fn new(chain_id: ChainId) -> Self {
        GetCurrentHeadMessage { chain_id }
    }
}

#[cfg(test)]
mod test {
    use std::{fs::File, io::Read, path::PathBuf};

    use crate::p2p::binary_message::BinaryRead;

    #[test]
    fn test_decode_current_head_33k() {
        let dir = std::env::var("CARGO_MANIFEST_DIR").expect("`CARGO_MANIFEST_DIR` is not set");
        let path = PathBuf::from(dir)
            .join("resources")
            .join("current-head.big.msg");
        let data = File::open(path)
            .and_then(|mut file| {
                let mut data = Vec::new();
                file.read_to_end(&mut data)?;
                Ok(data)
            })
            .unwrap();
        let _nom = super::CurrentHeadMessage::from_bytes(&data)
            .expect("Binary message is unreadable by nom");
    }
}
