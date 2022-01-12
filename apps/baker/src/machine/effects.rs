// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use redux_rs::{ActionWithMeta, Store};

use super::{action::Action, service::ServiceDefault, state::State};

pub fn effects(store: &mut Store<State, ServiceDefault, Action>, action: &ActionWithMeta<Action>) {
    match &action.action {
        Action::RunWithLocalNode(_) => {
            let ServiceDefault { client, .. } = &store.service();

            client.wait_bootstrapped().unwrap();
        }
        Action::Bootstrapped(_) => {
            let ServiceDefault { client, log } = &store.service();
            slog::info!(log, "Node is bootstrapped.");

            client.spawn_monitor_main_head().unwrap();
        }
        Action::NewHeadSeen(_) => {
            let ServiceDefault { client, .. } = &store.service();

            client.spawn_monitor_operations().unwrap();
        }
        Action::NewOperationSeen(_) => {}
    }
}
