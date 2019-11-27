// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

mod service;
mod ask;
pub mod control_msg;

pub use service::spawn_server;
pub use service::RpcServiceEnvironment;
