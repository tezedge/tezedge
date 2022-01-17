// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::HashMap;
use std::fmt;
use std::net::SocketAddr;
use std::sync::{mpsc, Arc};

use crypto::hash::{BlockHash, ChainId};
use networking::network_channel::{
    NetworkChannelMsg, NetworkChannelRef, NetworkChannelTopic, PeerMessageReceived,
};
use storage::BlockHeaderWithHash;
use tezedge_actor_system::actors::*;
use tezos_api::ffi::ApplyBlockResponse;
use tezos_messages::p2p::encoding::prelude::{
    MetadataMessage, NetworkVersion, PeerMessageResponse,
};

use crate::peer::PeerId;

pub type ApplyBlockResult = Result<
    (
        Arc<ChainId>,
        Arc<BlockHeaderWithHash>,
        Arc<ApplyBlockResponse>,
    ),
    (),
>;

pub trait ActorsService {
    /// Send message to actors.
    fn send(&self, msg: ActorsMessageTo);

    /// Try to receive/read queued message from actors, if there is any.
    fn try_recv(&mut self) -> Result<ActorsMessageFrom, mpsc::TryRecvError>;

    fn register_apply_block_callback(
        &mut self,
        block_hash: Arc<BlockHash>,
        callback: ApplyBlockCallback,
    );

    fn call_apply_block_callback(&mut self, block_hash: &BlockHash, result: ApplyBlockResult);
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

pub type ApplyBlockCallbackFn = Box<dyn FnOnce(BlockHash, ApplyBlockResult) + Send>;

pub struct ApplyBlockCallback(ApplyBlockCallbackFn);

impl<F> From<F> for ApplyBlockCallback
where
    F: 'static + FnOnce(BlockHash, ApplyBlockResult) + Send,
{
    fn from(f: F) -> Self {
        Self(Box::new(f))
    }
}

impl fmt::Debug for ApplyBlockCallback {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplyBlockCallbackFn").finish()
    }
}

/// Message from actors.
#[derive(Debug)]
pub enum ActorsMessageFrom {
    PeerStalled(Arc<PeerId>),
    BlacklistPeer(Arc<PeerId>, String),
    SendMessage(Arc<PeerId>, Arc<PeerMessageResponse>),
    ApplyBlock {
        chain_id: Arc<ChainId>,
        block_hash: Arc<BlockHash>,
        callback: ApplyBlockCallback,
    },
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
        self.sender.send(msg)?;
        let _ = self.mio_waker.wake();
        Ok(())
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

    apply_block_callbacks: HashMap<Arc<BlockHash>, ApplyBlockCallback>,
}

impl ActorsServiceDefault {
    pub fn new(
        actors_channel: mpsc::Receiver<ActorsMessageFrom>,
        network_channel: NetworkChannelRef,
    ) -> Self {
        Self {
            actors_channel,
            network_channel,
            apply_block_callbacks: HashMap::new(),
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

    fn register_apply_block_callback(
        &mut self,
        block_hash: Arc<BlockHash>,
        callback: ApplyBlockCallback,
    ) {
        self.apply_block_callbacks.insert(block_hash, callback);
    }

    fn call_apply_block_callback(&mut self, block_hash: &BlockHash, result: ApplyBlockResult) {
        if let Some(callback) = self.apply_block_callbacks.remove(block_hash) {
            (callback.0)(block_hash.clone(), result);
        }
    }
}
