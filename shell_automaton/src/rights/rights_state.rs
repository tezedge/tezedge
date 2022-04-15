// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    collections::{BTreeMap, HashMap},
    time::Duration,
};

use crypto::hash::{ProtocolHash, TryFromPKError};
use redux_rs::ActionId;
use storage::cycle_storage::CycleData;
use tezos_messages::{
    base::signature_public_key::SignaturePublicKey,
    p2p::encoding::block_header::{BlockHeader, Level},
    protocol::{SupportedProtocol, UnsupportedProtocolError},
};

use crate::{
    protocol_runner::ProtocolRunnerToken,
    service::{rpc_service::RpcId, storage_service::StorageError},
    storage::{kv_block_additional_data, kv_block_header, kv_constants, kv_cycle_meta},
};

use super::{
    cycle_delegates::{CycleDelegatesState, Delegates, DelegatesError},
    cycle_eras::{CycleEras, CycleErasError, CycleErasState},
    rights_effects::RightsCalculationError,
    utils::{CycleError, Position},
    Cycle, RightsInput, RightsKey,
};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct RightsState {
    pub requests: HashMap<RightsKey, RightsRequest>,
    pub rpc_requests: HashMap<RightsKey, Vec<RpcId>>,
    pub cache: RightsCache,
    pub errors: Vec<(RightsInput, RightsRequest, RightsError)>,
    pub cycle_eras: CycleErasState,
    pub cycle_delegates: CycleDelegatesState,
}

impl RightsState {
    pub(crate) fn tenderbake_endorsing_rights(&self, level: Level) -> Option<&EndorsingRights> {
        self.cache
            .endorsing
            .get(&level)
            .map(|(_, endorsing_rights)| endorsing_rights)
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RightsCache {
    pub time: Duration,
    pub baking: BTreeMap<Level, (ActionId, BakingRightsOld)>,
    pub endorsing_old: BTreeMap<Level, (ActionId, EndorsingRightsOld)>,
    pub endorsing: BTreeMap<Level, (ActionId, EndorsingRights)>,
}

impl Default for RightsCache {
    fn default() -> Self {
        Self {
            time: Duration::from_secs(600),
            baking: Default::default(),
            endorsing_old: Default::default(),
            endorsing: Default::default(),
        }
    }
}

pub type Delegate = SignaturePublicKey;
pub type Slot = u16;
pub type Slots = Vec<Slot>;
pub type EndorsingPower = u16;

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EndorsingRightsOld {
    pub level: Level,
    pub slot_to_delegate: Vec<Delegate>,
    pub delegate_to_slots: BTreeMap<Delegate, Vec<Slot>>,
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EndorsingRights {
    pub level: Level,
    pub slots: BTreeMap<Slot, (Delegate, EndorsingPower)>,
    pub delegates: BTreeMap<Delegate, (Slot, EndorsingPower)>,
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BakingRightsOld {
    pub level: Level,
    pub priorities: Vec<Delegate>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, strum_macros::AsRefStr)]
pub enum RightsRequest {
    /// Request for endorsing rights for the given level is initialized.
    Init {
        start: ActionId,
    },
    PendingBlockHeader {
        start: ActionId,
    },
    BlockHeaderReady {
        start: ActionId,
        block_header: BlockHeader,
    },
    PendingProtocolHash {
        start: ActionId,
        block_header: BlockHeader,
    },
    ProtocolHashReady {
        start: ActionId,
        block_header: BlockHeader,
        proto_hash: ProtocolHash,
        protocol: SupportedProtocol,
    },
    PendingProtocolConstants {
        start: ActionId,
        block_header: BlockHeader,
        proto_hash: ProtocolHash,
        protocol: SupportedProtocol,
    },
    ProtocolConstantsReady {
        start: ActionId,
        proto_hash: ProtocolHash,
        block_header: BlockHeader,
        protocol: SupportedProtocol,
        protocol_constants: ProtocolConstants,
    },
    PendingCycleEras {
        start: ActionId,
        block_header: BlockHeader,
        proto_hash: ProtocolHash,
        protocol: SupportedProtocol,
        protocol_constants: ProtocolConstants,
    },
    PendingCycleErasFromContext {
        start: ActionId,
        block_header: BlockHeader,
        proto_hash: ProtocolHash,
        protocol: SupportedProtocol,
        protocol_constants: ProtocolConstants,
    },
    CycleErasReady {
        start: ActionId,
        block_header: BlockHeader,
        protocol: SupportedProtocol,
        protocol_constants: ProtocolConstants,
        cycle_eras: CycleEras,
    },
    PendingCycle {
        start: ActionId,
        block_header: BlockHeader,
        protocol: SupportedProtocol,
        protocol_constants: ProtocolConstants,
        cycle_eras: CycleEras,
    },
    CycleReady {
        start: ActionId,
        block_header: BlockHeader,
        protocol: SupportedProtocol,
        protocol_constants: ProtocolConstants,
        level: Level,
        cycle: Cycle,
        position: Position,
    },

    // pre-Ithaca
    PendingCycleData {
        start: ActionId,
        protocol: SupportedProtocol,
        protocol_constants: ProtocolConstants,
        level: Level,
        cycle: Cycle,
        position: Position,
    },
    CycleDataReady {
        start: ActionId,
        protocol: SupportedProtocol,
        protocol_constants: ProtocolConstants,
        level: Level,
        position: Position,
        cycle_data: CycleData,
    },
    PendingRightsCalculation {
        start: ActionId,
        protocol: SupportedProtocol,
        protocol_constants: ProtocolConstants,
        level: Level,
        position: Position,
        cycle_data: CycleData,
    },

    PendingCycleDelegates {
        start: ActionId,
        block_header: BlockHeader,
        level: Level,
        cycle: Cycle,
    },
    CycleDelegatesReady {
        start: ActionId,
        block_header: BlockHeader,
        level: Level,
        delegates: Delegates,
    },
    PendingRightsCalculationIthaca {
        start: ActionId,
        block_header: BlockHeader,
        level: Level,
        delegates: Delegates,
    },
    PendingRightsFromContextIthaca {
        start: ActionId,
        level: Level,
        delegates: Delegates,
        token: ProtocolRunnerToken,
    },

    EndorsingOldReady(EndorsingRightsOld),
    BakingOldReady(BakingRightsOld),
    EndorsingReady(EndorsingRights),
    Error(RightsError),
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, thiserror::Error)]
pub enum RightsError {
    #[error("Storage error: {0}")]
    Storage(#[from] StorageError),
    #[error("Missing block header")]
    MissingBlockHeader,
    #[error("Missing block header")]
    MissingProtocolHash,
    #[error("Missing block header")]
    MissingProtocolConstants,
    #[error("Error parsing protocol constants: {0}")]
    ParseProtocolConstants(String),
    #[error("Missing cycle eras: {0}")]
    MissingCycleEras(#[from] CycleErasError),
    #[error("Error calculating cycle: {0}")]
    Cycle(#[from] CycleError),
    #[error("Missing cycle meta data")]
    MissingCycleData,
    #[error("Error querying cycle delegates: {0}")]
    CycleDelegates(#[from] DelegatesError),
    #[error("Error fetching endorsing rights form context: {0}")]
    ContextRights(#[from] crate::service::protocol_runner_service::EndorsingRightsError),
    #[error("Error calculating endorsing rights: {0}")]
    Calculation(#[from] RightsCalculationError),
    #[error("Unsupported protocol: {0}")]
    UnsupportedProto(#[from] UnsupportedProtocolError),
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, thiserror::Error)]
pub enum RightsRpcError {
    #[error("Error calculating delegate hash")]
    Hash(#[from] TryFromPKError),
    #[error("Numeric conversion error")]
    Num,
    #[error(transparent)]
    Other(#[from] RightsError),
}

impl From<kv_block_header::Error> for RightsError {
    fn from(error: kv_block_header::Error) -> Self {
        match error {
            kv_block_header::Error::Storage(error) => Self::Storage(error),
            kv_block_header::Error::NotFound => Self::MissingBlockHeader,
        }
    }
}

impl From<kv_block_additional_data::Error> for RightsError {
    fn from(error: kv_block_additional_data::Error) -> Self {
        match error {
            kv_block_additional_data::Error::Storage(error) => Self::Storage(error),
            kv_block_additional_data::Error::NotFound => Self::MissingProtocolHash,
        }
    }
}

impl From<kv_constants::Error> for RightsError {
    fn from(error: kv_constants::Error) -> Self {
        match error {
            kv_constants::Error::Storage(error) => Self::Storage(error),
            kv_constants::Error::NotFound => Self::MissingProtocolConstants,
        }
    }
}

impl From<kv_cycle_meta::Error> for RightsError {
    fn from(error: kv_cycle_meta::Error) -> Self {
        match error {
            kv_cycle_meta::Error::Storage(error) => Self::Storage(error),
            kv_cycle_meta::Error::NotFound => Self::MissingCycleData,
        }
    }
}

impl From<serde_json::Error> for RightsError {
    fn from(error: serde_json::Error) -> Self {
        Self::ParseProtocolConstants(error.to_string())
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DelegateSelection {
    Random,
    RoundRobin,
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProtocolConstants {
    pub blocks_per_cycle: i32,
    pub preserved_cycles: u8,
    pub nonce_length: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endorsers_per_block: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delegate_selection: Option<DelegateSelection>,
}

impl ProtocolConstants {
    pub fn blocks_per_cycle(&self) -> Option<i32> {
        Some(self.blocks_per_cycle)
    }
    pub fn preserved_cycles(&self) -> Option<u8> {
        Some(self.preserved_cycles)
    }
    pub fn endorsers_per_block(&self) -> Option<u16> {
        self.endorsers_per_block.clone()
    }
    pub fn nonce_length(&self) -> Option<u8> {
        Some(self.nonce_length)
    }

    pub fn delegate_selection(&self) -> Option<DelegateSelection> {
        self.delegate_selection.clone()
    }
}
