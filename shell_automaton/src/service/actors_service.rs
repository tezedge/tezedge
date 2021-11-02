// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::net::SocketAddr;
use std::sync::{mpsc, Arc};
use tezedge_actor_system::actors::*;

use networking::network_channel::{
    NetworkChannelMsg, NetworkChannelRef, NetworkChannelTopic, PeerMessageReceived,
};
use tezos_messages::p2p::encoding::prelude::{
    MetadataMessage, NetworkVersion, PeerMessageResponse,
};

use crate::peer::PeerId;

pub trait ActorsService {
    /// Send message to actors.
    fn send(&self, msg: ActorsMessageTo);

    /// Try to receive/read queued message from actors, if there is any.
    fn try_recv(&mut self) -> Result<ActorsMessageFrom, mpsc::TryRecvError>;
}

/// Message to actors.
#[derive(Debug, Clone)]
pub enum ActorsMessageTo {
    PeerHandshaked(Arc<PeerId>, MetadataMessage, Arc<NetworkVersion>),
    PeerDisconnected(SocketAddr),
    PeerMessageReceived(PeerMessageReceived),
}

impl From<ActorsMessageTo> for NetworkChannelMsg {
    fn from(msg: ActorsMessageTo) -> Self {
        match msg {
            ActorsMessageTo::PeerHandshaked(id, metadata, version) => {
                Self::PeerBootstrapped(id, metadata, version)
            }
            ActorsMessageTo::PeerDisconnected(address) => Self::PeerDisconnected(address),
            ActorsMessageTo::PeerMessageReceived(address) => Self::PeerMessageReceived(address),
        }
    }
}

/// Message from actors.
#[derive(Debug, Clone)]
pub enum ActorsMessageFrom {
    PeerStalled(Arc<PeerId>),
    BlacklistPeer(Arc<PeerId>, String),
    SendMessage(Arc<PeerId>, Arc<PeerMessageResponse>),
    Shutdown,
}

pub type AutomatonReceiver = mpsc::Receiver<ActorsMessageFrom>;

/// Sender for sending messages from Actor system to Automaton.
pub struct AutomatonSyncSender {
    sender: mpsc::SyncSender<ActorsMessageFrom>,
    mio_waker: Arc<mio::Waker>,
}

impl AutomatonSyncSender {
    pub fn send(&self, msg: ActorsMessageFrom) -> Result<(), mpsc::SendError<ActorsMessageFrom>> {
        let _ = self.mio_waker.wake();
        self.sender.send(msg)
    }
}

impl Clone for AutomatonSyncSender {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            mio_waker: self.mio_waker.clone(),
        }
    }
}

/// Channel for communcation from Actor System to Automaton.
pub fn sync_channel(
    mio_waker: Arc<mio::Waker>,
    bound: usize,
) -> (AutomatonSyncSender, AutomatonReceiver) {
    let (tx, rx) = mpsc::sync_channel(bound);

    let sender = AutomatonSyncSender {
        sender: tx,
        mio_waker,
    };

    (sender, rx)
}

#[derive(Debug)]
pub struct ActorsServiceDefault {
    /// Channel receiver half for receiving messages from actors.
    actors_channel: mpsc::Receiver<ActorsMessageFrom>,

    /// NetworkChannel is used to send messages/notifications to actors.
    network_channel: NetworkChannelRef,
}

impl ActorsServiceDefault {
    pub fn new(
        actors_channel: mpsc::Receiver<ActorsMessageFrom>,
        network_channel: NetworkChannelRef,
    ) -> Self {
        Self {
            actors_channel,
            network_channel,
        }
    }
}

impl ActorsService for ActorsServiceDefault {
    fn send(&self, msg: ActorsMessageTo) {
        self.network_channel.tell(
            Publish {
                msg: msg.into(),
                topic: NetworkChannelTopic::NetworkEvents.into(),
            },
            None,
        );
    }

    fn try_recv(&mut self) -> Result<ActorsMessageFrom, mpsc::TryRecvError> {
        self.actors_channel.try_recv()
    }
}
