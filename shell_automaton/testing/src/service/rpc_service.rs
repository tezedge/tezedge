// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

pub use shell_automaton::service::rpc_service::{RpcResponse, RpcService};
use shell_automaton::service::service_async_channel::ResponseTryRecvError;

#[derive(Debug, Clone)]
pub struct RpcServiceDummy {}

impl RpcServiceDummy {
    pub fn new() -> Self {
        Self {}
    }
}

impl RpcService for RpcServiceDummy {
    #[inline(always)]
    fn try_recv(&mut self) -> Result<RpcResponse, ResponseTryRecvError> {
        Err(ResponseTryRecvError::Empty)
    }
}
