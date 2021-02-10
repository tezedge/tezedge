// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use getset::Getters;
use serde::{Deserialize, Serialize};

use crypto::hash::{ChainId, HashType};
use tezos_encoding::encoding::{Encoding, Field, HasEncoding};
use tezos_encoding::has_encoding;

use crate::cached_data;
use crate::p2p::binary_message::cache::BinaryDataCache;

use super::block_header::BlockHeader;
use super::limits::BLOCK_HEADER_MAX_SIZE;
use super::mempool::Mempool;

#[derive(Serialize, Deserialize, Debug, Getters, Clone)]
pub struct CurrentHeadMessage {
    #[get = "pub"]
    chain_id: ChainId,
    #[get = "pub"]
    current_block_header: BlockHeader,
    #[get = "pub"]
    current_mempool: Mempool,
    #[serde(skip_serializing)]
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
has_encoding!(CurrentHeadMessage, CURRENT_HEAD_MESSAGE_ENCODING, {
    Encoding::Obj(vec![
        Field::new("chain_id", Encoding::Hash(HashType::ChainId)),
        Field::new(
            "current_block_header",
            Encoding::bounded_dynamic(BLOCK_HEADER_MAX_SIZE, BlockHeader::encoding().clone()),
        ),
        Field::new("current_mempool", Mempool::encoding().clone()),
    ])
});

// -----------------------------------------------------------------------------------------------
#[derive(Serialize, Deserialize, Debug, Getters, Clone)]
pub struct GetCurrentHeadMessage {
    #[get = "pub"]
    chain_id: ChainId,

    #[serde(skip_serializing)]
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
has_encoding!(GetCurrentHeadMessage, GET_CURRENT_HEAD_MESSAGE_ENCODING, {
    Encoding::Obj(vec![Field::new(
        "chain_id",
        Encoding::Hash(HashType::ChainId),
    )])
});
