// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use networking::network_channel::PeerMessageReceived;
use redux_rs::{ActionWithId, Store};
use tezos_messages::p2p::binary_message::BinaryRead;
use tezos_messages::p2p::encoding::peer::{PeerMessage, PeerMessageResponse};
use tezos_messages::p2p::encoding::prelude::AdvertiseMessage;

use crate::peer::binary_message::read::PeerBinaryMessageReadInitAction;
use crate::peer::message::read::PeerMessageReadErrorAction;
use crate::peer::message::write::PeerMessageWriteInitAction;
use crate::peers::add::multi::PeersAddMultiAction;
use crate::peers::graylist::PeersGraylistAddressAction;
use crate::service::actors_service::{ActorsMessageTo, ActorsService};
use crate::service::{RandomnessService, Service};
use crate::{Action, State};

use super::{PeerMessageReadInitAction, PeerMessageReadSuccessAction};

pub fn peer_message_read_effects<S>(
    store: &mut Store<State, S, Action>,
    action: &ActionWithId<Action>,
) where
    S: Service,
{
    match &action.action {
        Action::PeerMessageReadInit(action) => {
            store.dispatch(
                PeerBinaryMessageReadInitAction {
                    address: action.address,
                }
                .into(),
            );
        }
        Action::PeerBinaryMessageReadReady(action) => {
            match store.state().peers.get(&action.address) {
                Some(peer) => match peer.status.as_handshaked() {
                    Some(_handshaked) => (),
                    None => return,
                },
                None => return,
            };

            match PeerMessageResponse::from_bytes(&action.message) {
                Ok(mut message) => {
                    // Set size hint to unencrypted encoded message size.
                    // Maybe we should set encrypted size instead? Since
                    // that's the actual size of data transmitted.
                    message.set_size_hint(action.message.len());

                    store.dispatch(
                        PeerMessageReadSuccessAction {
                            address: action.address,
                            message: message.into(),
                        }
                        .into(),
                    );
                }
                Err(err) => {
                    store.dispatch(
                        PeerMessageReadErrorAction {
                            address: action.address,
                            error: err.into(),
                        }
                        .into(),
                    );
                }
            }
        }
        Action::PeerMessageReadSuccess(action) => {
            store
                .service()
                .actors()
                .send(ActorsMessageTo::PeerMessageReceived(PeerMessageReceived {
                    peer_address: action.address,
                    message: action.message.clone(),
                }));

            match &action.message.message() {
                PeerMessage::Bootstrap => {
                    let potential_peers =
                        store.state.get().peers.potential_iter().collect::<Vec<_>>();
                    let advertise_peers = store
                        .service
                        .randomness()
                        .choose_potential_peers_for_advertise(&potential_peers);
                    store.dispatch(
                        PeerMessageWriteInitAction {
                            address: action.address,
                            message: PeerMessageResponse::from(AdvertiseMessage::new(
                                advertise_peers,
                            ))
                            .into(),
                        }
                        .into(),
                    );
                }
                PeerMessage::Advertise(msg) => {
                    store.dispatch(
                        PeersAddMultiAction {
                            addresses: msg.id().iter().filter_map(|x| x.parse().ok()).collect(),
                        }
                        .into(),
                    );
                }
                _ => {}
            }

            // try to read next message.
            store.dispatch(
                PeerMessageReadInitAction {
                    address: action.address,
                }
                .into(),
            );
        }
        Action::PeerMessageReadError(action) => {
            store.dispatch(
                PeersGraylistAddressAction {
                    address: action.address,
                }
                .into(),
            );
        }
        _ => {}
    }
}
