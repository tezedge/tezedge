use shell_automaton::service::WebsocketService;

pub use shell_automaton::service::websocket_service::{WebsocketMessage, WebsocketSendError};

#[derive(Debug, Clone)]
pub struct WebsocketServiceDummy {}

impl WebsocketServiceDummy {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for WebsocketServiceDummy {
    fn default() -> Self {
        Self::new()
    }
}

impl WebsocketService for WebsocketServiceDummy {
    fn message_send(&mut self, _: WebsocketMessage) -> Result<(), WebsocketSendError> {
        Ok(())
    }
}
