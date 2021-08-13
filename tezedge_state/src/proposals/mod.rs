use serde::{Deserialize, Serialize};
pub use tla_sm::Proposal;

mod tick_proposal;
pub use tick_proposal::*;

mod extend_potential_peers_proposal;
pub use extend_potential_peers_proposal::*;

mod new_peer_connect_proposal;
pub use new_peer_connect_proposal::*;

mod peer_readable_proposal;
pub use peer_readable_proposal::*;

mod peer_writable_proposal;
pub use peer_writable_proposal::*;

mod send_peer_message_proposal;
pub use send_peer_message_proposal::*;

mod peer_handshake_message_proposal;
pub use peer_handshake_message_proposal::*;

mod peer_message_proposal;
pub use peer_message_proposal::*;

pub mod peer_handshake_message;
pub use peer_handshake_message::{PeerHandshakeMessage, PeerHandshakeMessageError};

mod peer_disconnect_proposal;
pub use peer_disconnect_proposal::*;

mod peer_disconnected_proposal;
pub use peer_disconnected_proposal::*;

mod peer_blacklist_proposal;
pub use peer_blacklist_proposal::*;

mod pending_request_proposal;
pub use pending_request_proposal::*;

pub trait MaybeRecordedProposal {
    type Proposal: Proposal;

    fn as_proposal(self) -> Self::Proposal;
}

impl<'a, T> MaybeRecordedProposal for T
where
    T: Proposal,
{
    type Proposal = T;

    fn as_proposal(self) -> Self::Proposal {
        self
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum RecordedProposal {
    ExtendPotentialPeersProposal(RecordedExtendPotentialPeersProposal),
    NewPeerConnectProposal(RecordedNewPeerConnectProposal),
    PeerBlacklistProposal(RecordedPeerBlacklistProposal),
    PeerDisconnectProposal(RecordedPeerDisconnectProposal),
    PeerDisconnectedProposal(RecordedPeerDisconnectedProposal),
    PeerMessageProposal(RecordedPeerMessageProposal),
    PeerReadableProposal(RecordedPeerReadableProposal),
    PeerWritableProposal(RecordedPeerWritableProposal),
    PendingRequestProposal(RecordedPendingRequestProposal),
    SendPeerMessageProposal(RecordedSendPeerMessageProposal),
    TickProposal(RecordedTickProposal),
}

impl From<RecordedExtendPotentialPeersProposal> for RecordedProposal {
    fn from(proposal: RecordedExtendPotentialPeersProposal) -> Self {
        Self::ExtendPotentialPeersProposal(proposal)
    }
}

impl From<RecordedNewPeerConnectProposal> for RecordedProposal {
    fn from(proposal: RecordedNewPeerConnectProposal) -> Self {
        Self::NewPeerConnectProposal(proposal)
    }
}

impl From<RecordedPeerBlacklistProposal> for RecordedProposal {
    fn from(proposal: RecordedPeerBlacklistProposal) -> Self {
        Self::PeerBlacklistProposal(proposal)
    }
}

impl From<RecordedPeerDisconnectProposal> for RecordedProposal {
    fn from(proposal: RecordedPeerDisconnectProposal) -> Self {
        Self::PeerDisconnectProposal(proposal)
    }
}

impl From<RecordedPeerDisconnectedProposal> for RecordedProposal {
    fn from(proposal: RecordedPeerDisconnectedProposal) -> Self {
        Self::PeerDisconnectedProposal(proposal)
    }
}

impl From<RecordedPeerMessageProposal> for RecordedProposal {
    fn from(proposal: RecordedPeerMessageProposal) -> Self {
        Self::PeerMessageProposal(proposal)
    }
}

impl From<RecordedPeerReadableProposal> for RecordedProposal {
    fn from(proposal: RecordedPeerReadableProposal) -> Self {
        Self::PeerReadableProposal(proposal)
    }
}

impl From<RecordedPeerWritableProposal> for RecordedProposal {
    fn from(proposal: RecordedPeerWritableProposal) -> Self {
        Self::PeerWritableProposal(proposal)
    }
}

impl From<RecordedPendingRequestProposal> for RecordedProposal {
    fn from(proposal: RecordedPendingRequestProposal) -> Self {
        Self::PendingRequestProposal(proposal)
    }
}

impl From<RecordedSendPeerMessageProposal> for RecordedProposal {
    fn from(proposal: RecordedSendPeerMessageProposal) -> Self {
        Self::SendPeerMessageProposal(proposal)
    }
}

impl From<RecordedTickProposal> for RecordedProposal {
    fn from(proposal: RecordedTickProposal) -> Self {
        Self::TickProposal(proposal)
    }
}
