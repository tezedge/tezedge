// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::btree_map::Entry as BTreeMapEntry;
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crypto::hash::{BlockHash, BlockMetadataHash, BlockPayloadHash, OperationMetadataListListHash};
use storage::BlockHeaderWithHash;
use tezos_messages::p2p::encoding::block_header::Level;
use tezos_messages::p2p::encoding::operation::Operation;
use tezos_messages::protocol::SupportedProtocol;

use crate::request::RequestId;
use crate::service::storage_service::{BlockCycleInfo, StorageError};

mod serde_as_str {
    use serde::{de::Error as _, Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S, T>(value: &T, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        T: ToString,
    {
        value.to_string().serialize(s)
    }

    pub fn deserialize<'de, D, T>(deserializer: D) -> Result<T, D::Error>
    where
        D: Deserializer<'de>,
        T: std::str::FromStr,
        <T as std::str::FromStr>::Err: std::fmt::Display,
    {
        String::deserialize(deserializer)?
            .parse::<T>()
            .map_err(|e| D::Error::custom(format!("{}", e)))
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProtocolConstants {
    pub proof_of_work_nonce_size: u8,
    pub nonce_length: u8,

    pub blocks_per_cycle: i32,
    pub blocks_per_commitment: i32,
    #[serde(rename = "max_operations_time_to_live")]
    pub max_operations_ttl: i32,

    #[serde(with = "serde_as_str")]
    pub proof_of_work_threshold: i64,

    pub consensus_committee_size: u32,
    pub quorum_min: u16,
    pub quorum_max: u16,
    pub consensus_threshold: u16,
    pub min_proposal_quorum: i32,

    #[serde(with = "serde_as_str", rename = "minimal_block_delay")]
    pub min_block_delay: u64,
    #[serde(with = "serde_as_str")]
    pub delay_increment_per_round: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum CurrentHeadState {
    Idle,

    RehydrateInit {
        time: u64,
    },
    RehydratePending {
        time: u64,
        storage_req_id: RequestId,
    },
    RehydrateError {
        time: u64,
        error: StorageError,
    },
    RehydrateSuccess {
        time: u64,
        head: BlockHeaderWithHash,
        head_pred: Option<BlockHeaderWithHash>,

        block_metadata_hash: Option<BlockMetadataHash>,
        ops_metadata_hash: Option<OperationMetadataListListHash>,
        pred_block_metadata_hash: Option<BlockMetadataHash>,
        pred_ops_metadata_hash: Option<OperationMetadataListListHash>,

        cycle: Option<BlockCycleInfo>,

        operations: Vec<Vec<Operation>>,

        constants: Option<ProtocolConstants>,
        /// Last `TTL - 2` cemented blocks.
        cemented_live_blocks: BTreeMap<BlockHash, Level>,

        /// `block_header.proto` -> `SupportedProtocol`
        proto_cache: BTreeMap<u8, SupportedProtocol>,
    },

    Rehydrated {
        head: BlockHeaderWithHash,
        head_pred: Option<BlockHeaderWithHash>,

        payload_hash: Option<BlockPayloadHash>,
        payload_round: Option<i32>,
        // Needed for mempool prevalidator's begin construction
        // for prevalidation request.
        block_metadata_hash: Option<BlockMetadataHash>,
        ops_metadata_hash: Option<OperationMetadataListListHash>,
        pred_block_metadata_hash: Option<BlockMetadataHash>,
        pred_ops_metadata_hash: Option<OperationMetadataListListHash>,

        cycle: Option<BlockCycleInfo>,

        operations: Vec<Vec<Operation>>,

        constants: Option<ProtocolConstants>,
        /// Stores all applied blocks on last 2 levels.
        applied_blocks: BTreeMap<BlockHash, (BlockHeaderWithHash, Option<i32>)>,
        /// Live blocks size equals to `operations_max_ttl`, which at
        /// the moment is last 120 blocks. So this will contain 118 blocks
        /// last cemented blocks so that
        /// `cemented_live_blocks` -> `HEAD~1` -> `HEAD` = last 120 blocks.
        ///
        /// `Map` is used as a container instead of `Vec`, because we
        /// only need this data in memory for two things:
        /// 1. Return it in `live_blocks` rpc (this along with the
        ///    `head` and `head_pred`). Blocks in the result must be
        ///    sorted in a lexicographical order. That is exactly how
        ///    `BTreeSet` will sort them and `BTreeSet::iter()` will
        ///    give us already sorted values.
        /// 2. To check if the operation's branch is included in the
        ///    live blocks.
        cemented_live_blocks: BTreeMap<BlockHash, Level>,

        /// `block_header.proto` -> `SupportedProtocol`
        proto_cache: BTreeMap<u8, SupportedProtocol>,
    },
}

impl CurrentHeadState {
    #[inline(always)]
    pub fn new() -> Self {
        Self::Idle
    }

    pub fn rehydrated(head: BlockHeaderWithHash, head_pred: Option<BlockHeaderWithHash>) -> Self {
        let applied_blocks: BTreeMap<BlockHash, _> = IntoIterator::into_iter([
            Some((head.hash.clone(), (head.clone(), None))),
            head_pred
                .as_ref()
                .map(|pred| (pred.hash.clone(), (pred.clone(), None))),
        ])
        .flatten()
        .filter(|(_, (b, _))| b.header.level() + 1 >= head.header.level())
        .collect();

        let payload_hash = head.header.payload_hash();
        let payload_round = head.header.payload_round();

        Self::Rehydrated {
            head,
            head_pred,
            payload_hash,
            payload_round,
            block_metadata_hash: None,
            ops_metadata_hash: None,
            pred_block_metadata_hash: None,
            pred_ops_metadata_hash: None,
            cycle: None,
            operations: vec![],
            constants: None,
            applied_blocks,
            cemented_live_blocks: Default::default(),
            proto_cache: Default::default(),
        }
    }

    pub fn set_block_metadata_hash(&mut self, value: Option<BlockMetadataHash>) -> &mut Self {
        if let Self::Rehydrated {
            block_metadata_hash,
            ..
        } = self
        {
            *block_metadata_hash = value;
        }
        self
    }

    pub fn set_ops_metadata_hash(
        &mut self,
        value: Option<OperationMetadataListListHash>,
    ) -> &mut Self {
        if let Self::Rehydrated {
            ops_metadata_hash, ..
        } = self
        {
            *ops_metadata_hash = value;
        }
        self
    }

    pub fn set_pred_block_metadata_hash(&mut self, value: Option<BlockMetadataHash>) -> &mut Self {
        if let Self::Rehydrated {
            pred_block_metadata_hash,
            ..
        } = self
        {
            *pred_block_metadata_hash = value;
        }
        self
    }

    pub fn set_pred_ops_metadata_hash(
        &mut self,
        value: Option<OperationMetadataListListHash>,
    ) -> &mut Self {
        if let Self::Rehydrated {
            pred_ops_metadata_hash,
            ..
        } = self
        {
            *pred_ops_metadata_hash = value;
        }
        self
    }

    pub fn set_cycle(&mut self, value: Option<BlockCycleInfo>) -> &mut Self {
        if let Self::Rehydrated { cycle, .. } = self {
            *cycle = value;
        }
        self
    }

    pub fn set_operations(&mut self, value: Vec<Vec<Operation>>) -> &mut Self {
        if let Self::Rehydrated { operations, .. } = self {
            *operations = value;
        }
        self
    }

    pub fn set_constants(&mut self, value: Option<ProtocolConstants>) -> &mut Self {
        if let Self::Rehydrated { constants, .. } = self {
            *constants = value;
        }
        self
    }

    pub fn set_cemented_live_blocks(&mut self, value: BTreeMap<BlockHash, Level>) -> &mut Self {
        if let Self::Rehydrated {
            cemented_live_blocks,
            ..
        } = self
        {
            *cemented_live_blocks = value;
        }
        self
    }

    pub fn set_proto_cache(&mut self, value: BTreeMap<u8, SupportedProtocol>) -> &mut Self {
        if let Self::Rehydrated { proto_cache, .. } = self {
            *proto_cache = value;
        }
        self
    }

    pub fn add_applied_block(
        &mut self,
        block: &BlockHeaderWithHash,
        max_operations_ttl: Option<i32>,
    ) -> &mut Self {
        if let Self::Rehydrated { applied_blocks, .. } = self {
            if !applied_blocks.contains_key(&block.hash) {
                applied_blocks.insert(block.hash.clone(), (block.clone(), max_operations_ttl));
                applied_blocks.retain(|_, (b, _)| b.header.level() + 1 >= block.header.level());
            }
        }
        self
    }

    pub fn add_cemented_live_block(&mut self, hash: BlockHash, level: Level) -> &mut Self {
        if let Self::Rehydrated {
            cemented_live_blocks,
            constants,
            ..
        } = self
        {
            if let BTreeMapEntry::Vacant(e) = cemented_live_blocks.entry(hash) {
                e.insert(level);
                let ttl = constants
                    .as_ref()
                    .map(|v| v.max_operations_ttl)
                    .unwrap_or(120) as usize;
                let target_len = ttl - 1;
                while cemented_live_blocks.len() > target_len {
                    let min = cemented_live_blocks
                        .iter()
                        .min_by_key(|(_, level)| *level)
                        .map(|(hash, _)| hash.clone());
                    if let Some(hash) = min {
                        cemented_live_blocks.remove(&hash);
                    }
                }
            }
        }
        self
    }

    pub fn get(&self) -> Option<&BlockHeaderWithHash> {
        match self {
            Self::Rehydrated { head, .. } => Some(head),
            _ => None,
        }
    }

    pub fn get_pred(&self) -> Option<&BlockHeaderWithHash> {
        match self {
            Self::Rehydrated { head_pred, .. } => head_pred.as_ref(),
            _ => None,
        }
    }

    pub fn hash(&self) -> Option<&BlockHash> {
        self.get().map(|v| &v.hash)
    }

    pub fn pred_hash(&self) -> Option<&BlockHash> {
        self.get().map(|v| v.header.predecessor())
    }

    pub fn cemented_block_hash(&self) -> Option<&BlockHash> {
        self.get_pred().map(|v| v.header.predecessor())
    }

    pub fn level(&self) -> Option<Level> {
        self.get().map(|v| v.header.level())
    }

    pub fn payload_hash(&self) -> Option<&BlockPayloadHash> {
        match self {
            Self::Rehydrated { payload_hash, .. } => payload_hash.as_ref(),
            _ => None,
        }
    }

    pub fn payload_round(&self) -> Option<i32> {
        match self {
            Self::Rehydrated { payload_round, .. } => *payload_round,
            _ => None,
        }
    }

    pub fn round(&self) -> Option<i32> {
        match self {
            Self::Rehydrated { head, .. } => head.header.fitness().round(),
            _ => None,
        }
    }

    pub fn block_metadata_hash(&self) -> Option<&BlockMetadataHash> {
        match self {
            Self::Rehydrated {
                block_metadata_hash,
                ..
            } => block_metadata_hash.as_ref(),
            _ => None,
        }
    }

    pub fn ops_metadata_hash(&self) -> Option<&OperationMetadataListListHash> {
        match self {
            Self::Rehydrated {
                ops_metadata_hash, ..
            } => ops_metadata_hash.as_ref(),
            _ => None,
        }
    }

    pub fn pred_block_metadata_hash(&self) -> Option<&BlockMetadataHash> {
        match self {
            Self::Rehydrated {
                pred_block_metadata_hash,
                ..
            } => pred_block_metadata_hash.as_ref(),
            _ => None,
        }
    }

    pub fn pred_ops_metadata_hash(&self) -> Option<&OperationMetadataListListHash> {
        match self {
            Self::Rehydrated {
                pred_ops_metadata_hash,
                ..
            } => pred_ops_metadata_hash.as_ref(),
            _ => None,
        }
    }

    pub fn cycle_info(&self) -> Option<BlockCycleInfo> {
        match self {
            Self::Rehydrated { cycle, .. } => cycle.clone(),
            _ => None,
        }
    }

    pub fn next_block_cycle(&self) -> Option<BlockCycleInfo> {
        match self {
            Self::Rehydrated {
                constants, cycle, ..
            } => {
                let mut cycle = cycle.clone()?;
                if cycle.position + 1 == constants.as_ref()?.blocks_per_cycle {
                    cycle.position = 0;
                    cycle.cycle += 1;
                    Some(cycle)
                } else {
                    Some(cycle)
                }
            }
            _ => None,
        }
    }

    pub fn operations(&self) -> Option<&Vec<Vec<Operation>>> {
        match self {
            Self::Rehydrated { operations, .. } => Some(operations),
            _ => None,
        }
    }

    pub fn constants(&self) -> Option<&ProtocolConstants> {
        match self {
            Self::Rehydrated { constants, .. } => constants.as_ref(),
            _ => None,
        }
    }

    pub fn is_applied(&self, block_hash: &BlockHash) -> bool {
        match self {
            Self::Rehydrated { applied_blocks, .. } => applied_blocks.contains_key(block_hash),
            _ => false,
        }
    }

    pub fn is_cemented_live_block(&self, block_hash: &BlockHash) -> bool {
        match self {
            Self::Rehydrated {
                cemented_live_blocks,
                ..
            } => cemented_live_blocks.contains_key(block_hash),
            _ => false,
        }
    }

    pub fn is_live_block(&self, block_hash: &BlockHash) -> bool {
        match self {
            Self::Rehydrated {
                cemented_live_blocks,
                head,
                head_pred,
                ..
            } => {
                &head.hash == block_hash
                    || head_pred.as_ref().map_or(false, |p| &p.hash == block_hash)
                    || cemented_live_blocks.contains_key(block_hash)
            }
            _ => false,
        }
    }

    pub fn protocol(&self) -> Option<SupportedProtocol> {
        match self {
            Self::Rehydrated {
                head, proto_cache, ..
            } => proto_cache.get(&head.header.proto()).cloned(),
            _ => None,
        }
    }

    pub fn protocol_from_id(&self, proto: u8) -> Option<SupportedProtocol> {
        match self {
            Self::Rehydrated { proto_cache, .. } => proto_cache.get(&proto).cloned(),
            _ => None,
        }
    }

    pub fn is_precheckable(&self) -> bool {
        self.protocol().map_or(false, |protocol| {
            matches!(
                protocol,
                SupportedProtocol::Proto010
                    | SupportedProtocol::Proto011
                    | SupportedProtocol::Proto012
                    | SupportedProtocol::Proto013
            )
        })
    }

    pub fn max_operations_ttl(&self) -> Option<i32> {
        match self {
            Self::Rehydrated {
                head,
                applied_blocks,
                ..
            } => applied_blocks.get(&head.hash).and_then(|(_, v)| v.clone()),
            _ => None,
        }
    }

    pub fn predecessor_max_operations_ttl(&self) -> Option<i32> {
        match self {
            Self::Rehydrated {
                head_pred: Some(head_pred),
                applied_blocks,
                ..
            } => applied_blocks
                .get(&head_pred.hash)
                .and_then(|(_, v)| v.clone()),
            _ => None,
        }
    }
}

impl Default for CurrentHeadState {
    fn default() -> Self {
        Self::new()
    }
}

// #[derive(Serialize, Deserialize, Debug, Clone)]
// pub struct BlockHashWithLevelOrdByHash {
//     hash: BlockHash,
//     level: Level,
// }

// impl BlockHashWithLevelOrdByHash {
//     pub fn new(hash: BlockHash, level: Level) -> Self {
//         Self { hash, level }
//     }

//     pub fn hash(&self) -> &BlockHash {
//         &self.hash
//     }

//     pub fn level(&self) -> Level {
//         self.level
//     }
// }

// impl Ord for BlockHashWithLevelOrdByHash {
//     fn cmp(&self, other: &Self) -> std::cmp::Ordering {
//         self.hash.cmp(&other.hash)
//     }
// }
