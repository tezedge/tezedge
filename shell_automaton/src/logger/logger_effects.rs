// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use tezos_messages::p2p::encoding::block_header::display_fitness;

use crate::{Action, ActionWithMeta, Service, Store};

#[allow(unused)]
pub fn logger_effects<S: Service>(store: &mut Store<S>, action: &ActionWithMeta) {
    // eprintln!("[+] Action: {}", action.action.as_ref());
    // eprintln!("[+] Action: {:#?}", &action);
    // eprintln!("[+] State: {:#?}\n", store.state());

    let state = store.state.get();
    let log = &state.log;

    match &action.action {
        Action::CurrentHeadUpdate(content) => {
            slog::info!(log, "CurrentHead Updated";
                "level" => content.new_head.header.level(),
                "hash" => content.new_head.hash.to_string(),
                "fitness" => display_fitness(content.new_head.header.fitness()));
            slog::debug!(log, "CurrentHead Updated - full header";
                "new_head" => slog::FnValue(|_| format!("{:?}", content.new_head)));
        }
        Action::PeerCurrentHeadUpdate(content) => {
            slog::info!(log, "Peer CurrentHead Updated";
                "peer_address" => content.address,
                "peer_pkh" => state.peer_public_key_hash_b58check(content.address),
                "level" => content.current_head.header.level(),
                "hash" => content.current_head.hash.to_string(),
                "fitness" => display_fitness(content.current_head.header.fitness()));
            slog::debug!(log, "Peer CurrentHead Updated - full header";
                "new_head" => slog::FnValue(|_| format!("{:?}", content.current_head)));
        }
        Action::BootstrapFromPeerCurrentHead(content) => {
            let current_head = match state.current_head.get() {
                Some(v) => v,
                None => return,
            };
            slog::info!(log, "Bootstrapping from a new current head";
                "peer_address" => content.peer,
                "peer_pkh" => state.peer_public_key_hash_b58check(content.peer),
                "current_head_level" => current_head.header.level(),
                "current_head_hash" => current_head.hash.to_string(),
                "current_head_fitness" => display_fitness(current_head.header.fitness()),
                "new_head_level" => content.current_head.header.level(),
                "new_head_hash" => content.current_head.hash.to_string(),
                "new_head_fitness" => display_fitness(content.current_head.header.fitness()));
        }
        Action::BootstrapError(content) => {
            let current_head = match state.current_head.get() {
                Some(v) => v,
                None => return,
            };
            slog::warn!(log, "Bootstrap pipeline failed";
                "error" => format!("{:?}", content.error),
                "bootstrap_state" => format!("{:?}", state.bootstrap),
                "block_applier_state" => format!("{:?}", state.block_applier));
        }
        Action::BootstrapPeerBlockHeaderGetTimeout(content) => {
            slog::warn!(log, "Fetching BlockHeader from peer timed out";
                "peer_address" => content.peer,
                "peer_pkh" => state.peer_public_key_hash_b58check(content.peer),
                "block_hash" => content.block_hash.to_string());
        }
        Action::BootstrapPeerBlockOperationsGetTimeout(content) => {
            slog::warn!(log, "Fetching BlockOperations from peer timed out";
                "peer_address" => content.peer,
                "peer_pkh" => state.peer_public_key_hash_b58check(content.peer),
                "block_hash" => content.block_hash.to_string());
        }

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
            slog::info!(log, "Peer Handshaking successful";
            "address" => content.address.to_string(),
            "public_key_hash" => slog::FnValue(|_| state.peer_public_key_hash_b58check(content.address)));
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
        Action::PeerMessageReadSuccess(content) => {
            slog::trace!(log, "Received message from a peer";
                "address" => content.address.to_string(),
                "public_key_hash" => slog::FnValue(|_| state.peer_public_key_hash_b58check(content.address)),
                "message" => content.message.message());
        }
        Action::PeerMessageWriteInit(content) => {
            slog::trace!(log, "Sending message to a peer";
                "address" => content.address.to_string(),
                "public_key_hash" => slog::FnValue(|_| state.peer_public_key_hash_b58check(content.address)),
                "message" => content.message.message());
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
        Action::BlockApplierApplyProtocolRunnerApplyRetry(content) => {
            slog::warn!(log, "Block application failed! Retrying...";
                        "error" => format!("{:?}", content.reason),
                        "block_hash" => format!("{:?}", content.block_hash));
        }
        Action::BlockApplierApplyError(content) => {
            slog::error!(log, "Block application failed";
                "error" => format!("{:?}", content.error));
        }
        Action::ProtocolRunnerReady(_) => {
            slog::info!(log, "Protocol Runner initialized");
        }
        Action::ProtocolRunnerNotifyStatus(_) => {
            slog::info!(
                log,
                "Notified Protocol Runner status to the rest of the system"
            );
        }
        Action::ProtocolRunnerInitRuntimeError(content) => {
            slog::error!(log, "Protocol Runner runtime initialization failed";
                "error" => format!("{:?}", content.error));
        }
        Action::ProtocolRunnerInitContextError(content) => {
            slog::error!(log, "Protocol Runner context initialization failed";
                "error" => format!("{:?}", content.error));
        }
        Action::ProtocolRunnerInitContextIpcServerError(content) => {
            slog::error!(log, "Protocol Runner context ipc server initialization failed";
                "error" => format!("{:?}", content.error));
        }
        _ => {}
    }
}
