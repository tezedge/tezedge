// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::VecDeque;
use std::io;
use std::net::SocketAddr;
use std::time::Duration;

use shell_automaton::event::Event;
use shell_automaton::peer::PeerToken;
use shell_automaton::service::MioService;

pub(super) mod chunking;

mod peer_mocked;
pub use peer_mocked::*;

mod peer_mocked_id;
pub use peer_mocked_id::*;
use shell_automaton::service::mio_service::{MioPeerRefMut, PeerConnectionIncomingAcceptError};
use tezos_identity::Identity;

#[derive(Clone)]
pub struct MioServiceMocked {
    listening_for_incoming_connections: bool,
    buffer: Vec<u8>,
    peers: Vec<MioPeerMocked>,
    /// Backlog of mocked incoming connections.
    backlog: VecDeque<MioPeerMockedId>,
}

impl MioServiceMocked {
    pub fn new(_: SocketAddr, buffer_size: usize) -> Self {
        Self {
            listening_for_incoming_connections: false,
            buffer: vec![0; buffer_size],
            peers: vec![],
            backlog: VecDeque::new(),
        }
    }

    pub fn peer_init(&mut self, pow_target: f64) -> MioPeerMockedId {
        let peer_id = MioPeerMockedId::new_unchecked(self.peers.len());
        self.peers.push(MioPeerMocked::new(
            peer_id.to_ipv4(),
            MioPeerStreamMocked::new(pow_target),
        ));
        peer_id
    }

    pub fn peer_init_with_identity(&mut self, identity: Identity) -> MioPeerMockedId {
        let peer_id = MioPeerMockedId::new_unchecked(self.peers.len());
        self.peers.push(MioPeerMocked::new(
            peer_id.to_ipv4(),
            MioPeerStreamMocked::with_identity(identity),
        ));
        peer_id
    }

    /// Panics if peer with such id is not found.
    pub fn peer(&mut self, id: MioPeerMockedId) -> &mut MioPeerMocked {
        &mut self.peers[id.index()]
    }

    pub fn backlog_push(&mut self, id: MioPeerMockedId) {
        self.backlog.push_back(id);
    }
}

impl MioService for MioServiceMocked {
    type PeerStream = MioPeerStreamMocked;
    type Events = ();
    type InternalEvent = ();

    fn wait_for_events(&mut self, _: &mut Self::Events, _: Option<Duration>) {
        unimplemented!()
    }

    fn transform_event(&mut self, _: &Self::InternalEvent) -> Event {
        unimplemented!()
    }

    fn peer_connection_incoming_listen_start(&mut self) -> io::Result<()> {
        self.listening_for_incoming_connections = true;
        Ok(())
    }

    fn peer_connection_incoming_listen_stop(&mut self) {
        self.listening_for_incoming_connections = false;
    }

    fn peer_connection_incoming_accept(
        &mut self,
    ) -> Result<(PeerToken, MioPeerRefMut<Self::PeerStream>), PeerConnectionIncomingAcceptError>
    {
        if self.listening_for_incoming_connections {
            return Err(PeerConnectionIncomingAcceptError::ServerNotListening);
        }

        let buffer = &mut self.buffer;

        if let Some(peer_id) = self.backlog.pop_front() {
            let peer = &mut self.peers[peer_id.index()].stream;
            return match peer.conn_state() {
                ConnectedState::OutgoingInit => {
                    *peer.conn_state_mut() = ConnectedState::OutgoingAccepted;
                    Ok((
                        peer_id.to_token(),
                        MioPeerRefMut::new(buffer, peer_id.to_ipv4(), peer),
                    ))
                }
                _ => panic!("mio peer in invalid state {:?}", peer.conn_state()),
            };
        }

        Err(PeerConnectionIncomingAcceptError::WouldBlock)
    }

    fn peer_connection_init(&mut self, address: SocketAddr) -> io::Result<PeerToken> {
        let peer_id = MioPeerMockedId::from_ipv4(&address);
        let conn_state = self.peer(peer_id).stream.conn_state_mut();

        if !matches!(conn_state, ConnectedState::Disconnected) {
            panic!(
                "trying to connect to already connected peer: {:?}",
                conn_state
            );
        }

        *conn_state = ConnectedState::IncomingInit;

        Ok(peer_id.to_token())
    }

    fn peer_disconnect(&mut self, token: PeerToken) {
        *self.peer(token.into()).stream.conn_state_mut() = ConnectedState::Disconnected;
    }

    fn peer_get(&mut self, token: PeerToken) -> Option<MioPeerRefMut<Self::PeerStream>> {
        let buffer = &mut self.buffer;
        let peer_id = MioPeerMockedId::from_token(&token);
        let peer = &mut self.peers[peer_id.index()].stream;
        let peer_ref = MioPeerRefMut::new(buffer, peer_id.to_ipv4(), peer);

        match peer_ref.stream.conn_state() {
            ConnectedState::Disconnected => None,
            ConnectedState::IncomingInit => Some(peer_ref),
            ConnectedState::IncomingConnected => Some(peer_ref),
            ConnectedState::OutgoingInit => None,
            ConnectedState::OutgoingAccepted => Some(peer_ref),
            ConnectedState::OutgoingConnected => Some(peer_ref),
        }
    }
}
