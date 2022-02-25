// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::{Action, ActionWithMeta, State};

use super::chunk::read::PeerChunkReadPartAction;
use super::chunk::write::PeerChunkWritePartAction;
use super::{
    PeerIOLoopState, PeerTryReadLoopFinishAction, PeerTryReadLoopStartAction,
    PeerTryWriteLoopFinishAction, PeerTryWriteLoopStartAction,
};

pub fn peer_reducer(state: &mut State, action: &ActionWithMeta) {
    match &action.action {
        Action::PeerTryReadLoopStart(PeerTryReadLoopStartAction { address }) => {
            if let Some(peer) = state.peers.get_mut(address) {
                peer.try_read_loop = PeerIOLoopState::Started {
                    time: action.time_as_nanos(),
                };
            }
        }
        Action::PeerTryReadLoopFinish(PeerTryReadLoopFinishAction { address, result }) => {
            if let Some(peer) = state.peers.get_mut(address) {
                peer.try_read_loop = PeerIOLoopState::Finished {
                    time: action.time_as_nanos(),
                    result: result.clone(),
                };
            }
        }
        Action::PeerTryWriteLoopStart(PeerTryWriteLoopStartAction { address }) => {
            if let Some(peer) = state.peers.get_mut(address) {
                peer.try_write_loop = PeerIOLoopState::Started {
                    time: action.time_as_nanos(),
                };
            }
        }
        Action::PeerTryWriteLoopFinish(PeerTryWriteLoopFinishAction { address, result }) => {
            if let Some(peer) = state.peers.get_mut(address) {
                peer.try_write_loop = PeerIOLoopState::Finished {
                    time: action.time_as_nanos(),
                    result: result.clone(),
                };
            }
        }
        _ => (),
    }
}
