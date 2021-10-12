use peer::connection::outgoing::PeerConnectionOutgoingRandomInitAction;
use redux_rs::Store;

pub mod io_error_kind;

pub mod event;
use event::Event;

pub mod action;
pub use action::{Action, ActionId, ActionWithId};

pub mod config;
pub use config::Config;

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
    pub fn make_progress(&mut self) {
        self.store
            .service()
            .mio()
            .wait_for_events(&mut self.events, None);

        for event in self.events.into_iter() {
            match self.store.service().mio().transform_event(event) {
                Event::P2pServer(p2p_server_event) => self.store.dispatch(p2p_server_event.into()),
                Event::P2pPeer(p2p_peer_event) => self.store.dispatch(p2p_peer_event.into()),
                Event::Wakeup(wakeup_event) => self.store.dispatch(wakeup_event.into()),
                _ => {}
            }
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
