// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use tezos_messages::p2p::encoding::connection::ConnectionMessage;
use tla_sm::Acceptor;

use crate::proposals::{PendingRequestMsg, PendingRequestProposal};
use crate::{
    Effects, HandshakeStep, PendingRequest, RequestState, RetriableRequestState, TezedgeState,
};

impl<'a, Efs> Acceptor<PendingRequestProposal<'a, Efs>> for TezedgeState
where
    Efs: Effects,
{
    /// Handle status update for pending request.
    fn accept(&mut self, proposal: PendingRequestProposal<'a, Efs>) {
        if let Err(_err) = self.validate_proposal(&proposal) {
            return;
        }

        if let Some(req) = self.requests.get_mut(proposal.req_id) {
            // Unexpected requests can happen in fuzzer or simulator
            // environment, but it shouldn't happen in real world.
            match &req.request {
                PendingRequest::StartListeningForNewPeers => match proposal.message {
                    PendingRequestMsg::StartListeningForNewPeersSuccess => {
                        self.requests.remove(proposal.req_id);
                        slog::info!(&self.log, "Started listening for new p2p connections"; "port" => self.config.port.to_string());
                    }
                    PendingRequestMsg::StartListeningForNewPeersError { error } => {
                        slog::warn!(&self.log, "Failed to start listening for incoming connections"; "error" => format!("{:?}", error));
                        req.status = RetriableRequestState::Retry {
                            at: self.time + self.config.periodic_react_interval,
                        };
                        // self.requests.remove(proposal.req_id);
                    }
                    msg => {
                        slog::trace!(&self.log, "Unexpected request update"; "request" => "StartListeningForNewPeers", "update" => format!("{:?}", msg))
                    }
                },
                PendingRequest::StopListeningForNewPeers => match proposal.message {
                    PendingRequestMsg::StopListeningForNewPeersSuccess => {
                        self.requests.remove(proposal.req_id);
                    }
                    msg => {
                        slog::trace!(&self.log, "Unexpected request update"; "request" => "StopListeningForNewPeers", "update" => format!("{:?}", msg))
                    }
                },
                PendingRequest::ConnectPeer { peer, .. } => match proposal.message {
                    PendingRequestMsg::ConnectPeerPending => {
                        req.status = RetriableRequestState::Pending { at: self.time };
                    }
                    PendingRequestMsg::ConnectPeerSuccess => {
                        let peer_address = *peer;
                        let sent_conn_msg = ConnectionMessage::try_new(
                            self.config.port,
                            &self.identity.public_key,
                            &self.identity.proof_of_work_stamp,
                            proposal.effects.get_nonce(&peer_address),
                            self.shell_compatibility_version.to_network_version(),
                        )
                        .unwrap();

                        if let Some(peer) = self.pending_peers.get_mut(&peer_address) {
                            peer.step = HandshakeStep::Connect {
                                sent_conn_msg,
                                sent: RequestState::Idle { at: self.time },
                                received: None,
                            };
                        }
                        self.requests.remove(proposal.req_id);
                    }
                    PendingRequestMsg::ConnectPeerError => {
                        let peer = *peer;
                        slog::warn!(&self.log, "Disconnecting peer!"; "reason" => "Initiating connection failed!");
                        self.blacklist_peer(peer);
                        self.requests.remove(proposal.req_id);
                    }
                    msg => {
                        slog::trace!(&self.log, "Unexpected request update"; "request" => "ConnectPeer", "update" => format!("{:?}", msg))
                    }
                },
                PendingRequest::DisconnectPeer { .. } => match proposal.message {
                    PendingRequestMsg::DisconnectPeerPending => {
                        req.status = RetriableRequestState::Pending { at: self.time };
                    }
                    PendingRequestMsg::DisconnectPeerSuccess => {
                        self.requests.remove(proposal.req_id);
                    }
                    msg => {
                        slog::trace!(&self.log, "Unexpected request update"; "request" => "DisconnectPeer", "update" => format!("{:?}", msg))
                    }
                },
                PendingRequest::BlacklistPeer { .. } => match proposal.message {
                    PendingRequestMsg::BlacklistPeerPending => {
                        req.status = RetriableRequestState::Pending { at: self.time };
                    }
                    PendingRequestMsg::BlacklistPeerSuccess => {
                        self.requests.remove(proposal.req_id);
                    }
                    msg => {
                        slog::trace!(&self.log, "Unexpected request update"; "request" => "BlacklistPeer", "update" => format!("{:?}", msg))
                    }
                },
                PendingRequest::PeerMessageReceived { .. } => match proposal.message {
                    PendingRequestMsg::PeerMessageReceivedNotified => {
                        self.requests.remove(proposal.req_id);
                    }
                    msg => {
                        slog::trace!(&self.log, "Unexpected request update"; "request" => "PeerMessageReceived", "update" => format!("{:?}", msg))
                    }
                },
                PendingRequest::NotifyHandshakeSuccessful { .. } => match proposal.message {
                    PendingRequestMsg::HandshakeSuccessfulNotified => {
                        self.requests.remove(proposal.req_id);
                    }
                    msg => {
                        slog::trace!(&self.log, "Unexpected request update"; "request" => "NotifyHandshakeSuccessful", "update" => format!("{:?}", msg))
                    }
                },
            }
        } else {
            slog::warn!(&self.log, "Request update received for non-existant request"; "req_id" => proposal.req_id, "message" => format!("{:?}", proposal.message));
        }

        self.adjust_p2p_state(proposal.effects);
        self.periodic_react(proposal.effects);
    }
}
