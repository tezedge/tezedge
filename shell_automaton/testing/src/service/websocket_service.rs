use shell_automaton::service::WebsocketService;

pub use shell_automaton::service::websocket_service::{WebsocketMessage, WebsocketSendError};

#[derive(Debug, Clone)]
pub struct WebsocketServiceDummy {}

impl WebsocketServiceDummy {
    pub fn new() -> Self {
        Self {}
    }
}

impl WebsocketService for WebsocketServiceDummy {
    fn message_send(&mut self, message: WebsocketMessage) -> Result<(), WebsocketSendError> {
        todo!()
    }
}
