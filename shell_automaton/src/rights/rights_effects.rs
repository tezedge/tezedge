// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    collections::BTreeMap,
    convert::{TryFrom, TryInto},
    num::TryFromIntError,
    time::Instant,
};

use slog::{error, trace, warn, FnValue};
use storage::cycle_storage::CycleData;
use tezos_api::ffi::{ProtocolRpcRequest, RpcMethod, RpcRequest};
use tezos_messages::{
    base::{
        signature_public_key::{SignaturePublicKey, SignaturePublicKeyHash},
        ConversionError,
    },
    protocol::SupportedProtocol,
};

use crate::{
    rights::{BakingRightsOld, Delegate},
    service::{protocol_runner_service::ProtocolRunnerResult, ProtocolRunnerService},
};
use crate::{
    service::RpcService,
    storage::{
        kv_block_additional_data, kv_block_header, kv_constants,
        kv_cycle_meta::{self, CycleKey},
    },
    Action, ActionWithMeta, Service, Store,
};

use super::{
    cycle_delegates::rights_cycle_delegates_actions::*,
    cycle_eras::rights_cycle_eras_actions::*,
    rights_actions::*,
    utils::{baking_rights_owner, endorser_rights_owner, get_cycle, Position, TezosPRNGError},
    EndorsingRightsOld, ProtocolConstants, RightsInput, RightsRequest, RightsRpcError, Slot,
};

pub fn rights_effects<S>(store: &mut Store<S>, action: &ActionWithMeta)
where
    S: Service,
{
    let requests = &store.state.get().rights.requests;
    let cache = &store.state.get().rights.cache;
    let log = &store.state.get().log;
    match &action.action {
        // Main entry action
        Action::RightsGet(RightsGetAction { key }) => {
            match &key.0 {
                RightsInput::Baking(_) => {
                    if let Some((_, baking_rights)) = key
                        .level()
                        .as_ref()
                        .and_then(|level| cache.baking.get(level))
                        .cloned()
                    {
                        trace!(log, "Baking rights using cache"; "key" => FnValue(|_| format!("{:?}", key)));
                        store.dispatch(RightsBakingOldReadyAction {
                            key: key.clone(),
                            baking_rights,
                        });
                        return;
                    }
                }
                RightsInput::Endorsing(_) => {
                    if let Some((_, _)) = key
                        .level()
                        .as_ref()
                        .and_then(|level| cache.endorsing.get(level))
                    {
                        trace!(log, "Endorsing rights using cache"; "key" => FnValue(|_| format!("{:?}", key)));
                        store.dispatch(RightsEndorsingReadyAction {
                            key: key.clone(),
                        });
                        return;
                    }
                }
            }
            if !requests.contains_key(key) {
                trace!(log, "Endorsing rights using full cycle"; "key" => FnValue(|_| format!("{:?}", key)));
                store.dispatch(RightsInitAction { key: key.clone() });
            } else {
                trace!(log, "Endorsing rights already in progress"; "key" => FnValue(|_| format!("{:?}", key)));
            }
        }

        // RPC actions
        Action::RightsRpcGet(RightsRpcGetAction { key, .. }) => {
            store.dispatch(RightsGetAction { key: key.clone() });
        }
        Action::RightsBakingOldReady(RightsBakingOldReadyAction { baking_rights, key }) => {
            for rpc_id in store
                .state
                .get()
                .rights
                .rpc_requests
                .get(key)
                .cloned()
                .unwrap_or_default()
            {
                match baking_rights
                    .priorities
                    .iter()
                    .enumerate()
                    .map(|(priority, delegate)| {
                        Ok(BakingRightsPriority {
                            priority: priority.try_into().map_err(|_| RightsRpcError::Num)?,
                            delegate: SignaturePublicKeyHash::try_from(delegate.clone())?,
                        })
                    })
                    .collect::<Result<_, RightsRpcError>>()
                {
                    Ok(baking_rights) => store.dispatch(RightsRpcBakingReadyAction {
                        rpc_id,
                        baking_rights,
                    }),

                    Err(err) => store.dispatch(RightsRpcErrorAction { rpc_id, error: err }),
                };
            }
            store.dispatch(RightsRpcPruneAction { key: key.clone() });
        }
        Action::RightsEndorsingOldReady(RightsEndorsingOldReadyAction {
            endorsing_rights,
            key,
        }) => {
            for rpc_id in store
                .state
                .get()
                .rights
                .rpc_requests
                .get(key)
                .cloned()
                .unwrap_or_default()
            {
                match endorsing_rights
                    .delegate_to_slots
                    .iter()
                    .map(|(delegate, slots)| {
                        Ok((
                            SignaturePublicKeyHash::try_from(delegate.clone())?,
                            slots.clone(),
                        ))
                    })
                    .collect::<Result<_, RightsRpcError>>()
                {
                    Ok(endorsing_rights) => store.dispatch(RightsRpcEndorsingReadyAction {
                        rpc_id,
                        endorsing_rights,
                    }),

                    Err(err) => store.dispatch(RightsRpcErrorAction { rpc_id, error: err }),
                };
            }
            store.dispatch(RightsRpcPruneAction { key: key.clone() });
        }
        Action::RightsRpcBakingReady(RightsRpcBakingReadyAction {
            rpc_id,
            baking_rights,
        }) => store.service.rpc().respond(*rpc_id, baking_rights.clone()),
        Action::RightsRpcEndorsingReady(RightsRpcEndorsingReadyAction {
            rpc_id,
            endorsing_rights,
        }) => store
            .service
            .rpc()
            .respond(*rpc_id, endorsing_rights.clone()),
        Action::RightsRpcError(RightsRpcErrorAction { rpc_id, error }) => {
            store.service.rpc().respond(*rpc_id, error.clone())
        }

        Action::RightsInit(RightsInitAction { key }) => {
            store.dispatch(RightsGetBlockHeaderAction { key: key.clone() });
        }
        // get block header from kv store
        Action::RightsGetBlockHeader(RightsGetBlockHeaderAction { key }) => {
            if let Some(RightsRequest::PendingBlockHeader { .. }) = requests.get(key) {
                store.dispatch(kv_block_header::StorageBlockHeaderGetAction::new(
                    key.block().clone(),
                ));
            }
        }
        Action::StorageBlockHeaderOk(kv_block_header::StorageBlockHeaderOkAction {
            key,
            value,
        }) => {
            for key in requests
                .iter()
                .filter_map(|(rights_key, _)| {
                    if rights_key.block() == key {
                        Some(rights_key)
                    } else {
                        None
                    }
                })
                .cloned()
                .collect::<Vec<_>>()
            {
                store.dispatch(RightsBlockHeaderReadyAction {
                    key,
                    block_header: value.clone(),
                });
            }
        }
        Action::StorageBlockHeaderError(kv_block_header::StorageBlockHeaderErrorAction {
            key,
            error,
        }) => {
            for key in requests
                .iter()
                .filter_map(|(rights_key, _)| {
                    if rights_key.block() == key {
                        Some(rights_key)
                    } else {
                        None
                    }
                })
                .cloned()
                .collect::<Vec<_>>()
            {
                store.dispatch(RightsErrorAction {
                    key,
                    error: error.clone().into(),
                });
            }
        }
        Action::RightsBlockHeaderReady(RightsBlockHeaderReadyAction { key, .. }) => {
            if let Some(RightsRequest::BlockHeaderReady { .. }) = requests.get(key) {
                store.dispatch(RightsGetProtocolHashAction { key: key.clone() });
            }
        }

        // get protocol hash as a part of additional block data from kv store
        Action::RightsGetProtocolHash(RightsGetProtocolHashAction { key, .. }) => {
            if let Some(RightsRequest::PendingProtocolHash { .. }) = requests.get(key) {
                store.dispatch(
                    kv_block_additional_data::StorageBlockAdditionalDataGetAction::new(
                        key.block().clone(),
                    ),
                );
            }
        }
        Action::StorageBlockAdditionalDataOk(
            kv_block_additional_data::StorageBlockAdditionalDataOkAction { key, value },
        ) => {
            let proto_hash = &value.next_protocol_hash;
            let rights_keys: Vec<_> = requests
                .iter()
                .filter_map(|(rights_key, _)| {
                    if rights_key.block() == key {
                        Some(rights_key)
                    } else {
                        None
                    }
                })
                .cloned()
                .collect();
            match SupportedProtocol::try_from(proto_hash) {
                Ok(protocol) => {
                    for rights_key in rights_keys {
                        store.dispatch(RightsProtocolHashReadyAction {
                            key: rights_key.clone(),
                            proto_hash: proto_hash.clone(),
                            protocol: protocol.clone(),
                        });
                    }
                }
                Err(err) => {
                    for rights_key in rights_keys {
                        store.dispatch(RightsErrorAction {
                            key: rights_key.clone(),
                            error: err.clone().into(),
                        });
                    }
                }
            };
        }
        Action::StorageBlockAdditionalDataError(
            kv_block_additional_data::StorageBlockAdditionalDataErrorAction { key, error },
        ) => {
            for key in requests
                .iter()
                .filter_map(|(rights_key, _)| {
                    if rights_key.block() == key {
                        Some(rights_key)
                    } else {
                        None
                    }
                })
                .cloned()
                .collect::<Vec<_>>()
            {
                store.dispatch(RightsErrorAction {
                    key,
                    error: error.clone().into(),
                });
            }
        }
        Action::RightsProtocolHashReady(RightsProtocolHashReadyAction { key, .. }) => {
            if let Some(RightsRequest::ProtocolHashReady { .. }) = requests.get(key) {
                store.dispatch(RightsGetProtocolConstantsAction { key: key.clone() });
            }
        }

        // get protocol constants from kv store
        Action::RightsGetProtocolConstants(RightsGetProtocolConstantsAction { key }) => {
            if let Some(RightsRequest::PendingProtocolConstants { proto_hash, .. }) =
                requests.get(key)
            {
                let key = proto_hash.clone();
                store.dispatch(kv_constants::StorageConstantsGetAction::new(key));
            }
        }
        Action::StorageConstantsOk(kv_constants::StorageConstantsOkAction { key, value }) => {
            for key in requests
                .iter()
                .filter_map(|(rights_key, request)| {
                    if let RightsRequest::PendingProtocolConstants {
                        proto_hash: data_proto_hash,
                        ..
                    } = request
                    {
                        if data_proto_hash == key {
                            Some(rights_key)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .cloned()
                .collect::<Vec<_>>()
            {
                match serde_json::from_str::<ProtocolConstants>(value) {
                    Ok(constants) => {
                        store.dispatch(RightsProtocolConstantsReadyAction { key, constants });
                    }
                    Err(err) => {
                        slog::warn!(store.state().log, "Error parsing constants value"; "value" => value);
                        store.dispatch(RightsErrorAction {
                            key,
                            error: err.into(),
                        });
                    }
                }
            }
        }
        Action::StorageConstantsError(kv_constants::StorageConstantsErrorAction { key, error }) => {
            let rights_keys: Vec<_> = requests
                .iter()
                .filter_map(|(rights_key, request)| {
                    if let RightsRequest::PendingProtocolConstants {
                        proto_hash: data_proto_hash,
                        ..
                    } = request
                    {
                        if data_proto_hash == key {
                            Some(rights_key)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .cloned()
                .collect();
            for rights_key in rights_keys {
                store.dispatch(RightsErrorAction {
                    key: rights_key,
                    error: error.clone().into(),
                });
            }
        }
        Action::RightsProtocolConstantsReady(RightsProtocolConstantsReadyAction {
            key, ..
        }) => {
            if let Some(RightsRequest::ProtocolConstantsReady {
                ..
            }) = requests.get(key)
            {
                store.dispatch(RightsGetCycleErasAction { key: key.clone() });
            }
        }

        // get cycle eras from kv store
        Action::RightsGetCycleEras(RightsGetCycleErasAction { key }) => {
            match requests.get(key) {
                Some(RightsRequest::PendingCycleEras {
                    proto_hash,
                    block_header,
                    ..
                }) => {
                    let protocol_hash = proto_hash.clone();
                    let block_header = block_header.clone();
                    store.dispatch(RightsCycleErasGetAction {
                        protocol_hash,
                        block_hash: key.block().clone(),
                        block_header,
                    });
                }
                Some(RightsRequest::CycleErasReady {
                    ..
                }) => {
                    store.dispatch(RightsGetCycleAction { key: key.clone() });
                }
                _ => {}
            }
        }
        Action::RightsCycleErasSuccess(RightsCycleErasSuccessAction { protocol_hash }) => {
            let cycle_eras = if let Some(v) = store
                .state
                .get()
                .rights
                .cycle_eras
                .get_result(protocol_hash)
            {
                v.clone()
            } else {
                return;
            };
            for key in requests
                .iter()
                .filter_map(|(rights_key, request)| {
                    if matches!(request, RightsRequest::PendingCycleEras { proto_hash, .. } if proto_hash == protocol_hash ) {
                        Some(rights_key)
                    } else {
                        None
                    }
                })
                .cloned()
                .collect::<Vec<_>>()
            {
                store.dispatch(RightsCycleErasReadyAction {
                    key,
                    cycle_eras: cycle_eras.clone(),
                });
            }
        }
        Action::RightsCycleErasError(RightsCycleErasErrorAction { protocol_hash }) => {
            let cycle_eras_error =
                if let Some(v) = store.state.get().rights.cycle_eras.get_error(protocol_hash) {
                    v.clone()
                } else {
                    return;
                };
            for key in requests
                .iter()
                .filter_map(|(rights_key, request)| {
                    if matches!(request, RightsRequest::PendingCycleEras { proto_hash, .. } if proto_hash == protocol_hash ) {
                        Some(rights_key)
                    } else {
                        None
                    }
                })
                .cloned()
                .collect::<Vec<_>>()
            {
                store.dispatch(RightsErrorAction {
                    key,
                    error: cycle_eras_error.clone().into(),
                });
            }
        }
        Action::RightsCycleErasReady(RightsCycleErasReadyAction { key, .. }) => {
            store.dispatch(RightsGetCycleAction { key: key.clone() });
        }

        // get cycle for the requested block
        Action::RightsGetCycle(RightsGetCycleAction { key }) => {
            if let Some(RightsRequest::PendingCycle {
                block_header,
                cycle_eras,
                protocol_constants,
                ..
            }) = requests.get(key)
            {
                let block_level = block_header.level();
                let preserved_cycles =
                    if let Some(preserved_cycles) = protocol_constants.preserved_cycles() {
                        preserved_cycles
                    } else {
                        store.dispatch(RightsErrorAction {
                            key: key.clone(),
                            error: RightsCalculationError::MissingProtocolConstant(
                                "preserved_cycles".into(),
                            )
                            .into(),
                        });
                        return;
                    };
                let get_cycle = get_cycle(block_level, key.level(), cycle_eras, preserved_cycles);
                match get_cycle {
                    Ok((cycle, position)) => store.dispatch(RightsCycleReadyAction {
                        key: key.clone(),
                        cycle,
                        position,
                    }),
                    Err(err) => store.dispatch(RightsErrorAction {
                        key: key.clone(),
                        error: err.into(),
                    }),
                };
            }
        }
        Action::RightsCycleReady(RightsCycleReadyAction { key, .. }) => {
            if let Some(RightsRequest::CycleReady { protocol, .. }) = requests.get(key) {
                if protocol < &SupportedProtocol::Proto012 {
                    store.dispatch(RightsGetCycleDataAction { key: key.clone() });
                } else {
                    store.dispatch(RightsGetCycleDelegatesAction { key: key.clone() });
                }
            }
        }

        // pre-Ithaca

        // get cycle data kv store
        Action::RightsGetCycleData(RightsGetCycleDataAction { key }) => {
            if let Some(RightsRequest::PendingCycleData { cycle, .. }) = requests.get(key) {
                let key = *cycle;
                store.dispatch(kv_cycle_meta::StorageCycleMetaGetAction::new(key));
            }
        }
        Action::StorageCycleMetaOk(kv_cycle_meta::StorageCycleMetaOkAction {
            key: CycleKey(key),
            value,
        }) => {
            for key in requests
                .iter()
                .filter_map(|(rights_key, request)| {
                    if let RightsRequest::PendingCycleData { cycle, .. } = request {
                        if cycle == key {
                            Some(rights_key)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .cloned()
                .collect::<Vec<_>>()
            {
                store.dispatch(RightsCycleDataReadyAction {
                    key,
                    cycle_data: value.clone(),
                });
            }
        }
        Action::StorageCycleMetaError(kv_cycle_meta::StorageCycleMetaErrorAction {
            key: CycleKey(key),
            error,
        }) => {
            for key in requests
                .iter()
                .filter_map(|(rights_key, request)| {
                    if let RightsRequest::PendingCycleData { cycle, .. } = request {
                        if cycle == key {
                            Some(rights_key)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .cloned()
                .collect::<Vec<_>>()
            {
                store.dispatch(RightsErrorAction {
                    key,
                    error: error.clone().into(),
                });
            }
        }
        Action::RightsCycleDataReady(RightsCycleDataReadyAction { key, .. }) => {
            store.dispatch(RightsCalculateAction { key: key.clone() });
        }

        Action::RightsCalculateEndorsingRights(RightsCalculateAction { key }) => {
            if let Some(RightsRequest::PendingRightsCalculation {
                start,
                protocol: _,
                protocol_constants,
                level,
                cycle_data,
                position,
            }) = requests.get(key)
            {
                trace!(&store.state.get().log, "calculating rights"; "level" => level, "key" => FnValue(|_| format!("{:#?}", key)));
                let prereq_dur = action.id.duration_since(*start);

                let time = Instant::now();
                match &key.0 {
                    crate::rights::RightsInput::Baking(_) => {
                        match calculate_baking_rights(cycle_data, protocol_constants, *position, 64)
                        {
                            Ok(priorities) => {
                                let dur = Instant::now() - time;
                                let log = &store.state.get().log;
                                trace!(log, "Baking rights successfully calculated";
                                       "level" => level,
                                       "prerequisites duration" => FnValue(|_| prereq_dur.as_millis()),
                                       "duration" => FnValue(|_| dur.as_millis())
                                );
                                let level = *level;
                                store.dispatch(RightsBakingOldReadyAction {
                                    key: key.clone(),
                                    baking_rights: BakingRightsOld { level, priorities },
                                });
                            }
                            Err(err) => {
                                store.dispatch(RightsErrorAction {
                                    key: key.clone(),
                                    error: err.into(),
                                });
                            }
                        }
                    }
                    crate::rights::RightsInput::Endorsing(_) => {
                        match calculate_endorsing_rights(cycle_data, protocol_constants, *position)
                        {
                            Ok((delegate_to_slots, slot_to_delegate)) => {
                                let dur = Instant::now() - time;
                                let log = &store.state.get().log;
                                trace!(log, "Endorsing rights successfully calculated";
                                       "level" => level,
                                       "prerequisites duration" => FnValue(|_| prereq_dur.as_millis()),
                                       "duration" => FnValue(|_| dur.as_millis())
                                );
                                let level = *level;
                                store.dispatch(RightsEndorsingOldReadyAction {
                                    key: key.clone(),
                                    endorsing_rights: EndorsingRightsOld {
                                        level,
                                        slot_to_delegate,
                                        delegate_to_slots,
                                    },
                                });
                            }
                            Err(err) => {
                                store.dispatch(RightsErrorAction {
                                    key: key.clone(),
                                    error: err.into(),
                                });
                            }
                        }
                    }
                }
            }
        }

        // Ithaca

        Action::RightsGetCycleDelegates(RightsGetCycleDelegatesAction { key }) => {
            match requests.get(key) {
                Some(RightsRequest::PendingCycleDelegates {
                    cycle,
                    block_header,
                    ..
                }) => {
                    let cycle = *cycle;
                    let block_header = block_header.clone();
                    store.dispatch(RightsCycleDelegatesGetAction { cycle, block_header });
                }
                Some(RightsRequest::CycleDelegatesReady { .. }) => {
                    store.dispatch(RightsCalculateIthacaAction { key: key.clone() });
                }
                _ => {}
            }
        }
        Action::RightsCycleDelegatesSuccess(RightsCycleDelegatesSuccessAction { cycle, delegates }) => {
            for key in requests.iter().filter_map(|(k, v)| if matches!(v, RightsRequest::PendingCycleDelegates { cycle: c, .. } if c == cycle) { Some(k) } else { None }).cloned().collect::<Vec<_>>() {
                store.dispatch(RightsCycleDelegatesReadyAction { key, delegates: delegates.clone() });
            }
        }
        Action::RightsCycleDelegatesError(RightsCycleDelegatesErrorAction { cycle, error }) => {
            for key in requests.iter().filter_map(|(k, v)| if matches!(v, RightsRequest::PendingCycleDelegates { cycle: c, .. } if c == cycle) { Some(k) } else { None }).cloned().collect::<Vec<_>>() {
                store.dispatch(RightsErrorAction { key, error: error.clone().into() });
            }
        }

        Action::RightsCycleDelegatesReady(RightsCycleDelegatesReadyAction { key, .. }) => {
            store.dispatch(RightsCalculateIthacaAction { key: key.clone() });
        }

        Action::RightsCalculateIthaca(RightsCalculateIthacaAction { key }) => {
            if let Some(RightsRequest::PendingRightsCalculationIthaca { block_header, level, .. } ) = requests.get(key)
            {
                let req = ProtocolRpcRequest {
                    block_header: block_header.clone(),
                    chain_arg: "main".to_string(),
                    chain_id: store.state().config.chain_id.clone(),
                    request: RpcRequest {
                        body: String::new(),
                        accept: None,
                        content_type: None,
                        context_path: format!(
                            "/chains/main/blocks/head/helpers/endorsing_rights?level={level}"
                        ),
                        meth: RpcMethod::GET,
                    },
                };
                let token = store.service.protocol_runner().get_endorsing_rights(req);
                store.dispatch(RightsContextRequestedAction { key: key.clone(), token });
            }
        }
        Action::ProtocolRunnerResponse(resp) => if let ProtocolRunnerResult::GetEndorsingRights((token, result)) = &resp.result {
            if let Some(key) = requests.iter().find_map(|(k, v)| {
                if matches!(v, RightsRequest::PendingRightsFromContextIthaca { token: t, .. } if token == t) {
                    Some(k)
                } else {
                    None
                }
            }).cloned() {
                match result {
                    Ok(Ok(result)) => {
                            store.dispatch(RightsIthacaContextSuccessAction {
                                key,
                                endorsing_rights: result.clone(),
                            });
                    }
                    Ok(Err(err)) => {
                        store.dispatch(RightsErrorAction {
                            key,
                            error: err.clone().into(),
                        });
                    }
                    _ => {}
                }
            }
        }

        Action::RightsIthacaContextSuccess(RightsIthacaContextSuccessAction { key, .. }) => {
            if let Some(RightsRequest::EndorsingReady(_)) = requests.get(key) {
                store.dispatch(RightsEndorsingReadyAction {
                    key: key.clone(),
                });
            }
        }


        Action::RightsError(RightsErrorAction { key, error }) => {
            warn!(log, "Error getting rights"; "key" => format!("{:?}", key), "error" => error.to_string());
            for rpc_id in store
                .state
                .get()
                .rights
                .rpc_requests
                .get(key)
                .cloned()
                .unwrap_or_default()
            {
                store.dispatch(RightsRpcErrorAction {
                    rpc_id,
                    error: error.clone().into(),
                });
            }
            store.dispatch(RightsRpcPruneAction { key: key.clone() });
        }

        _ => (),
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, thiserror::Error)]
pub enum RightsCalculationError {
    #[error("Integer conversion error: {0}")]
    FromInt(String),
    #[error("Signature conversion error: {0}")]
    Conversion(#[from] ConversionError),
    #[error("Error calculating pseudo-random number: `{0}`")]
    Prng(#[from] TezosPRNGError),
    #[error("Missing protocol constant: `{0}`")]
    MissingProtocolConstant(String),
}

impl From<TryFromIntError> for RightsCalculationError {
    fn from(error: TryFromIntError) -> Self {
        Self::FromInt(error.to_string())
    }
}

fn calculate_endorsing_rights(
    cycle_meta_data: &CycleData,
    constants: &ProtocolConstants,
    cycle_position: Position,
) -> Result<(BTreeMap<Delegate, Vec<Slot>>, Vec<Delegate>), RightsCalculationError> {
    // build a reverse map of rols so we have access in O(1)
    let mut rolls_map = BTreeMap::new();

    for (delegate, rolls) in cycle_meta_data.rolls_data() {
        for roll in rolls {
            rolls_map.insert(
                *roll,
                SignaturePublicKey::from_tagged_bytes(delegate.to_vec())?,
            );
        }
    }

    let endorsers_per_block = if let Some(endorsers_per_block) = constants.endorsers_per_block() {
        endorsers_per_block
    } else {
        return Err(RightsCalculationError::MissingProtocolConstant(
            "endorsers_per_block".into(),
        ));
    };
    let nonce_length = if let Some(nonce_length) = constants.nonce_length() {
        nonce_length
    } else {
        return Err(RightsCalculationError::MissingProtocolConstant(
            "nonce_length".into(),
        ));
    };
    let endorsers_slots = (0..endorsers_per_block)
        .map(|slot| {
            endorser_rights_owner(
                nonce_length,
                cycle_meta_data,
                &rolls_map,
                cycle_position,
                slot,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut endorser_to_slots = BTreeMap::new();
    for (slot, delegate) in endorsers_slots.iter().enumerate() {
        endorser_to_slots
            .entry(delegate.clone())
            .or_insert_with(Vec::new)
            .push(slot.try_into()?);
    }
    Ok((endorser_to_slots, endorsers_slots))
}

fn calculate_baking_rights(
    cycle_meta_data: &CycleData,
    constants: &ProtocolConstants,
    cycle_position: Position,
    max_priority: u16,
) -> Result<Vec<Delegate>, RightsCalculationError> {
    // build a reverse map of rols so we have access in O(1)
    let mut rolls_map = BTreeMap::new();

    for (delegate, rolls) in cycle_meta_data.rolls_data() {
        for roll in rolls {
            rolls_map.insert(
                *roll,
                SignaturePublicKey::from_tagged_bytes(delegate.to_vec())?,
            );
        }
    }

    let nonce_length = if let Some(nonce_length) = constants.nonce_length() {
        nonce_length
    } else {
        return Err(RightsCalculationError::MissingProtocolConstant(
            "nonce_length".into(),
        ));
    };
    let priorities = (0..max_priority)
        .map(|priority| {
            baking_rights_owner(
                nonce_length,
                cycle_meta_data,
                &rolls_map,
                cycle_position,
                priority,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(priorities)
}
