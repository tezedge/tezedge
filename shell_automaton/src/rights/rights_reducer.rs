// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::hash_map::Entry;

use crate::{Action, State};
use redux_rs::ActionWithMeta;

use super::{rights_actions::*, RightsRequest};

pub fn rights_reducer(state: &mut State, action: &ActionWithMeta<Action>) {
    let requests = &mut state.rights.requests;
    match &action.action {
        // RPC actions
        Action::RightsRpcGet(RightsRpcGetAction { key, rpc_id }) => {
            state
                .rights
                .rpc_requests
                .entry(key.clone())
                .or_default()
                .push(*rpc_id);
        }
        Action::RightsPruneRpcRequest(RightsRpcPruneAction { key }) => {
            state.rights.rpc_requests.remove(key);
        }

        // Auxiliary actions
        Action::RightsInit(RightsInitAction { key }) if !requests.contains_key(key) => {
            requests.insert(key.clone(), RightsRequest::Init { start: action.id });
        }
        Action::RightsGetBlockHeader(RightsGetBlockHeaderAction { key }) => {
            requests.get_mut(key).map(|request| {
                if let RightsRequest::Init { start } = request {
                    *request = RightsRequest::PendingBlockHeader { start: *start };
                }
            });
        }
        Action::RightsBlockHeaderReady(RightsBlockHeaderReadyAction { key, block_header }) => {
            requests.get_mut(key).map(|request| {
                if let RightsRequest::PendingBlockHeader { start } = request {
                    *request = RightsRequest::BlockHeaderReady {
                        start: *start,
                        block_header: block_header.clone(),
                    };
                }
            });
        }
        Action::RightsGetProtocolHash(RightsGetProtocolHashAction { key }) => {
            requests.get_mut(key).map(|request| {
                if let RightsRequest::BlockHeaderReady {
                    start,
                    block_header,
                } = request
                {
                    *request = RightsRequest::PendingProtocolHash {
                        start: *start,
                        block_header: block_header.clone(),
                    };
                }
            });
        }
        Action::RightsProtocolHashReady(RightsProtocolHashReadyAction {
            key,
            proto_hash,
            protocol,
        }) => {
            requests.get_mut(key).map(|request| {
                if let RightsRequest::PendingProtocolHash {
                    start,
                    block_header,
                } = request
                {
                    *request = RightsRequest::ProtocolHashReady {
                        start: *start,
                        block_header: block_header.clone(),
                        proto_hash: proto_hash.clone(),
                        protocol: protocol.clone(),
                    };
                }
            });
        }
        Action::RightsGetProtocolConstants(RightsGetProtocolConstantsAction { key }) => {
            requests.get_mut(key).map(|request| {
                if let RightsRequest::ProtocolHashReady {
                    start,
                    block_header,
                    proto_hash,
                    protocol,
                } = request
                {
                    *request = RightsRequest::PendingProtocolConstants {
                        start: *start,
                        block_header: block_header.clone(),
                        proto_hash: proto_hash.clone(),
                        protocol: protocol.clone(),
                    };
                }
            });
        }
        Action::RightsProtocolConstantsReady(RightsProtocolConstantsReadyAction {
            key,
            constants,
        }) => {
            requests.get_mut(key).map(|request| {
                if let RightsRequest::PendingProtocolConstants {
                    start,
                    block_header,
                    proto_hash,
                    protocol,
                } = request
                {
                    *request = RightsRequest::ProtocolConstantsReady {
                        start: *start,
                        block_header: block_header.clone(),
                        proto_hash: proto_hash.clone(),
                        protocol: protocol.clone(),
                        protocol_constants: constants.clone(),
                    };
                }
            });
        }
        Action::RightsGetCycleEras(RightsGetCycleErasAction { key }) => {
            requests.get_mut(key).map(|request| {
                if let RightsRequest::ProtocolConstantsReady {
                    start,
                    block_header,
                    proto_hash,
                    protocol,
                    protocol_constants,
                } = request
                {
                    *request = RightsRequest::PendingCycleEras {
                        start: *start,
                        block_header: block_header.clone(),
                        proto_hash: proto_hash.clone(),
                        protocol: protocol.clone(),
                        protocol_constants: protocol_constants.clone(),
                    };
                }
            });
        }
        Action::RightsCycleErasReady(RightsCycleErasReadyAction { key, cycle_eras }) => {
            requests.get_mut(key).map(|request| {
                if let RightsRequest::PendingCycleEras {
                    start,
                    block_header,
                    proto_hash: _,
                    protocol,
                    protocol_constants,
                } = request
                {
                    *request = RightsRequest::CycleErasReady {
                        start: *start,
                        block_header: block_header.clone(),
                        protocol: protocol.clone(),
                        protocol_constants: protocol_constants.clone(),
                        cycle_eras: cycle_eras.clone(),
                    };
                }
            });
        }
        Action::RightsGetCycle(RightsGetCycleAction { key }) => {
            requests.get_mut(key).map(|request| {
                if let RightsRequest::CycleErasReady {
                    start,
                    block_header,
                    protocol,
                    protocol_constants,
                    cycle_eras,
                } = request
                {
                    *request = RightsRequest::PendingCycle {
                        start: *start,
                        block_header: block_header.clone(),
                        protocol: protocol.clone(),
                        protocol_constants: protocol_constants.clone(),
                        cycle_eras: cycle_eras.clone(),
                    };
                }
            });
        }
        Action::RightsCycleReady(RightsCycleReadyAction {
            key,
            cycle,
            position,
        }) => {
            requests.get_mut(key).map(|request| {
                if let RightsRequest::PendingCycle {
                    start,
                    block_header,
                    protocol,
                    protocol_constants,
                    cycle_eras: _,
                } = request
                {
                    *request = RightsRequest::CycleReady {
                        start: *start,
                        protocol: protocol.clone(),
                        protocol_constants: protocol_constants.clone(),
                        level: key.level().unwrap_or(block_header.level()),
                        cycle: *cycle,
                        position: *position,
                    };
                }
            });
        }

        Action::RightsGetCycleData(RightsGetCycleDataAction { key }) => {
            requests.get_mut(key).map(|request| {
                if let RightsRequest::CycleReady {
                    start,
                    protocol,
                    protocol_constants,
                    level,
                    cycle,
                    position,
                } = request
                {
                    *request = RightsRequest::PendingCycleData {
                        start: *start,
                        protocol: protocol.clone(),
                        protocol_constants: protocol_constants.clone(),
                        level: *level,
                        cycle: *cycle,
                        position: *position,
                    };
                }
            });
        }
        Action::RightsCycleDataReady(RightsCycleDataReadyAction { key, cycle_data }) => {
            requests.get_mut(key).map(|request| {
                if let RightsRequest::PendingCycleData {
                    start,
                    protocol,
                    protocol_constants,
                    level,
                    cycle: _,
                    position,
                } = request
                {
                    *request = RightsRequest::CycleDataReady {
                        start: *start,
                        protocol: protocol.clone(),
                        protocol_constants: protocol_constants.clone(),
                        level: *level,
                        position: *position,
                        cycle_data: cycle_data.clone(),
                    };
                }
            });
        }
        Action::RightsCalculateEndorsingRights(RightsCalculateAction { key }) => {
            requests.get_mut(key).map(|request| {
                if let RightsRequest::CycleDataReady {
                    start,
                    protocol,
                    protocol_constants,
                    level,
                    cycle_data,
                    position,
                } = request
                {
                    *request = RightsRequest::PendingRightsCalculation {
                        start: *start,
                        protocol: protocol.clone(),
                        protocol_constants: protocol_constants.clone(),
                        level: *level,
                        cycle_data: cycle_data.clone(),
                        position: *position,
                    };
                }
            });
        }
        Action::RightsBakingReady(RightsBakingReadyAction { key, baking_rights }) => {
            if let Some(RightsRequest::PendingRightsCalculation { .. }) = requests.remove(key) {
                let cache = &mut state.rights.cache.baking;
                let duration = state.rights.cache.time;
                cache.retain(|_, (timestamp, _)| action.id.duration_since(*timestamp) < duration);
                slog::trace!(&state.log, "cached endorsing rights"; "level" => baking_rights.level);
                cache.insert(baking_rights.level, (action.id, baking_rights.clone()));
            }
        }
        Action::RightsEndorsingReady(RightsEndorsingReadyAction {
            key,
            endorsing_rights,
        }) => {
            if let Entry::Occupied(entry) = requests.entry(key.clone()) {
                if let RightsRequest::PendingRightsCalculation { .. } = entry.get() {
                    entry.remove();
                }
                let cache = &mut state.rights.cache.endorsing;
                let duration = state.rights.cache.time;
                cache.retain(|_, (timestamp, _)| action.id.duration_since(*timestamp) < duration);
                slog::trace!(&state.log, "cached endorsing rights"; "level" => endorsing_rights.level);
                cache.insert(
                    endorsing_rights.level,
                    (action.id, endorsing_rights.clone()),
                );
            }
        }
        Action::RightsError(RightsErrorAction { key, error }) => {
            if let Some(request) = requests.remove(key) {
                state
                    .rights
                    .errors
                    .push((key.clone().into(), request, error.clone()));
            }
        }
        _ => (),
    }
}
