// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use riker::actors::*;

use tezos_encoding::hash::BlockHash;
use std::sync::Arc;
use tezos_messages::p2p::encoding::prelude::*;
use failure::_core::str::FromStr;
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct BlockApplied {
    pub hash: BlockHash,
    pub level: i32,
    pub header: Arc<BlockHeader>,
    pub block_header_info: Option<BlockHeaderInfo>,
    pub block_header_proto_info: HashMap<String, String>,
}

#[derive(Clone, Debug)]
pub struct BlockHeaderInfo {
    pub priority: i32,
    pub proof_of_work_nonce: String,
    pub signature: String,
}

impl FromStr for BlockHeaderInfo {
    type Err = serde_json::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let des: HashMap<&str, &str> = serde_json::from_str(s)?;
        Ok(Self {
            priority: if let Some(val) = des.get("priority") {
                val.parse().unwrap_or(0)
            } else {
                0
            },
            proof_of_work_nonce: (*des.get("proof_of_work_nonce").unwrap_or(&"")).into(),
            signature: (*des.get("signature").unwrap_or(&"")).into(),
        })
    }
}

/// Notify actors that system is about to shut down
#[derive(Clone, Debug)]
pub struct ShuttingDown;

#[derive(Clone, Debug)]
pub struct BlockReceived {
    pub hash: BlockHash,
    pub level: i32,
}

/// We have received message from another peer
#[derive(Clone, Debug)]
pub struct AllBlockOperationsReceived {
    pub hash: BlockHash,
    pub level: i32,
}

/// Shell channel event message.
#[derive(Clone, Debug)]
pub enum ShellChannelMsg {
    BlockApplied(BlockApplied),
    BlockReceived(BlockReceived),
    AllBlockOperationsReceived(AllBlockOperationsReceived),
    ShuttingDown(ShuttingDown),
}

impl From<BlockApplied> for ShellChannelMsg {
    fn from(msg: BlockApplied) -> Self {
        ShellChannelMsg::BlockApplied(msg)
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
    /// Ordinary cvents generated from shell layer
    ShellEvents,
    /// Control event
    ShellCommands
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
        fact.actor_of(Props::new(ShellChannel::new), ShellChannel::name())
    }

    fn name() -> &'static str {
        "shell-event-channel"
    }

    fn new() -> Self {
        ShellChannel(Channel::new())
    }
}

type ChannelCtx<Msg> = Context<ChannelMsg<Msg>>;

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
