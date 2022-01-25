// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    collections::{BTreeMap, HashMap},
    time::Duration,
};

use crypto::hash::{ProtocolHash, TryFromPKError};
use redux_rs::ActionId;
use storage::{cycle_eras_storage::CycleErasData, cycle_storage::CycleData};
use tezos_messages::{
    base::signature_public_key::SignaturePublicKey,
    p2p::encoding::block_header::{BlockHeader, Level},
    protocol::{SupportedProtocol, UnsupportedProtocolError},
};

use crate::{
    service::{rpc_service::RpcId, storage_service::StorageError},
    storage::{
        kv_block_additional_data, kv_block_header, kv_constants, kv_cycle_eras, kv_cycle_meta,
    },
};

use super::{
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
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, derive_more::From)]
pub enum RightsResult {
    Baking(BakingRights),
    Endorsing(EndorsingRights),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RightsCache {
    pub time: Duration,
    pub rights: BTreeMap<Level, (ActionId, RightsResult)>,
}

impl Default for RightsCache {
    fn default() -> Self {
        Self {
            time: Duration::from_secs(600),
            rights: Default::default(),
        }
    }
}

pub type Delegate = SignaturePublicKey;
pub type Slot = u16;
pub type Slots = Vec<Slot>;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EndorsingRights {
    pub level: Level,
    pub slot_to_delegate: Vec<Delegate>,
    pub delegate_to_slots: HashMap<Delegate, Vec<Slot>>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BakingRights {
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
    CycleErasReady {
        start: ActionId,
        block_header: BlockHeader,
        protocol: SupportedProtocol,
        protocol_constants: ProtocolConstants,
        cycle_eras: CycleErasData,
    },
    PendingCycle {
        start: ActionId,
        protocol: SupportedProtocol,
        block_header: BlockHeader,
        protocol_constants: ProtocolConstants,
        cycle_eras: CycleErasData,
    },
    CycleReady {
        start: ActionId,
        protocol: SupportedProtocol,
        protocol_constants: ProtocolConstants,
        level: Level,
        cycle: Cycle,
        position: Position,
    },
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
    Ready(EndorsingRights),
    BakingRightsReady(BakingRights),
    Error(RightsError),
}

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
    #[error("Missing cycle eras")]
    MissingCycleEras,
    #[error("Error calculating cycle: {0}")]
    Cycle(#[from] CycleError),
    #[error("Missing cycle meta data")]
    MissingCycleData,
    #[error("Error calculating endorsing rights: {0}")]
    Calculation(#[from] RightsCalculationError),
    #[error("Unsupported protocol: {0}")]
    UnsupportedProto(#[from] UnsupportedProtocolError),
}

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

impl From<kv_cycle_eras::Error> for RightsError {
    fn from(error: kv_cycle_eras::Error) -> Self {
        match error {
            kv_cycle_eras::Error::Storage(error) => Self::Storage(error),
            kv_cycle_eras::Error::NotFound => Self::MissingCycleEras,
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProtocolConstants {
    pub(super) blocks_per_cycle: i32,
    pub(super) preserved_cycles: u8,
    pub(super) endorsers_per_block: u16,
    pub(super) nonce_length: u8,
}
