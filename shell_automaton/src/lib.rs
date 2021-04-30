use redux_rs::Store;

pub mod io_error_kind;

pub mod event;
use event::Event;

pub mod action;
use action::Action;

pub mod config;
use config::default_config;
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
use crate::storage::block_header::put::StorageBlockHeadersPutAction;
use crate::storage::state_snapshot::create::StorageStateSnapshotCreateAction;

pub mod rpc;

pub mod service;
use crate::service::RpcServiceDefault;
use service::mio_service::MioInternalEventsContainer;
use service::{
    DnsServiceDefault, MioService, MioServiceDefault, RandomnessServiceDefault,
    StorageServiceDefault,
};
pub use service::{Service, ServiceDefault};

pub mod tmp;
use tmp::persistent_storage::{gen_block_headers, init_storage};

pub type Port = u16;

pub struct ShellAutomaton<Serv, Events> {
    /// Container for internal events.
    events: Events,
    store: Store<State, Serv, Action>,
}

impl<Serv: Service, Events> ShellAutomaton<Serv, Events> {
    pub fn new(initial_state: State, service: Serv, events: Events) -> Self {
        let mut store = Store::new(reducer, service, initial_state);

        store.add_middleware(effects);

        Self { events, store }
    }

    pub fn init<P>(&mut self, peers_dns_lookup_addrs: P)
    where
        P: IntoIterator<Item = (String, Port)>,
    {
        // Persist initial state.
        self.store
            .dispatch(StorageStateSnapshotCreateAction {}.into());

        for (address, port) in peers_dns_lookup_addrs.into_iter() {
            self.store
                .dispatch(PeersDnsLookupInitAction { address, port }.into());
        }
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
