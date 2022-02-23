// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    collections::BTreeMap,
    fmt,
    time::{Duration, SystemTime},
};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crypto::hash::{
    BlockHash, BlockPayloadHash, ContextHash, ContractTz1Hash, NonceHash,
    OperationListListHash, ProtocolHash, Signature,
};
use tezos_encoding::{enc::BinWriter, encoding::HasEncoding};
use tezos_encoding_derive::NomReader;
use tezos_messages::{
    p2p::binary_message::BinaryRead,
    protocol::proto_012::operation::{
        Contents, InlinedEndorsementMempoolContents, InlinedEndorsementMempoolContentsEndorsementVariant, Operation, InlinedPreendorsementContents,
        PreendorsementOperation,
    },
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Timestamp(pub u64);

impl Timestamp {
    pub fn duration_from_now(&self) -> Duration {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("the unix epoch has begun");

        // TODO: check overflow
        Duration::from_secs(self.0) - now
    }

    pub fn now() -> Self {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("the unix epoch has begun");
        Timestamp(now.as_secs())
    }
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Prequorum {
    pub level: i32,
    pub round: i32,
    pub payload_hash: BlockPayloadHash,
    pub firsts_slot: Vec<u16>,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct BlockPayload {
    pub votes_payload: Vec<Operation>,
    pub anonymous_payload: Vec<Operation>,
    pub managers_payload: Vec<Operation>,
}

impl fmt::Debug for BlockPayload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("BlockPayload")
            .field(&"content omitted")
            .finish()
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BlockInfo {
    pub hash: BlockHash,
    pub level: i32,
    pub timestamp: i64,
    pub payload_hash: BlockPayloadHash,
    pub payload_round: i32,
    pub round: i32,
    pub protocol: ProtocolHash,
    pub next_protocol: ProtocolHash,
    pub prequorum: Option<Prequorum>,
    pub quorum: Vec<InlinedEndorsementMempoolContentsEndorsementVariant>,
    pub payload: BlockPayload,
}

impl BlockInfo {
    fn extract_consensus_operations(
        ops: &[Operation],
    ) -> (Option<Prequorum>, Vec<InlinedEndorsementMempoolContentsEndorsementVariant>) {
        let mut prequorum = Prequorum {
            level: 0,
            round: 0,
            payload_hash: BlockPayloadHash(vec![0; 32]),
            firsts_slot: vec![],
        };
        let mut quorum = vec![];
        for content in ops.iter().map(|op| &op.contents).flatten() {
            match content {
                Contents::Preendorsement(v) => {
                    prequorum.level = v.level;
                    prequorum.round = v.round;
                    prequorum.payload_hash = v.block_payload_hash.clone();
                    prequorum.firsts_slot.push(v.slot);
                }
                Contents::Endorsement(op) => {
                    quorum.push(InlinedEndorsementMempoolContentsEndorsementVariant {
                        slot: op.slot,
                        level: op.level,
                        round: op.round,
                        block_payload_hash: op.block_payload_hash.clone(),
                    });
                }
                _ => (),
            }
        }
        let prequorum = if prequorum.firsts_slot.is_empty() {
            None
        } else {
            Some(prequorum)
        };
        (prequorum, quorum)
    }

    pub fn new_with_full_header(
        header: FullHeader,
        protocols: Protocols,
        operations: [Vec<Operation>; 4],
    ) -> Self {
        let Protocols {
            protocol,
            next_protocol,
        } = protocols;
        let [consensus_payload, votes_payload, anonymous_payload, managers_payload] = operations;
        let (prequorum, quorum) = Self::extract_consensus_operations(&consensus_payload);

        BlockInfo {
            hash: header.hash,
            level: header.level,
            timestamp: header
                .timestamp
                .parse::<DateTime<Utc>>()
                .unwrap()
                .timestamp(),
            payload_hash: header.payload_hash.unwrap_or(BlockPayloadHash(vec![0; 32])),
            payload_round: header.payload_round.unwrap_or(0),
            // TODO: is it correct?
            round: header.payload_round.unwrap_or(0),
            protocol,
            next_protocol,
            prequorum,
            quorum,
            payload: BlockPayload {
                votes_payload,
                anonymous_payload,
                managers_payload,
            },
        }
    }

    pub fn new(
        shell_header: ShellBlockHeader,
        protocols: Protocols,
        operations: [Vec<Operation>; 4],
    ) -> Self {
        let Protocols {
            protocol,
            next_protocol,
        } = protocols;
        let [consensus_payload, votes_payload, anonymous_payload, managers_payload] = operations;
        let (prequorum, quorum) = Self::extract_consensus_operations(&consensus_payload);

        let protocol_data_bytes = hex::decode(&shell_header.protocol_data).unwrap();
        let protocol_data = ProtocolBlockHeader::from_bytes(protocol_data_bytes).unwrap();

        BlockInfo {
            hash: shell_header.hash,
            level: shell_header.level,
            timestamp: shell_header
                .timestamp
                .parse::<DateTime<Utc>>()
                .unwrap()
                .timestamp(),
            payload_hash: protocol_data.payload_hash,
            payload_round: protocol_data.payload_round,
            // TODO: is it correct?
            round: protocol_data.payload_round,
            protocol,
            next_protocol,
            prequorum,
            quorum,
            payload: BlockPayload {
                votes_payload,
                anonymous_payload,
                managers_payload,
            },
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Proposal {
    pub block: BlockInfo,
    pub predecessor: BlockInfo,
}

#[derive(Debug)]
pub struct LockedRound {
    pub round: i32,
    pub payload_hash: BlockPayloadHash,
}

#[derive(Debug)]
pub struct EndorsablePayload {
    pub proposal: Proposal,
    pub prequorum: Prequorum,
}

#[derive(Debug)]
pub struct ElectedBlock {
    pub proposal: Proposal,
    pub quorum: Vec<InlinedEndorsementMempoolContentsEndorsementVariant>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DelegateSlots {
    pub slot: Option<u16>,
    pub delegates: BTreeMap<ContractTz1Hash, Slots>,
}

#[derive(Clone, Serialize, Deserialize, Default)]
pub struct Slots(pub Vec<u16>);

impl fmt::Debug for Slots {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let slots = self.0.iter().fold(String::new(), |a, s| format!("{a}{s},"));
        write!(f, "{}", slots)
    }
}

#[derive(Debug)]
pub struct LevelState {
    pub current_level: i32,
    pub latest_proposal: Proposal,
    pub locked_round: Option<LockedRound>,
    pub endorsable_payload: Option<EndorsablePayload>,
    pub elected_block: Option<ElectedBlock>,
    pub delegate_slots: DelegateSlots,
    pub next_level_delegate_slots: DelegateSlots,
    pub next_level_proposed_round: Option<i32>,

    pub mempool: Mempool,
}

#[derive(Debug, Default)]
pub struct Mempool {
    pub endorsements: Vec<InlinedEndorsementMempoolContentsEndorsementVariant>,
    pub preendorsements: Vec<PreendorsementOperation>,
    pub payload: BlockPayload,
    // other ops
}

#[derive(Debug)]
pub enum Phase {
    NonProposer,
    CollectingPreendorsements,
    #[allow(dead_code)]
    CollectingEndorsements,
}

#[derive(Debug)]
pub struct RoundState {
    pub current_round: i32,
    pub current_phase: Phase,
    pub next_timeout: Option<Timestamp>,
}

////////////////////////////////////////////////////////////////////////////////////////////////////

#[derive(Deserialize, Serialize, Debug)]
pub struct Protocols {
    pub protocol: ProtocolHash,
    pub next_protocol: ProtocolHash,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ShellBlockHeader {
    pub hash: BlockHash,
    pub level: i32,
    pub proto: u8,
    pub predecessor: BlockHash,
    pub timestamp: String,
    pub validation_pass: u8,
    pub operations_hash: OperationListListHash,
    pub fitness: Vec<String>,
    pub context: ContextHash,
    pub protocol_data: String,
}

#[derive(Deserialize)]
pub struct FullHeader {
    pub hash: BlockHash,
    pub level: i32,
    pub proto: u8,
    pub predecessor: BlockHash,
    pub timestamp: String,
    pub validation_pass: u8,
    pub operations_hash: OperationListListHash,
    pub fitness: Vec<String>,
    pub context: ContextHash,
    pub payload_hash: Option<BlockPayloadHash>,
    pub payload_round: Option<i32>,
}

// signature watermark: 0x11 | chain_id
#[derive(BinWriter, HasEncoding, NomReader, Serialize, Clone, Debug)]
pub struct ProtocolBlockHeader {
    pub payload_hash: BlockPayloadHash,
    pub payload_round: i32,
    #[encoding(sized = "8", bytes)]
    pub proof_of_work_nonce: Vec<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed_nonce_hash: Option<NonceHash>,
    pub liquidity_baking_escape_vote: bool,
    pub signature: Signature,
}
