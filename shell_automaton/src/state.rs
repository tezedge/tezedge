// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime};

use ::storage::persistent::SchemaError;
use tezos_messages::p2p::encoding::block_header::Level;

use crate::block_applier::BlockApplierState;
use crate::config::Config;
use crate::current_head::CurrentHeads;
use crate::mempool::MempoolState;
use crate::paused_loops::PausedLoopsState;
use crate::peer::connection::incoming::accept::PeerConnectionIncomingAcceptState;
use crate::peers::PeersState;
use crate::prechecker::PrecheckerState;
use crate::protocol_runner::ProtocolRunnerState;
use crate::rights::RightsState;
use crate::shutdown::ShutdownState;
use crate::storage::StorageState;
use crate::{ActionId, ActionKind, ActionWithMeta};

use redux_rs::SafetyCondition;

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
    pub protocol_runner: ProtocolRunnerState,
    pub block_applier: BlockApplierState,

    pub mempool: MempoolState,

    pub prechecker: PrecheckerState,

    pub rights: RightsState,

    pub current_heads: CurrentHeads,

    pub stats: super::stats::Stats,

    pub paused_loops: PausedLoopsState,

    /// Action before the `last_action`.
    pub prev_action: ActionIdWithKind,
    pub last_action: ActionIdWithKind,
    pub applied_actions_count: u64,

    pub shutdown: ShutdownState,
}

impl State {
    /// This constant is used for solving, if something is bootstrapped
    /// This means, that the compared level, should be on at least 99%.
    pub const HIGH_LEVEL_MARGIN_PERCENTAGE: i32 = 99;

    pub fn new(config: Config) -> Self {
        let block_applier = BlockApplierState::new(&config);
        Self {
            log: Default::default(),
            config,
            peers: PeersState::new(),
            peer_connection_incoming_accept: PeerConnectionIncomingAcceptState::Idle { time: 0 },
            storage: StorageState::new(),
            mempool: MempoolState::default(),
            rights: RightsState::default(),
            protocol_runner: ProtocolRunnerState::Idle,
            block_applier,

            prechecker: PrecheckerState::default(),

            current_heads: CurrentHeads::default(),

            stats: super::stats::Stats::default(),

            paused_loops: PausedLoopsState::new(),

            prev_action: ActionIdWithKind {
                id: ActionId::ZERO,
                kind: ActionKind::Init,
            },
            last_action: ActionIdWithKind {
                id: ActionId::ZERO,
                kind: ActionKind::Init,
            },
            applied_actions_count: 0,

            shutdown: ShutdownState::new(),
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

    pub fn current_head_level(&self) -> Option<Level> {
        self.mempool
            .local_head_state
            .as_ref()
            .map(|v| v.header.level())
    }

    /// Global bootstrap status is considered as bootstrapped, only if
    /// number of bootstrapped peers is above threshold.
    pub fn is_bootstrapped(&self) -> bool {
        let current_head_level = match self.current_head_level() {
            Some(v) => v,
            None => return false,
        };

        let bootstrapped_peers_len = self
            .peers
            .iter()
            .filter_map(|(_, p)| p.status.as_handshaked())
            .filter_map(|peer| peer.current_head_level)
            .filter_map(|level| {
                // calculate what percentage is our current head of
                // peer's current head. If percentage is greater than
                // or equal to `Self::HIGH_LEVEL_MARGIN_PERCENTAGE`,
                // then we are in sync with the peer.
                current_head_level
                    .checked_mul(100)
                    .and_then(|l| l.checked_div(level))
            })
            .filter(|perc| *perc >= Self::HIGH_LEVEL_MARGIN_PERCENTAGE)
            .count();

        bootstrapped_peers_len >= self.config.peers_bootstrapped_min
    }

    /// If shutdown was initiated and finished or not.
    pub fn is_shutdown(&self) -> bool {
        matches!(self.shutdown, ShutdownState::Success { .. })
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

#[derive(Debug)]
pub enum SafetyConditionError {
    ConnectedPeersMaxedOut,
    PotentialPeersMaxedOut,
    ConnectedPeerIsBlacklisted
}

impl SafetyCondition for State {
    type Error = SafetyConditionError;

    fn check_safety_condition(&self) -> Result<(), Self::Error> {

        if self.peers.connected_len() > self.config.peers_connected_max {
            return Err(SafetyConditionError::ConnectedPeersMaxedOut)
        }

        if self.peers.potential_len() > self.config.peers_potential_max {
            return Err(SafetyConditionError::PotentialPeersMaxedOut)
        }

        for (peer_addr, _) in self.peers.connected_iter() {
            if self.peers.is_blacklisted(&peer_addr.ip()) {
                return Err(SafetyConditionError::ConnectedPeerIsBlacklisted)
            }
        }

        Ok(())
    }
}
