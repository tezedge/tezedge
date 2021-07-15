// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use warp::ws::Message;

mod ws_manager;
pub(crate) mod ws_messages;
mod ws_server;

pub use ws_manager::{WebsocketHandler, WebsocketHandlerMsg};

// keep the clients in a HashMap
// not using a HashSet, becouse UnboundedSender does not implement Eq
type Clients = Arc<RwLock<HashMap<String, Client>>>;
type Client = Option<mpsc::UnboundedSender<std::result::Result<Message, warp::Error>>>;
