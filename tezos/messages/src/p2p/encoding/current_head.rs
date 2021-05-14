// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use getset::Getters;
use serde::{Deserialize, Serialize};

use crypto::hash::{ChainId, HashType};
use tezos_encoding::encoding::{Encoding, Field, HasEncoding, HasEncodingTest};
use tezos_encoding::has_encoding_test;
use tezos_encoding::nom::NomReader;

use crate::cached_data;
use crate::p2p::binary_message::cache::BinaryDataCache;

use super::block_header::BlockHeader;
use super::limits::BLOCK_HEADER_MAX_SIZE;
use super::mempool::Mempool;

#[derive(Serialize, Deserialize, Debug, Getters, Clone, HasEncoding, NomReader, PartialEq)]
pub struct CurrentHeadMessage {
    #[get = "pub"]
    chain_id: ChainId,
    #[get = "pub"]
    #[encoding(dynamic = "BLOCK_HEADER_MAX_SIZE")]
    current_block_header: BlockHeader,
    #[get = "pub"]
    current_mempool: Mempool,
    #[serde(skip_serializing)]
    #[encoding(skip)]
    body: BinaryDataCache,
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
            body: Default::default(),
        }
    }
}

cached_data!(CurrentHeadMessage, body);
has_encoding_test!(CurrentHeadMessage, CURRENT_HEAD_MESSAGE_ENCODING, {
    Encoding::Obj(
        "CurrentHeadMessage",
        vec![
            Field::new("chain_id", Encoding::Hash(HashType::ChainId)),
            Field::new(
                "current_block_header",
                Encoding::bounded_dynamic(
                    BLOCK_HEADER_MAX_SIZE,
                    BlockHeader::encoding_test().clone(),
                ),
            ),
            Field::new("current_mempool", Mempool::encoding_test().clone()),
        ],
    )
});

// -----------------------------------------------------------------------------------------------
#[derive(Serialize, Deserialize, Debug, Getters, Clone, HasEncoding, NomReader)]
pub struct GetCurrentHeadMessage {
    #[get = "pub"]
    chain_id: ChainId,

    #[serde(skip_serializing)]
    #[encoding(skip)]
    body: BinaryDataCache,
}

impl GetCurrentHeadMessage {
    pub fn new(chain_id: ChainId) -> Self {
        GetCurrentHeadMessage {
            chain_id,
            body: Default::default(),
        }
    }
}

cached_data!(GetCurrentHeadMessage, body);
has_encoding_test!(GetCurrentHeadMessage, GET_CURRENT_HEAD_MESSAGE_ENCODING, {
    Encoding::Obj(
        "GetCurrentHeadMessage",
        vec![Field::new("chain_id", Encoding::Hash(HashType::ChainId))],
    )
});

#[cfg(test)]
mod test {
    use std::{fs::File, io::Read, path::PathBuf};

    use tezos_encoding::assert_encodings_match;

    use crate::p2p::binary_message::{BinaryMessageNom, BinaryMessageSerde};

    #[test]
    fn test_current_head_encoding_schema() {
        assert_encodings_match!(super::CurrentHeadMessage);
    }

    #[test]
    fn test_get_current_head_encoding_schema() {
        assert_encodings_match!(super::GetCurrentHeadMessage);
    }

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
        let serde = <super::CurrentHeadMessage as BinaryMessageSerde>::from_bytes(&data)
            .expect("Binary message is unreadable by serde");
        let nom = <super::CurrentHeadMessage as BinaryMessageNom>::from_bytes(&data)
            .expect("Binary message is unreadable by nom");
        assert_eq!(serde, nom);
    }
}
