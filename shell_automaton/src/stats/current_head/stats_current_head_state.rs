// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{collections::HashMap, net::SocketAddr};

use crypto::hash::BlockHash;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct CurrentHeadStats {
    pub pending_messages: HashMap<SocketAddr, PendingMessage>,
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum PendingMessage {
    CurrentHead {
        block_hash: BlockHash,
    },
    OperationsForBlocks {
        block_hash: BlockHash,
        validation_pass: i8,
    },
}
