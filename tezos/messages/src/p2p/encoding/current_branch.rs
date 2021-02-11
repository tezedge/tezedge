// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use getset::Getters;
use serde::{Deserialize, Serialize};

use crypto::hash::{BlockHash, ChainId, HashType};
use tezos_encoding::encoding::{Encoding, Field, HasEncoding, SchemaType};
use tezos_encoding::has_encoding;

use crate::cached_data;
use crate::p2p::binary_message::cache::BinaryDataCache;
use crate::p2p::encoding::block_header::BlockHeader;

pub const HISTORY_MAX_SIZE: u8 = 200;

#[derive(Clone, Serialize, Deserialize, Debug, Getters)]
pub struct CurrentBranchMessage {
    #[get = "pub"]
    chain_id: ChainId,
    #[get = "pub"]
    current_branch: CurrentBranch,
    #[serde(skip_serializing)]
    body: BinaryDataCache,
}

impl CurrentBranchMessage {
    pub fn new(chain_id: ChainId, current_branch: CurrentBranch) -> Self {
        CurrentBranchMessage {
            chain_id,
            current_branch,
            body: Default::default(),
        }
    }
}

cached_data!(CurrentBranchMessage, body);
has_encoding!(CurrentBranchMessage, CURRENT_BRANCH_MESSAGE_ENCODING, {
    Encoding::Obj(vec![
        Field::new("chain_id", Encoding::Hash(HashType::ChainId)),
        Field::new("current_branch", CurrentBranch::encoding().clone()),
    ])
});

// -----------------------------------------------------------------------------------------------
#[derive(Clone, Serialize, Deserialize, Debug, Getters)]
pub struct CurrentBranch {
    #[get = "pub"]
    current_head: BlockHeader,
    /// These hashes go from the top of the chain to the bottom (to genesis)
    #[get = "pub"]
    history: Vec<BlockHash>,
    #[serde(skip_serializing)]
    body: BinaryDataCache,
}

impl CurrentBranch {
    pub fn new(current_head: BlockHeader, history: Vec<BlockHash>) -> Self {
        CurrentBranch {
            current_head,
            history,
            body: Default::default(),
        }
    }
}

cached_data!(CurrentBranch, body);
has_encoding!(CurrentBranch, CURRENT_BRANCH_ENCODING, {
    Encoding::Obj(vec![
        Field::new(
            "current_head",
            Encoding::dynamic(BlockHeader::encoding().clone()),
        ),
        Field::new(
            "history",
            Encoding::Split(Arc::new(|schema_type| match schema_type {
                SchemaType::Json => Encoding::Unit, // TODO: decode as list of hashes when history is needed
                SchemaType::Binary => Encoding::list(Encoding::Hash(HashType::BlockHash)),
            })),
        ),
    ])
});

// -----------------------------------------------------------------------------------------------
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GetCurrentBranchMessage {
    pub chain_id: ChainId,

    #[serde(skip_serializing)]
    body: BinaryDataCache,
}

impl GetCurrentBranchMessage {
    pub fn new(chain_id: ChainId) -> Self {
        GetCurrentBranchMessage {
            chain_id,
            body: Default::default(),
        }
    }
}

cached_data!(GetCurrentBranchMessage, body);
has_encoding!(
    GetCurrentBranchMessage,
    GET_CURRENT_BRANCH_MESSAGE_ENCODING,
    {
        Encoding::Obj(vec![Field::new(
            "chain_id",
            Encoding::Hash(HashType::ChainId),
        )])
    }
);
