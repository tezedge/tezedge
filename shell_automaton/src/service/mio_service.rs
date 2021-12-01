// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use mio::net::{TcpListener, TcpSocket, TcpStream};
use serde::{Deserialize, Serialize};
use slab::Slab;
use tungstenite::handshake::server::NoCallback;
use tungstenite::{HandshakeError, ServerHandshake};
use tungstenite::handshake::MidHandshake;
use std::io::{self, Read, Write};
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;
use tungstenite::protocol::{WebSocket, Message};

use crate::event::{Event, P2pPeerEvent, P2pPeerUnknownEvent, P2pServerEvent, WakeupEvent, WebsocketClientEvent, WebsocketServerEvent};
use crate::io_error_kind::IOErrorKind;
use crate::peer::PeerToken;

pub type MioInternalEvent = mio::event::Event;
pub type MioInternalEventsContainer = mio::Events;

/// We will receive listening socket server events with this token, when
/// there are incoming connections that need to be accepted.
pub const MIO_SERVER_TOKEN: mio::Token = mio::Token(usize::MAX);

/// Event with this token will be issued, when `mio::Waker::wake` is called.
pub const MIO_WAKE_TOKEN: mio::Token = mio::Token(usize::MAX - 1);

/// WS token bounds
pub const MIO_WEBSOCKET_SERVER_TOKEN: mio::Token = mio::Token(usize::MAX - 2);

pub const WEBSOCKET_EVENT_TOKEN_MIN: mio::Token = mio::Token(10_000);
pub const WEBSOCKET_EVENT_TOKEN_MAX: mio::Token = mio::Token(19_999);

pub type MioPeerDefault = MioPeer<TcpStream>;
pub type WsHandshakeCompleted = bool;

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
    ) -> Result<(PeerToken, MioPeerRefMut<Self::PeerStream>), PeerConnectionIncomingAcceptError>;

    fn peer_connection_init(&mut self, address: SocketAddr) -> io::Result<PeerToken>;
    fn peer_disconnect(&mut self, token: PeerToken);

    fn peer_get(&mut self, token: PeerToken) -> Option<MioPeerRefMut<Self::PeerStream>>;

    // TODO: websocket stuff
    fn websocket_connection_incoming_listen_start(&mut self) -> io::Result<()>;
    fn websocket_connection_incoming_listen_stop(&mut self);
    fn websocket_connection_incoming_accept(&mut self) -> Result<(PeerToken, WsHandshakeCompleted), WebSocketIncomingAcceptError>;
    fn websocket_connection_continue_handshaking(&mut self, token: PeerToken) -> Result<(PeerToken, WsHandshakeCompleted), WebSocketIncomingAcceptError>;
    fn websocket_disconnect(&mut self, token: PeerToken);
    fn websocket_client_get(&self, token: PeerToken) -> Option<&MioWsClient>;
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

impl<Stream: Clone> Clone for MioPeer<Stream> {
    fn clone(&self) -> Self {
        Self {
            address: self.address,
            stream: self.stream.clone(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum WebSocketIncomingAcceptError {
    ConnectionError(PeerConnectionIncomingAcceptError),

    /// Websocket Handshake error
    HandshakeError,

    ///
    ConnectionNotFound,

    /// Connection already established
    ConnectionAlreadyEstablished
}

type HandshakeStatus = MidHandshake<ServerHandshake<TcpStream, NoCallback>>;
pub enum MioWsConnection {
    MidHandshake(HandshakeStatus),
    Accepted(WebSocket<TcpStream>),
}

impl MioWsConnection {
    /// Starts the handshaking proceess
    pub fn new_connection(stream: TcpStream) -> Result<(Self, WsHandshakeCompleted), WebSocketIncomingAcceptError> {
        match tungstenite::accept(stream) {
            Ok(finalized) => Ok((Self::Accepted(finalized), true)),
            // Would block
            Err(HandshakeError::Interrupted(state)) => Ok((Self::MidHandshake(state), false)),
            // TODO: do not ignore this error
            Err(HandshakeError::Failure(_)) => Err(WebSocketIncomingAcceptError::HandshakeError),
        }
    }

    /// Continues the handshaking process by creating a new instance
    pub fn continue_handshaking(connection: Self) -> Result<(Self, WsHandshakeCompleted), WebSocketIncomingAcceptError> {
        match connection {
            Self::Accepted(_) => Err(WebSocketIncomingAcceptError::ConnectionAlreadyEstablished),
            Self::MidHandshake(state) => {
                match state.handshake() {
                    Ok(ws) => Ok((Self::Accepted(ws), true)),
                    // Would block
                    Err(HandshakeError::Interrupted(state)) => Ok((Self::MidHandshake(state), false)),
                    // TODO: do not ignore this error
                    Err(HandshakeError::Failure(_)) => Err(WebSocketIncomingAcceptError::HandshakeError),
                }
            }
        }
    }

    /// If true, it means the connection is live and we have completed the handshake process
    pub fn is_established(&self) -> bool {
        match self {
            Self::Accepted(_) => true,
            Self::MidHandshake(_) => false
        }
    }
}

// TODO: do we need more info?
pub struct MioWsClient {
    pub connection: MioWsConnection,
}

impl MioWsClient {
    pub fn new(connection: MioWsConnection) -> Self {
        Self { connection }
    }
}

pub struct MioPeerRefMut<'a, Stream> {
    /// Pre-allocated buffer that will be used as a temporary container
    /// for doing read syscall.
    buffer: &'a mut [u8],
    pub address: SocketAddr,
    pub stream: &'a mut Stream,
}

impl<'a, Stream> MioPeerRefMut<'a, Stream> {
    pub fn new(buffer: &'a mut [u8], address: SocketAddr, stream: &'a mut Stream) -> Self {
        Self {
            buffer,
            address,
            stream,
        }
    }
}

impl<'a, S: Read> MioPeerRefMut<'a, S> {
    #[inline]
    pub fn read(&mut self, limit: usize) -> io::Result<&[u8]> {
        let limit = limit.min(self.buffer.len());
        let read_bytes = self.stream.read(&mut self.buffer[0..limit])?;

        Ok(&self.buffer[0..read_bytes])
    }
}

impl<'a, S: Write> MioPeerRefMut<'a, S> {
    #[inline(always)]
    pub fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.stream.write(buf)
    }
}

pub struct MioServiceDefault {
    listen_addr: SocketAddr,
    websocket_addr: SocketAddr,

    /// Backlog size for incoming connections.
    ///
    /// Incoming connections are put in kernel's backlog, this is limit
    /// for that backlog. So if queue of incoming connections get to
    /// this limit, more connections will be instantly rejected.
    backlog_size: u32,

    /// Pre-allocated buffer that will be (re)used as a temporary
    /// container for doing read syscalls when reading from peer.
    buffer: Vec<u8>,

    poll: mio::Poll,
    waker: Arc<mio::Waker>,
    server: Option<TcpListener>,
    websocket_server: Option<TcpListener>,

    peers: Slab<MioPeer<TcpStream>>,
    websocket_clients: Slab<MioWsClient>,
}

impl MioServiceDefault {
    const DEFAULT_BACKLOG_SIZE: u32 = 255;

    pub fn new(listen_addr: SocketAddr, websocket_addr: SocketAddr, buffer_size: usize) -> Self {
        let poll = mio::Poll::new().expect("failed to create mio::Poll");
        let waker = Arc::new(
            mio::Waker::new(poll.registry(), MIO_WAKE_TOKEN).expect("failed to create mio::Waker"),
        );

        Self {
            listen_addr,
            websocket_addr,
            backlog_size: Self::DEFAULT_BACKLOG_SIZE,
            buffer: vec![0; buffer_size],
            poll,
            waker,
            server: None,
            websocket_server: None,
            peers: Slab::new(),
            websocket_clients: Slab::new(),
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
        } else if event.token() == MIO_WEBSOCKET_SERVER_TOKEN {
            WebsocketServerEvent {}.into()
        } else {
            let is_closed = event.is_error() || event.is_read_closed() || event.is_write_closed();
            let peer_token = PeerToken::new_unchecked(event.token().0);

            if event.token() >= WEBSOCKET_EVENT_TOKEN_MIN && event.token() < WEBSOCKET_EVENT_TOKEN_MAX {
                WebsocketClientEvent {
                    token: peer_token,
                }.into()
            } else {
                match self.peer_get(peer_token) {
                    Some(peer) => {
                        P2pPeerEvent {
                            token: peer_token,
                            address: peer.address,
        
                            is_readable: event.is_readable(),
                            is_writable: event.is_writable(),
                            is_closed,
                        }.into()
                    }
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
    ) -> Result<(PeerToken, MioPeerRefMut<Self::PeerStream>), PeerConnectionIncomingAcceptError>
    {
        let buffer = &mut self.buffer;
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
            let peer_ref_mut = MioPeerRefMut::new(buffer, peer.address, &mut peer.stream);

            Ok((PeerToken::new_unchecked(token.0), peer_ref_mut))
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

    fn peer_get(&mut self, token: PeerToken) -> Option<MioPeerRefMut<Self::PeerStream>> {
        let buffer = &mut self.buffer;
        match self.peers.get_mut(token.index()) {
            Some(peer) => Some(MioPeerRefMut::new(buffer, peer.address, &mut peer.stream)),
            None => None,
        }
    }

    // Note: I think these should be merged with the peer_.. variant with some tweaks
    fn websocket_connection_incoming_listen_start(&mut self) -> io::Result<()> {
        if self.websocket_server.is_none() {
            let socket = match self.listen_addr.ip() {
                IpAddr::V4(_) => TcpSocket::new_v4()?,
                IpAddr::V6(_) => TcpSocket::new_v6()?,
            };

            // read more details about why not on windows in mio docs
            // for [mio::TcpListener::bind].
            #[cfg(not(windows))]
            socket.set_reuseaddr(true)?;

            socket.bind(self.websocket_addr)?;

            let mut server = socket.listen(self.backlog_size)?;

            self.poll.registry().register(
                &mut server,
                MIO_WEBSOCKET_SERVER_TOKEN,
                mio::Interest::READABLE,
            )?;

            self.websocket_server = Some(server);
        }
        Ok(())
    }

    fn websocket_connection_incoming_listen_stop(&mut self) {
        drop(self.websocket_server.take());
    }

    fn websocket_connection_incoming_accept(&mut self) -> Result<(PeerToken, WsHandshakeCompleted), WebSocketIncomingAcceptError> {
        let server = &mut self.websocket_server;
        let poll = &mut self.poll;
        let websocket_clients = &mut self.websocket_clients;

        if let Some(server) = server.as_mut() {
            let (mut stream, address) = server
                .accept()
                .map_err(|e| WebSocketIncomingAcceptError::ConnectionError(PeerConnectionIncomingAcceptError::accept_error(e)))?;

            let websocket_client_entry = websocket_clients.vacant_entry();
            let token = mio::Token(websocket_client_entry.key() + WEBSOCKET_EVENT_TOKEN_MIN.0);

            poll.registry()
                .register(
                    &mut stream,
                    token,
                    mio::Interest::READABLE | mio::Interest::WRITABLE,
                )
                .map_err(|e| WebSocketIncomingAcceptError::ConnectionError(PeerConnectionIncomingAcceptError::poll_register_error(e)))?;

            let (websocket_connection, is_handshake_complete) = MioWsConnection::new_connection(stream)?;

            let client = MioWsClient::new(websocket_connection);
            websocket_client_entry.insert(client);

            Ok((PeerToken::new_unchecked(token.0), is_handshake_complete))
        } else {
            Err(WebSocketIncomingAcceptError::ConnectionError(PeerConnectionIncomingAcceptError::ServerNotListening))
        }
    }

    fn websocket_disconnect(&mut self, token: PeerToken) {
        let index = token.index();
        if self.websocket_clients.contains(index) {
            let mut client = self.websocket_clients.remove(index);
        }
    }

    fn websocket_connection_continue_handshaking(&mut self, token: PeerToken) -> Result<(PeerToken, WsHandshakeCompleted), WebSocketIncomingAcceptError> {
        let websocket_clients = &mut self.websocket_clients;

        let key = token.index() - WEBSOCKET_EVENT_TOKEN_MIN.0;

        if let Some(client) = websocket_clients.try_remove(key) {
            let (connection, is_handshake_complete) = MioWsConnection::continue_handshaking(client.connection)?;
            websocket_clients.insert(MioWsClient::new(connection));
            Ok((token, is_handshake_complete))
        } else {
            Err(WebSocketIncomingAcceptError::ConnectionNotFound)
        }
    }

    fn websocket_client_get(&self, token: PeerToken) -> Option<&MioWsClient> {
        let key = token.index() - WEBSOCKET_EVENT_TOKEN_MIN.0;
        self.websocket_clients.get(key)
    }
}
