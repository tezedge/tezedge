// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::peers::PeerBlacklistState;
use crate::{Action, ActionWithMeta, State};

pub fn peers_graylist_reducer(state: &mut State, action: &ActionWithMeta) {
    match &action.action {
        Action::PeersGraylistIpAdd(action_content) => {
            state.peers.ip_blacklist_entry(action_content.ip).or_insert(
                PeerBlacklistState::Graylisted {
                    since: action.time_as_nanos(),
                },
            );
        }
        Action::PeersGraylistIpRemove(action_content) => {
            state.peers.remove_blacklisted_ip(&action_content.ip);
        }
        _ => {}
    }
}
