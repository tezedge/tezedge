// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use getset::Getters;
use serde::{Deserialize, Serialize};

use crypto::hash::OperationHash;
use tezos_encoding::enc::BinWriter;
use tezos_encoding::encoding::HasEncoding;
use tezos_encoding::nom::NomReader;

use super::limits::{MEMPOOL_MAX_OPERATIONS, MEMPOOL_MAX_SIZE};

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(
    Clone,
    Serialize,
    Deserialize,
    Debug,
    Eq,
    PartialEq,
    Default,
    Getters,
    HasEncoding,
    NomReader,
    BinWriter,
    tezos_encoding::generator::Generated,
)]
#[encoding(bounded = "MEMPOOL_MAX_SIZE")]
pub struct Mempool {
    #[get = "pub"]
    #[encoding(dynamic, list = "MEMPOOL_MAX_OPERATIONS")]
    pub known_valid: Vec<OperationHash>,
    #[get = "pub"]
    #[encoding(dynamic, dynamic, list = "MEMPOOL_MAX_OPERATIONS")]
    pub pending: Vec<OperationHash>,
}

impl Mempool {
    pub fn new(known_valid: Vec<OperationHash>, pending: Vec<OperationHash>) -> Self {
        Mempool {
            known_valid,
            pending,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.known_valid.is_empty() && self.pending.is_empty()
    }
}
