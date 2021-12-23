// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime};

use ::storage::persistent::SchemaError;

use crate::config::Config;
use crate::mempool::MempoolState;
use crate::paused_loops::PausedLoopsState;
use crate::peer::connection::incoming::accept::PeerConnectionIncomingAcceptState;
use crate::peers::PeersState;
use crate::protocol::ProtocolState;
use crate::rights::RightsState;
use crate::storage::StorageState;
use crate::{ActionId, ActionKind, ActionWithMeta};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ActionIdWithKind {
    id: ActionId,
    kind: ActionKind,
}

impl ActionIdWithKind {
    #[inline(always)]
    pub fn id(&self) -> ActionId {
        self.id
    }

    #[inline(always)]
    pub fn kind(&self) -> ActionKind {
        self.kind
    }

    #[inline(always)]
    pub fn time_as_nanos(&self) -> u64 {
        self.id.into()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct State {
    #[serde(skip)]
    pub log: crate::Logger,
    pub config: Config,

    pub peers: PeersState,
    pub peer_connection_incoming_accept: PeerConnectionIncomingAcceptState,
    pub storage: StorageState,

    pub mempool: MempoolState,

    pub rights: RightsState,

    pub paused_loops: PausedLoopsState,

    pub protocol: ProtocolState,

    /// Action before the `last_action`.
    pub prev_action: ActionIdWithKind,
    pub last_action: ActionIdWithKind,
    pub applied_actions_count: u64,
}

impl State {
    pub fn new(config: Config) -> Self {
        Self {
            log: Default::default(),
            config,
            peers: PeersState::new(),
            peer_connection_incoming_accept: PeerConnectionIncomingAcceptState::Idle { time: 0 },
            storage: StorageState::new(),
            mempool: MempoolState::default(),
            rights: RightsState::default(),

            paused_loops: PausedLoopsState::new(),

            protocol: ProtocolState::default(),

            prev_action: ActionIdWithKind {
                id: ActionId::ZERO,
                kind: ActionKind::Init,
            },
            last_action: ActionIdWithKind {
                id: ActionId::ZERO,
                kind: ActionKind::Init,
            },
            applied_actions_count: 0,
        }
    }

    pub fn set_logger<T>(&mut self, logger: T)
    where
        T: Into<crate::Logger>,
    {
        self.log = logger.into();
    }

    #[inline(always)]
    pub(crate) fn set_last_action(&mut self, action: &ActionWithMeta) {
        let prev_action = std::mem::replace(
            &mut self.last_action,
            ActionIdWithKind {
                id: action.id,
                kind: action.action.kind(),
            },
        );
        self.prev_action = prev_action;
    }

    #[inline(always)]
    pub fn time(&self) -> SystemTime {
        SystemTime::UNIX_EPOCH + self.duration_since_epoch()
    }

    #[inline(always)]
    pub fn time_as_nanos(&self) -> u64 {
        self.last_action.time_as_nanos()
    }

    #[inline(always)]
    pub fn duration_since_epoch(&self) -> Duration {
        Duration::from_nanos(self.time_as_nanos())
    }

    #[inline(always)]
    pub fn mio_timeout(&self) -> Option<Duration> {
        // If we have paused loops, then set mio timeout to zero
        // so that epoll syscall for events returns instantly, instead
        // of blocking up until timeout or until there are some events.
        if !self.paused_loops.is_empty() {
            Some(Duration::ZERO)
        } else {
            Some(self.config.min_time_interval())
        }
    }
}

impl storage::persistent::Encoder for State {
    fn encode(&self) -> Result<Vec<u8>, SchemaError> {
        rmp_serde::to_vec(self).map_err(|_| SchemaError::EncodeError)
    }
}

impl storage::persistent::Decoder for State {
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        rmp_serde::from_slice(bytes).map_err(|_| SchemaError::DecodeError)
    }
}
