use serde::{Deserialize, Serialize};

use crate::{service::websocket_service::WebsocketMessage, EnablingCondition, State};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebsocketSendMessageAction {
    pub message: WebsocketMessage,
}

impl EnablingCondition<State> for WebsocketSendMessageAction {
    fn is_enabled(&self, state: &State) -> bool {
        let _ = state;
        true
    }
}
