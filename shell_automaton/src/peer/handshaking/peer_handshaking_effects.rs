use crypto::crypto_box::{CryptoKey, PrecomputedKey, PublicKey};
use crypto::nonce::{generate_nonces, NoncePair};
use networking::PeerId;
use redux_rs::{ActionWithId, Store};
use std::sync::Arc;
use tezos_messages::p2p::binary_message::{BinaryChunk, BinaryRead, BinaryWrite};
use tezos_messages::p2p::encoding::ack::AckMessage;
use tezos_messages::p2p::encoding::connection::ConnectionMessage;
use tezos_messages::p2p::encoding::metadata::MetadataMessage;
use tezos_messages::p2p::encoding::peer::{PeerMessage, PeerMessageResponse};

use crate::action::Action;
use crate::peer::binary_message::read::PeerBinaryMessageReadInitAction;
use crate::peer::binary_message::read::PeerBinaryMessageReadState;
use crate::peer::binary_message::write::PeerBinaryMessageWriteSetContentAction;
use crate::peer::chunk::read::PeerChunkReadInitAction;
use crate::peer::chunk::read::PeerChunkReadState;
use crate::peer::chunk::write::PeerChunkWriteSetContentAction;
use crate::peer::handshaking::{
    PeerHandshakingConnectionMessageEncodeAction, PeerHandshakingConnectionMessageInitAction,
    PeerHandshakingConnectionMessageWriteAction, PeerHandshakingMetadataMessageInitAction,
};
use crate::peer::message::read::PeerMessageReadInitAction;
use crate::peer::message::write::PeerMessageWriteInitAction;
use crate::peer::{PeerCrypto, PeerStatus, PeerTryReadAction, PeerTryWriteAction};
use crate::service::actors_service::ActorsMessageTo;
use crate::service::{ActorsService, RandomnessService, Service};
use crate::State;

use super::{
    PeerHandshaking, PeerHandshakingAckMessageDecodeAction, PeerHandshakingAckMessageEncodeAction,
    PeerHandshakingAckMessageInitAction, PeerHandshakingAckMessageReadAction,
    PeerHandshakingAckMessageWriteAction, PeerHandshakingConnectionMessageDecodeAction,
    PeerHandshakingConnectionMessageReadAction, PeerHandshakingEncryptionInitAction,
    PeerHandshakingError, PeerHandshakingErrorAction, PeerHandshakingFinishAction,
    PeerHandshakingMetadataMessageDecodeAction, PeerHandshakingMetadataMessageEncodeAction,
    PeerHandshakingMetadataMessageReadAction, PeerHandshakingMetadataMessageWriteAction,
    PeerHandshakingStatus,
};

pub fn peer_handshaking_effects<S>(
    store: &mut Store<State, S, Action>,
    action: &ActionWithId<Action>,
) where
    S: Service,
{
    match &action.action {
        Action::PeerHandshakingInit(action) => {
            let nonce = store.service().randomness().get_nonce(action.address);
            let config = &store.state().config;
            match ConnectionMessage::try_new(
                config.port,
                &config.identity.public_key,
                &config.identity.proof_of_work_stamp,
                nonce.clone(),
                config.shell_compatibility_version.to_network_version(),
            ) {
                Ok(connection_message) => store.dispatch(
                    PeerHandshakingConnectionMessageInitAction {
                        address: action.address,
                        message: connection_message,
                    }
                    .into(),
                ),
                Err(err) => store.dispatch(
                    PeerHandshakingErrorAction {
                        address: action.address,
                        error: PeerHandshakingError::from(err),
                    }
                    .into(),
                ),
            }
        }
        Action::PeerHandshakingConnectionMessageInit(action) => match action.message.as_bytes() {
            Ok(binary_message) => store.dispatch(
                PeerHandshakingConnectionMessageEncodeAction {
                    address: action.address,
                    binary_message,
                }
                .into(),
            ),
            Err(err) => store.dispatch(
                PeerHandshakingErrorAction {
                    address: action.address,
                    error: PeerHandshakingError::from(err),
                }
                .into(),
            ),
        },
        Action::PeerHandshakingConnectionMessageEncode(action) => {
            if let Some(peer) = store.state.get().peers.get(&action.address) {
                match &peer.status {
                    PeerStatus::Handshaking(PeerHandshaking {
                        status: PeerHandshakingStatus::ConnectionMessageEncoded { binary_message },
                        ..
                    }) => match BinaryChunk::from_content(&binary_message) {
                        Ok(chunk) => store.dispatch(
                            PeerHandshakingConnectionMessageWriteAction {
                                address: action.address,
                                chunk,
                            }
                            .into(),
                        ),
                        Err(err) => store.dispatch(
                            PeerHandshakingErrorAction {
                                address: action.address,
                                error: err.into(),
                            }
                            .into(),
                        ),
                    },
                    _ => {}
                }
            }
        }
        Action::PeerHandshakingConnectionMessageWrite(action) => {
            if let Some(peer) = store.state.get().peers.get(&action.address) {
                match &peer.status {
                    PeerStatus::Handshaking(PeerHandshaking {
                        status:
                            PeerHandshakingStatus::ConnectionMessageWritePending { local_chunk, .. },
                        ..
                    }) => {
                        let content = local_chunk.content().to_vec();
                        store.dispatch(
                            PeerChunkWriteSetContentAction {
                                address: action.address,
                                content,
                            }
                            .into(),
                        );
                    }
                    _ => {}
                }
            }
        }
        Action::PeerChunkWriteReady(action) => {
            if let Some(peer) = store.state.get().peers.get(&action.address) {
                match &peer.status {
                    PeerStatus::Handshaking(PeerHandshaking {
                        status: PeerHandshakingStatus::ConnectionMessageWritePending { .. },
                        ..
                    }) => {
                        store.dispatch(
                            PeerHandshakingConnectionMessageReadAction {
                                address: action.address,
                            }
                            .into(),
                        );
                    }
                    _ => {}
                }
            }
        }
        Action::PeerHandshakingConnectionMessageRead(action) => {
            store.dispatch(
                PeerChunkReadInitAction {
                    address: action.address,
                }
                .into(),
            );
        }
        Action::PeerChunkReadReady(action) => {
            if let Some(peer) = store.state.get().peers.get(&action.address) {
                match &peer.status {
                    PeerStatus::Handshaking(PeerHandshaking {
                        status:
                            PeerHandshakingStatus::ConnectionMessageReadPending {
                                chunk_state:
                                    PeerChunkReadState::Ready {
                                        chunk: remote_chunk,
                                    },
                                ..
                            },
                        ..
                    }) => match ConnectionMessage::from_bytes(remote_chunk) {
                        Ok(connection_message) => match BinaryChunk::from_content(&remote_chunk) {
                            Ok(remote_chunk) => store.dispatch(
                                PeerHandshakingConnectionMessageDecodeAction {
                                    address: action.address,
                                    message: connection_message,
                                    remote_chunk,
                                }
                                .into(),
                            ),

                            Err(err) => store.dispatch(
                                PeerHandshakingErrorAction {
                                    address: action.address,
                                    error: err.into(),
                                }
                                .into(),
                            ),
                        },
                        Err(err) => store.dispatch(
                            PeerHandshakingErrorAction {
                                address: action.address,
                                error: err.into(),
                            }
                            .into(),
                        ),
                    },
                    _ => {}
                }
            }
        }
        Action::PeerHandshakingConnectionMessageDecode(action) => {
            if let Some(peer) = store.state.get().peers.get(&action.address) {
                match &peer.status {
                    PeerStatus::Handshaking(PeerHandshaking {
                        incoming,
                        status:
                            PeerHandshakingStatus::ConnectionMessageReady {
                                local_chunk,
                                remote_chunk,
                                remote_message,
                            },
                        ..
                    }) => {
                        let nonce_pair =
                            match generate_nonces(local_chunk.raw(), remote_chunk.raw(), *incoming) {
                                Ok(v) => v,
                                Err(err) => {
                                    store.dispatch(
                                        PeerHandshakingErrorAction {
                                            address: action.address,
                                            error: err.into(),
                                        }
                                        .into(),
                                    );
                                    return;
                                }
                            };

                        let public_key = match PublicKey::from_bytes(&remote_message.public_key) {
                            Ok(v) => v,
                            Err(err) => {
                                store.dispatch(
                                    PeerHandshakingErrorAction {
                                        address: action.address,
                                        error: err.into(),
                                    }
                                    .into(),
                                );
                                return;
                            }
                        };

                        let precomputed_key = PrecomputedKey::precompute(
                            &public_key,
                            &store.state.get().config.identity.secret_key,
                        );

                        let crypto = PeerCrypto::new(precomputed_key, nonce_pair);

                        store.dispatch(
                            PeerHandshakingEncryptionInitAction {
                                address: action.address,
                                crypto,
                            }
                            .into(),
                        );
                    }
                    _ => {}
                }
            }
        }

        Action::PeerHandshakingEncryptionInit(action) => {
            if let Some(peer) = store.state.get().peers.get(&action.address) {
                match &peer.status {
                    PeerStatus::Handshaking(PeerHandshaking {
                        status: PeerHandshakingStatus::EncryptionReady { .. },
                        ..
                    }) => {
                        let config = &store.state.get().config;
                        let metadata_message =
                            MetadataMessage::new(config.disable_mempool, config.private_node);
                        store.dispatch(
                            PeerHandshakingMetadataMessageInitAction {
                                address: action.address,
                                message: metadata_message,
                            }
                            .into(),
                        );
                    }
                    _ => {}
                }
            }
        }

        Action::PeerHandshakingMetadataMessageInit(action) => {
            if let Some(peer) = store.state.get().peers.get(&action.address) {
                match &peer.status {
                    PeerStatus::Handshaking(PeerHandshaking {
                        status: PeerHandshakingStatus::MetadataMessageInit { message, .. },
                        ..
                    }) => match message.as_bytes() {
                        Ok(binary_message) => store.dispatch(
                            PeerHandshakingMetadataMessageEncodeAction {
                                address: action.address,
                                binary_message,
                            }
                            .into(),
                        ),
                        Err(err) => store.dispatch(
                            PeerHandshakingErrorAction {
                                address: action.address,
                                error: err.into(),
                            }
                            .into(),
                        ),
                    },
                    _ => {}
                }
            }
        }

        Action::PeerHandshakingMetadataMessageEncode(action) => {
            if let Some(peer) = store.state.get().peers.get(&action.address) {
                match &peer.status {
                    PeerStatus::Handshaking(PeerHandshaking {
                        status: PeerHandshakingStatus::MetadataMessageEncoded { .. },
                        ..
                    }) => store.dispatch(
                        PeerHandshakingMetadataMessageWriteAction {
                            address: action.address,
                        }
                        .into(),
                    ),
                    _ => {}
                }
            }
        }
        Action::PeerHandshakingMetadataMessageWrite(action) => {
            if let Some(peer) = store.state.get().peers.get(&action.address) {
                match &peer.status {
                    PeerStatus::Handshaking(PeerHandshaking {
                        status:
                            PeerHandshakingStatus::MetadataMessageWritePending {
                                binary_message, ..
                            },
                        ..
                    }) => {
                        let message = binary_message.clone();
                        store.dispatch(
                            PeerBinaryMessageWriteSetContentAction {
                                address: action.address,
                                message,
                            }
                            .into(),
                        )
                    }
                    _ => {}
                }
            }
        }
        Action::PeerBinaryMessageWriteReady(action) => {
            if let Some(peer) = store.state.get().peers.get(&action.address) {
                match &peer.status {
                    PeerStatus::Handshaking(PeerHandshaking {
                        status: PeerHandshakingStatus::MetadataMessageWritePending { .. },
                        ..
                    }) => store.dispatch(
                        PeerHandshakingMetadataMessageReadAction {
                            address: action.address,
                        }
                        .into(),
                    ),
                    PeerStatus::Handshaking(PeerHandshaking {
                        status: PeerHandshakingStatus::AckMessageWritePending { .. },
                        ..
                    }) => store.dispatch(
                        PeerHandshakingAckMessageReadAction {
                            address: action.address,
                        }
                        .into(),
                    ),
                    _ => {}
                }
            }
        }
        Action::PeerHandshakingMetadataMessageRead(action) => {
            if let Some(peer) = store.state.get().peers.get(&action.address) {
                match &peer.status {
                    PeerStatus::Handshaking(PeerHandshaking {
                        status: PeerHandshakingStatus::MetadataMessageReadPending { .. },
                        ..
                    }) => store.dispatch(
                        PeerBinaryMessageReadInitAction {
                            address: action.address,
                        }
                        .into(),
                    ),
                    _ => {}
                }
            }
        }
        Action::PeerBinaryMessageReadReady(action) => {
            if let Some(peer) = store.state.get().peers.get(&action.address) {
                match &peer.status {
                    PeerStatus::Handshaking(PeerHandshaking { status, .. }) => match status {
                        PeerHandshakingStatus::MetadataMessageReadPending {
                            binary_message_state: PeerBinaryMessageReadState::Ready { message, .. },
                            ..
                        } => match MetadataMessage::from_bytes(&message) {
                            Ok(message) => store.dispatch(
                                PeerHandshakingMetadataMessageDecodeAction {
                                    address: action.address,
                                    message,
                                }
                                .into(),
                            ),
                            Err(err) => store.dispatch(
                                PeerHandshakingErrorAction {
                                    address: action.address,
                                    error: err.into(),
                                }
                                .into(),
                            ),
                        },
                        PeerHandshakingStatus::AckMessageReadPending {
                            binary_message_state: PeerBinaryMessageReadState::Ready { message, .. },
                            ..
                        } => match AckMessage::from_bytes(&message) {
                            Ok(message) => store.dispatch(
                                PeerHandshakingAckMessageDecodeAction {
                                    address: action.address,
                                    message,
                                }
                                .into(),
                            ),
                            Err(err) => store.dispatch(
                                PeerHandshakingErrorAction {
                                    address: action.address,
                                    error: err.into(),
                                }
                                .into(),
                            ),
                        },
                        _ => {}
                    },
                    _ => {}
                }
            }
        }
        Action::PeerHandshakingMetadataMessageDecode(action) => {
            if let Some(peer) = store.state.get().peers.get(&action.address) {
                match &peer.status {
                    PeerStatus::Handshaking(PeerHandshaking {
                        status: PeerHandshakingStatus::MetadataMessageReady { .. },
                        ..
                    }) => {
                        let message = AckMessage::Ack;
                        store.dispatch(
                            PeerHandshakingAckMessageInitAction {
                                address: action.address,
                                message,
                            }
                            .into(),
                        )
                    }
                    _ => {}
                }
            }
        }

        // ack message
        Action::PeerHandshakingAckMessageInit(action) => {
            if let Some(peer) = store.state.get().peers.get(&action.address) {
                match &peer.status {
                    PeerStatus::Handshaking(PeerHandshaking {
                        status: PeerHandshakingStatus::AckMessageInit { message, .. },
                        ..
                    }) => match message.as_bytes() {
                        Ok(binary_message) => store.dispatch(
                            PeerHandshakingAckMessageEncodeAction {
                                address: action.address,
                                binary_message,
                            }
                            .into(),
                        ),
                        Err(err) => store.dispatch(
                            PeerHandshakingErrorAction {
                                address: action.address,
                                error: err.into(),
                            }
                            .into(),
                        ),
                    },
                    _ => {}
                }
            }
        }

        Action::PeerHandshakingAckMessageEncode(action) => {
            if let Some(peer) = store.state.get().peers.get(&action.address) {
                match &peer.status {
                    PeerStatus::Handshaking(PeerHandshaking {
                        status: PeerHandshakingStatus::AckMessageEncoded { .. },
                        ..
                    }) => store.dispatch(
                        PeerHandshakingAckMessageWriteAction {
                            address: action.address,
                        }
                        .into(),
                    ),
                    _ => {}
                }
            }
        }
        Action::PeerHandshakingAckMessageWrite(action) => {
            if let Some(peer) = store.state.get().peers.get(&action.address) {
                match &peer.status {
                    PeerStatus::Handshaking(PeerHandshaking {
                        status: PeerHandshakingStatus::AckMessageWritePending { binary_message, .. },
                        ..
                    }) => {
                        let message = binary_message.clone();
                        store.dispatch(
                            PeerBinaryMessageWriteSetContentAction {
                                address: action.address,
                                message,
                            }
                            .into(),
                        )
                    }
                    _ => {}
                }
            }
        }
        // see above
        // Action::PeerBinaryMessageWriteReady(action) => {}
        Action::PeerHandshakingAckMessageRead(action) => {
            if let Some(peer) = store.state.get().peers.get(&action.address) {
                match &peer.status {
                    PeerStatus::Handshaking(PeerHandshaking {
                        status: PeerHandshakingStatus::AckMessageReadPending { .. },
                        ..
                    }) => store.dispatch(
                        PeerBinaryMessageReadInitAction {
                            address: action.address,
                        }
                        .into(),
                    ),
                    _ => {}
                }
            }
        }

        Action::PeerHandshakingAckMessageDecode(action) => {
            if let Some(peer) = store.state.get().peers.get(&action.address) {
                match &peer.status {
                    PeerStatus::Handshaking(PeerHandshaking {
                        status: PeerHandshakingStatus::AckMessageReady { .. },
                        ..
                    }) => store.dispatch(
                        PeerHandshakingFinishAction {
                            address: action.address,
                        }
                        .into(),
                    ),
                    _ => {}
                }
            }
        }

        Action::PeerHandshakingFinish(action) => {
            let peer_handshaked = match store.state.get().peers.get(&action.address) {
                Some(peer) => match &peer.status {
                    PeerStatus::Handshaked(v) => v,
                    _ => return,
                },
                None => return,
            };
            store.service.actors().send(ActorsMessageTo::PeerHandshaked(
                Arc::new(PeerId {
                    address: action.address,
                    public_key_hash: peer_handshaked.public_key_hash.clone(),
                }),
                MetadataMessage::new(
                    peer_handshaked.disable_mempool,
                    peer_handshaked.private_node,
                ),
                Arc::new(peer_handshaked.version.clone()),
            ));

            store.dispatch(
                PeerMessageReadInitAction {
                    address: action.address,
                }
                .into(),
            );
        }

        _ => {}
    }
}
