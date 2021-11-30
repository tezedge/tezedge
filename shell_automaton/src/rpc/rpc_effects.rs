// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::mempool::MempoolOperationInjectAction;
use crate::rights::{EndorsingRightsKey, RightsRpcEndorsingRightsGetAction};
use crate::service::rpc_service::RpcResponse;
use crate::service::{RpcService, Service};
use crate::{Action, ActionWithMeta, Store};

pub fn rpc_effects<S: Service>(store: &mut Store<S>, action: &ActionWithMeta) {
    match &action.action {
        Action::WakeupEvent(_) => {
            while let Ok((msg, rpc_id)) = store.service().rpc().try_recv() {
                match msg {
                    RpcResponse::GetCurrentGlobalState { channel } => {
                        let _ = channel.send(store.state.get().clone());
                    }
                    RpcResponse::InjectOperation {
                        operation,
                        operation_hash,
                    } => {
                        store.dispatch(MempoolOperationInjectAction {
                            operation,
                            operation_hash,
                            rpc_id,
                        });
                    }
                    RpcResponse::GetEndorsingRights { block_hash, level } => {
                        store.dispatch(RightsRpcEndorsingRightsGetAction {
                            key: EndorsingRightsKey {
                                current_block_hash: block_hash,
                                level,
                            },
                            rpc_id,
                        });
                    }
                }
            }
        }
        _ => {}
    }
}
