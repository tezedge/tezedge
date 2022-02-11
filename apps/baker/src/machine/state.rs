// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{collections::BTreeMap, time::Duration};

use chrono::{DateTime, Utc};

use crypto::hash::{BlockHash, BlockPayloadHash, ChainId, ContractTz1Hash};
use tezos_encoding::enc::BinWriter;
use tezos_messages::protocol::proto_012::operation::{InlinedEndorsementMempoolContents, InlinedPreendorsementContents};

#[derive(Debug)]
pub enum State {
    Initial,
    RpcError(String),
    ContextConstantsParseError,
    GotChainId(ChainId),
    Ready {
        config: Config,
        // TODO: rename
        predecessor_head_data: Option<BlockData>,
        current_head_data: Option<BlockData>,
    },
}

#[derive(BinWriter, Debug)]
pub struct PreendorsementUnsignedOperation {
    pub branch: BlockHash,
    pub content: InlinedPreendorsementContents,
}

#[derive(BinWriter, Debug)]
pub struct EndorsementUnsignedOperation {
    pub branch: BlockHash,
    pub content: InlinedEndorsementMempoolContents,
}

// TODO: rename
#[derive(Debug)]
pub struct BlockData {
    pub predecessor: BlockHash,
    pub block_hash: BlockHash,

    pub slot: Option<u16>,
    pub validators: BTreeMap<ContractTz1Hash, Vec<u16>>,
    pub level: i32,
    pub round: i32,
    pub timestamp: DateTime<Utc>,
    pub payload_hash: BlockPayloadHash,

    pub seen_preendorsement: usize,
    pub preendorsement: Option<PreendorsementUnsignedOperation>,
    pub endorsement: Option<EndorsementUnsignedOperation>,
}

#[derive(Debug)]
pub struct Config {
    pub chain_id: ChainId,
    pub quorum_size: usize,
    pub minimal_block_delay: Duration,
    pub delay_increment_per_round: Duration,
}
