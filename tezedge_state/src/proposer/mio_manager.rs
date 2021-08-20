// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use mio::net::{TcpListener, TcpSocket, TcpStream};
use slab::Slab;
use std::collections::BTreeMap;
use std::io;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::*;
use crate::PeerAddress;

pub type MioEvent = mio::event::Event;
pub type NetPeer = Peer<TcpStream>;

pub const MIO_SERVER_TOKEN: mio::Token = mio::Token(usize::MAX);
pub const MIO_WAKE_TOKEN: mio::Token = mio::Token(usize::MAX - 1);

impl NetworkEvent for MioEvent {
    #[inline(always)]
    fn is_server_event(&self) -> bool {
        self.token() == MIO_SERVER_TOKEN
    }

    #[inline(always)]
    fn is_waker_event(&self) -> bool {
        self.token() == MIO_WAKE_TOKEN
    }

    #[inline(always)]
    fn is_readable(&self) -> bool {
        self.is_readable()
    }

    #[inline(always)]
    fn is_writable(&self) -> bool {
        self.is_writable()
    }

    #[inline(always)]
    fn is_read_closed(&self) -> bool {
        self.is_error() || self.is_read_closed()
    }

    #[inline(always)]
    fn is_write_closed(&self) -> bool {
        self.is_write_closed()
    }
}

#[derive(Debug)]
pub struct MioEvents {
    mio_events: mio::Events,
    tick_event_time: Option<Instant>,
}

impl MioEvents {
    pub fn new() -> Self {
        Self {
            mio_events: mio::Events::with_capacity(0),
            tick_event_time: None,
        }
    }
}

impl<'a> IntoIterator for &'a MioEvents {
    type Item = EventRef<'a, MioEvent>;
    type IntoIter = MioEventsIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        let tick_event_time = self
            .tick_event_time
            .filter(|_| self.mio_events.is_empty())
            .clone();
        MioEventsIter {
            mio_events_iter: self.mio_events.iter(),
            tick_event_time,
        }
    }
}

pub struct MioEventsIter<'a> {
    mio_events_iter: mio::event::Iter<'a>,
    tick_event_time: Option<Instant>,
}

impl<'a> Iterator for MioEventsIter<'a> {
    type Item = EventRef<'a, MioEvent>;

    fn next(&mut self) -> Option<Self::Item> {
        let tick_event_time = &mut self.tick_event_time;

        self.mio_events_iter
            .next()
            .map(|event| EventRef::Network(event))
            .or_else(|| tick_event_time.take().map(|time| Event::Tick(time)))
    }
}

impl Events for MioEvents {
    fn set_limit(&mut self, limit: usize) {
        self.mio_events = mio::Events::with_capacity(limit);
    }
}

pub struct MioManager {
    listen_addr: SocketAddr,
    backlog_size: u32,

    poll: mio::Poll,
    waker: Arc<mio::Waker>,
    server: Option<TcpListener>,

    address_to_token: BTreeMap<PeerAddress, usize>,
    peers: Slab<NetPeer>,
}

impl MioManager {
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
            address_to_token: BTreeMap::new(),
            peers: Slab::new(),
        }
    }

    /// Waker can be used to wake up mio from another thread.
    pub fn waker(&self) -> Arc<mio::Waker> {
        self.waker.clone()
    }
}

impl Manager for MioManager {
    type Stream = TcpStream;
    type NetworkEvent = MioEvent;
    type Events = MioEvents;

    fn start_listening_to_server_events(&mut self) -> io::Result<()> {
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

    fn stop_listening_to_server_events(&mut self) {
        drop(self.server.take());
    }

    fn accept_connection(&mut self, _: &Self::NetworkEvent) -> Option<&mut Peer<Self::Stream>> {
        let server = &mut self.server;
        let poll = &mut self.poll;
        let peers = &mut self.peers;
        let address_to_token = &mut self.address_to_token;

        if let Some(server) = server.as_mut() {
            match server.accept() {
                Ok((mut stream, address)) => {
                    let peer_entry = peers.vacant_entry();
                    let token = mio::Token(peer_entry.key());

                    let registered_poll = poll.registry().register(
                        &mut stream,
                        token,
                        mio::Interest::READABLE | mio::Interest::WRITABLE,
                    );

                    match registered_poll {
                        Ok(_) => {
                            address_to_token.insert(address.into(), peer_entry.key());
                            Some(peer_entry.insert(NetPeer::new(address.into(), stream)))
                        }
                        Err(err) => {
                            eprintln!("error while registering poll: {:?}", err);
                            None
                        }
                    }
                }
                Err(err) => {
                    match err.kind() {
                        io::ErrorKind::WouldBlock => {}
                        _ => {
                            eprintln!("error while accepting connection: {:?}", err);
                        }
                    }
                    None
                }
            }
        } else {
            None
        }
    }

    fn wait_for_events(&mut self, events: &mut Self::Events, timeout: Option<Duration>) {
        match self.poll.poll(&mut events.mio_events, timeout) {
            Ok(_) => {}
            Err(err) => {
                eprintln!("Mio Poll::poll failed! Error: {:?}", err);
            }
        };

        if events.mio_events.is_empty() {
            events.tick_event_time = Some(Instant::now());
        }
    }

    fn get_peer_for_event_mut(&mut self, event: &Self::NetworkEvent) -> Option<&mut NetPeer> {
        self.peers.get_mut(event.token().into())
    }

    fn get_peer(&mut self, address: &PeerAddress) -> Option<&mut NetPeer> {
        match self.address_to_token.get(address) {
            Some(&token) => self.peers.get_mut(token),
            None => None,
        }
    }

    fn get_peer_or_connect_mut(&mut self, address: &PeerAddress) -> io::Result<&mut NetPeer> {
        if let Some(&token) = self.address_to_token.get(address) {
            if let Some(_) = self.peers.get(token) {
                return Ok(self.peers.get_mut(token).unwrap());
            }
        }

        let poll = &mut self.poll;
        let address_to_token = &mut self.address_to_token;
        let peers = &mut self.peers;

        let peer_entry = peers.vacant_entry();
        let token = mio::Token(peer_entry.key());

        match TcpStream::connect(address.into()) {
            Ok(mut stream) => {
                poll.registry().register(
                    &mut stream,
                    token,
                    mio::Interest::READABLE | mio::Interest::WRITABLE,
                )?;

                let peer = NetPeer::new(address.clone(), stream);

                address_to_token.insert(address.clone(), token.into());
                Ok(peer_entry.insert(peer))
            }
            Err(err) => {
                self.address_to_token.remove(address);
                Err(err)
            }
        }
    }

    fn disconnect_peer(&mut self, peer: &PeerAddress) {
        if let Some(token) = self.address_to_token.remove(peer) {
            if self.peers.contains(token) {
                let mut peer = self.peers.remove(token);
                let _ = self.poll.registry().deregister(&mut peer.stream);
            }
        }
    }
}
