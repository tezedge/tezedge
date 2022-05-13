// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use tezos_api::ffi::{ProtocolRpcRequest, RpcMethod, RpcRequest};

use crate::{
    service::{protocol_runner_service::ProtocolRunnerResult, ProtocolRunnerService},
    Action,
};

use super::{rights_cycle_delegates_actions::*, CycleDelegatesQueryState};

pub fn rights_cycle_delegates_effects<S>(
    store: &mut crate::Store<S>,
    action: &crate::ActionWithMeta,
) where
    S: crate::Service,
{
    let delegates_state = &store.state.get().rights.cycle_delegates;
    match &action.action {
        Action::RightsCycleDelegatesGet(RightsCycleDelegatesGetAction { cycle, block_header }) => {
            let req = ProtocolRpcRequest {
                block_header: block_header.clone(),
                chain_arg: "main".to_string(),
                chain_id: store.state().config.chain_id.clone(),
                request: RpcRequest {
                    body: String::new(),
                    accept: None,
                    content_type: None,
                    context_path: format!(
                        "/chains/main/blocks/head/context/raw/json/cycle/{cycle}/delegate_sampler_state/"
                    ),
                    meth: RpcMethod::GET,
                },
            };
            let cycle = *cycle;
            let token = store.service.protocol_runner().get_cycle_delegates(req);
            store.dispatch(RightsCycleDelegatesRequestedAction { cycle, token });
        }
        Action::ProtocolRunnerResponse(resp) => match &resp.result {
            ProtocolRunnerResult::GetCycleDelegates((token, result)) => {
                for cycle in delegates_state
                    .iter()
                    .filter_map(|(k, v)| {
                        if matches!(&v.state, CycleDelegatesQueryState::ContextRequested(t) if t == token) {
                            Some(k)
                        } else {
                            None
                        }
                    })
                    .cloned()
                    .collect::<Vec<_>>()
                {
                    match result {
                        Ok(Ok(delegates)) => {
                                store.dispatch(RightsCycleDelegatesSuccessAction {
                                    cycle,
                                    delegates: delegates.clone(),
                            });
                        }
                        Ok(Err(err)) => {
                            store.dispatch(RightsCycleDelegatesErrorAction {
                                cycle,
                                error: err.clone().into(),
                            });
                        }
                        Err(err) => {
                            store.dispatch(RightsCycleDelegatesErrorAction {
                                cycle,
                                error: err.clone().into(),
                            });
                        }
                    }
                }
            }
            _ => {}
        },

        _ => (),
    }
}
