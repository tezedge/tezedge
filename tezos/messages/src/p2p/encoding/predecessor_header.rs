// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use getset::Getters;
use serde::{Deserialize, Serialize};

use crypto::hash::BlockHash;
use tezos_encoding::enc::BinWriter;
use tezos_encoding::encoding::HasEncoding;
use tezos_encoding::nom::NomReader;

use super::block_header::BlockHeader;

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(
    Clone, Serialize, Deserialize, Eq, PartialEq, Debug, Getters, HasEncoding, NomReader, BinWriter,
)]
pub struct GetPredecessorHeaderMessage {
    block_hash: BlockHash,
    offset: i32,
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(
    Clone, Serialize, Deserialize, Eq, PartialEq, Debug, Getters, HasEncoding, NomReader, BinWriter,
)]
pub struct PredecessorHeaderMessage {
    block_hash: BlockHash,
    offset: i32,
    header: BlockHeader,
}
