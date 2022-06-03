// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use slog::FnValue;
use tezos_api::ffi::{ProtocolRpcRequest, RpcMethod, RpcRequest};
use tezos_encoding::{binary_reader::BinaryReaderError, nom::NomReader};
use tezos_messages::p2p::binary_message::BinaryRead;

use crate::{
    service::{protocol_runner_service::ProtocolRunnerResult, ProtocolRunnerService},
    storage::kv_cycle_eras,
    Action,
};

use super::{
    rights_cycle_eras_actions::*, CycleEra, CycleEras, CycleErasQuery, CycleErasQueryState,
};

pub fn rights_cycle_eras_effects<S>(store: &mut crate::Store<S>, action: &crate::ActionWithMeta)
where
    S: crate::Service,
{
    let log = &store.state.get().log;
    let cycle_eras = &store.state.get().rights.cycle_eras;
    match &action.action {
        Action::RightsCycleErasGet(RightsCycleErasGetAction { protocol_hash, .. }) => {
            store.dispatch(kv_cycle_eras::StorageCycleErasGetAction::new(
                protocol_hash.clone(),
            ));
        }
        Action::StorageCycleErasOk(kv_cycle_eras::StorageCycleErasOkAction { key, value }) => {
            if matches!(
                cycle_eras.get_state(key),
                Some(CycleErasQueryState::PendingKV)
            ) {
                store.dispatch(RightsCycleErasKVSuccessAction {
                    protocol_hash: key.clone(),
                    cycle_eras: value.iter().map(CycleEra::from).collect(),
                });
            }
        }
        Action::StorageCycleErasError(kv_cycle_eras::StorageCycleErasErrorAction {
            key,
            error,
        }) => {
            if matches!(error, kv_cycle_eras::Error::NotFound) {
                slog::debug!(log, "Cycle eras are missing inkey-value storage"; "error" => FnValue(|_| error.to_string()));
            } else {
                slog::warn!(log, "Error getting cycle eras from key-value storage"; "error" => FnValue(|_| error.to_string()));
            }
            if matches!(
                cycle_eras.get_state(key),
                Some(CycleErasQueryState::PendingKV)
            ) {
                store.dispatch(RightsCycleErasKVErrorAction {
                    protocol_hash: key.clone(),
                    kv_store_error: error.clone(),
                });
            }
        }

        Action::RightsCycleErasKVError(RightsCycleErasKVErrorAction { protocol_hash, .. }) => {
            if let Some(CycleErasQuery {
                block_header,
                block_hash,
                state: CycleErasQueryState::PendingContext(..),
            }) = cycle_eras.get(protocol_hash)
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
                            "/chains/main/blocks/${block_hash}/context/raw/bytes/v1/cycle_eras"
                        ),
                        meth: RpcMethod::GET,
                    },
                };
                let token = store.service.protocol_runner().get_context_raw_bytes(req);
                store.dispatch(RightsCycleErasContextRequestedAction {
                    protocol_hash: protocol_hash.clone(),
                    token,
                });
            }
        }
        Action::ProtocolRunnerResponse(resp) => {
            if let ProtocolRunnerResult::GetContextRawBytes((token, result)) = &resp.result {
                for protocol_hash in cycle_eras
                    .iter()
                    .filter_map(|(k, v)| {
                        if matches!(&v.state, CycleErasQueryState::ContextRequested(t, _) if t == token) {
                            Some(k)
                        } else {
                            None
                        }
                    })
                    .cloned()
                    .collect::<Vec<_>>()
                {
                    match result {
                        Ok(Ok(bytes)) => match cycle_eras_from_bytes(bytes) {
                            Ok(cycle_eras) => {
                                store.dispatch(RightsCycleErasContextSuccessAction {
                                    protocol_hash,
                                    cycle_eras,
                                });}
                            Err(err) => {
                                store.dispatch(RightsCycleErasContextErrorAction {
                                    protocol_hash,
                                    error: err.into(),
                                });
                            }
                        },
                        Ok(Err(err)) => {
                            store.dispatch(RightsCycleErasContextErrorAction {
                                protocol_hash,
                                error: err.clone().into(),
                            });
                        }
                        Err(err) => {
                            store.dispatch(RightsCycleErasContextErrorAction {
                                protocol_hash,
                                error: err.clone().into(),
                            });
                        }
                    }
                }
            }
        }
        Action::RightsCycleErasKVSuccess(RightsCycleErasKVSuccessAction {
            protocol_hash, ..
        })
        | Action::RightsCycleErasContextSuccess(RightsCycleErasContextSuccessAction {
            protocol_hash,
            ..
        }) => {
            store.dispatch(RightsCycleErasSuccessAction {
                protocol_hash: protocol_hash.clone(),
            });
        }
        Action::RightsCycleErasContextError(RightsCycleErasContextErrorAction {
            protocol_hash,
            error,
        }) => {
            slog::warn!(log, "Error getting cycle eras from context"; "error" => FnValue(|_| error.to_string()));
            store.dispatch(RightsCycleErasErrorAction {
                protocol_hash: protocol_hash.clone(),
            });
        }
        _ => (),
    }
}

fn cycle_eras_from_bytes(bytes: &[u8]) -> Result<CycleEras, BinaryReaderError> {
    #[derive(NomReader)]
    struct Encoding {
        #[encoding(dynamic)]
        eras: Vec<CycleEra>,
    }

    Encoding::from_bytes(bytes).map(|result| result.eras)
}
