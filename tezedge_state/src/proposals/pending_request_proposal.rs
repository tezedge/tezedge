use std::time::Instant;
use tla_sm::Proposal;

#[derive(Debug, Clone)]
pub enum PendingRequestMsg {
    StartListeningForNewPeersPending,
    StartListeningForNewPeersSuccess,

    StopListeningForNewPeersPending,
    StopListeningForNewPeersSuccess,

    SendPeerAckPending,
    SendPeerAckSuccess,

    ConnectPeerPending,
    ConnectPeerSuccess,
    ConnectPeerError,

    DisconnectPeerPending,
    DisconnectPeerSuccess,

    BlacklistPeerPending,
    BlacklistPeerSuccess,

    PeerMessageReceivedNotified,

    /// Handshake which was successful was notified.
    HandshakeSuccessfulNotified,
}

pub struct PendingRequestProposal<'a, Efs> {
    pub effects: &'a mut Efs,
    pub at: Instant,
    pub req_id: usize,
    pub message: PendingRequestMsg,
}

impl<'a, Efs> Proposal for PendingRequestProposal<'a, Efs> {
    fn time(&self) -> Instant {
        self.at
    }
}
