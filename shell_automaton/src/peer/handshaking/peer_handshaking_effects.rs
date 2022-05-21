// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crypto::crypto_box::{CryptoKey, PrecomputedKey, PublicKey};
use crypto::nonce::generate_nonces;
use crypto::proof_of_work::{PowError, PowResult};
use networking::PeerId;
use std::sync::Arc;
use tezos_messages::p2p::binary_message::{BinaryChunk, BinaryRead, BinaryWrite};
use tezos_messages::p2p::encoding::ack::{AckMessage, NackInfo, NackMotive};
use tezos_messages::p2p::encoding::connection::ConnectionMessage;
use tezos_messages::p2p::encoding::metadata::MetadataMessage;

use crate::action::Action;
use crate::peer::binary_message::read::PeerBinaryMessageReadInitAction;
use crate::peer::binary_message::read::PeerBinaryMessageReadState;
use crate::peer::binary_message::write::PeerBinaryMessageWriteSetContentAction;
use crate::peer::chunk::read::PeerChunkReadInitAction;
use crate::peer::chunk::read::PeerChunkReadState;
use crate::peer::chunk::write::PeerChunkWriteSetContentAction;
use crate::peer::disconnection::PeerDisconnectAction;
use crate::peer::handshaking::{
    PeerHandshakingConnectionMessageEncodeAction, PeerHandshakingConnectionMessageInitAction,
    PeerHandshakingConnectionMessageWriteAction, PeerHandshakingMetadataMessageInitAction,
};
use crate::peer::message::read::PeerMessageReadInitAction;
use crate::peer::{PeerCrypto, PeerStatus};
use crate::peers::graylist::{PeerGraylistReason, PeersGraylistAddressAction};
use crate::service::actors_service::ActorsMessageTo;
use crate::service::{ActorsService, RandomnessService, Service};
use crate::{ActionWithMeta, Store};

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

fn check_proof_of_work(pow_target: f64, conn_msg_bytes: &[u8]) -> PowResult {
    if conn_msg_bytes.len() < 58 {
        Err(PowError::CheckFailed)
    } else {
        // skip first 2 bytes which are for port.
        crypto::proof_of_work::check_proof_of_work(&conn_msg_bytes[2..58], pow_target)
    }
}

pub fn peer_handshaking_effects<S>(store: &mut Store<S>, action: &ActionWithMeta)
where
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
                nonce,
                config.shell_compatibility_version.to_network_version(),
            ) {
                Ok(connection_message) => {
                    store.dispatch(PeerHandshakingConnectionMessageInitAction {
                        address: action.address,
                        message: connection_message,
                    });
                }
                Err(err) => {
                    store.dispatch(PeerHandshakingErrorAction {
                        address: action.address,
                        error: PeerHandshakingError::from(err),
                    });
                }
            }
        }
        Action::PeerHandshakingConnectionMessageInit(action) => match action.message.as_bytes() {
            Ok(binary_message) => {
                store.dispatch(PeerHandshakingConnectionMessageEncodeAction {
                    address: action.address,
                    binary_message,
                });
            }
            Err(err) => {
                store.dispatch(PeerHandshakingErrorAction {
                    address: action.address,
                    error: PeerHandshakingError::from(err),
                });
            }
        },
        Action::PeerHandshakingConnectionMessageEncode(action) => {
            if let Some(peer) = store.state.get().peers.get(&action.address) {
                if let PeerStatus::Handshaking(PeerHandshaking {
                    status: PeerHandshakingStatus::ConnectionMessageEncoded { binary_message, .. },
                    ..
                }) = &peer.status
                {
                    match BinaryChunk::from_content(binary_message) {
                        Ok(chunk) => {
                            store.dispatch(PeerHandshakingConnectionMessageWriteAction {
                                address: action.address,
                                chunk,
                            });
                        }
                        Err(err) => {
                            store.dispatch(PeerHandshakingErrorAction {
                                address: action.address,
                                error: err.into(),
                            });
                        }
                    }
                }
            }
        }
        Action::PeerHandshakingConnectionMessageWrite(action) => {
            if let Some(peer) = store.state.get().peers.get(&action.address) {
                if let PeerStatus::Handshaking(PeerHandshaking {
                    status: PeerHandshakingStatus::ConnectionMessageWritePending { local_chunk, .. },
                    ..
                }) = &peer.status
                {
                    let content = local_chunk.content().to_vec();
                    store.dispatch(PeerChunkWriteSetContentAction {
                        address: action.address,
                        content,
                    });
                }
            }
        }
        Action::PeerChunkWriteReady(action) => {
            if let Some(peer) = store.state.get().peers.get(&action.address) {
                if let PeerStatus::Handshaking(PeerHandshaking {
                    status: PeerHandshakingStatus::ConnectionMessageWritePending { .. },
                    ..
                }) = &peer.status
                {
                    store.dispatch(PeerHandshakingConnectionMessageReadAction {
                        address: action.address,
                    });
                }
            }
        }
        Action::PeerHandshakingConnectionMessageRead(action) => {
            store.dispatch(PeerChunkReadInitAction {
                address: action.address,
            });
        }
        Action::PeerChunkReadReady(action) => {
            let state = store.state.get();
            if let Some(peer) = state.peers.get(&action.address) {
                if let PeerStatus::Handshaking(PeerHandshaking {
                    status:
                        PeerHandshakingStatus::ConnectionMessageReadPending {
                            chunk_state:
                                PeerChunkReadState::Ready {
                                    chunk: remote_chunk,
                                },
                            ..
                        },
                    ..
                }) = &peer.status
                {
                    // check proof of work.
                    if let Err(err) = check_proof_of_work(state.config.pow_target, remote_chunk) {
                        return {
                            store.dispatch(PeerHandshakingErrorAction {
                                address: action.address,
                                error: err.into(),
                            });
                        };
                    }

                    let connection_message = match ConnectionMessage::from_bytes(remote_chunk) {
                        Ok(v) => v,
                        Err(err) => {
                            return {
                                store.dispatch(PeerHandshakingErrorAction {
                                    address: action.address,
                                    error: err.into(),
                                });
                            }
                        }
                    };

                    match BinaryChunk::from_content(remote_chunk) {
                        Ok(remote_chunk) => {
                            store.dispatch(PeerHandshakingConnectionMessageDecodeAction {
                                address: action.address,
                                message: connection_message,
                                remote_chunk,
                            });
                        }
                        Err(err) => {
                            store.dispatch(PeerHandshakingErrorAction {
                                address: action.address,
                                error: err.into(),
                            });
                        }
                    }
                }
            }
        }
        Action::PeerHandshakingConnectionMessageDecode(action) => {
            if let Some(peer) = store.state.get().peers.get(&action.address) {
                if let PeerStatus::Handshaking(PeerHandshaking {
                    incoming,
                    status:
                        PeerHandshakingStatus::ConnectionMessageReady {
                            local_chunk,
                            remote_chunk,
                            remote_message,
                            ..
                        },
                    ..
                }) = &peer.status
                {
                    let nonce_pair =
                        match generate_nonces(local_chunk.raw(), remote_chunk.raw(), *incoming) {
                            Ok(v) => v,
                            Err(err) => {
                                store.dispatch(PeerHandshakingErrorAction {
                                    address: action.address,
                                    error: err.into(),
                                });
                                return;
                            }
                        };

                    let public_key = match PublicKey::from_bytes(&remote_message.public_key) {
                        Ok(v) => v,
                        Err(err) => {
                            store.dispatch(PeerHandshakingErrorAction {
                                address: action.address,
                                error: err.into(),
                            });
                            return;
                        }
                    };

                    let precomputed_key = PrecomputedKey::precompute(
                        &public_key,
                        &store.state.get().config.identity.secret_key,
                    );

                    let crypto = PeerCrypto::new(precomputed_key, nonce_pair);

                    store.dispatch(PeerHandshakingEncryptionInitAction {
                        address: action.address,
                        crypto,
                    });
                }
            }
        }

        Action::PeerHandshakingEncryptionInit(action) => {
            if let Some(peer) = store.state.get().peers.get(&action.address) {
                if let PeerStatus::Handshaking(PeerHandshaking {
                    status: PeerHandshakingStatus::EncryptionReady { .. },
                    ..
                }) = &peer.status
                {
                    let config = &store.state.get().config;
                    let metadata_message =
                        MetadataMessage::new(config.disable_mempool, config.private_node);
                    store.dispatch(PeerHandshakingMetadataMessageInitAction {
                        address: action.address,
                        message: metadata_message,
                    });
                }
            }
        }

        Action::PeerHandshakingMetadataMessageInit(action) => {
            if let Some(peer) = store.state.get().peers.get(&action.address) {
                if let PeerStatus::Handshaking(PeerHandshaking {
                    status: PeerHandshakingStatus::MetadataMessageInit { message, .. },
                    ..
                }) = &peer.status
                {
                    match message.as_bytes() {
                        Ok(binary_message) => {
                            store.dispatch(PeerHandshakingMetadataMessageEncodeAction {
                                address: action.address,
                                binary_message,
                            });
                        }
                        Err(err) => {
                            store.dispatch(PeerHandshakingErrorAction {
                                address: action.address,
                                error: err.into(),
                            });
                        }
                    }
                }
            }
        }

        Action::PeerHandshakingMetadataMessageEncode(action) => {
            if let Some(peer) = store.state.get().peers.get(&action.address) {
                if let PeerStatus::Handshaking(PeerHandshaking {
                    status: PeerHandshakingStatus::MetadataMessageEncoded { .. },
                    ..
                }) = &peer.status
                {
                    store.dispatch(PeerHandshakingMetadataMessageWriteAction {
                        address: action.address,
                    });
                }
            }
        }
        Action::PeerHandshakingMetadataMessageWrite(action) => {
            if let Some(peer) = store.state.get().peers.get(&action.address) {
                if let PeerStatus::Handshaking(PeerHandshaking {
                    status:
                        PeerHandshakingStatus::MetadataMessageWritePending { binary_message, .. },
                    ..
                }) = &peer.status
                {
                    let message = binary_message.clone();
                    store.dispatch(PeerBinaryMessageWriteSetContentAction {
                        address: action.address,
                        message,
                    });
                }
            }
        }
        Action::PeerBinaryMessageWriteReady(action) => {
            if let Some(peer) = store.state.get().peers.get(&action.address) {
                match &peer.status {
                    PeerStatus::Handshaking(PeerHandshaking {
                        status: PeerHandshakingStatus::MetadataMessageWritePending { .. },
                        ..
                    }) => {
                        store.dispatch(PeerHandshakingMetadataMessageReadAction {
                            address: action.address,
                        });
                    }
                    PeerStatus::Handshaking(PeerHandshaking {
                        status: PeerHandshakingStatus::AckMessageWritePending { .. },
                        ..
                    }) => {
                        store.dispatch(PeerHandshakingAckMessageReadAction {
                            address: action.address,
                        });
                    }
                    _ => {}
                }
            }
        }
        Action::PeerHandshakingMetadataMessageRead(action) => {
            if let Some(peer) = store.state.get().peers.get(&action.address) {
                if let PeerStatus::Handshaking(PeerHandshaking {
                    status: PeerHandshakingStatus::MetadataMessageReadPending { .. },
                    ..
                }) = &peer.status
                {
                    store.dispatch(PeerBinaryMessageReadInitAction {
                        address: action.address,
                    });
                }
            }
        }
        Action::PeerBinaryMessageReadReady(action) => {
            if let Some(peer) = store.state.get().peers.get(&action.address) {
                if let PeerStatus::Handshaking(PeerHandshaking { status, .. }) = &peer.status {
                    match status {
                        PeerHandshakingStatus::MetadataMessageReadPending {
                            binary_message_state: PeerBinaryMessageReadState::Ready { message, .. },
                            ..
                        } => match MetadataMessage::from_bytes(&message) {
                            Ok(message) => {
                                store.dispatch(PeerHandshakingMetadataMessageDecodeAction {
                                    address: action.address,
                                    message,
                                });
                            }
                            Err(err) => {
                                store.dispatch(PeerHandshakingErrorAction {
                                    address: action.address,
                                    error: err.into(),
                                });
                            }
                        },
                        PeerHandshakingStatus::AckMessageReadPending {
                            binary_message_state: PeerBinaryMessageReadState::Ready { message, .. },
                            ..
                        } => match AckMessage::from_bytes(&message) {
                            Ok(message) => {
                                store.dispatch(PeerHandshakingAckMessageDecodeAction {
                                    address: action.address,
                                    message,
                                });
                            }
                            Err(err) => {
                                store.dispatch(PeerHandshakingErrorAction {
                                    address: action.address,
                                    error: err.into(),
                                });
                            }
                        },
                        _ => {}
                    }
                }
            }
        }
        Action::PeerHandshakingMetadataMessageDecode(action) => {
            let state = store.state.get();
            if let Some(peer) = state.peers.get(&action.address) {
                if let PeerStatus::Handshaking(PeerHandshaking {
                    status: PeerHandshakingStatus::MetadataMessageReady { .. },
                    nack_motive,
                    ..
                }) = &peer.status
                {
                    let message = match nack_motive.as_ref() {
                        Some(motive) => {
                            let potential_peers = state.peers.potential_iter().collect::<Vec<_>>();
                            let nack_potential_peers = store
                                .service
                                .randomness()
                                .choose_potential_peers_for_nack(&potential_peers)
                                .into_iter()
                                .map(|x| x.to_string())
                                .collect::<Vec<_>>();

                            AckMessage::Nack(NackInfo::new(*motive, &nack_potential_peers))
                        }
                        None => AckMessage::Ack,
                    };
                    store.dispatch(PeerHandshakingAckMessageInitAction {
                        address: action.address,
                        message,
                    });
                }
            }
        }

        // ack message
        Action::PeerHandshakingAckMessageInit(action) => {
            if let Some(peer) = store.state.get().peers.get(&action.address) {
                if let PeerStatus::Handshaking(PeerHandshaking {
                    status: PeerHandshakingStatus::AckMessageInit { message, .. },
                    ..
                }) = &peer.status
                {
                    match message.as_bytes() {
                        Ok(binary_message) => {
                            store.dispatch(PeerHandshakingAckMessageEncodeAction {
                                address: action.address,
                                binary_message,
                            });
                        }
                        Err(err) => {
                            store.dispatch(PeerHandshakingErrorAction {
                                address: action.address,
                                error: err.into(),
                            });
                        }
                    }
                }
            }
        }

        Action::PeerHandshakingAckMessageEncode(action) => {
            if let Some(peer) = store.state.get().peers.get(&action.address) {
                if let PeerStatus::Handshaking(PeerHandshaking {
                    status: PeerHandshakingStatus::AckMessageEncoded { .. },
                    ..
                }) = &peer.status
                {
                    store.dispatch(PeerHandshakingAckMessageWriteAction {
                        address: action.address,
                    });
                }
            }
        }
        Action::PeerHandshakingAckMessageWrite(action) => {
            if let Some(peer) = store.state.get().peers.get(&action.address) {
                if let PeerStatus::Handshaking(PeerHandshaking {
                    status: PeerHandshakingStatus::AckMessageWritePending { binary_message, .. },
                    ..
                }) = &peer.status
                {
                    let message = binary_message.clone();
                    store.dispatch(PeerBinaryMessageWriteSetContentAction {
                        address: action.address,
                        message,
                    });
                }
            }
        }
        // see above
        // Action::PeerBinaryMessageWriteReady(action) => {}
        Action::PeerHandshakingAckMessageRead(action) => {
            if let Some(peer) = store.state.get().peers.get(&action.address) {
                if let PeerStatus::Handshaking(PeerHandshaking {
                    status: PeerHandshakingStatus::AckMessageReadPending { .. },
                    ..
                }) = &peer.status
                {
                    store.dispatch(PeerBinaryMessageReadInitAction {
                        address: action.address,
                    });
                }
            }
        }

        Action::PeerHandshakingAckMessageDecode(action) => {
            if let Some(peer) = store.state.get().peers.get(&action.address) {
                if let PeerStatus::Handshaking(PeerHandshaking {
                    status: PeerHandshakingStatus::AckMessageReady { remote_message, .. },
                    ..
                }) = &peer.status
                {
                    let nack_motive = match remote_message {
                        AckMessage::Ack => {
                            store.dispatch(PeerHandshakingFinishAction {
                                address: action.address,
                            });
                            return;
                        }
                        // TODO: use potential peers in nack message.
                        AckMessage::Nack(info) => match *info.motive() {
                            NackMotive::AlreadyConnected => {
                                store.dispatch(PeerDisconnectAction {
                                    address: action.address,
                                });
                                return;
                            }
                            motive => motive,
                        },
                        AckMessage::NackV0 => NackMotive::NoMotive,
                    };
                    // peer nacked us so we should graylist him.
                    store.dispatch(PeersGraylistAddressAction {
                        address: action.address,
                        reason: PeerGraylistReason::NackReceived(nack_motive),
                    });
                }
            }
        }

        Action::PeerHandshakingFinish(action) => {
            let state = store.state.get();

            let peer_handshaked = match state.peers.get(&action.address) {
                Some(peer) => match &peer.status {
                    PeerStatus::Handshaked(v) => v,
                    status => {
                        if let PeerStatus::Handshaking(handshaking) = status {
                            match handshaking.nack_motive {
                                Some(NackMotive::AlreadyConnected) => {
                                    store.dispatch(PeerDisconnectAction {
                                        address: action.address,
                                    });
                                    return;
                                }
                                Some(motive) => {
                                    store.dispatch(PeersGraylistAddressAction {
                                        address: action.address,
                                        reason: PeerGraylistReason::NackSent(motive),
                                    });
                                    return;
                                }
                                None => {}
                            }
                        }
                        store.dispatch(PeersGraylistAddressAction {
                            address: action.address,
                            reason: PeerGraylistReason::Unknown,
                        });
                        return;
                    }
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

            store.dispatch(PeerMessageReadInitAction {
                address: action.address,
            });
        }

        Action::PeerHandshakingError(action) => {
            store.dispatch(PeersGraylistAddressAction {
                address: action.address,
                reason: PeerGraylistReason::HandshakeError,
            });
        }
        _ => {}
    }
}
