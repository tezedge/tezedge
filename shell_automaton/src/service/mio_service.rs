// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use mio::net::{TcpListener, TcpSocket, TcpStream};
use serde::{Deserialize, Serialize};
use slab::Slab;
use std::io::{self, Read, Write};
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use crate::event::{Event, P2pPeerEvent, P2pPeerUnknownEvent, P2pServerEvent, WakeupEvent};
use crate::io_error_kind::IOErrorKind;
use crate::peer::PeerToken;

pub type MioInternalEvent = mio::event::Event;
pub type MioInternalEventsContainer = mio::Events;

/// We will receive listening socket server events with this token, when
/// there are incoming connections that need to be accepted.
pub const MIO_SERVER_TOKEN: mio::Token = mio::Token(usize::MAX);

/// Event with this token will be issued, when `mio::Waker::wake` is called.
pub const MIO_WAKE_TOKEN: mio::Token = mio::Token(usize::MAX - 1);

pub type MioPeerDefault = MioPeer<TcpStream>;

pub trait MioService {
    type PeerStream: Read + Write;
    type Events;
    type InternalEvent;

    fn wait_for_events(&mut self, events: &mut Self::Events, timeout: Option<Duration>);
    fn transform_event(&mut self, event: &Self::InternalEvent) -> Event;

    fn peer_connection_incoming_listen_start(&mut self) -> io::Result<()>;
    fn peer_connection_incoming_listen_stop(&mut self);
    fn peer_connection_incoming_accept(
        &mut self,
    ) -> Result<(PeerToken, &mut MioPeer<Self::PeerStream>), PeerConnectionIncomingAcceptError>;

    fn peer_connection_init(&mut self, address: SocketAddr) -> io::Result<PeerToken>;
    fn peer_disconnect(&mut self, token: PeerToken);

    fn peer_get(&mut self, token: PeerToken) -> Option<&mut MioPeer<Self::PeerStream>>;
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PeerConnectionIncomingAcceptError {
    /// Not ready to make further progress.
    WouldBlock,

    /// Server isn't listening for incoming peer connections.
    ServerNotListening,

    /// Failure when trying to accept peer connection.
    Accept(IOErrorKind),

    /// Failure when registering peer socket in [mio::Registry].
    PollRegister(IOErrorKind),
}

impl PeerConnectionIncomingAcceptError {
    pub fn accept_error(error: io::Error) -> Self {
        match error.kind() {
            io::ErrorKind::WouldBlock => Self::WouldBlock,
            err_kind => Self::Accept(err_kind.into()),
        }
    }

    pub fn poll_register_error(error: io::Error) -> Self {
        match error.kind() {
            io::ErrorKind::WouldBlock => Self::WouldBlock,
            err_kind => Self::PollRegister(err_kind.into()),
        }
    }
}

pub struct MioPeer<Stream> {
    pub address: SocketAddr,
    pub stream: Stream,
}

impl<Stream> MioPeer<Stream> {
    pub fn new(address: SocketAddr, stream: Stream) -> Self {
        Self { address, stream }
    }
}

pub struct MioServiceDefault {
    listen_addr: SocketAddr,

    /// Backlog size for incoming connections.
    ///
    /// Incoming connections are put in kernel's backlog, this is limit
    /// for that backlog. So if queue of incoming connections get to
    /// this limit, more connections will be instantly rejected.
    backlog_size: u32,

    poll: mio::Poll,
    waker: Arc<mio::Waker>,
    server: Option<TcpListener>,

    peers: Slab<MioPeer<TcpStream>>,
}

impl MioServiceDefault {
    const DEFAULT_BACKLOG_SIZE: u32 = 255;

    pub fn new(listen_addr: SocketAddr) -> Self {
        let poll = mio::Poll::new().expect("failed to create mio::Poll");
        let waker = Arc::new(
            mio::Waker::new(poll.registry(), MIO_WAKE_TOKEN).expect("failed to create mio::Waker"),
        );
        Self {
            listen_addr,
            backlog_size: Self::DEFAULT_BACKLOG_SIZE,
            poll,
            waker,
            server: None,
            peers: Slab::new(),
        }
    }

    /// Waker can be used to wake up mio from another thread.
    pub fn waker(&self) -> Arc<mio::Waker> {
        self.waker.clone()
    }
}

impl MioService for MioServiceDefault {
    type PeerStream = TcpStream;
    type Events = MioInternalEventsContainer;
    type InternalEvent = MioInternalEvent;

    fn wait_for_events(&mut self, events: &mut Self::Events, timeout: Option<Duration>) {
        match self.poll.poll(events, timeout) {
            Ok(_) => {}
            Err(err) => {
                eprintln!("Mio Poll::poll failed! Error: {:?}", err);
            }
        };
    }

    fn transform_event(&mut self, event: &Self::InternalEvent) -> Event {
        if event.token() == MIO_WAKE_TOKEN {
            WakeupEvent {}.into()
        } else if event.token() == MIO_SERVER_TOKEN {
            P2pServerEvent {}.into()
        } else {
            let is_closed = event.is_error() || event.is_read_closed() || event.is_write_closed();
            let peer_token = PeerToken::new_unchecked(event.token().0);

            match self.peer_get(peer_token) {
                Some(peer) => P2pPeerEvent {
                    token: peer_token,
                    address: peer.address,

                    is_readable: event.is_readable(),
                    is_writable: event.is_writable(),
                    is_closed,
                }
                .into(),
                None => P2pPeerUnknownEvent {
                    token: peer_token,

                    is_readable: event.is_readable(),
                    is_writable: event.is_writable(),
                    is_closed,
                }
                .into(),
            }
        }
    }

    fn peer_connection_incoming_listen_start(&mut self) -> io::Result<()> {
        if self.server.is_none() {
            let socket = match self.listen_addr.ip() {
                IpAddr::V4(_) => TcpSocket::new_v4()?,
                IpAddr::V6(_) => TcpSocket::new_v6()?,
            };

            // read more details about why not on windows in mio docs
            // for [mio::TcpListener::bind].
            #[cfg(not(windows))]
            socket.set_reuseaddr(true)?;

            socket.bind(self.listen_addr)?;

            let mut server = socket.listen(self.backlog_size)?;

            self.poll.registry().register(
                &mut server,
                MIO_SERVER_TOKEN,
                mio::Interest::READABLE,
            )?;

            self.server = Some(server);
        }
        Ok(())
    }

    fn peer_connection_incoming_listen_stop(&mut self) {
        drop(self.server.take());
    }

    fn peer_connection_incoming_accept(
        &mut self,
    ) -> Result<(PeerToken, &mut MioPeer<Self::PeerStream>), PeerConnectionIncomingAcceptError>
    {
        let server = &mut self.server;
        let poll = &mut self.poll;
        let peers = &mut self.peers;

        if let Some(server) = server.as_mut() {
            let (mut stream, address) = server
                .accept()
                .map_err(|err| PeerConnectionIncomingAcceptError::accept_error(err))?;

            let peer_entry = peers.vacant_entry();
            let token = mio::Token(peer_entry.key());

            poll.registry()
                .register(
                    &mut stream,
                    token,
                    mio::Interest::READABLE | mio::Interest::WRITABLE,
                )
                .map_err(|err| PeerConnectionIncomingAcceptError::poll_register_error(err))?;

            let peer = peer_entry.insert(MioPeer::new(address.into(), stream));
            Ok((PeerToken::new_unchecked(token.0), peer))
        } else {
            Err(PeerConnectionIncomingAcceptError::ServerNotListening)
        }
    }

    fn peer_connection_init(&mut self, address: SocketAddr) -> io::Result<PeerToken> {
        let poll = &mut self.poll;
        let peers = &mut self.peers;

        let peer_entry = peers.vacant_entry();
        let token = mio::Token(peer_entry.key());

        match TcpStream::connect(address) {
            Ok(mut stream) => {
                poll.registry().register(
                    &mut stream,
                    token,
                    mio::Interest::READABLE | mio::Interest::WRITABLE,
                )?;

                let peer = MioPeer::new(address.clone(), stream);

                peer_entry.insert(peer);
                Ok(PeerToken::new_unchecked(token.0))
            }
            Err(err) => Err(err),
        }
    }

    fn peer_disconnect(&mut self, token: PeerToken) {
        let index = token.index();
        if self.peers.contains(index) {
            let mut peer = self.peers.remove(index);
            let _ = self.poll.registry().deregister(&mut peer.stream);
        }
    }

    fn peer_get(&mut self, token: PeerToken) -> Option<&mut MioPeer<Self::PeerStream>> {
        self.peers.get_mut(token.index())
    }
}
