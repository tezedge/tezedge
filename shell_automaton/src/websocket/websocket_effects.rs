use slog::{info, warn};

use crate::{
    service::{
        websocket_service::{DummyPeerStatusMessage, WebsocketMessage},
        WebsocketService,
    },
    Action, ActionWithMeta, Service, Store,
};

use super::WebsocketSendMessageAction;

#[allow(unused)]
pub fn websocket_effects<S: Service>(store: &mut Store<S>, action: &ActionWithMeta) {
    match &action.action {
        Action::WebsocketSendMessage(action) => {
            if let Err(e) = store
                .service()
                .websocket()
                .message_send(action.message.clone())
            {
                warn!(
                    store.state().log,
                    "Failed to send the message to websocket service: {:?}", e
                )
            } else {
                info!(store.state().log, "Send Ok");
            }
        }
        // TODO: (monitoring-refactor) just for quick testing purposes
        Action::PeerConnectionOutgoingSuccess(action) => {
            store.dispatch(WebsocketSendMessageAction {
                message: WebsocketMessage::PeerStatus(DummyPeerStatusMessage {
                    address: action.address,
                }),
            });
        }
        Action::WakeupEvent(_) => {}
        _ => {}
    }
}
