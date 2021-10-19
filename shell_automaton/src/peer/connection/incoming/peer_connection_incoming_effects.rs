use redux_rs::{ActionWithId, Store};

use crate::peer::connection::PeerConnectionState;
use crate::peer::disconnection::PeerDisconnectAction;
use crate::peer::handshaking::PeerHandshakingInitAction;
use crate::peer::PeerStatus;
use crate::service::Service;
use crate::{action::Action, State};

use super::{PeerConnectionIncomingState, PeerConnectionIncomingSuccessAction};

pub fn peer_connection_incoming_effects<S>(
    store: &mut Store<State, S, Action>,
    action: &ActionWithId<Action>,
) where
    S: Service,
{
    match &action.action {
        Action::P2pPeerEvent(event) => {
            // when we receive first writable event from mio,
            // that's when we know that we successfuly connected
            // to the peer.
            if !event.is_writable() {
                return;
            }
            let address = event.address();

            let peer = match store.state.get().peers.get(&address) {
                Some(v) => v,
                None => return,
            };

            match &peer.status {
                PeerStatus::Connecting(connection_state) => match connection_state {
                    PeerConnectionState::Incoming(PeerConnectionIncomingState::Pending {
                        ..
                    }) => {
                        store.dispatch(PeerConnectionIncomingSuccessAction { address }.into());
                    }
                    _ => {}
                },
                _ => {}
            }
        }
        Action::PeerConnectionIncomingSuccess(action) => store.dispatch(
            PeerHandshakingInitAction {
                address: action.address,
            }
            .into(),
        ),
        Action::PeerConnectionIncomingError(action) => store.dispatch(
            PeerDisconnectAction {
                address: action.address,
            }
            .into(),
        ),
        _ => {}
    }
}
