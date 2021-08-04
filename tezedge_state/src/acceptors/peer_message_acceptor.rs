use std::net::SocketAddr;
use std::sync::Arc;

use tezos_messages::p2p::encoding::prelude::{AdvertiseMessage, PeerMessage};
use tla_sm::Acceptor;

use crate::proposals::{ExtendPotentialPeersProposal, PeerMessageProposal};
use crate::{Effects, PendingRequest, PendingRequestState, RequestState, TezedgeState};

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
            #[cfg(test)]
            assert_ne!(_err, crate::InvalidProposalError::ProposalOutdated);
            return;
        }

        if let Some(peer) = self.connected_peers.get_mut(&proposal.peer) {
            // handle connected peer messages.
            match proposal.message.message() {
                PeerMessage::Bootstrap => {
                    let msg = AdvertiseMessage::new(
                        proposal
                            .effects
                            .choose_potential_peers_for_advertise(&self.potential_peers)
                            .into_iter()
                            .map(|x| x.into()),
                    );
                    peer.enqueue_send_message(msg.into());
                }
                PeerMessage::Advertise(message) => {
                    self.accept(ExtendPotentialPeersProposal {
                        effects: proposal.effects,
                        at: proposal.at,
                        peers: message
                            .id()
                            .iter()
                            .filter_map(|str_ip_port| str_ip_port.parse::<SocketAddr>().ok()),
                    });
                }
                // messages not handled in state machine for now.
                // create a request to notify proposer about the message,
                // which in turn will notify actor system.
                _ => {
                    self.requests.insert(PendingRequestState {
                        request: PendingRequest::PeerMessageReceived {
                            peer: proposal.peer,
                            message: Arc::new(proposal.message),
                        },
                        status: RequestState::Idle { at: proposal.at },
                    });
                }
            }
        } else {
            slog::warn!(&self.log, "Blacklisting peer"; "peer_address" => proposal.peer.to_string(), "reason" => "Received PeerMessage from not connected(handshake not done) or non-existant peer");
            self.blacklist_peer(proposal.at, proposal.peer);
        }

        self.adjust_p2p_state(proposal.at, proposal.effects);
        self.periodic_react(proposal.at, proposal.effects);
    }
}
