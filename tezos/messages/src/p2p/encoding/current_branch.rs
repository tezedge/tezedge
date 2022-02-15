// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use getset::Getters;
use serde::{Deserialize, Serialize};

use crypto::hash::{BlockHash, ChainId};
use tezos_encoding::enc::BinWriter;
use tezos_encoding::encoding::HasEncoding;
use tezos_encoding::nom::NomReader;

use crate::p2p::encoding::block_header::BlockHeader;

use super::limits::CURRENT_BRANCH_HISTORY_MAX_LENGTH;

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(
    Clone,
    Serialize,
    Deserialize,
    Debug,
    Getters,
    HasEncoding,
    NomReader,
    BinWriter,
    PartialEq,
    tezos_encoding::generator::Generated,
)]
pub struct CurrentBranchMessage {
    #[get = "pub"]
    chain_id: ChainId,
    #[get = "pub"]
    current_branch: CurrentBranch,
}

impl CurrentBranchMessage {
    pub fn new(chain_id: ChainId, current_branch: CurrentBranch) -> Self {
        CurrentBranchMessage {
            chain_id,
            current_branch,
        }
    }
}

// -----------------------------------------------------------------------------------------------
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(
    Clone,
    Serialize,
    Deserialize,
    Debug,
    Getters,
    HasEncoding,
    NomReader,
    BinWriter,
    PartialEq,
    tezos_encoding::generator::Generated,
)]
pub struct CurrentBranch {
    #[get = "pub"]
    #[encoding(dynamic = "super::limits::BLOCK_HEADER_MAX_SIZE")]
    current_head: BlockHeader,
    /// These hashes go from the top of the chain to the bottom (to genesis)
    #[get = "pub"]
    #[encoding(list = "CURRENT_BRANCH_HISTORY_MAX_LENGTH")]
    history: Vec<BlockHash>,
}

impl CurrentBranch {
    pub fn new(current_head: BlockHeader, history: Vec<BlockHash>) -> Self {
        CurrentBranch {
            current_head,
            history,
        }
    }
}

// -----------------------------------------------------------------------------------------------
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(
    Serialize,
    Deserialize,
    Debug,
    Clone,
    HasEncoding,
    NomReader,
    BinWriter,
    tezos_encoding::generator::Generated,
)]
pub struct GetCurrentBranchMessage {
    pub chain_id: ChainId,
}

impl GetCurrentBranchMessage {
    pub fn new(chain_id: ChainId) -> Self {
        GetCurrentBranchMessage { chain_id }
    }
}

#[cfg(test)]
mod test {
    use std::{fs::File, io::Read, path::PathBuf};

    use crate::p2p::binary_message::BinaryRead;

    #[test]
    fn test_decode_current_branch_big() {
        let dir = std::env::var("CARGO_MANIFEST_DIR").expect("`CARGO_MANIFEST_DIR` is not set");
        let path = PathBuf::from(dir)
            .join("resources")
            .join("current-branch.big.msg");
        let data = File::open(path)
            .and_then(|mut file| {
                let mut data = Vec::new();
                file.read_to_end(&mut data)?;
                Ok(data)
            })
            .unwrap();
        let _nom =
            super::CurrentBranchMessage::from_bytes(&data).expect("Binary message is unreadable");
    }
}
