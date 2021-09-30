use redux_rs::ActionWithId;

use crate::{
    action::Action,
    peer::{
        binary_message::{
            read::peer_binary_message_read_state::PeerBinaryMessageReadState,
            write::peer_binary_message_write_state::PeerBinaryMessageWriteState,
        },
        chunk::{
            read::peer_chunk_read_state::PeerChunkReadState,
            write::peer_chunk_write_state::PeerChunkWriteState,
        },
        connection::{
            incoming::PeerConnectionIncomingState, outgoing::PeerConnectionOutgoingState,
            PeerConnectionState,
        },
        handshaking::PeerCrypto,
        PeerStatus,
    },
    State,
};

use super::{PeerHandshaking, PeerHandshakingStatus};

pub fn peer_handshaking_reducer(state: &mut State, action: &ActionWithId<Action>) {
    use Action::*;
    match &action.action {
        PeerHandshakingInit(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                match peer.status {
                    PeerStatus::Connecting(PeerConnectionState::Outgoing(
                        PeerConnectionOutgoingState::Success { token },
                    ))
                    | PeerStatus::Connecting(PeerConnectionState::Incoming(
                        PeerConnectionIncomingState::Success { token },
                    )) => {
                        peer.status = PeerStatus::Handshaking(PeerHandshaking {
                            token,
                            incoming: false,
                            status: PeerHandshakingStatus::Init,
                        });
                    }
                    _ => {}
                }
            };
        }
        PeerHandshakingConnectionMessageInit(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                match &mut peer.status {
                    PeerStatus::Handshaking(PeerHandshaking { status, .. }) => match status {
                        PeerHandshakingStatus::Init => {
                            *status = PeerHandshakingStatus::ConnectionMessageInit {
                                message: action.message.clone(),
                            }
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }
        }
        Action::PeerHandshakingConnectionMessageEncode(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                match &mut peer.status {
                    PeerStatus::Handshaking(PeerHandshaking { status, .. }) => match status {
                        PeerHandshakingStatus::ConnectionMessageInit { .. } => {
                            *status = PeerHandshakingStatus::ConnectionMessageEncoded {
                                binary_message: action.binary_message.clone(),
                            }
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }
        }
        Action::PeerHandshakingConnectionMessageWrite(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                match peer.status {
                    PeerStatus::Handshaking(PeerHandshaking { ref mut status, .. }) => match status
                    {
                        PeerHandshakingStatus::ConnectionMessageEncoded { .. } => {
                            *status = PeerHandshakingStatus::ConnectionMessageWritePending {
                                local_chunk: action.chunk.clone(),
                                chunk_state: PeerChunkWriteState::Init,
                            }
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }
        }
        Action::PeerHandshakingConnectionMessageRead(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                match peer.status {
                    PeerStatus::Handshaking(PeerHandshaking { ref mut status, .. }) => match status
                    {
                        PeerHandshakingStatus::ConnectionMessageWritePending {
                            local_chunk,
                            ..
                        } => {
                            *status = PeerHandshakingStatus::ConnectionMessageReadPending {
                                local_chunk: local_chunk.clone(),
                                chunk_state: PeerChunkReadState::Init,
                            }
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }
        }
        Action::PeerHandshakingConnectionMessageDecode(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                match peer.status {
                    PeerStatus::Handshaking(PeerHandshaking { ref mut status, .. }) => match status
                    {
                        PeerHandshakingStatus::ConnectionMessageReadPending {
                            local_chunk,
                            chunk_state: PeerChunkReadState::Ready { .. },
                            ..
                        } => {
                            *status = PeerHandshakingStatus::ConnectionMessageReady {
                                local_chunk: local_chunk.clone(),
                                remote_chunk: action.remote_chunk.clone(),
                                remote_message: action.message.clone(),
                            }
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }
        }

        Action::PeerHandshakingEncryptionInit(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                match &mut peer.status {
                    PeerStatus::Handshaking(PeerHandshaking { status, .. }) => match status {
                        PeerHandshakingStatus::ConnectionMessageReady {
                            remote_message, ..
                        } => {
                            *status = PeerHandshakingStatus::EncryptionReady {
                                crypto: action.crypto.clone(),
                                remote_connection_message: remote_message.clone(),
                            }
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }
        }

        /////////////////// metadata exchange
        Action::PeerHandshakingMetadataMessageInit(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                match &mut peer.status {
                    PeerStatus::Handshaking(PeerHandshaking { status, .. }) => match status {
                        PeerHandshakingStatus::EncryptionReady {
                            crypto,
                            remote_connection_message,
                        } => {
                            *status = PeerHandshakingStatus::MetadataMessageInit {
                                message: action.message.clone(),
                                crypto: crypto.clone(),
                                remote_connection_message: remote_connection_message.clone(),
                            }
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }
        }

        Action::PeerHandshakingMetadataMessageEncode(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                match &mut peer.status {
                    PeerStatus::Handshaking(PeerHandshaking { status, .. }) => match status {
                        PeerHandshakingStatus::MetadataMessageInit {
                            crypto,
                            remote_connection_message,
                            ..
                        } => {
                            *status = PeerHandshakingStatus::MetadataMessageEncoded {
                                binary_message: action.binary_message.clone(),
                                crypto: crypto.clone(),
                                remote_connection_message: remote_connection_message.clone(),
                            }
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }
        }

        Action::PeerHandshakingMetadataMessageWrite(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                match &mut peer.status {
                    PeerStatus::Handshaking(PeerHandshaking { status, .. }) => match status {
                        PeerHandshakingStatus::MetadataMessageEncoded {
                            binary_message,
                            crypto,
                            remote_connection_message,
                        } => {
                            let (crypto, remote_nonce) = crypto.clone().split_for_writing();
                            *status = PeerHandshakingStatus::MetadataMessageWritePending {
                                binary_message: binary_message.clone(),
                                binary_message_state: PeerBinaryMessageWriteState::Init { crypto },
                                remote_nonce,
                                remote_connection_message: remote_connection_message.clone(),
                            }
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }
        }

        Action::PeerHandshakingMetadataMessageRead(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                match &mut peer.status {
                    PeerStatus::Handshaking(PeerHandshaking { status, .. }) => match status {
                        PeerHandshakingStatus::MetadataMessageWritePending {
                            binary_message_state: PeerBinaryMessageWriteState::Ready { crypto },
                            remote_nonce,
                            remote_connection_message,
                            ..
                        } => {
                            let (crypto, local_nonce) = PeerCrypto::unsplit_after_writing(
                                crypto.clone(),
                                remote_nonce.clone(),
                            )
                            .split_for_reading();
                            *status = PeerHandshakingStatus::MetadataMessageReadPending {
                                binary_message_state: PeerBinaryMessageReadState::Init { crypto },
                                local_nonce,
                                remote_connection_message: remote_connection_message.clone(),
                            }
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }
        }

        Action::PeerHandshakingMetadataMessageDecode(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                match &mut peer.status {
                    PeerStatus::Handshaking(PeerHandshaking { status, .. }) => match status {
                        PeerHandshakingStatus::MetadataMessageReadPending {
                            binary_message_state: PeerBinaryMessageReadState::Ready { crypto, .. },
                            local_nonce,
                            remote_connection_message,
                        } => {
                            let crypto = PeerCrypto::unsplit_after_reading(
                                crypto.clone(),
                                local_nonce.clone(),
                            );
                            *status = PeerHandshakingStatus::MetadataMessageReady {
                                remote_message: action.message.clone(),
                                crypto: crypto.clone(),
                                remote_connection_message: remote_connection_message.clone(),
                            }
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }
        }

        /////////////////// ack exchange
        Action::PeerHandshakingAckMessageInit(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                match &mut peer.status {
                    PeerStatus::Handshaking(PeerHandshaking { status, .. }) => match status {
                        PeerHandshakingStatus::MetadataMessageReady {
                            remote_message,
                            crypto,
                            remote_connection_message,
                        } => {
                            *status = PeerHandshakingStatus::AckMessageInit {
                                message: action.message.clone(),
                                crypto: crypto.clone(),
                                remote_connection_message: remote_connection_message.clone(),
                                remote_metadata_message: remote_message.clone(),
                            }
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }
        }

        Action::PeerHandshakingAckMessageEncode(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                match &mut peer.status {
                    PeerStatus::Handshaking(PeerHandshaking { status, .. }) => match status {
                        PeerHandshakingStatus::AckMessageInit {
                            crypto,
                            remote_connection_message,
                            remote_metadata_message,
                            ..
                        } => {
                            *status = PeerHandshakingStatus::AckMessageEncoded {
                                binary_message: action.binary_message.clone(),
                                crypto: crypto.clone(),
                                remote_connection_message: remote_connection_message.clone(),
                                remote_metadata_message: remote_metadata_message.clone(),
                            }
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }
        }

        Action::PeerHandshakingAckMessageWrite(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                match &mut peer.status {
                    PeerStatus::Handshaking(PeerHandshaking { status, .. }) => match status {
                        PeerHandshakingStatus::AckMessageEncoded {
                            binary_message,
                            crypto,
                            remote_connection_message,
                            remote_metadata_message,
                        } => {
                            let (crypto, remote_nonce) = crypto.clone().split_for_writing();
                            *status = PeerHandshakingStatus::AckMessageWritePending {
                                binary_message: binary_message.clone(),
                                binary_message_state: PeerBinaryMessageWriteState::Init { crypto },
                                remote_nonce,
                                remote_connection_message: remote_connection_message.clone(),
                                remote_metadata_message: remote_metadata_message.clone(),
                            }
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }
        }

        Action::PeerHandshakingAckMessageRead(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                match &mut peer.status {
                    PeerStatus::Handshaking(PeerHandshaking { status, .. }) => match status {
                        PeerHandshakingStatus::AckMessageWritePending {
                            binary_message_state: PeerBinaryMessageWriteState::Ready { crypto },
                            remote_nonce,
                            remote_connection_message,
                            remote_metadata_message,
                            ..
                        } => {
                            let (crypto, local_nonce) = PeerCrypto::unsplit_after_writing(
                                crypto.clone(),
                                remote_nonce.clone(),
                            )
                            .split_for_reading();
                            *status = PeerHandshakingStatus::AckMessageReadPending {
                                binary_message_state: PeerBinaryMessageReadState::Init { crypto },
                                local_nonce,
                                remote_connection_message: remote_connection_message.clone(),
                                remote_metadata_message: remote_metadata_message.clone(),
                            }
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }
        }

        Action::PeerHandshakingAckMessageDecode(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                match &mut peer.status {
                    PeerStatus::Handshaking(PeerHandshaking { status, .. }) => match status {
                        PeerHandshakingStatus::AckMessageReadPending {
                            binary_message_state: PeerBinaryMessageReadState::Ready { crypto, .. },
                            local_nonce,
                            remote_connection_message,
                            remote_metadata_message,
                        } => {
                            let crypto = PeerCrypto::unsplit_after_reading(
                                crypto.clone(),
                                local_nonce.clone(),
                            );
                            *status = PeerHandshakingStatus::AckMessageReady {
                                remote_message: action.message.clone(),
                                crypto: crypto.clone(),
                                remote_connection_message: remote_connection_message.clone(),
                                remote_metadata_message: remote_metadata_message.clone(),
                            }
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }
        }

        _ => {}
    }
}
