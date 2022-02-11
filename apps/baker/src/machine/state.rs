// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{time::Duration, collections::BTreeMap};

use chrono::{DateTime, Utc};

use crypto::hash::{ChainId, BlockHash, ContractTz1Hash};
use tezos_encoding::enc::BinWriter;
use tezos_messages::protocol::proto_012::operation::{InlinedEndorsementMempoolContents, InlinedPreendorsementContents};

use crate::types::ProtocolBlockHeader;

#[derive(Debug)]
pub enum State {
    Initial,
    RpcError(String),
    ContextConstantsParseError,
    GotChainId(ChainId),
    Ready {
        config: Config,
        // TODO: rename
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
    pub slot: Option<u16>,
    pub validators: BTreeMap<ContractTz1Hash, Vec<u16>>,
    pub level: i32,
    pub predecessor: BlockHash,

    pub block_hash: BlockHash,
    pub timestamp: DateTime<Utc>,
    pub protocol_data: ProtocolBlockHeader,

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
