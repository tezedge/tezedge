// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crypto::hash::{BlockHash, ProtocolHash};
use storage::{cycle_eras_storage::CycleErasData, cycle_storage::CycleData};
use tezos_messages::base::signature_public_key::SignaturePublicKeyHash;
use tezos_messages::p2p::encoding::block_header::BlockHeader;

use crate::service::rpc_service::RpcId;
use crate::storage::kv_block_header;
use crate::{EnablingCondition, State};

use super::{
    utils::Position, Cycle, EndorsingRights, EndorsingRightsError, EndorsingRightsKey,
    ProtocolConstants,
};
use super::{EndorsingRightsRpcError, Slots};

// Entry actions

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RightsGetEndorsingRightsAction {
    pub key: EndorsingRightsKey,
}

impl EnablingCondition<State> for RightsGetEndorsingRightsAction {
    fn is_enabled(&self, state: &State) -> bool {
        let _ = state;
        true
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RightsEndorsingRightsInitAction {
    pub key: EndorsingRightsKey,
}

impl EnablingCondition<State> for RightsEndorsingRightsInitAction {
    fn is_enabled(&self, state: &State) -> bool {
        !state.rights.requests.contains_key(&self.key)
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

// RPC actions

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RightsRpcEndorsingRightsGetAction {
    pub rpc_id: RpcId,
    pub key: EndorsingRightsKey,
}

impl EnablingCondition<State> for RightsRpcEndorsingRightsGetAction {
    fn is_enabled(&self, _state: &State) -> bool {
        true
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RightsRpcEndorsingRightsReadyAction {
    pub rpc_id: RpcId,
    pub endorsing_rights: BTreeMap<SignaturePublicKeyHash, Slots>,
}

impl EnablingCondition<State> for RightsRpcEndorsingRightsReadyAction {
    fn is_enabled(&self, _state: &State) -> bool {
        true
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RightsRpcEndorsingRightsErrorAction {
    pub rpc_id: RpcId,
    pub error: EndorsingRightsRpcError,
}

impl EnablingCondition<State> for RightsRpcEndorsingRightsErrorAction {
    fn is_enabled(&self, _state: &State) -> bool {
        true
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RightsRpcEndorsingRightsPruneAction {
    pub rpc_id: RpcId,
}

impl EnablingCondition<State> for RightsRpcEndorsingRightsPruneAction {
    fn is_enabled(&self, _state: &State) -> bool {
        true
    }
}

// Auxiliary actions

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
