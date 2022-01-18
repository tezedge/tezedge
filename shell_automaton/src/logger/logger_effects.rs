// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::{Action, ActionWithMeta, Service, Store};

#[allow(unused)]
pub fn logger_effects<S: Service>(store: &mut Store<S>, action: &ActionWithMeta) {
    // eprintln!("[+] Action: {}", action.action.as_ref());
    // eprintln!("[+] Action: {:#?}", &action);
    // eprintln!("[+] State: {:#?}\n", store.state());

    let log = &store.state().log;

    match &action.action {
        Action::PeerConnectionOutgoingError(content) => {
            slog::warn!(log, "Failed to connect (outgoing) to peer";
                "address" => content.address.to_string(),
                "error" => format!("{:?}", content.error));
        }
        Action::PeerConnectionOutgoingSuccess(content) => {
            slog::info!(log, "Connected (outgoing) to peer"; "address" => content.address.to_string());
        }
        Action::PeerConnectionIncomingSuccess(content) => {
            slog::info!(log, "Connected (incoming) to peer"; "address" => content.address.to_string());
        }
        Action::PeerHandshakingInit(content) => {
            slog::info!(log, "Initiated handshaking with peer"; "address" => content.address.to_string());
        }
        Action::PeerConnectionClosed(content) => {
            slog::warn!(log, "Peer connection closed"; "address" => content.address.to_string());
        }
        Action::PeerChunkReadError(content) => {
            slog::warn!(log, "Error while reading chunk from peer";
                "address" => content.address.to_string(),
                "error" => format!("{:?}", content.error));
        }
        Action::PeerChunkWriteError(content) => {
            slog::warn!(log, "Error while writing chunk to peer";
                "address" => content.address.to_string(),
                "error" => format!("{:?}", content.error));
        }
        Action::PeerBinaryMessageReadError(content) => {
            slog::warn!(log, "Error while reading binary message from peer";
                "address" => content.address.to_string(),
                "error" => format!("{:?}", content.error));
        }
        Action::PeerBinaryMessageWriteError(content) => {
            slog::warn!(log, "Error while writing binary message to peer";
                "address" => content.address.to_string(),
                "error" => format!("{:?}", content.error));
        }
        Action::PeerHandshakingError(content) => {
            slog::warn!(log, "Peer Handshaking failed";
                "address" => content.address.to_string(),
                "error" => format!("{:?}", content.error));
        }
        Action::PeerHandshakingFinish(content) => {
            slog::info!(log, "Peer Handshaking successful"; "address" => content.address.to_string());
        }
        Action::PeerDisconnect(content) => {
            slog::warn!(log, "Disconnecting peer"; "address" => content.address.to_string());
        }

        Action::PeersGraylistAddress(content) => {
            slog::warn!(log, "Graylisting peer ip"; "address" => content.address.to_string());
        }
        Action::PeersGraylistIpRemove(content) => {
            slog::info!(log, "Whitelisting peer ip"; "ip" => content.ip.to_string());
        }
        Action::PeersCheckTimeoutsSuccess(content) => {
            if !content.peer_timeouts.is_empty() {
                slog::warn!(log, "Peers timed out";
                    "timeouts" => format!("{:?}", content.peer_timeouts));
            }
        }

        Action::StorageResponseReceived(content) => match &content.response.result {
            Ok(_) => {}
            Err(err) => {
                slog::error!(log, "Error response received from storage thread";
                    "error" => format!("{:?}", err));
            }
        },
        Action::StorageBlocksGenesisInitCommitResultGetError(content) => {
            slog::error!(log, "Error when getting genesis commit result from protocol";
                "error" => &content.error);
        }
        Action::BlockApplierApplyError(content) => {
            slog::error!(log, "Block application failed";
                "error" => format!("{:?}", content.error));
        }
        _ => {}
    }
}
