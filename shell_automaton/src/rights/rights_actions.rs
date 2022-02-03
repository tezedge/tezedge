// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crypto::hash::{BlockHash, ProtocolHash};
use storage::{cycle_eras_storage::CycleErasData, cycle_storage::CycleData};
use tezos_messages::base::signature_public_key::SignaturePublicKeyHash;
use tezos_messages::p2p::encoding::block_header::BlockHeader;
use tezos_messages::protocol::SupportedProtocol;

use crate::service::rpc_service::RpcId;
use crate::storage::kv_block_header;
use crate::{EnablingCondition, State};

use super::{utils::Position, Cycle, EndorsingRights, ProtocolConstants, RightsError, RightsKey};
use super::{BakingRights, RightsRpcError, Slots};

// Entry actions

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RightsGetAction {
    pub key: RightsKey,
}

impl EnablingCondition<State> for RightsGetAction {
    fn is_enabled(&self, state: &State) -> bool {
        let _ = state;
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RightsInitAction {
    pub key: RightsKey,
}

impl EnablingCondition<State> for RightsInitAction {
    fn is_enabled(&self, state: &State) -> bool {
        !state.rights.requests.contains_key(&self.key)
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RightsEndorsingReadyAction {
    pub key: RightsKey,
    pub endorsing_rights: EndorsingRights,
}

impl EnablingCondition<State> for RightsEndorsingReadyAction {
    fn is_enabled(&self, state: &State) -> bool {
        let _ = state;
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RightsBakingReadyAction {
    pub key: RightsKey,
    pub baking_rights: BakingRights,
}

impl EnablingCondition<State> for RightsBakingReadyAction {
    fn is_enabled(&self, state: &State) -> bool {
        let _ = state;
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RightsErrorAction {
    pub key: RightsKey,
    pub error: RightsError,
}

impl EnablingCondition<State> for RightsErrorAction {
    fn is_enabled(&self, state: &State) -> bool {
        let _ = state;
        true
    }
}

// RPC actions
#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RightsRpcGetAction {
    pub key: RightsKey,
    pub rpc_id: RpcId,
}

impl EnablingCondition<State> for RightsRpcGetAction {
    fn is_enabled(&self, _state: &State) -> bool {
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RightsRpcEndorsingReadyAction {
    pub rpc_id: RpcId,
    pub endorsing_rights: BTreeMap<SignaturePublicKeyHash, Slots>,
}

impl EnablingCondition<State> for RightsRpcEndorsingReadyAction {
    fn is_enabled(&self, _state: &State) -> bool {
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BakingRightsPriority {
    pub delegate: SignaturePublicKeyHash,
    pub priority: u16,
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RightsRpcBakingReadyAction {
    pub rpc_id: RpcId,
    pub baking_rights: Vec<BakingRightsPriority>,
}

impl EnablingCondition<State> for RightsRpcBakingReadyAction {
    fn is_enabled(&self, _state: &State) -> bool {
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RightsRpcErrorAction {
    pub rpc_id: RpcId,
    pub error: RightsRpcError,
}

impl EnablingCondition<State> for RightsRpcErrorAction {
    fn is_enabled(&self, _state: &State) -> bool {
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RightsRpcPruneAction {
    pub key: RightsKey,
}

impl EnablingCondition<State> for RightsRpcPruneAction {
    fn is_enabled(&self, _state: &State) -> bool {
        true
    }
}

// Auxiliary actions
#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RightsGetBlockHeaderAction {
    pub key: RightsKey,
}

impl EnablingCondition<State> for RightsGetBlockHeaderAction {
    fn is_enabled(&self, state: &State) -> bool {
        let _ = state;
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RightsBlockHeaderReadyAction {
    pub key: RightsKey,
    pub block_header: BlockHeader,
}

impl EnablingCondition<State> for RightsBlockHeaderReadyAction {
    fn is_enabled(&self, state: &State) -> bool {
        let _ = state;
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RightsGetProtocolHashAction {
    pub key: RightsKey,
}

impl EnablingCondition<State> for RightsGetProtocolHashAction {
    fn is_enabled(&self, state: &State) -> bool {
        let _ = state;
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RightsProtocolHashStorageReadyAction {
    pub key: BlockHash,
    pub proto_hash: ProtocolHash,
}

impl EnablingCondition<State> for RightsProtocolHashStorageReadyAction {
    fn is_enabled(&self, state: &State) -> bool {
        let _ = state;
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RightsProtocolHashStorageErrorAction {
    pub key: BlockHash,
    pub error: kv_block_header::Error,
}

impl EnablingCondition<State> for RightsProtocolHashStorageErrorAction {
    fn is_enabled(&self, state: &State) -> bool {
        let _ = state;
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RightsProtocolHashReadyAction {
    pub key: RightsKey,
    pub proto_hash: ProtocolHash,
    pub protocol: SupportedProtocol,
}

impl EnablingCondition<State> for RightsProtocolHashReadyAction {
    fn is_enabled(&self, state: &State) -> bool {
        let _ = state;
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RightsGetProtocolConstantsAction {
    pub key: RightsKey,
}

impl EnablingCondition<State> for RightsGetProtocolConstantsAction {
    fn is_enabled(&self, state: &State) -> bool {
        let _ = state;
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RightsProtocolConstantsReadyAction {
    pub key: RightsKey,
    pub constants: ProtocolConstants,
}

impl EnablingCondition<State> for RightsProtocolConstantsReadyAction {
    fn is_enabled(&self, state: &State) -> bool {
        let _ = state;
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RightsGetCycleErasAction {
    pub key: RightsKey,
}

impl EnablingCondition<State> for RightsGetCycleErasAction {
    fn is_enabled(&self, state: &State) -> bool {
        let _ = state;
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RightsCycleErasReadyAction {
    pub key: RightsKey,
    pub cycle_eras: CycleErasData,
}

impl EnablingCondition<State> for RightsCycleErasReadyAction {
    fn is_enabled(&self, state: &State) -> bool {
        let _ = state;
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RightsGetCycleAction {
    pub key: RightsKey,
}

impl EnablingCondition<State> for RightsGetCycleAction {
    fn is_enabled(&self, state: &State) -> bool {
        let _ = state;
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RightsCycleReadyAction {
    pub key: RightsKey,
    pub cycle: Cycle,
    pub position: Position,
}

impl EnablingCondition<State> for RightsCycleReadyAction {
    fn is_enabled(&self, state: &State) -> bool {
        let _ = state;
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RightsGetCycleDataAction {
    pub key: RightsKey,
}

impl EnablingCondition<State> for RightsGetCycleDataAction {
    fn is_enabled(&self, state: &State) -> bool {
        let _ = state;
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RightsCycleDataReadyAction {
    pub key: RightsKey,
    pub cycle_data: CycleData,
}

impl EnablingCondition<State> for RightsCycleDataReadyAction {
    fn is_enabled(&self, state: &State) -> bool {
        let _ = state;
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RightsCalculateAction {
    pub key: RightsKey,
}

impl EnablingCondition<State> for RightsCalculateAction {
    fn is_enabled(&self, state: &State) -> bool {
        let _ = state;
        true
    }
}
