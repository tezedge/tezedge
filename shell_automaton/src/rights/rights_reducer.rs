// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::hash_map::Entry;

use crate::{Action, State};
use redux_rs::ActionWithMeta;

use super::{rights_actions::*, EndorsingRightsRequest};

pub fn rights_reducer(state: &mut State, action: &ActionWithMeta<Action>) {
    let requests = &mut state.rights.requests;
    match &action.action {
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
        Action::RightsEndorsingRightsInit(RightsEndorsingRightsInitAction { key })
            if !requests.contains_key(key) =>
        {
            requests.insert(
                key.clone(),
                EndorsingRightsRequest::Init { start: action.id },
            );
        }
        Action::RightsEndorsingRightsGetBlockHeader(
            RightsEndorsingRightsGetBlockHeaderAction { key },
        ) => {
            requests.get_mut(key).map(|request| {
                if let EndorsingRightsRequest::Init { start } = request {
                    *request = EndorsingRightsRequest::PendingBlockHeader { start: *start };
                }
            });
        }
        Action::RightsEndorsingRightsBlockHeaderReady(
            RightsEndorsingRightsBlockHeaderReadyAction { key, block_header },
        ) => {
            requests.get_mut(key).map(|request| {
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
            requests.get_mut(key).map(|request| {
                if let EndorsingRightsRequest::BlockHeaderReady {
                    block_header,
                    start,
                } = request
                {
                    *request = EndorsingRightsRequest::PendingProtocolHash {
                        block_header: block_header.clone(),
                        start: *start,
                    };
                }
            });
        }
        Action::RightsEndorsingRightsProtocolHashReady(
            RightsEndorsingRightsProtocolHashReadyAction { key, proto_hash },
        ) => {
            requests.get_mut(key).map(|request| {
                if let EndorsingRightsRequest::PendingProtocolHash {
                    block_header,
                    start,
                } = request
                {
                    *request = EndorsingRightsRequest::ProtocolHashReady {
                        proto_hash: proto_hash.clone(),
                        block_header: block_header.clone(),
                        start: *start,
                    };
                }
            });
        }
        Action::RightsEndorsingRightsGetProtocolConstants(
            RightsEndorsingRightsGetProtocolConstantsAction { key },
        ) => {
            requests.get_mut(key).map(|request| {
                if let EndorsingRightsRequest::ProtocolHashReady {
                    proto_hash,
                    block_header,
                    start,
                } = request
                {
                    *request = EndorsingRightsRequest::PendingProtocolConstants {
                        proto_hash: proto_hash.clone(),
                        block_header: block_header.clone(),
                        start: *start,
                    };
                }
            });
        }
        Action::RightsEndorsingRightsProtocolConstantsReady(
            RightsEndorsingRightsProtocolConstantsReadyAction { key, constants },
        ) => {
            requests.get_mut(key).map(|request| {
                if let EndorsingRightsRequest::PendingProtocolConstants {
                    proto_hash,
                    block_header,
                    start,
                } = request
                {
                    *request = EndorsingRightsRequest::ProtocolConstantsReady {
                        protocol_constants: constants.clone(),
                        proto_hash: proto_hash.clone(),
                        block_header: block_header.clone(),
                        start: *start,
                    };
                }
            });
        }
        Action::RightsEndorsingRightsGetCycleEras(RightsEndorsingRightsGetCycleErasAction {
            key,
        }) => {
            requests.get_mut(key).map(|request| {
                if let EndorsingRightsRequest::ProtocolConstantsReady {
                    protocol_constants,
                    proto_hash,
                    block_header,
                    start,
                } = request
                {
                    *request = EndorsingRightsRequest::PendingCycleEras {
                        protocol_constants: protocol_constants.clone(),
                        proto_hash: proto_hash.clone(),
                        block_header: block_header.clone(),
                        start: *start,
                    };
                }
            });
        }
        Action::RightsEndorsingRightsCycleErasReady(
            RightsEndorsingRightsCycleErasReadyAction { key, cycle_eras },
        ) => {
            requests.get_mut(key).map(|request| {
                if let EndorsingRightsRequest::PendingCycleEras {
                    protocol_constants,
                    block_header,
                    start,
                    ..
                } = request
                {
                    *request = EndorsingRightsRequest::CycleErasReady {
                        cycle_eras: cycle_eras.clone(),
                        protocol_constants: protocol_constants.clone(),
                        block_header: block_header.clone(),
                        start: *start,
                    };
                }
            });
        }
        Action::RightsEndorsingRightsGetCycle(RightsEndorsingRightsGetCycleAction { key }) => {
            requests.get_mut(key).map(|request| {
                if let EndorsingRightsRequest::CycleErasReady {
                    cycle_eras,
                    protocol_constants,
                    block_header,
                    start,
                } = request
                {
                    *request = EndorsingRightsRequest::PendingCycle {
                        cycle_eras: cycle_eras.clone(),
                        protocol_constants: protocol_constants.clone(),
                        block_header: block_header.clone(),
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
            requests.get_mut(key).map(|request| {
                if let EndorsingRightsRequest::PendingCycle {
                    protocol_constants,
                    block_header,
                    start,
                    ..
                } = request
                {
                    *request = EndorsingRightsRequest::CycleReady {
                        level: key.level.unwrap_or(block_header.level()),
                        cycle: *cycle,
                        position: *position,
                        protocol_constants: protocol_constants.clone(),
                        start: *start,
                    };
                }
            });
        }

        Action::RightsEndorsingRightsGetCycleData(RightsEndorsingRightsGetCycleDataAction {
            key,
        }) => {
            requests.get_mut(key).map(|request| {
                if let EndorsingRightsRequest::CycleReady {
                    level,
                    cycle,
                    position,
                    protocol_constants,
                    start,
                } = request
                {
                    *request = EndorsingRightsRequest::PendingCycleData {
                        level: *level,
                        cycle: *cycle,
                        position: *position,
                        protocol_constants: protocol_constants.clone(),
                        start: *start,
                    };
                }
            });
        }
        Action::RightsEndorsingRightsCycleDataReady(
            RightsEndorsingRightsCycleDataReadyAction { key, cycle_data },
        ) => {
            requests.get_mut(key).map(|request| {
                if let EndorsingRightsRequest::PendingCycleData {
                    level,
                    position,
                    protocol_constants,
                    start,
                    ..
                } = request
                {
                    *request = EndorsingRightsRequest::CycleDataReady {
                        level: *level,
                        position: *position,
                        protocol_constants: protocol_constants.clone(),
                        cycle_data: cycle_data.clone(),
                        start: *start,
                    };
                }
            });
        }
        Action::RightsEndorsingRightsCalculate(RightsEndorsingRightsCalculateAction { key }) => {
            requests.get_mut(key).map(|request| {
                if let EndorsingRightsRequest::CycleDataReady {
                    level,
                    cycle_data,
                    position,
                    protocol_constants,
                    start,
                    ..
                } = request
                {
                    *request = EndorsingRightsRequest::PendingRights {
                        level: *level,
                        cycle_data: cycle_data.clone(),
                        position: *position,
                        protocol_constants: protocol_constants.clone(),
                        start: *start,
                    };
                }
            });
        }
        Action::RightsEndorsingRightsReady(RightsEndorsingRightsReadyAction {
            key,
            endorsing_rights,
        }) => {
            if let Entry::Occupied(entry) = requests.entry(key.clone()) {
                if let EndorsingRightsRequest::PendingRights { .. } = entry.get() {
                    entry.remove();
                }
                let cache = &mut state.rights.cache.endorsing_rights;
                let duration = state.rights.cache.time;
                cache.retain(|_, (timestamp, _)| action.id.duration_since(*timestamp) < duration);
                slog::trace!(&state.log, "cached endorsing rights"; "level" => endorsing_rights.level);
                cache.insert(
                    endorsing_rights.level,
                    (action.id, endorsing_rights.clone()),
                );
            }
        }
        Action::RightsEndorsingRightsError(RightsEndorsingRightsErrorAction { key, error }) => {
            if let Entry::Occupied(mut entry) = requests.entry(key.clone()) {
                entry.insert(EndorsingRightsRequest::Error(error.clone()));
            }
        }
        _ => (),
    }
}
