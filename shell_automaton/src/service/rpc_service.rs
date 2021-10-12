use std::sync::Arc;

use crate::State;

use super::service_async_channel::{
    worker_channel, ResponseTryRecvError, ServiceWorkerAsyncRequester, ServiceWorkerAsyncResponder,
    ServiceWorkerAsyncResponderSender,
};

pub trait RpcService {
    /// Try to receive/read queued message, if there is any.
    fn try_recv(&mut self) -> Result<RpcResponse, ResponseTryRecvError>;
}

#[derive(Debug)]
pub enum RpcResponse {
    GetCurrentGlobalState {
        channel: tokio::sync::oneshot::Sender<State>,
    },
}

pub type RpcShellAutomatonChannel = ServiceWorkerAsyncResponder<(), RpcResponse>;
pub type RpcShellAutomatonSender = ServiceWorkerAsyncResponderSender<RpcResponse>;

#[derive(Debug)]
pub struct RpcServiceDefault {
    channel: ServiceWorkerAsyncRequester<(), RpcResponse>,
}

impl RpcServiceDefault {
    pub fn new(mio_waker: Arc<mio::Waker>, bound: usize) -> (Self, RpcShellAutomatonChannel) {
        let (c1, c2) = worker_channel(mio_waker, bound);

        (Self::new_with_channel(c1), c2)
    }

    pub fn new_with_channel(channel: ServiceWorkerAsyncRequester<(), RpcResponse>) -> Self {
        Self { channel }
    }
}

impl RpcService for RpcServiceDefault {
    #[inline(always)]
    fn try_recv(&mut self) -> Result<RpcResponse, ResponseTryRecvError> {
        self.channel.try_recv()
    }
}
