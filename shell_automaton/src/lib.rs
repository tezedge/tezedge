#![feature(deadline_api)]

use std::time::Duration;

use peer::{
    connection::outgoing::PeerConnectionOutgoingRandomInitAction, PeerTryReadAction,
    PeerTryWriteAction,
};
use redux_rs::Store;

pub mod io_error_kind;

pub mod event;
use event::Event;

pub mod action;
pub use action::{Action, ActionId, ActionKind, ActionWithId};

pub mod config;
pub use config::{Config, Quota};

mod state;
pub use state::State;

mod reducer;
pub use reducer::reducer;

mod effects;
pub use effects::effects;

pub mod request;

pub mod shell_compatibility_version;

pub mod peer;

pub mod peers;
use peers::dns_lookup::PeersDnsLookupInitAction;

pub mod storage;
use crate::storage::state_snapshot::create::StorageStateSnapshotCreateAction;

pub mod rpc;

pub mod actors;

pub mod service;
use service::MioService;
pub use service::{Service, ServiceDefault};
pub type Port = u16;

pub struct ShellAutomaton<Serv, Events> {
    /// Container for internal events.
    events: Events,
    store: Store<State, Serv, Action>,
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

    pub fn init<P>(&mut self, peers_dns_lookup_addrs: P)
    where
        P: IntoIterator<Item = (String, Port)>,
    {
        // Persist initial state.
        self.store
            .dispatch(StorageStateSnapshotCreateAction {}.into());

        // TODO: create action for it.
        if let Err(err) = self
            .store
            .service
            .mio()
            .peer_connection_incoming_listen_start()
        {
            eprintln!("P2p: failed to start server. Error: {:?}", err);
        }

        for (address, port) in peers_dns_lookup_addrs.into_iter() {
            self.store
                .dispatch(PeersDnsLookupInitAction { address, port }.into());
        }
        self.store
            .dispatch(PeerConnectionOutgoingRandomInitAction {}.into());
    }
}

impl<Serv, Events, Mio> ShellAutomaton<Serv, Events>
where
    Serv: Service<Mio = Mio>,
    Mio: MioService<Events = Events>,
    for<'a> &'a Events: IntoIterator<Item = &'a <Serv::Mio as MioService>::InternalEvent>,
{
    /// Making progress for the automaton by processing all available MIO events
    /// and readable and writable peers. Returns `false` if there were none to
    /// make the next iteration wait for more events.
    pub fn make_progress(&mut self, dont_wait: bool) -> bool {
        let mio_timeout = if dont_wait {
            Duration::ZERO
        } else {
            self.store.state().config.min_time_interval()
        };

        self.wait_for_mio_events(Some(mio_timeout));

        // dispatch existing mio events
        let were_mio_events = self.process_mio_events();
        // dispatch readable/writable peers
        let were_active_peers = self.process_active_peers();

        if !were_mio_events && !were_active_peers {
            self.store.dispatch(Action::MioTimeoutEvent);
            self.store.dispatch(Action::DispatchRecursionReset);
            false
        } else {
            true
        }
    }

    fn wait_for_mio_events(&mut self, timeout: Option<Duration>) {
        self.store
            .service()
            .mio()
            .wait_for_events(&mut self.events, timeout);
    }

    fn process_mio_events(&mut self) -> bool {
        let mut were_events = false;

        // process new mio events
        for event in self.events.into_iter() {
            were_events = true;

            match self.store.service().mio().transform_event(event) {
                Event::P2pServer(p2p_server_event) => self.store.dispatch(p2p_server_event.into()),
                Event::P2pPeer(p2p_peer_event) => self.store.dispatch(p2p_peer_event.into()),
                Event::Wakeup(wakeup_event) => self.store.dispatch(wakeup_event.into()),
                _ => {}
            }
            self.store.dispatch(Action::DispatchRecursionReset);
        }

        were_events
    }

    fn process_active_peers(&mut self) -> bool {
        let mut were_events = false;
        let quota_restore_duration_millis =
            self.store.state.get().config.quota.restore_duration_millis;
        let now = self.store.state.get().last_action.id();
        let peers = &self.store.state.get().peers;
        let readable_peers = peers
            .readable(now, quota_restore_duration_millis)
            .map(|(address, _)| address.clone())
            .collect::<Vec<_>>();
        let writable_peers = peers
            .writable(now, quota_restore_duration_millis)
            .map(|(address, _)| address.clone())
            .collect::<Vec<_>>();

        for address in readable_peers {
            were_events = true;
            self.store.dispatch(PeerTryReadAction { address }.into());
            self.store.dispatch(Action::DispatchRecursionReset);
        }
        for address in writable_peers {
            were_events = true;
            self.store.dispatch(PeerTryWriteAction { address }.into());
            self.store.dispatch(Action::DispatchRecursionReset);
        }

        were_events
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
