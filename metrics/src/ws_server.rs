use ws::{Handler, Sender, Factory};
use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};

/// Server
/// *should* handle incoming messages, for now not really necessary,
/// as only periodical pushes are necessary
pub struct WsServer {
    count: Arc<AtomicUsize>,
}

impl WsServer {
    pub fn new(count: Arc<AtomicUsize>) -> Self {
        Self {
            count
        }
    }
}

impl Factory for WsServer {
    type Handler = WsClient;

    fn connection_made(&mut self, ws: Sender) -> Self::Handler {
        self.count.fetch_add(1, Ordering::SeqCst);
        Self::Handler::new(ws)
    }

    fn connection_lost(&mut self, _: Self::Handler) {
        self.count.fetch_sub(1, Ordering::SeqCst);
    }
}


/// Client handler
/// Contains bare-bones implementation, as we do not need any fancy
/// client handling, for now.
#[derive(Clone)]
pub struct WsClient {
    ws: Sender
}

impl WsClient {
    pub fn new(ws: Sender) -> Self {
        Self { ws }
    }

    pub fn connection_id(&self) -> u32 {
        self.ws.connection_id()
    }
}

impl Handler for WsClient {}
