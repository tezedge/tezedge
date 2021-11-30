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

                if peer.quota.bytes_read == 0 {
                    return;
                }
                let duration_since_restore_millis = action
                    .id
                    .duration_since(peer.quota.read_timestamp)
                    .as_millis();
                if duration_since_restore_millis
                    >= state.config.quota.restore_duration_millis as u128
                {
                    peer.quota.bytes_read = 0;
                    peer.quota.read_timestamp = action.id;
                    peer.quota.reject_read = false;
                }
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

                if peer.quota.bytes_written == 0 {
                    return;
                }
                let duration_since_restore_millis = action
                    .id
                    .duration_since(peer.quota.write_timestamp)
                    .as_millis();
                if duration_since_restore_millis
                    >= state.config.quota.restore_duration_millis as u128
                {
                    peer.quota.bytes_written = 0;
                    peer.quota.write_timestamp = action.id;
                    peer.quota.reject_write = false;
                }
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
        Action::PeerChunkReadPart(PeerChunkReadPartAction { address, bytes }) => {
            if let Some(peer) = state.peers.get_mut(&address) {
                peer.quota.bytes_read += bytes.len();
                peer.quota.reject_read = peer.quota.bytes_read >= state.config.quota.read_quota;
            }
        }
        Action::PeerChunkWritePart(PeerChunkWritePartAction { address, written }) => {
            if let Some(peer) = state.peers.get_mut(&address) {
                peer.quota.bytes_written += written;
                peer.quota.reject_write =
                    peer.quota.bytes_written >= state.config.quota.write_quota;
            }
        }
        _ => (),
    }
}
