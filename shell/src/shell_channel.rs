// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! Shell channel is used to transmit high level shell messages.

use std::collections::HashMap;

use getset::Getters;
use riker::actors::*;

use crypto::hash::{BlockHash, OperationHash, ProtocolHash};
use storage::block_storage::BlockJsonData;
use storage::BlockHeaderWithHash;
use tezos_api::ffi::ValidateOperationResult;
use tezos_messages::p2p::encoding::operation::MempoolOperationType;
use tezos_messages::p2p::encoding::prelude::Operation;

/// Message informing actors about successful block application by protocol
#[derive(Clone, Debug, Getters)]
pub struct BlockApplied {
    #[get = "pub"]
    header: BlockHeaderWithHash,
    #[get = "pub"]
    json_data: BlockJsonData,
}

impl BlockApplied {
    pub fn new(header: BlockHeaderWithHash, json_data: BlockJsonData) -> Self {
        Self { header, json_data }
    }
}

/// Notify actors that system is about to shut down
#[derive(Clone, Debug)]
pub struct ShuttingDown;

/// Message informing actors about receiving block header
#[derive(Clone, Debug)]
pub struct BlockReceived {
    pub hash: BlockHash,
    pub level: i32,
}

/// Message informing actors about receiving all operations for a specific block
#[derive(Clone, Debug)]
pub struct AllBlockOperationsReceived {
    pub hash: BlockHash,
    pub level: i32,
}

// Notify actors that operations should by validated by mempool
#[derive(Clone, Debug)]
pub struct MempoolOperationReceived {
    pub operation_hash: OperationHash,
    pub operation_type: MempoolOperationType,
}

#[derive(Clone, Debug)]
pub struct MempoolCurrentState {
    pub protocol: Option<ProtocolHash>,
    pub result: ValidateOperationResult,
    pub operations: HashMap<OperationHash, Operation>,
}

/// Shell channel event message.
#[derive(Clone, Debug)]
pub enum ShellChannelMsg {
    BlockApplied(BlockApplied),
    BlockReceived(BlockReceived),
    AllBlockOperationsReceived(AllBlockOperationsReceived),
    MempoolOperationReceived(MempoolOperationReceived),
    MempoolValidationResultChanged(MempoolCurrentState),
    ShuttingDown(ShuttingDown),
}

impl From<BlockApplied> for ShellChannelMsg {
    fn from(msg: BlockApplied) -> Self {
        ShellChannelMsg::BlockApplied(msg)
    }
}

impl From<MempoolOperationReceived> for ShellChannelMsg {
    fn from(msg: MempoolOperationReceived) -> Self {
        ShellChannelMsg::MempoolOperationReceived(msg)
    }
}

impl From<MempoolCurrentState> for ShellChannelMsg {
    fn from(msg: MempoolCurrentState) -> Self {
        ShellChannelMsg::MempoolValidationResultChanged(msg)
    }
}

impl From<BlockReceived> for ShellChannelMsg {
    fn from(msg: BlockReceived) -> Self {
        ShellChannelMsg::BlockReceived(msg)
    }
}

impl From<AllBlockOperationsReceived> for ShellChannelMsg {
    fn from(msg: AllBlockOperationsReceived) -> Self {
        ShellChannelMsg::AllBlockOperationsReceived(msg)
    }
}

impl From<ShuttingDown> for ShellChannelMsg {
    fn from(msg: ShuttingDown) -> Self {
        ShellChannelMsg::ShuttingDown(msg)
    }
}

/// Represents various topics
pub enum ShellChannelTopic {
    /// Ordinary events generated from shell layer
    ShellEvents,
    /// Control event
    ShellCommands,
}

impl From<ShellChannelTopic> for Topic {
    fn from(evt: ShellChannelTopic) -> Self {
        match evt {
            ShellChannelTopic::ShellEvents => Topic::from("shell.events"),
            ShellChannelTopic::ShellCommands => Topic::from("shell.command")
        }
    }
}

/// This struct represents shell event bus where all shell events must be published.
pub struct ShellChannel(Channel<ShellChannelMsg>);

pub type ShellChannelRef = ChannelRef<ShellChannelMsg>;

impl ShellChannel {
    pub fn actor(fact: &impl ActorRefFactory) -> Result<ShellChannelRef, CreateError> {
        fact.actor_of::<ShellChannel>(ShellChannel::name())
    }

    fn name() -> &'static str {
        "shell-event-channel"
    }
}

type ChannelCtx<Msg> = Context<ChannelMsg<Msg>>;

impl ActorFactory for ShellChannel {
    fn create() -> Self {
        ShellChannel(Channel::default())
    }
}

impl Actor for ShellChannel {
    type Msg = ChannelMsg<ShellChannelMsg>;

    fn pre_start(&mut self, ctx: &ChannelCtx<ShellChannelMsg>) {
        self.0.pre_start(ctx);
    }

    fn recv(
        &mut self,
        ctx: &ChannelCtx<ShellChannelMsg>,
        msg: ChannelMsg<ShellChannelMsg>,
        sender: Sender,
    ) {
        self.0.receive(ctx, msg, sender);
    }
}
