// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

pub use shell_automaton::service::rpc_service::{RpcId, RpcRecvError, RpcResponse, RpcService};

#[derive(Debug, Clone)]
pub struct RpcServiceDummy {}

impl RpcServiceDummy {
    pub fn new() -> Self {
        Self {}
    }
}

impl RpcService for RpcServiceDummy {
    fn try_recv(&mut self) -> Result<(RpcResponse, RpcId), RpcRecvError> {
        Err(RpcRecvError::Empty)
    }

    fn respond<J>(&mut self, call_id: RpcId, json: J) where J: serde::Serialize {
        let _ = (call_id, serde_json::to_value(json));
    }
}
