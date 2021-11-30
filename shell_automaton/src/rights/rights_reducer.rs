// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::hash_map::Entry;

use crate::{Action, State};
use redux_rs::ActionWithMeta;

use super::{
    EndorsingRightsRequest, RightsEndorsingRightsBlockHeaderReadyAction,
    RightsEndorsingRightsCalculateAction, RightsEndorsingRightsCycleDataReadyAction,
    RightsEndorsingRightsCycleErasReadyAction, RightsEndorsingRightsCycleReadyAction,
    RightsEndorsingRightsErrorAction, RightsEndorsingRightsGetBlockHeaderAction,
    RightsEndorsingRightsGetCycleAction, RightsEndorsingRightsGetCycleDataAction,
    RightsEndorsingRightsGetCycleErasAction, RightsEndorsingRightsGetProtocolConstantsAction,
    RightsEndorsingRightsGetProtocolHashAction, RightsEndorsingRightsProtocolConstantsReadyAction,
    RightsEndorsingRightsProtocolHashReadyAction, RightsEndorsingRightsReadyAction,
    RightsGetEndorsingRightsAction, RightsRpcEndorsingRightsGetAction,
    RightsRpcEndorsingRightsPruneAction, RightsState,
};

pub fn rights_reducer(state: &mut State, action: &ActionWithMeta<Action>) {
    let endorsing_rights_state = &mut state.rights.endorsing_rights;
    match &action.action {
        // Main entry action
        Action::RightsGetEndorsingRights(RightsGetEndorsingRightsAction { key })
            if !endorsing_rights_state.contains_key(key) =>
        {
            endorsing_rights_state.insert(
                key.clone(),
                EndorsingRightsRequest::Init { start: action.id },
            );
        }

        // RPC actions
        Action::RightsRpcEndorsingRightsGet(RightsRpcEndorsingRightsGetAction { rpc_id, key })
            if !state.rights.rpc_requests.contains_key(rpc_id) =>
        {
            state
                .rights
                .rpc_requests
                .insert(rpc_id.clone(), key.clone());
        }
        Action::RightsRpcEndorsingRightsPrune(RightsRpcEndorsingRightsPruneAction { rpc_id })
            if state.rights.rpc_requests.contains_key(rpc_id) =>
        {
            state.rights.rpc_requests.remove(rpc_id);
        }

        // Auxiliary actions
        Action::RightsEndorsingRightsGetBlockHeader(
            RightsEndorsingRightsGetBlockHeaderAction { key },
        ) => {
            endorsing_rights_state.get_mut(key).map(|request| {
                if let EndorsingRightsRequest::Init { start } = request {
                    *request = EndorsingRightsRequest::PendingBlockHeader { start: *start };
                }
            });
        }
        Action::RightsEndorsingRightsBlockHeaderReady(
            RightsEndorsingRightsBlockHeaderReadyAction { key, block_header },
        ) => {
            endorsing_rights_state.get_mut(key).map(|request| {
                if let EndorsingRightsRequest::PendingBlockHeader { start } = request {
                    *request = EndorsingRightsRequest::BlockHeaderReady {
                        block_header: block_header.clone(),
                        start: *start,
                    };
                }
            });
        }
        Action::RightsEndorsingRightsGetProtocolHash(
            RightsEndorsingRightsGetProtocolHashAction { key },
        ) => {
            endorsing_rights_state.get_mut(key).map(|request| {
                if let EndorsingRightsRequest::BlockHeaderReady {
                    block_header: res_block_header,
                    start,
                } = request
                {
                    *request = EndorsingRightsRequest::PendingProtocolHash {
                        block_header: res_block_header.clone(),
                        start: *start,
                    };
                }
            });
        }
        Action::RightsEndorsingRightsProtocolHashReady(
            RightsEndorsingRightsProtocolHashReadyAction { key, proto_hash },
        ) => {
            endorsing_rights_state.get_mut(key).map(|request| {
                if let EndorsingRightsRequest::PendingProtocolHash {
                    block_header: data_block_header,
                    start,
                } = request
                {
                    *request = EndorsingRightsRequest::ProtocolHashReady {
                        proto_hash: proto_hash.clone(),
                        block_header: data_block_header.clone(),
                        start: *start,
                    };
                }
            });
        }
        Action::RightsEndorsingRightsGetProtocolConstants(
            RightsEndorsingRightsGetProtocolConstantsAction { key },
        ) => {
            endorsing_rights_state.get_mut(key).map(|request| {
                if let EndorsingRightsRequest::ProtocolHashReady {
                    proto_hash: res_proto_hash,
                    block_header: data_block_header,
                    start,
                } = request
                {
                    *request = EndorsingRightsRequest::PendingProtocolConstants {
                        proto_hash: res_proto_hash.clone(),
                        block_header: data_block_header.clone(),
                        start: *start,
                    };
                }
            });
        }
        Action::RightsEndorsingRightsProtocolConstantsReady(
            RightsEndorsingRightsProtocolConstantsReadyAction { key, constants },
        ) => {
            endorsing_rights_state.get_mut(key).map(|request| {
                if let EndorsingRightsRequest::PendingProtocolConstants {
                    proto_hash: data_proto_hash,
                    block_header: data_block_header,
                    start,
                } = request
                {
                    *request = EndorsingRightsRequest::ProtocolConstantsReady {
                        protocol_constants: constants.clone(),
                        proto_hash: data_proto_hash.clone(),
                        block_header: data_block_header.clone(),
                        start: *start,
                    };
                }
            });
        }
        Action::RightsEndorsingRightsGetCycleEras(RightsEndorsingRightsGetCycleErasAction {
            key,
        }) => {
            endorsing_rights_state.get_mut(key).map(|request| {
                if let EndorsingRightsRequest::ProtocolConstantsReady {
                    protocol_constants: res_protocol_constants,
                    proto_hash: data_proto_hash,
                    block_header: data_block_header,
                    start,
                } = request
                {
                    *request = EndorsingRightsRequest::PendingCycleEras {
                        protocol_constants: res_protocol_constants.clone(),
                        proto_hash: data_proto_hash.clone(),
                        block_header: data_block_header.clone(),
                        start: *start,
                    };
                }
            });
        }
        Action::RightsEndorsingRightsCycleErasReady(
            RightsEndorsingRightsCycleErasReadyAction { key, cycle_eras },
        ) => {
            endorsing_rights_state.get_mut(key).map(|request| {
                if let EndorsingRightsRequest::PendingCycleEras {
                    protocol_constants: data_protocol_constants,
                    block_header: data_block_header,
                    start,
                    ..
                } = request
                {
                    *request = EndorsingRightsRequest::CycleErasReady {
                        cycle_eras: cycle_eras.clone(),
                        protocol_constants: data_protocol_constants.clone(),
                        block_header: data_block_header.clone(),
                        start: *start,
                    };
                }
            });
        }
        Action::RightsEndorsingRightsGetCycle(RightsEndorsingRightsGetCycleAction { key }) => {
            endorsing_rights_state.get_mut(key).map(|request| {
                if let EndorsingRightsRequest::CycleErasReady {
                    cycle_eras: res_cycle_eras,
                    protocol_constants: data_protocol_constants,
                    block_header: data_block_header,
                    start,
                } = request
                {
                    *request = EndorsingRightsRequest::PendingCycle {
                        cycle_eras: res_cycle_eras.clone(),
                        protocol_constants: data_protocol_constants.clone(),
                        block_header: data_block_header.clone(),
                        start: *start,
                    };
                }
            });
        }
        Action::RightsEndorsingRightsCycleReady(RightsEndorsingRightsCycleReadyAction {
            key,
            cycle,
            position,
        }) => {
            endorsing_rights_state.get_mut(key).map(|request| {
                if let EndorsingRightsRequest::PendingCycle {
                    protocol_constants: data_protocol_constants,
                    start,
                    ..
                } = request
                {
                    *request = EndorsingRightsRequest::CycleReady {
                        cycle: *cycle,
                        position: *position,
                        protocol_constants: data_protocol_constants.clone(),
                        start: *start,
                    };
                }
            });
        }

        Action::RightsEndorsingRightsGetCycleData(RightsEndorsingRightsGetCycleDataAction {
            key,
        }) => {
            endorsing_rights_state.get_mut(key).map(|request| {
                if let EndorsingRightsRequest::CycleReady {
                    cycle: res_cycle,
                    position: res_position,
                    protocol_constants: data_protocol_constants,
                    start,
                } = request
                {
                    *request = EndorsingRightsRequest::PendingCycleData {
                        cycle: *res_cycle,
                        position: *res_position,
                        protocol_constants: data_protocol_constants.clone(),
                        start: *start,
                    };
                }
            });
        }
        Action::RightsEndorsingRightsCycleDataReady(
            RightsEndorsingRightsCycleDataReadyAction { key, cycle_data },
        ) => {
            endorsing_rights_state.get_mut(key).map(|request| {
                if let EndorsingRightsRequest::PendingCycleData {
                    position: data_position,
                    protocol_constants: data_protocol_constants,
                    start,
                    ..
                } = request
                {
                    *request = EndorsingRightsRequest::CycleDataReady {
                        cycle_data: cycle_data.clone(),
                        position: *data_position,
                        protocol_constants: data_protocol_constants.clone(),
                        start: *start,
                    };
                }
            });
        }
        Action::RightsEndorsingRightsCalculate(RightsEndorsingRightsCalculateAction { key }) => {
            endorsing_rights_state.get_mut(key).map(|request| {
                if let EndorsingRightsRequest::CycleDataReady {
                    cycle_data: res_cycle_data,
                    position: data_position,
                    protocol_constants: data_protocol_constants,
                    start,
                    ..
                } = request
                {
                    *request = EndorsingRightsRequest::PendingRights {
                        cycle_data: res_cycle_data.clone(),
                        position: *data_position,
                        protocol_constants: data_protocol_constants.clone(),
                        start: *start,
                    };
                }
            });
        }
        Action::RightsEndorsingRightsReady(RightsEndorsingRightsReadyAction {
            key,
            endorsing_rights,
        }) if RightsState::should_cache_endorsing_rights(key) => {
            endorsing_rights_state.get_mut(key).map(|request| {
                if let EndorsingRightsRequest::PendingRights { .. } = request {
                    *request = EndorsingRightsRequest::Ready(endorsing_rights.clone());
                }
            });
        }
        Action::RightsEndorsingRightsReady(RightsEndorsingRightsReadyAction { key, .. }) => {
            if let Entry::Occupied(entry) = endorsing_rights_state.entry(key.clone()) {
                if let EndorsingRightsRequest::PendingRights { .. } = entry.get() {
                    entry.remove();
                }
            }
        }
        Action::RightsEndorsingRightsError(RightsEndorsingRightsErrorAction { key, error }) => {
            if let Entry::Occupied(mut entry) = endorsing_rights_state.entry(key.clone()) {
                entry.insert(EndorsingRightsRequest::Error(error.clone()));
            }
        }
        _ => (),
    }
}
