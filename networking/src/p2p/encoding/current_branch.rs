// Copyright (c) SimpleStaking and Tezos-RS Contributors
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use tezos_encoding::encoding::{Encoding, Field, HasEncoding, SchemaType};
use tezos_encoding::hash::{BlockHash, ChainId, HashEncoding, HashType};

use crate::p2p::encoding::block_header::BlockHeader;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct CurrentBranchMessage {
    pub chain_id: ChainId,
    pub current_branch: CurrentBranch,
}

impl HasEncoding for CurrentBranchMessage {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("chain_id", Encoding::Hash(HashEncoding::new(HashType::ChainId))),
            Field::new("current_branch", CurrentBranch::encoding())
        ])
    }
}

// -----------------------------------------------------------------------------------------------
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct CurrentBranch {
    pub current_head: BlockHeader,
    pub history: Vec<BlockHash>,
}

impl HasEncoding for CurrentBranch {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("current_head", Encoding::dynamic(BlockHeader::encoding())),
            Field::new("history", Encoding::Split(Arc::new(|schema_type|
                match schema_type {
                    SchemaType::Json => Encoding::Unit, // TODO: decode as list of hashes when history is needed
                    SchemaType::Binary => Encoding::list(Encoding::Hash(HashEncoding::new(HashType::BlockHash)))
                }
            )))
        ])
    }
}

// -----------------------------------------------------------------------------------------------
#[derive(Serialize, Deserialize, Debug)]
pub struct GetCurrentBranchMessage {
    pub chain_id: ChainId,
}

impl GetCurrentBranchMessage {
    pub fn new(chain_id: ChainId) -> Self {
        GetCurrentBranchMessage { chain_id }
    }
}

impl HasEncoding for GetCurrentBranchMessage {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("chain_id", Encoding::Hash(HashEncoding::new(HashType::ChainId)))
        ])
    }
}