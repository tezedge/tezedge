// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use getset::Getters;
use serde::{Deserialize, Serialize};

use crypto::hash::ChainId;
use tezos_encoding::enc::BinWriter;
use tezos_encoding::encoding::HasEncoding;
use tezos_encoding::nom::NomReader;

use super::prelude::BlockLocator;

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(
    Clone, Serialize, Deserialize, Eq, PartialEq, Debug, Getters, HasEncoding, NomReader, BinWriter,
)]
pub struct GetProtocolBranchMessage {
    chain_id: ChainId,
    proto_level: u8,
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(
    Clone, Serialize, Deserialize, Eq, PartialEq, Debug, Getters, HasEncoding, NomReader, BinWriter,
)]
pub struct ProtocolBranchMessage {
    chain_id: ChainId,
    proto_level: u8,
    locator: BlockLocator,
}
