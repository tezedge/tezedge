use redux_rs::{ActionId, ActionWithId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::net::SocketAddr;

use ::storage::persistent::BincodeEncoded;

use crate::config::Config;
use crate::peer::connection::incoming::accept::PeerConnectionIncomingAcceptState;
use crate::peer::Peer;
use crate::peers::dns_lookup::PeersDnsLookupState;
use crate::storage::StorageState;
use crate::{Action, ActionKind};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ActionIdWithKind {
    pub id: ActionId,
    pub kind: ActionKind,
}

impl ActionIdWithKind {
    pub fn update<A>(&mut self, action: A)
    where
        A: Into<ActionIdWithKind>,
    {
        *self = action.into()
    }
}

impl<'a> From<&'a ActionWithId<Action>> for ActionIdWithKind {
    fn from(action: &'a ActionWithId<Action>) -> Self {
        Self {
            id: action.id,
            kind: action.into(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct State {
    pub config: Config,
    pub peers: BTreeMap<SocketAddr, Peer>,
    pub peers_dns_lookup: Option<PeersDnsLookupState>,
    pub peer_connection_incoming_accept: PeerConnectionIncomingAcceptState,
    pub storage: StorageState,

    pub last_action: ActionIdWithKind,
    pub applied_actions_count: u64,
}

impl State {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            peers: BTreeMap::new(),
            peers_dns_lookup: None,
            peer_connection_incoming_accept: PeerConnectionIncomingAcceptState::Idle,
            storage: StorageState::new(),

            last_action: ActionIdWithKind {
                id: ActionId::ZERO,
                kind: ActionKind::Init,
            },
            applied_actions_count: 0,
        }
    }
}

impl BincodeEncoded for State {}
