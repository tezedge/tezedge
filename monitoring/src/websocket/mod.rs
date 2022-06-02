// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

pub mod ws_json_rpc;
mod ws_manager;
pub(crate) mod ws_messages;
mod ws_server;

pub use ws_manager::{WebsocketHandler, WebsocketHandlerMsg};

type RpcClients = Arc<RwLock<HashMap<String, RpcClient>>>;
pub type RpcClient = Option<mpsc::UnboundedSender<json_rpc_types::Response<serde_json::Value, ()>>>;
