// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use crypto::hash::{BlockHash, ProtocolHash};
use storage::{cycle_eras_storage::CycleErasData, cycle_storage::CycleData};
use tezos_messages::p2p::encoding::block_header::BlockHeader;

use crate::storage::kv_block_header;
use crate::{State, EnablingCondition};

use super::{
    utils::Position, Cycle, EndorsingRights, EndorsingRightsError, EndorsingRightsKey,
    ProtocolConstants,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RightsGetEndorsingRightsAction {
    //pub block_hash: BlockHash,
    pub key: EndorsingRightsKey,
}

impl EnablingCondition<State> for RightsGetEndorsingRightsAction {
    fn is_enabled(&self, state: &State) -> bool {
        let _ = state;
        true
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RightsEndorsingRightsReadyAction {
    pub key: EndorsingRightsKey,
    pub endorsing_rights: EndorsingRights,
}

impl EnablingCondition<State> for RightsEndorsingRightsReadyAction {
    fn is_enabled(&self, state: &State) -> bool {
        let _ = state;
        true
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RightsEndorsingRightsErrorAction {
    pub key: EndorsingRightsKey,
    pub error: EndorsingRightsError,
}

impl EnablingCondition<State> for RightsEndorsingRightsErrorAction {
    fn is_enabled(&self, state: &State) -> bool {
        let _ = state;
        true
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RightsEndorsingRightsGetBlockHeaderAction {
    pub key: EndorsingRightsKey,
}

impl EnablingCondition<State> for RightsEndorsingRightsGetBlockHeaderAction {
    fn is_enabled(&self, state: &State) -> bool {
        let _ = state;
        true
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RightsEndorsingRightsBlockHeaderReadyAction {
    pub key: EndorsingRightsKey,
    pub block_header: BlockHeader,
}

impl EnablingCondition<State> for RightsEndorsingRightsBlockHeaderReadyAction {
    fn is_enabled(&self, state: &State) -> bool {
        let _ = state;
        true
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RightsEndorsingRightsGetProtocolHashAction {
    pub key: EndorsingRightsKey,
}

impl EnablingCondition<State> for RightsEndorsingRightsGetProtocolHashAction {
    fn is_enabled(&self, state: &State) -> bool {
        let _ = state;
        true
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RightsEndorsingRightsProtocolHashStorageReadyAction {
    pub key: BlockHash,
    pub proto_hash: ProtocolHash,
}

impl EnablingCondition<State> for RightsEndorsingRightsProtocolHashStorageReadyAction {
    fn is_enabled(&self, state: &State) -> bool {
        let _ = state;
        true
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RightsEndorsingRightsProtocolHashStorageErrorAction {
    pub key: BlockHash,
    pub error: kv_block_header::Error,
}

impl EnablingCondition<State> for RightsEndorsingRightsProtocolHashStorageErrorAction {
    fn is_enabled(&self, state: &State) -> bool {
        let _ = state;
        true
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RightsEndorsingRightsProtocolHashReadyAction {
    pub key: EndorsingRightsKey,
    pub proto_hash: ProtocolHash,
}

impl EnablingCondition<State> for RightsEndorsingRightsProtocolHashReadyAction {
    fn is_enabled(&self, state: &State) -> bool {
        let _ = state;
        true
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RightsEndorsingRightsGetProtocolConstantsAction {
    pub key: EndorsingRightsKey,
}

impl EnablingCondition<State> for RightsEndorsingRightsGetProtocolConstantsAction {
    fn is_enabled(&self, state: &State) -> bool {
        let _ = state;
        true
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RightsEndorsingRightsProtocolConstantsReadyAction {
    pub key: EndorsingRightsKey,
    pub constants: ProtocolConstants,
}

impl EnablingCondition<State> for RightsEndorsingRightsProtocolConstantsReadyAction {
    fn is_enabled(&self, state: &State) -> bool {
        let _ = state;
        true
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RightsEndorsingRightsGetCycleErasAction {
    pub key: EndorsingRightsKey,
}

impl EnablingCondition<State> for RightsEndorsingRightsGetCycleErasAction {
    fn is_enabled(&self, state: &State) -> bool {
        let _ = state;
        true
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RightsEndorsingRightsCycleErasReadyAction {
    pub key: EndorsingRightsKey,
    pub cycle_eras: CycleErasData,
}

impl EnablingCondition<State> for RightsEndorsingRightsCycleErasReadyAction {
    fn is_enabled(&self, state: &State) -> bool {
        let _ = state;
        true
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RightsEndorsingRightsGetCycleAction {
    pub key: EndorsingRightsKey,
}

impl EnablingCondition<State> for RightsEndorsingRightsGetCycleAction {
    fn is_enabled(&self, state: &State) -> bool {
        let _ = state;
        true
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RightsEndorsingRightsCycleReadyAction {
    pub key: EndorsingRightsKey,
    pub cycle: Cycle,
    pub position: Position,
}

impl EnablingCondition<State> for RightsEndorsingRightsCycleReadyAction {
    fn is_enabled(&self, state: &State) -> bool {
        let _ = state;
        true
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RightsEndorsingRightsGetCycleDataAction {
    pub key: EndorsingRightsKey,
}

impl EnablingCondition<State> for RightsEndorsingRightsGetCycleDataAction {
    fn is_enabled(&self, state: &State) -> bool {
        let _ = state;
        true
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RightsEndorsingRightsCycleDataReadyAction {
    pub key: EndorsingRightsKey,
    pub cycle_data: CycleData,
}

impl EnablingCondition<State> for RightsEndorsingRightsCycleDataReadyAction {
    fn is_enabled(&self, state: &State) -> bool {
        let _ = state;
        true
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RightsEndorsingRightsCalculateAction {
    pub key: EndorsingRightsKey,
}

impl EnablingCondition<State> for RightsEndorsingRightsCalculateAction {
    fn is_enabled(&self, state: &State) -> bool {
        let _ = state;
        true
    }
}
