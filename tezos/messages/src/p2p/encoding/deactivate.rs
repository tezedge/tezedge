// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use getset::Getters;
use serde::{Deserialize, Serialize};

use crypto::hash::ChainId;
use tezos_encoding::encoding::HasEncoding;
use tezos_encoding::nom::NomReader;

#[derive(Serialize, Deserialize, Debug, Getters, Clone, HasEncoding, NomReader)]
pub struct DeactivateMessage {
    #[get = "pub"]
    deactivate: ChainId,
}

impl DeactivateMessage {
    pub fn new(deactivate: ChainId) -> Self {
        Self { deactivate }
    }
}
