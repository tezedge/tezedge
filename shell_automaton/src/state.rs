// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::time::{Duration, SystemTime};

use ::storage::persistent::SchemaError;
use crypto::hash::{BlockHash, CryptoboxPublicKeyHash};
use storage::BlockHeaderWithHash;
use tezos_messages::p2p::encoding::block_header::Level;

use crate::block_applier::BlockApplierState;
use crate::bootstrap::BootstrapState;
use crate::config::Config;
use crate::current_head::CurrentHeadState;
use crate::current_head_precheck::CurrentHeads;
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

    pub bootstrap: BootstrapState,
    pub mempool: MempoolState,

    pub prechecker: PrecheckerState,

    pub rights: RightsState,

    pub current_head: CurrentHeadState,
    pub current_heads: CurrentHeads,

    pub stats: super::stats::Stats,

    pub rpc: super::rpc::RpcState,

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
            bootstrap: BootstrapState::new(),
            mempool: MempoolState::default(),
            rights: RightsState::default(),
            protocol_runner: ProtocolRunnerState::Idle,
            block_applier,

            prechecker: PrecheckerState::default(),

            current_head: CurrentHeadState::new(),
            current_heads: CurrentHeads::default(),

            stats: super::stats::Stats::default(),

            rpc: super::rpc::RpcState::default(),

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
        self.current_head.get().map(|head| head.header.level())
    }

    pub fn current_head_candidate_level(&self) -> Option<Level> {
        self.current_head_level().map(|l| l + 1)
    }

    pub fn current_head_hash(&self) -> Option<&BlockHash> {
        self.current_head.get().map(|head| &head.hash)
    }

    pub fn best_remote_level(&self) -> Option<i32> {
        self.peers
            .handshaked_iter()
            .filter_map(|(_, peer)| peer.current_head.as_ref())
            .map(|head| head.header.level())
            .max()
    }

    /// Global bootstrap status is considered as bootstrapped, only if
    /// number of bootstrapped peers is above threshold.
    pub fn is_bootstrapped(&self) -> bool {
        const SYNC_LATENCY: i64 = 150;

        let current_head_timestamp = match self.current_head.get() {
            Some(v) => v.header.timestamp(),
            None => return false,
        };

        let bootstrapped_peers_len = self
            .peers
            .handshaked_iter()
            .filter_map(|(_, peer)| peer.current_head.as_ref())
            .map(|current_head| current_head_timestamp - current_head.header.timestamp())
            .filter(|latency| latency.abs() < SYNC_LATENCY)
            .count();

        bootstrapped_peers_len >= self.config.peers_bootstrapped_min
    }

    pub fn peer_public_key_hash(&self, peer: SocketAddr) -> Option<&CryptoboxPublicKeyHash> {
        self.peers.get(&peer).and_then(|p| p.public_key_hash())
    }

    pub fn peer_public_key_hash_b58check(&self, peer: SocketAddr) -> Option<String> {
        self.peers
            .get(&peer)
            .and_then(|p| p.public_key_hash_b58check())
    }

    /// Global bootstrap status is considered as bootstrapped, only if
    /// number of bootstrapped peers is above threshold.
    ///
    /// NOTE that it uses level+/-1 criteria instead of `Self::HIGH_LEVEL_MARGIN_PERCENTAGE`
    /// to match against peers level.
    pub fn is_bootstrapped_strict(&self) -> bool {
        let current_head_level = match self.current_head_level() {
            Some(v) => v,
            None => return false,
        };

        let bootstrapped_peers_len = self
            .peers
            .iter()
            .filter_map(|(_, p)| p.status.as_handshaked())
            .filter_map(|peer| peer.current_head.as_ref())
            .map(|head| head.header.level())
            .filter(|level| (level - current_head_level).abs() <= 1)
            .count();

        bootstrapped_peers_len >= self.config.peers_bootstrapped_min
    }

    pub fn can_accept_new_head(&self, head: &BlockHeaderWithHash) -> bool {
        let current_head = match self.current_head.get() {
            Some(v) => v,
            None => return false,
        };

        if head.header.level() < current_head.header.level() {
            false
        } else if head.header.level() == current_head.header.level() {
            self.is_same_head(head.header.level(), &head.hash)
                || head.header.fitness() > current_head.header.fitness()
        } else {
            true
        }
    }

    pub fn is_same_head(&self, level: Level, hash: &BlockHash) -> bool {
        self.current_head
            .get()
            .filter(|current_head| current_head.header.level() == level)
            .filter(|current_head| &current_head.hash == hash)
            .is_some()
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
    ConnectedPeerIsBlacklisted,
}

impl SafetyCondition for State {
    type Error = SafetyConditionError;

    fn check_safety_condition(&self) -> Result<(), Self::Error> {
        /*
            TODO: safety conditions checks are temporarily disabled.
            If enabled, these conditions are quickly violated by the Actions-Fuzzer.
            Once these are fixed the following code can be un-commented.
        */
        /*
        if self.peers.connected_len() > self.config.peers_connected_max {
            return Err(SafetyConditionError::ConnectedPeersMaxedOut)
        }

        if self.peers.potential_len() > self.config.peers_potential_max {
            return Err(SafetyConditionError::PotentialPeersMaxedOut);
        }

        for (peer_addr, _) in self.peers.connected_iter() {
            if self.peers.is_blacklisted(&peer_addr.ip()) {
                return Err(SafetyConditionError::ConnectedPeerIsBlacklisted);
            }
        }
        */
        Ok(())
    }
}
