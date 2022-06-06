// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![cfg_attr(feature = "fuzzing", feature(no_coverage))]

pub mod io_error_kind;

pub mod event;
use event::Event;

pub mod action;
pub use action::{
    Action, ActionId, ActionKind, ActionWithMeta, EnablingCondition, MioTimeoutEvent,
    MioWaitForEventsAction,
};

pub mod config;
pub use config::Config;

pub mod logger;
pub use logger::Logger;

mod state;
pub use state::State;

mod reducer;
pub use reducer::reducer;

mod effects;
pub use effects::{check_timeouts, effects};

pub mod paused_loops;
use paused_loops::PausedLoopsResumeAllAction;

pub mod request;

pub mod shell_compatibility_version;

pub mod peer;

pub mod peers;

pub mod storage;
use crate::storage::state_snapshot::create::StorageStateSnapshotCreateInitAction;

pub mod mempool;

pub mod protocol_runner;
use protocol_runner::ProtocolRunnerStartAction;

pub mod block_applier;

pub mod bootstrap;

pub mod rpc;

pub mod actors;

#[cfg(feature = "fuzzing")]
pub mod fuzzing;

pub mod rights;

pub mod prechecker;

pub mod shutdown;

pub mod current_head;
pub mod current_head_precheck;

pub mod baker;

pub mod stats;

pub mod service;
use service::MioService;
pub use service::{Service, ServiceDefault};

pub type Port = u16;

pub type Store<Service> = redux_rs::Store<State, Service, Action>;

pub struct ShellAutomaton<Serv, Events> {
    /// Container for internal events.
    events: Events,
    store: Store<Serv>,
}

impl<Serv: Service, Events> ShellAutomaton<Serv, Events> {
    pub fn new(initial_state: State, service: Serv, events: Events) -> Self {
        let store = Store::new(
            reducer,
            effects,
            service,
            initial_state.config.initial_time,
            initial_state,
        );

        Self { events, store }
    }

    pub fn store(&self) -> &Store<Serv> {
        &self.store
    }

    pub fn init(&mut self) {
        // Persist initial state.
        self.store.dispatch(StorageStateSnapshotCreateInitAction {});

        // TODO: create action for it.
        if let Err(err) = self
            .store
            .service
            .mio()
            .peer_connection_incoming_listen_start()
        {
            eprintln!("P2p: failed to start server. Error: {:?}", err);
        }

        self.store.dispatch(ProtocolRunnerStartAction {});
    }

    #[inline(always)]
    pub fn is_shutdown(&self) -> bool {
        self.store.state().is_shutdown()
    }
}

impl<Serv, Events, Mio> ShellAutomaton<Serv, Events>
where
    Serv: Service<Mio = Mio>,
    Mio: MioService<Events = Events>,
    for<'a> &'a Events: IntoIterator<Item = &'a <Serv::Mio as MioService>::InternalEvent>,
{
    pub fn make_progress(&mut self) {
        let mio_timeout = self.store.state().mio_timeout();

        check_timeouts(&mut self.store);
        self.store.dispatch(MioWaitForEventsAction {});
        self.store
            .service()
            .mio()
            .wait_for_events(&mut self.events, mio_timeout);

        let mut no_events = true;

        for event in self.events.into_iter() {
            no_events = false;

            match self.store.service().mio().transform_event(event) {
                Event::P2pServer(p2p_server_event) => self.store.dispatch(p2p_server_event),
                Event::P2pPeer(p2p_peer_event) => self.store.dispatch(p2p_peer_event),
                Event::Wakeup(wakeup_event) => self.store.dispatch(wakeup_event),
                _ => false,
            };
        }

        if no_events {
            self.store.dispatch(MioTimeoutEvent {});
        }

        if !self.store.state().paused_loops.is_empty() {
            self.store.dispatch(PausedLoopsResumeAllAction {});
        }
    }
}

impl<Serv, Events> Clone for ShellAutomaton<Serv, Events>
where
    Serv: Clone,
    Events: Clone,
{
    fn clone(&self) -> Self {
        Self {
            events: self.events.clone(),
            store: self.store.clone(),
        }
    }
}
