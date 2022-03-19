// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use tezos_messages::ts_to_rfc3339;

use crate::{block_applier::BlockApplierApplyState, Action};

use super::{
    rpc_actions::{
        RpcBootstrappedAction, RpcBootstrappedDoneAction, RpcBootstrappedNewBlockAction,
        RpcMonitorValidBlocksAction,
    },
    BootstrapState,
};

pub fn rpc_reducer(state: &mut crate::State, action: &crate::ActionWithMeta) {
    match &action.action {
        Action::RpcBootstrapped(RpcBootstrappedAction { rpc_id }) => {
            let bootstrapped = &mut state.rpc.bootstrapped;
            bootstrapped.requests.insert(*rpc_id);
            if bootstrapped.state.is_none() {
                if let BlockApplierApplyState::Success { block, .. } = &state.block_applier.current
                {
                    bootstrapped.state = Some(BootstrapState {
                        json: serde_json::json!({
                            "block": block.hash,
                            "timestamp": block.header.timestamp(),
                        }),
                        is_bootstrapped: true,
                    });
                }
            }
        }
        Action::RpcBootstrappedNewBlock(RpcBootstrappedNewBlockAction {
            block,
            timestamp,
            is_bootstrapped,
        }) => {
            state.rpc.bootstrapped.state = Some(BootstrapState {
                json: serde_json::json!({
                    "block": block,
                    "timestamp": ts_to_rfc3339(*timestamp).unwrap_or_else(|_| "<invalid timestamp>".to_string()),
                }),
                is_bootstrapped: *is_bootstrapped,
            });
        }
        Action::RpcBootstrappedDone(RpcBootstrappedDoneAction { rpc_ids }) => {
            let bootstrapped = &mut state.rpc.bootstrapped;
            for rpc_id in rpc_ids {
                bootstrapped.requests.remove(rpc_id);
            }
            if bootstrapped.requests.is_empty() {
                bootstrapped.state = None;
            }
        }

        Action::RpcMonitorValidBlocks(RpcMonitorValidBlocksAction { rpc_id, query }) => {
            state
                .rpc
                .valid_blocks
                .requests
                .insert(*rpc_id, query.clone());
        }

        _ => (),
    }
}
