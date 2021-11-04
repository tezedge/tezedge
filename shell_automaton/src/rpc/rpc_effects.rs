// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use redux_rs::{ActionWithId, Store};

use crate::service::rpc_service::RpcResponse;
use crate::service::{RpcService, Service};
use crate::{Action, State};
use crate::mempool::MempoolOperationInjectAction;

pub fn rpc_effects<S: Service>(store: &mut Store<State, S, Action>, action: &ActionWithId<Action>) {
    match &action.action {
        Action::WakeupEvent(_) => {
            while let Ok((msg, rpc_id)) = store.service().rpc().try_recv() {
                match msg {
                    RpcResponse::GetCurrentGlobalState { channel } => {
                        let _ = channel.send(store.state.get().clone());
                    }
                    RpcResponse::InjectOperation { operation, operation_hash } => {
                        store.dispatch(
                            MempoolOperationInjectAction {
                                operation,
                                operation_hash,
                                rpc_id,
                            }
                            .into()
                        )
                    }
                }
            }
        }
        _ => {}
    }
}
