// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::convert::TryFrom;
use std::net::SocketAddr;
use std::sync::Arc;
use tezos_messages::p2p::encoding::prelude::{AdvertiseMessage, PeerMessage};
use tezos_messages::Head;
use tla_sm::Acceptor;

use crate::proposals::{ExtendPotentialPeersProposal, PeerMessageProposal};
use crate::{Effects, PendingRequest, PendingRequestState, RetriableRequestState, TezedgeState};

impl<'a, Efs> Acceptor<PeerMessageProposal<'a, Efs>> for TezedgeState
where
    Efs: Effects,
{
    /// Handle decrypted and decoded PeerMessage from connected_peer.
    ///
    /// This method isn't invoked by proposer, it's more of an internal
    /// method called, by another acceptor: Acceptor<PeerReadableProposal>.
    fn accept(&mut self, proposal: PeerMessageProposal<'a, Efs>) {
        if let Err(_err) = self.validate_proposal(&proposal) {
            return;
        }

        let TezedgeState {
            connected_peers,
            potential_peers,
            main_chain_id,
            current_head,
            log,
            ..
        } = self;

        if let Some(peer) = connected_peers.get_mut(&proposal.peer) {
            // handle connected peer messages.
            let was_handled = match proposal.message.message() {
                PeerMessage::Bootstrap => {
                    let msg = AdvertiseMessage::new(
                        proposal
                            .effects
                            .choose_potential_peers_for_advertise(&potential_peers)
                            .into_iter()
                            .map(|x| x.into()),
                    );
                    peer.enqueue_send_message(msg.into());
                    true
                }
                PeerMessage::Advertise(message) => {
                    self.accept_internal(ExtendPotentialPeersProposal {
                        effects: proposal.effects,
                        time_passed: Default::default(),
                        peers: message
                            .id()
                            .iter()
                            .filter_map(|str_ip_port| str_ip_port.parse::<SocketAddr>().ok()),
                    });
                    true
                }
                PeerMessage::CurrentBranch(message) => {
                    // check different chain?
                    if message.chain_id().ne(main_chain_id) {
                        // TODO: TE-680 - review main_chain_id validation?
                        self.disconnect_peer(proposal.peer);
                        return;
                    }

                    let remote_block_header = message.current_branch().current_head();
                    // TODO: move this block_hash calculation as a part of decoding process?
                    let remote_head = match Head::try_from(remote_block_header) {
                        Ok(head) => head,
                        Err(e) => {
                            slog::warn!(log, "Failed to calculate block_hash for block_header";
                                             "reason" => format!("{}", e));

                            // invalid block? better disconnect peer?
                            self.disconnect_peer(proposal.peer);
                            return;
                        }
                    };

                    // update current head state for peer, we want to know peer's best branch
                    peer.update_current_head(remote_head.clone());

                    // check if we can accept remote and bootstrap it
                    if !current_head.accept_remote_branch(remote_block_header, remote_head) {
                        slog::debug!(log, "Ignoring received (low) current branch head";
                                          "branch" => remote_head.block_hash().to_base58_check(),
                                          "level" => remote_block_header.level());
                        return;
                    }

                    // TODO: do handling real here and return true
                    false
                }
                // PeerMessage::CurrentHead(message) => {
                //     // different chain?
                //     if message.chain_id().ne(main_chain_id) {
                //         // TODO: TE-680 - review main_chain_id validation?
                //         self.disconnect_peer(proposal.peer);
                //         return;
                //     }
                //
                //     // update current head states
                //     let remote_head = message.current_block_header();
                //     peer.update_current_head(remote_head);
                //     self.update_remote_head(remote_head);
                //
                //     // TODO: do handling real here and return true
                //     false
                // }
                _ => false,
            };

            // messages not handled in state machine for now.
            // create a request to notify proposer about the message,
            // which in turn will notify actor system.
            if !was_handled {
                self.requests.insert(PendingRequestState {
                    request: PendingRequest::PeerMessageReceived {
                        peer: proposal.peer,
                        message: Arc::new(proposal.message),
                    },
                    status: RetriableRequestState::Idle { at: self.time },
                });
            }
        } else {
            slog::warn!(&self.log, "Blacklisting peer"; "peer_address" => proposal.peer.to_string(), "reason" => "Received PeerMessage from not connected(handshake not done) or non-existant peer");
            self.blacklist_peer(proposal.peer);
        }

        self.adjust_p2p_state(proposal.effects);
        self.periodic_react(proposal.effects);
    }
}
