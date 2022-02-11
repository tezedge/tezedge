// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! This channel is used to transmit p2p networking messages between actors.

use std::net::SocketAddr;
use std::sync::Arc;

use tezedge_actor_system::actors::*;

use crypto::hash::{BlockHash, ChainId};
use storage::BlockHeaderWithHash;
use tezos_messages::p2p::encoding::metadata::MetadataMessage;
use tezos_messages::p2p::encoding::peer::PeerMessageResponse;
use tezos_messages::p2p::encoding::version::NetworkVersion;

use crate::PeerId;

/// We have received message from another peer
#[derive(Clone, Debug)]
pub struct PeerMessageReceived {
    pub peer_address: SocketAddr,
    pub message: Arc<PeerMessageResponse>,
}

pub type NewCurrentHeadNotificationRef = Arc<NewCurrentHeadNotification>;

#[derive(Debug)]
pub struct NewCurrentHeadNotification {
    pub chain_id: Arc<ChainId>,
    pub block: Arc<BlockHeaderWithHash>,
    pub is_bootstrapped: bool,
    pub best_remote_level: Option<i32>,
}

impl NewCurrentHeadNotification {
    pub fn new(
        chain_id: Arc<ChainId>,
        block: Arc<BlockHeaderWithHash>,
        is_bootstrapped: bool,
        best_remote_level: Option<i32>,
    ) -> Self {
        Self {
            chain_id,
            block,
            is_bootstrapped,
            best_remote_level,
        }
    }
}

/// Message informing actors about receiving block header
#[derive(Clone, Debug)]
pub struct BlockReceived {
    pub hash: BlockHash,
    pub level: i32,
}

/// Message informing actors about receiving all operations for a specific block
#[derive(Clone, Debug)]
pub struct AllBlockOperationsReceived {
    pub level: i32,
}

/// Network channel event message.
#[derive(Clone, Debug)]
pub enum NetworkChannelMsg {
    /// Events
    PeerBootstrapped(Arc<PeerId>, MetadataMessage, Arc<NetworkVersion>),
    PeerDisconnected(SocketAddr),
    PeerMessageReceived(PeerMessageReceived),

    NewCurrentHead(NewCurrentHeadNotificationRef),
    BlockReceived(BlockReceived),
    BlockApplied(Arc<BlockHash>),
    AllBlockOperationsReceived(AllBlockOperationsReceived),
}

impl From<BlockReceived> for NetworkChannelMsg {
    fn from(msg: BlockReceived) -> Self {
        NetworkChannelMsg::BlockReceived(msg)
    }
}

impl From<AllBlockOperationsReceived> for NetworkChannelMsg {
    fn from(msg: AllBlockOperationsReceived) -> Self {
        NetworkChannelMsg::AllBlockOperationsReceived(msg)
    }
}

/// Represents various topics
pub enum NetworkChannelTopic {
    /// Events generated from networking layer
    NetworkEvents,
}

impl From<NetworkChannelTopic> for Topic {
    fn from(evt: NetworkChannelTopic) -> Self {
        match evt {
            NetworkChannelTopic::NetworkEvents => Topic::from("network.events"),
        }
    }
}

/// This struct represents network bus where all network events must be published.
pub struct NetworkChannel(Channel<NetworkChannelMsg>);

pub type NetworkChannelRef = ChannelRef<NetworkChannelMsg>;

impl NetworkChannel {
    pub fn actor(fact: &impl ActorRefFactory) -> Result<NetworkChannelRef, CreateError> {
        fact.actor_of::<NetworkChannel>(NetworkChannel::name())
    }

    fn name() -> &'static str {
        "network-event-channel"
    }
}

type ChannelCtx<Msg> = Context<ChannelMsg<Msg>>;

impl ActorFactory for NetworkChannel {
    fn create() -> Self {
        NetworkChannel(Channel::default())
    }
}

impl Actor for NetworkChannel {
    type Msg = ChannelMsg<NetworkChannelMsg>;

    fn pre_start(&mut self, ctx: &ChannelCtx<NetworkChannelMsg>) {
        self.0.pre_start(ctx);
    }

    fn recv(
        &mut self,
        ctx: &ChannelCtx<NetworkChannelMsg>,
        msg: ChannelMsg<NetworkChannelMsg>,
        sender: Sender,
    ) {
        self.0.receive(ctx, msg, sender);
    }
}
