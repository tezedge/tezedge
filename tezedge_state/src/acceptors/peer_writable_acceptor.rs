// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::io::{Read, Write};

use crate::chunking::WriteMessageError;
use crate::proposals::{PeerReadableProposal, PeerWritableProposal};
use crate::{Effects, HandshakeMessageType, P2pState, TezedgeState};
use tezos_messages::p2p::encoding::ack::NackMotive;
use tla_sm::Acceptor;

impl<'a, Efs, S> Acceptor<PeerWritableProposal<'a, Efs, S>> for TezedgeState
where
    Efs: Effects,
    S: Read + Write,
{
    /// Peer's stream might be ready for writing, try to write/flush
    /// pending messages to the provided stream.
    fn accept(&mut self, proposal: PeerWritableProposal<'a, Efs, S>) {
        if let Err(_err) = self.validate_proposal(&proposal) {
            return;
        }
        let time = self.time;

        if let Some(peer) = self.connected_peers.get_mut(&proposal.peer) {
            loop {
                match peer.write_to(proposal.stream) {
                    Ok(()) => {}
                    Err(WriteMessageError::Empty) | Err(WriteMessageError::Pending) => break,
                    Err(err) => {
                        slog::warn!(&self.log, "Write failed!"; "description" => "error while trying to write to connected peer stream.", "error" => format!("{:?}", err));
                        self.blacklist_peer(proposal.peer);
                        break;
                    }
                };
            }
        } else {
            let meta_msg = self.meta_msg();
            if let Some(peer) = self.pending_peers.get_mut(&proposal.peer) {
                loop {
                    match peer.write_to(proposal.stream) {
                        Ok(msg_type) => {
                            match msg_type {
                                HandshakeMessageType::Connection => {
                                    peer.send_conn_msg_successful(time, &self.identity);
                                }
                                HandshakeMessageType::Metadata => {
                                    peer.send_meta_msg_successful(time);
                                }
                                HandshakeMessageType::Ack => {
                                    peer.send_ack_msg_successful(time);
                                    if peer.is_handshake_finished() {
                                        let nack_motive = peer.nack_motive();
                                        let peer =
                                            self.pending_peers.remove(&proposal.peer).unwrap();
                                        if let Some(result) = peer.to_handshake_result() {
                                            self.set_peer_connected(
                                                proposal.effects,
                                                proposal.peer,
                                                result,
                                            );
                                            self.adjust_p2p_state(proposal.effects);
                                            // try to write and read from peer
                                            // after successful handshake.
                                            self.accept_internal(PeerReadableProposal {
                                                effects: proposal.effects,
                                                time_passed: Default::default(),
                                                peer: proposal.peer,
                                                stream: proposal.stream,
                                            });
                                            return self.accept_internal(proposal);
                                        } else {
                                            slog::warn!(&self.log, "Blacklisting peer";
                                            "peer_address" => proposal.peer.to_string(),
                                            "reason" => format!("Sent Nack({:?})", match nack_motive {
                                                Some(motive) => motive.to_string(),
                                                None => "[Unknown]".to_string(),
                                            }));
                                            slog::warn!(&self.log, "Blacklisting peer"; "peer_address" => proposal.peer.to_string(), "reason" => "Sent Nack");
                                            self.blacklist_peer(proposal.peer);
                                            self.adjust_p2p_state(proposal.effects);
                                            return self.periodic_react(proposal.effects);
                                        }
                                    }
                                }
                            }
                            // try reading from peer after succesfully sending a message.
                            return self.accept_internal(PeerReadableProposal::from(proposal));
                        }
                        Err(WriteMessageError::Empty) => {
                            let p2p_state = self.p2p_state;
                            let potential_peers = &self.potential_peers;

                            let result = peer
                                .enqueue_send_conn_msg(time)
                                .and_then(|enqueued| {
                                    if !enqueued {
                                        peer.enqueue_send_meta_msg(time, meta_msg.clone())
                                    } else {
                                        Ok(enqueued)
                                    }
                                })
                                .and_then(|enqueued| {
                                    if !enqueued {
                                        match p2p_state {
                                            P2pState::Pending
                                            | P2pState::PendingFull
                                            | P2pState::Ready
                                            | P2pState::ReadyFull => {}
                                            P2pState::ReadyMaxed => {
                                                peer.nack_peer(NackMotive::TooManyConnections);
                                            }
                                        }

                                        peer.enqueue_send_ack_msg(time, || {
                                            proposal
                                                .effects
                                                .choose_potential_peers_for_nack(potential_peers)
                                                .into_iter()
                                                .map(|x| x.to_string())
                                                .collect()
                                        })
                                    } else {
                                        Ok(enqueued)
                                    }
                                });
                            match result {
                                Ok(true) => {}
                                Ok(false) => break,
                                Err(err) => {
                                    slog::error!(&self.log, "Failed to enqueue send handshake message!"; "error" => format!("{:?}", err));
                                    #[cfg(test)]
                                    unreachable!(
                                        "enqueueing handshake messages should always succeed"
                                    );
                                    break;
                                }
                            }
                        }
                        Err(WriteMessageError::Pending) => break,
                        Err(err) => {
                            slog::warn!(&self.log, "Write failed!"; "description" => "error while trying to write to connected peer stream.", "error" => format!("{:?}", err));
                            self.blacklist_peer(proposal.peer);
                            break;
                        }
                    };
                }
            } else {
                // we received event for a non existant peer. Can happen
                // and its normal, unless mio is out of sync.
                self.disconnect_peer(proposal.peer);
            }
        }

        self.adjust_p2p_state(proposal.effects);
        self.periodic_react(proposal.effects);
    }
}
