// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::{Action, ActionWithMeta, State};

use super::{PeersDnsLookupState, PeersDnsLookupStatus};

pub fn peers_dns_lookup_reducer(state: &mut State, action: &ActionWithMeta) {
    match &action.action {
        Action::PeersDnsLookupInit(action) => {
            state.peers.dns_lookup = Some(PeersDnsLookupState {
                address: action.address.clone(),
                port: action.port,
                status: PeersDnsLookupStatus::Init,
            });
        }
        Action::PeersDnsLookupError(action) => {
            if let Some(dns_lookup_state) = state.peers.dns_lookup.as_mut() {
                if let PeersDnsLookupStatus::Init = dns_lookup_state.status {
                    dns_lookup_state.status = PeersDnsLookupStatus::Error {
                        error: action.error,
                    };
                }
            }
        }
        Action::PeersDnsLookupSuccess(action) => {
            if let Some(dns_lookup_state) = state.peers.dns_lookup.as_mut() {
                if let PeersDnsLookupStatus::Init = dns_lookup_state.status {
                    dns_lookup_state.status = PeersDnsLookupStatus::Success {
                        addresses: action.addresses.clone(),
                    };
                }
            }
        }
        Action::PeersDnsLookupCleanup(_) => {
            state.peers.dns_lookup.take();
        }
        _ => {}
    }
}
