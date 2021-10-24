use redux_rs::{ActionId, ActionWithId};
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime};

use ::storage::persistent::BincodeEncoded;

use crate::config::Config;
use crate::peer::connection::incoming::accept::PeerConnectionIncomingAcceptState;
use crate::peers::PeersState;
use crate::storage::StorageState;
use crate::yielded_operations::YieldedOperationsState;
use crate::{Action, ActionKind};

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
    pub config: Config,
    pub peers: PeersState,
    pub peer_connection_incoming_accept: PeerConnectionIncomingAcceptState,
    pub storage: StorageState,

    pub yielded_operations: YieldedOperationsState,

    /// Action before the `last_action`.
    pub prev_action: ActionIdWithKind,
    pub last_action: ActionIdWithKind,
    pub applied_actions_count: u64,
}

impl State {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            peers: PeersState::new(),
            peer_connection_incoming_accept: PeerConnectionIncomingAcceptState::Idle { time: 0 },
            storage: StorageState::new(),

            yielded_operations: YieldedOperationsState::new(),

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

    #[inline(always)]
    pub fn set_last_action(&mut self, action: &ActionWithId<Action>) {
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
        // If we have yielded operations, then set mio timeout to zero
        // so that epoll syscall for events returns instantly, instead
        // of blocking up until timeout or until there are some events.
        if !self.yielded_operations.is_empty() {
            Some(Duration::ZERO)
        } else {
            Some(self.config.min_time_interval())
        }
    }
}

impl BincodeEncoded for State {}
