// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::hash_map::Entry;

use crate::{Action, State};
use redux_rs::ActionWithMeta;

use super::{
    cycle_delegates::{CycleDelegatesQuery, CycleDelegatesQueryState},
    rights_actions::*,
    RightsRequest, Validators,
};

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
            if let Some(request) = requests.get_mut(key) {
                if let RightsRequest::Init { start } = request {
                    *request = RightsRequest::PendingBlockHeader { start: *start };
                }
            }
        }
        Action::RightsBlockHeaderReady(RightsBlockHeaderReadyAction { key, block_header }) => {
            if let Some(request) = requests.get_mut(key) {
                if let RightsRequest::PendingBlockHeader { start } = request {
                    *request = RightsRequest::BlockHeaderReady {
                        start: *start,
                        block_header: block_header.clone(),
                    };
                }
            }
        }
        Action::RightsGetProtocolHash(RightsGetProtocolHashAction { key }) => {
            if let Some(request) = requests.get_mut(key) {
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
            }
        }
        Action::RightsProtocolHashReady(RightsProtocolHashReadyAction {
            key,
            proto_hash,
            protocol,
        }) => {
            if let Some(request) = requests.get_mut(key) {
                if let RightsRequest::PendingProtocolHash {
                    start,
                    block_header,
                } = request
                {
                    *request = RightsRequest::ProtocolHashReady {
                        start: *start,
                        block_header: block_header.clone(),
                        proto_hash: proto_hash.clone(),
                        protocol: *protocol,
                    };
                }
            }
        }
        Action::RightsGetProtocolConstants(RightsGetProtocolConstantsAction { key }) => {
            if let Some(request) = requests.get_mut(key) {
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
                        protocol: *protocol,
                    };
                }
            }
        }
        Action::RightsProtocolConstantsReady(RightsProtocolConstantsReadyAction {
            key,
            constants,
        }) => {
            if let Some(request) = requests.get_mut(key) {
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
                        protocol: *protocol,
                        protocol_constants: constants.clone(),
                    };
                }
            }
        }
        Action::RightsGetCycleEras(RightsGetCycleErasAction { key }) => {
            if let Some(request) = requests.get_mut(key) {
                if let RightsRequest::ProtocolConstantsReady {
                    start,
                    block_header,
                    proto_hash,
                    protocol,
                    protocol_constants,
                } = request
                {
                    *request =
                        if let Some(cycle_eras) = state.rights.cycle_eras.get_result(proto_hash) {
                            RightsRequest::CycleErasReady {
                                start: *start,
                                block_header: block_header.clone(),
                                protocol: *protocol,
                                protocol_constants: protocol_constants.clone(),
                                cycle_eras: cycle_eras.clone(),
                            }
                        } else {
                            RightsRequest::PendingCycleEras {
                                start: *start,
                                block_header: block_header.clone(),
                                proto_hash: proto_hash.clone(),
                                protocol: *protocol,
                                protocol_constants: protocol_constants.clone(),
                            }
                        };
                }
            }
        }
        Action::RightsCycleErasReady(RightsCycleErasReadyAction { key, cycle_eras }) => {
            if let Some(request) = requests.get_mut(key) {
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
                        protocol: *protocol,
                        protocol_constants: protocol_constants.clone(),
                        cycle_eras: cycle_eras.clone(),
                    };
                }
            }
        }
        Action::RightsGetCycle(RightsGetCycleAction { key }) => {
            if let Some(request) = requests.get_mut(key) {
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
                        protocol: *protocol,
                        protocol_constants: protocol_constants.clone(),
                        cycle_eras: cycle_eras.clone(),
                    };
                }
            }
        }
        Action::RightsCycleReady(RightsCycleReadyAction {
            key,
            cycle,
            position,
        }) => {
            if let Some(request) = requests.get_mut(key) {
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
                        block_header: block_header.clone(),
                        protocol: *protocol,
                        protocol_constants: protocol_constants.clone(),
                        level: key.level().unwrap_or_else(|| block_header.level()),
                        cycle: *cycle,
                        position: *position,
                    };
                }
            }
        }

        Action::RightsGetCycleData(RightsGetCycleDataAction { key }) => {
            if let Some(request) = requests.get_mut(key) {
                if let RightsRequest::CycleReady {
                    start,
                    block_header: _,
                    protocol,
                    protocol_constants,
                    level,
                    cycle,
                    position,
                } = request
                {
                    *request = RightsRequest::PendingCycleData {
                        start: *start,
                        protocol: *protocol,
                        protocol_constants: protocol_constants.clone(),
                        level: *level,
                        cycle: *cycle,
                        position: *position,
                    };
                }
            }
        }
        Action::RightsCycleDataReady(RightsCycleDataReadyAction { key, cycle_data }) => {
            if let Some(request) = requests.get_mut(key) {
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
                        protocol: *protocol,
                        protocol_constants: protocol_constants.clone(),
                        level: *level,
                        position: *position,
                        cycle_data: cycle_data.clone(),
                    };
                }
            }
        }
        Action::RightsCalculateEndorsingRights(RightsCalculateAction { key }) => {
            if let Some(request) = requests.get_mut(key) {
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
                        protocol: *protocol,
                        protocol_constants: protocol_constants.clone(),
                        level: *level,
                        cycle_data: cycle_data.clone(),
                        position: *position,
                    };
                }
            }
        }

        Action::RightsGetCycleDelegates(RightsGetCycleDelegatesAction { key }) => {
            if let Some(request) = requests.get_mut(key) {
                if let RightsRequest::CycleReady {
                    start,
                    block_header,
                    level,
                    cycle,
                    ..
                } = request
                {
                    let req = state.rights.cycle_delegates.get(cycle);
                    *request = if let Some(CycleDelegatesQuery {
                        state: CycleDelegatesQueryState::Success(delegates),
                    }) = req
                    {
                        RightsRequest::CycleDelegatesReady {
                            start: *start,
                            block_header: block_header.clone(),
                            level: *level,
                            delegates: delegates.clone(),
                        }
                    } else {
                        RightsRequest::PendingCycleDelegates {
                            start: *start,
                            block_header: block_header.clone(),
                            level: *level,
                            cycle: *cycle,
                        }
                    };
                }
            }
        }
        Action::RightsCycleDelegatesReady(RightsCycleDelegatesReadyAction { key, delegates }) => {
            if let Some(request) = requests.get_mut(key) {
                if let RightsRequest::PendingCycleDelegates {
                    start,
                    block_header,
                    level,
                    cycle: _,
                } = request
                {
                    *request = RightsRequest::CycleDelegatesReady {
                        start: *start,
                        block_header: block_header.clone(),
                        level: *level,
                        delegates: delegates.clone(),
                    };
                }
            }
        }

        Action::RightsCalculateIthaca(RightsCalculateIthacaAction { key }) => {
            if let Some(request) = requests.get_mut(key) {
                if let RightsRequest::CycleDelegatesReady {
                    start,
                    block_header,
                    level,
                    delegates,
                } = request
                {
                    *request = RightsRequest::PendingRightsCalculationIthaca {
                        start: *start,
                        block_header: block_header.clone(),
                        level: *level,
                        delegates: delegates.clone(),
                    };
                }
            }
        }
        Action::RightsContextRequested(RightsContextRequestedAction { key, token }) => {
            if let Some(request) = requests.get_mut(key) {
                if let RightsRequest::PendingRightsCalculationIthaca {
                    start,
                    block_header: _,
                    level,
                    delegates,
                } = request
                {
                    *request = RightsRequest::PendingRightsFromContextIthaca {
                        start: *start,
                        level: *level,
                        delegates: delegates.clone(),
                        token: *token,
                    };
                }
            }
        }
        Action::RightsIthacaContextValidatorsSuccess(
            RightsIthacaContextValidatorsSuccessAction {
                key,
                validators: context_validators,
            },
        ) => {
            if let Some(request) = requests.get_mut(key) {
                if let RightsRequest::PendingRightsFromContextIthaca {
                    start: _,
                    level,
                    delegates,
                    ..
                } = request
                {
                    let validators_by_pk = context_validators
                        .validators
                        .iter()
                        .filter_map(|pkh| delegates.get(pkh).cloned())
                        .collect();
                    let slots_by_pk = context_validators
                        .slots
                        .iter()
                        .filter_map(|(pkh, rights)| {
                            delegates.get(pkh).cloned().map(|pk| (pk, rights.clone()))
                        })
                        .collect();
                    *request = RightsRequest::ValidatorsReady(Validators {
                        level: *level,
                        validators: context_validators.validators.clone(),
                        slots: context_validators.slots.clone(),
                        validators_by_pk,
                        slots_by_pk,
                    });
                }
            }
        }

        Action::RightsBakingOldReady(RightsBakingOldReadyAction { key, baking_rights }) => {
            if let Some(RightsRequest::PendingRightsCalculation { .. }) = requests.remove(key) {
                let cache = &mut state.rights.cache.baking;
                let duration = state.rights.cache.time;
                cache.retain(|_, (timestamp, _)| action.id.duration_since(*timestamp) < duration);
                slog::trace!(&state.log, "cached endorsing rights"; "level" => baking_rights.level);
                cache.insert(baking_rights.level, (action.id, baking_rights.clone()));
            }
        }
        Action::RightsEndorsingOldReady(RightsEndorsingOldReadyAction {
            key,
            endorsing_rights,
        }) => {
            if let Entry::Occupied(entry) = requests.entry(key.clone()) {
                if let RightsRequest::PendingRightsCalculation { .. } = entry.get() {
                    entry.remove();
                }
                let cache = &mut state.rights.cache.endorsing_old;
                let duration = state.rights.cache.time;
                cache.retain(|_, (timestamp, _)| action.id.duration_since(*timestamp) < duration);
                slog::trace!(&state.log, "cached endorsing rights"; "level" => endorsing_rights.level);
                cache.insert(
                    endorsing_rights.level,
                    (action.id, endorsing_rights.clone()),
                );
            }
        }
        Action::RightsValidatorsReady(RightsValidatorsReadyAction { key }) => {
            if let Entry::Occupied(entry) = requests.entry(key.clone()) {
                if let RightsRequest::ValidatorsReady(validators) = entry.get() {
                    let validators = validators.clone();
                    entry.remove();
                    let cache = &mut state.rights.cache.validators;
                    let duration = state.rights.cache.time;
                    cache.retain(|_, (timestamp, _)| {
                        action.id.duration_since(*timestamp) < duration
                    });
                    slog::trace!(&state.log, "cached endorsing rights"; "level" => validators.level);
                    cache.insert(validators.level, (action.id, validators));
                }
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
