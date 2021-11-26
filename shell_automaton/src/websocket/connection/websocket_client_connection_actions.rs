// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

use crate::service::mio_service::PeerConnectionIncomingAcceptError;
use crate::{EnablingCondition, State};
use crate::peer::PeerToken;

use crate::peer::connection::incoming::PeerConnectionIncomingStatePhase;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WebSocketConnectionIncomingAcceptSuccessAction {
    pub token: PeerToken,
    pub address: SocketAddr,
}

impl EnablingCondition<State> for WebSocketConnectionIncomingAcceptSuccessAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WebSocketConnectionIncomingErrorAction {
    pub address: SocketAddr,
    pub error: WebSocketConnectionIncomingError,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum WebSocketConnectionIncomingError {
    Timeout(PeerConnectionIncomingStatePhase),
}

impl EnablingCondition<State> for WebSocketConnectionIncomingError {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

/// Accept incoming ws connection.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WebSocketConnectionIncomingAcceptAction {}

impl EnablingCondition<State> for WebSocketConnectionIncomingAcceptAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WebSocketConnectionIncomingAcceptErrorAction {
    pub error: PeerConnectionIncomingAcceptError,
}

impl WebSocketConnectionIncomingAcceptErrorAction {
    pub fn new(error: PeerConnectionIncomingAcceptError) -> Self { Self { error } }
}

impl EnablingCondition<State> for WebSocketConnectionIncomingAcceptErrorAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

// TODO: rejection