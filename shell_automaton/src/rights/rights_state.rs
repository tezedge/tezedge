// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::HashMap;

use crypto::hash::ProtocolHash;
use redux_rs::ActionId;
use storage::{cycle_eras_storage::CycleErasData, cycle_storage::CycleData};
use tezos_messages::{
    base::signature_public_key::SignaturePublicKey, p2p::encoding::block_header::BlockHeader,
};

use crate::{
    service::storage_service::StorageError,
    storage::{
        kv_block_additional_data, kv_block_header, kv_constants, kv_cycle_eras, kv_cycle_meta,
    },
};

use super::{
    rights_effects::EndorsingRightsCalculationError,
    utils::{CycleError, Position},
    Cycle, EndorsingRightsKey,
};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct RightsState {
    pub endorsing_rights: HashMap<EndorsingRightsKey, EndorsingRightsRequest>,
}

impl RightsState {
    pub(super) fn should_cache_endorsing_rights(_key: &EndorsingRightsKey) -> bool {
        true
    }
}

pub type Delegate = SignaturePublicKey;
pub type Slot = u16;
pub type Slots = Vec<Slot>;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EndorsingRights {
    pub slot_to_delegate: Vec<Delegate>,
    pub delegate_to_slots: HashMap<Delegate, Vec<Slot>>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, strum_macros::AsRefStr)]
pub enum EndorsingRightsRequest {
    /// Request for endorsing rights for the given level is initialized.
    Init {
        start: ActionId,
    },
    PendingBlockHeader {
        start: ActionId,
    },
    BlockHeaderReady {
        block_header: BlockHeader,
        start: ActionId,
    },
    PendingProtocolHash {
        block_header: BlockHeader,
        start: ActionId,
    },
    ProtocolHashReady {
        proto_hash: ProtocolHash,
        block_header: BlockHeader,
        start: ActionId,
    },
    PendingProtocolConstants {
        proto_hash: ProtocolHash,
        block_header: BlockHeader,
        start: ActionId,
    },
    ProtocolConstantsReady {
        protocol_constants: ProtocolConstants,
        proto_hash: ProtocolHash,
        block_header: BlockHeader,
        start: ActionId,
    },
    PendingCycleEras {
        protocol_constants: ProtocolConstants,
        proto_hash: ProtocolHash,
        block_header: BlockHeader,
        start: ActionId,
    },
    CycleErasReady {
        cycle_eras: CycleErasData,
        protocol_constants: ProtocolConstants,
        block_header: BlockHeader,
        start: ActionId,
    },
    PendingCycle {
        cycle_eras: CycleErasData,
        protocol_constants: ProtocolConstants,
        block_header: BlockHeader,
        start: ActionId,
    },
    CycleReady {
        cycle: Cycle,
        position: Position,
        protocol_constants: ProtocolConstants,
        start: ActionId,
    },
    PendingCycleData {
        cycle: Cycle,
        position: Position,
        protocol_constants: ProtocolConstants,
        start: ActionId,
    },
    CycleDataReady {
        cycle_data: CycleData,
        position: Position,
        protocol_constants: ProtocolConstants,
        start: ActionId,
    },
    PendingRights {
        cycle_data: CycleData,
        position: Position,
        protocol_constants: ProtocolConstants,
        start: ActionId,
    },
    Ready(EndorsingRights),
    Error(EndorsingRightsError),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, thiserror::Error)]
pub enum EndorsingRightsError {
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
    Calculation(#[from] EndorsingRightsCalculationError),
}

/*
impl From<CycleError> for EndorsingRightsError {
    fn from(error: CycleError) -> Self {
        Self::Cycle(error)
    }
}
*/

impl From<kv_block_header::Error> for EndorsingRightsError {
    fn from(error: kv_block_header::Error) -> Self {
        match error {
            kv_block_header::Error::Storage(error) => Self::Storage(error),
            kv_block_header::Error::NotFound => Self::MissingBlockHeader,
        }
    }
}

impl From<kv_block_additional_data::Error> for EndorsingRightsError {
    fn from(error: kv_block_additional_data::Error) -> Self {
        match error {
            kv_block_additional_data::Error::Storage(error) => Self::Storage(error),
            kv_block_additional_data::Error::NotFound => Self::MissingProtocolHash,
        }
    }
}

impl From<kv_constants::Error> for EndorsingRightsError {
    fn from(error: kv_constants::Error) -> Self {
        match error {
            kv_constants::Error::Storage(error) => Self::Storage(error),
            kv_constants::Error::NotFound => Self::MissingProtocolConstants,
        }
    }
}

impl From<kv_cycle_eras::Error> for EndorsingRightsError {
    fn from(error: kv_cycle_eras::Error) -> Self {
        match error {
            kv_cycle_eras::Error::Storage(error) => Self::Storage(error),
            kv_cycle_eras::Error::NotFound => Self::MissingCycleEras,
        }
    }
}

impl From<kv_cycle_meta::Error> for EndorsingRightsError {
    fn from(error: kv_cycle_meta::Error) -> Self {
        match error {
            kv_cycle_meta::Error::Storage(error) => Self::Storage(error),
            kv_cycle_meta::Error::NotFound => Self::MissingCycleData,
        }
    }
}

impl From<serde_json::Error> for EndorsingRightsError {
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
