// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::Serialize;

pub use shell_automaton::service::rpc_service::{
    RpcId, RpcRecvError, RpcRequest, RpcRequestStream, RpcService,
};

#[derive(Debug, Clone)]
pub struct RpcServiceDummy {}

impl RpcServiceDummy {
    pub fn new() -> Self {
        Self {}
    }
}

impl RpcService for RpcServiceDummy {
    fn try_recv(&mut self) -> Result<(RpcRequest, RpcId), RpcRecvError> {
        Err(RpcRecvError::Empty)
    }

    fn respond<J>(&mut self, call_id: RpcId, json: J)
    where
        J: Serialize,
    {
        let _ = (call_id, json);
    }

    fn try_recv_stream(&mut self) -> Result<(RpcRequestStream, RpcId), RpcRecvError> {
        Err(RpcRecvError::Empty)
    }

    fn respond_stream(&mut self, call_id: RpcId, json: Option<serde_json::Value>) {
        let _ = (call_id, json);
    }
}
